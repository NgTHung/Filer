//! # Directory Paging
//!
//! This module owns bounded pipeline paging and cursor state for the scanner.
//!
//! A chain takes one of two routes, chosen by
//! [`crate::pipeline::PipelinePagingMode`]. When the pipeline preserves provider
//! order, the session holds the provider walk open and each page costs only the
//! rows it returns. When ordering or grouping needs every row before the first
//! page is correct, the chain walks the directory and keyset cursors keep
//! unchanged rows from repeating between pages.
//!
//! ```ignore
//! let sessions = PagingSessions::new();
//! let page = sessions.load_provider(provider, path, session, request, pipeline, &cx).await?;
//! ```

mod ordered;
mod selection;
mod session;
mod stream;

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::errors::{CoreError, ErrorCode};
use crate::model::directory::{
    DEFAULT_DIRECTORY_PAGE_SIZE, DirectoryPageRequest, DirectoryPageResult, DirectoryPageState,
};
use crate::model::node::NodeEntry;
use crate::model::session::SessionId;
use crate::pipeline::{PipelineConfig, effective_listing};
use crate::vfs::context::ProviderCx;
use crate::vfs::provider::{FsProvider, ProviderPaging, validate_page_limit};

use ordered::RetainedPage;
pub(crate) use selection::PageSelection;
use session::{
    CURSOR_PREFIX, Continuation, DEFAULT_PAGING_SESSION_CAPACITY, DEFAULT_RETAINED_ROW_BUDGET,
    PagingSession, PagingSessionStore, RetainedChain, next_cursor,
};
use stream::{StreamedPage, streams_pages, take_page};

#[derive(Clone)]
pub struct PagingSessions {
    sessions: Arc<Mutex<PagingSessionStore>>,
}

impl Default for PagingSessions {
    fn default() -> Self {
        Self::with_limits(DEFAULT_PAGING_SESSION_CAPACITY, DEFAULT_RETAINED_ROW_BUDGET)
    }
}

pub enum PageLoad {
    Page(DirectoryPageResult),
    Cancelled,
}

impl PagingSessions {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self::with_limits(capacity, DEFAULT_RETAINED_ROW_BUDGET)
    }

    /// Bound both how many chains are stored and how many rows they may retain.
    pub(crate) fn with_limits(capacity: usize, retained_budget: usize) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(PagingSessionStore::new(
                capacity,
                retained_budget,
            ))),
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.store().len()
    }

    #[cfg(test)]
    pub(crate) fn retained_rows(&self) -> usize {
        self.store().retained_rows()
    }

    fn store(&self) -> std::sync::MutexGuard<'_, PagingSessionStore> {
        self.sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn clear_session(&self, session: SessionId) {
        self.store().clear_owner(session);
    }

    pub async fn load_provider(
        &self,
        provider: &dyn FsProvider,
        path: &Path,
        owner: SessionId,
        request: DirectoryPageRequest,
        pipeline_config: &PipelineConfig,
        cx: &ProviderCx<'_>,
    ) -> Result<PageLoad, CoreError> {
        validate_page_limit(request.limit)?;
        let effective_request = DirectoryPageRequest {
            listing: effective_listing(pipeline_config, request.listing),
            ..request
        };
        let continuation = self.continuation(path, owner, &effective_request, pipeline_config)?;
        if effective_request.cursor.is_none() {
            self.clear_session(owner);
        }

        if streams_pages(pipeline_config) {
            let start_index = continuation
                .as_ref()
                .map(|state| state.start_index)
                .unwrap_or(0);
            let (resumed, walked) = match continuation {
                Some(session) if session.is_streaming() => {
                    (Some(session.into_continuation()), None)
                }
                Some(session) => (None, Some(session)),
                None => (
                    provider
                        .open_listing(path, effective_request.listing, cx)
                        .await?
                        .map(|stream| Continuation::Stream {
                            stream,
                            pending: VecDeque::new(),
                            exhausted: false,
                        }),
                    None,
                ),
            };
            if let Some(Continuation::Stream {
                stream,
                pending,
                exhausted,
            }) = resumed
            {
                return self
                    .streamed_page(
                        path,
                        owner,
                        effective_request,
                        pipeline_config,
                        stream,
                        pending,
                        exhausted,
                        start_index,
                        cx,
                    )
                    .await;
            }
            // A provider without a listing stream keeps the keyset walk, so its
            // continuation must survive the attempt to stream.
            return self
                .walked_page(
                    provider,
                    path,
                    owner,
                    effective_request,
                    pipeline_config,
                    walked,
                    cx,
                )
                .await;
        }

        let continuation = match continuation.map(|s| s.into_retained(effective_request.limit)) {
            Some(Ok(retained)) => {
                return Ok(PageLoad::Page(self.retained_page(
                    path,
                    owner,
                    effective_request,
                    pipeline_config,
                    retained,
                )));
            }
            Some(Err(session)) => Some(*session),
            None => None,
        };

        self.walked_page(
            provider,
            path,
            owner,
            effective_request,
            pipeline_config,
            continuation,
            cx,
        )
        .await
    }

    /// Serve a page out of rows an earlier walk already ordered.
    fn retained_page(
        &self,
        path: &Path,
        owner: SessionId,
        request: DirectoryPageRequest,
        pipeline: &PipelineConfig,
        retained: RetainedChain,
    ) -> DirectoryPageResult {
        let page = RetainedPage::take(retained.rows, retained.tail_complete, request.limit);
        self.finish_ordered_page(
            path,
            owner,
            request,
            pipeline,
            retained.start_index,
            retained.total_count,
            page,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_ordered_page(
        &self,
        path: &Path,
        owner: SessionId,
        request: DirectoryPageRequest,
        pipeline: &PipelineConfig,
        start_index: usize,
        total_count: Option<usize>,
        page: RetainedPage,
    ) -> DirectoryPageResult {
        let has_more = page.has_more();
        let RetainedPage {
            entries,
            rows,
            tail_complete,
            retain,
        } = page;
        let next_cursor = has_more
            .then(|| entries.last().cloned())
            .flatten()
            .map(|last| {
                let cursor = next_cursor();
                self.store().insert(
                    cursor.0.clone(),
                    PagingSession {
                        owner,
                        path: path.to_path_buf(),
                        request: DirectoryPageRequest {
                            cursor: None,
                            ..request.clone()
                        },
                        pipeline: pipeline.clone(),
                        continuation: if retain {
                            Continuation::Ordered {
                                rows,
                                last: Box::new(last),
                                tail_complete,
                            }
                        } else {
                            // Without a retention allowance the chain keeps only
                            // its boundary and rewalks, as it did before.
                            Continuation::Keyset {
                                last: Box::new(last),
                            }
                        },
                        start_index: start_index + entries.len(),
                        total_count,
                    },
                );
                cursor
            });
        let state = match next_cursor {
            Some(cursor) => DirectoryPageState::partial(entries.len(), total_count, cursor),
            None => DirectoryPageState::complete(entries.len(), total_count),
        }
        .with_window(start_index);
        DirectoryPageResult { entries, state }
    }

    /// Serve a page from a live provider walk that outlives the request.
    #[allow(clippy::too_many_arguments)]
    async fn streamed_page(
        &self,
        path: &Path,
        owner: SessionId,
        request: DirectoryPageRequest,
        pipeline_config: &PipelineConfig,
        stream: Box<dyn crate::vfs::listing_stream::DirectoryStream>,
        pending: VecDeque<NodeEntry>,
        exhausted: bool,
        start_index: usize,
        cx: &ProviderCx<'_>,
    ) -> Result<PageLoad, CoreError> {
        let page = match take_page(
            stream,
            pending,
            exhausted,
            request.limit,
            pipeline_config,
            cx,
        )
        .await
        {
            Ok(Some(page)) => page,
            Ok(None) => return Ok(PageLoad::Cancelled),
            Err(e) if e.code() == ErrorCode::Cancelled => return Ok(PageLoad::Cancelled),
            Err(e) => return Err(e),
        };
        Ok(PageLoad::Page(self.finish_streamed_page(
            path,
            owner,
            request,
            pipeline_config,
            start_index,
            page,
        )))
    }

    /// Serve a page by walking the directory and selecting an ordered window.
    #[allow(clippy::too_many_arguments)]
    async fn walked_page(
        &self,
        provider: &dyn FsProvider,
        path: &Path,
        owner: SessionId,
        request: DirectoryPageRequest,
        pipeline_config: &PipelineConfig,
        continuation: Option<PagingSession>,
        cx: &ProviderCx<'_>,
    ) -> Result<PageLoad, CoreError> {
        let lookahead = self.store().retention_allowance();
        let mut selection = PageSelection::with_lookahead(
            request.limit,
            lookahead,
            continuation
                .as_ref()
                .and_then(PagingSession::keyset_boundary)
                .cloned(),
            pipeline_config,
        );

        match provider.paging() {
            ProviderPaging::Fallback => {
                let entries = match cx
                    .race(
                        provider.scheme(),
                        provider.list_with_options(path, request.listing, cx),
                    )
                    .await
                {
                    Ok(entries) => entries,
                    Err(e) if e.code() == ErrorCode::Cancelled => return Ok(PageLoad::Cancelled),
                    Err(e) => return Err(e),
                };
                if !selection.extend(entries, cx) {
                    return Ok(PageLoad::Cancelled);
                }
            }
            ProviderPaging::Native => {
                let mut provider_cursor = None;
                loop {
                    let raw_page = match cx
                        .race(
                            provider.scheme(),
                            provider.list_page(
                                path,
                                DirectoryPageRequest {
                                    listing: request.listing,
                                    limit: DEFAULT_DIRECTORY_PAGE_SIZE,
                                    cursor: provider_cursor,
                                },
                                cx,
                            ),
                        )
                        .await
                    {
                        Ok(page) => page,
                        Err(e) if e.code() == ErrorCode::Cancelled => {
                            return Ok(PageLoad::Cancelled);
                        }
                        Err(e) => return Err(e),
                    };
                    let complete = raw_page.state.complete;
                    provider_cursor = raw_page.state.next_cursor;
                    let page_count = raw_page.entries.len();
                    if !selection.extend(raw_page.entries, cx) {
                        return Ok(PageLoad::Cancelled);
                    }
                    if complete || provider_cursor.is_none() || page_count == 0 {
                        break;
                    }
                }
            }
        }

        Ok(PageLoad::Page(self.finish_walked_page(
            path,
            owner,
            request,
            pipeline_config,
            continuation,
            selection,
        )))
    }

    pub fn load_cached(
        &self,
        entries: Vec<NodeEntry>,
        path: &Path,
        owner: SessionId,
        request: DirectoryPageRequest,
        pipeline_config: &PipelineConfig,
        cx: &ProviderCx<'_>,
    ) -> Result<PageLoad, CoreError> {
        validate_page_limit(request.limit)?;
        let effective_request = DirectoryPageRequest {
            listing: effective_listing(pipeline_config, request.listing),
            ..request
        };
        let continuation = self.continuation(path, owner, &effective_request, pipeline_config)?;
        if effective_request.cursor.is_none() {
            self.clear_session(owner);
        }
        let continuation = match continuation.map(|s| s.into_retained(effective_request.limit)) {
            Some(Ok(retained)) => {
                return Ok(PageLoad::Page(self.retained_page(
                    path,
                    owner,
                    effective_request,
                    pipeline_config,
                    retained,
                )));
            }
            Some(Err(session)) => Some(*session),
            None => None,
        };
        let lookahead = self.store().retention_allowance();
        let mut selection = PageSelection::with_lookahead(
            effective_request.limit,
            lookahead,
            continuation
                .as_ref()
                .and_then(PagingSession::keyset_boundary)
                .cloned(),
            pipeline_config,
        );
        if !selection.extend(entries, cx) {
            return Ok(PageLoad::Cancelled);
        }
        Ok(PageLoad::Page(self.finish_walked_page(
            path,
            owner,
            effective_request,
            pipeline_config,
            continuation,
            selection,
        )))
    }

    /// Take the stored session a cursor names, or explain why it cannot be used.
    fn continuation(
        &self,
        path: &Path,
        owner: SessionId,
        request: &DirectoryPageRequest,
        pipeline: &PipelineConfig,
    ) -> Result<Option<PagingSession>, CoreError> {
        let Some(cursor) = &request.cursor else {
            return Ok(None);
        };
        if !cursor.0.starts_with(CURSOR_PREFIX) {
            return Err(CoreError::invalid_input("Invalid directory paging cursor"));
        }
        let mut sessions = self.store();
        let state = sessions
            .get(&cursor.0)
            .ok_or_else(|| CoreError::invalid_input("Expired directory paging cursor"))?;
        if state.owner != owner
            || state.path != path
            || state.request.listing != request.listing
            || state.pipeline != *pipeline
        {
            return Err(CoreError::invalid_input(
                "Directory paging cursor does not match the request",
            ));
        }
        Ok(sessions.remove(&cursor.0))
    }

    fn finish_streamed_page(
        &self,
        path: &Path,
        owner: SessionId,
        request: DirectoryPageRequest,
        pipeline: &PipelineConfig,
        start_index: usize,
        page: StreamedPage,
    ) -> DirectoryPageResult {
        let loaded_count = start_index + page.entries.len();
        // A streaming chain only knows the total once it has seen the last row.
        let total_count = (!page.has_more()).then_some(loaded_count);
        let StreamedPage {
            entries,
            stream,
            pending,
            exhausted,
        } = page;
        let has_more = !pending.is_empty() || !exhausted;

        let next_cursor = has_more.then(|| {
            let cursor = next_cursor();
            self.store().insert(
                cursor.0.clone(),
                PagingSession {
                    owner,
                    path: path.to_path_buf(),
                    request: DirectoryPageRequest {
                        cursor: None,
                        ..request.clone()
                    },
                    pipeline: pipeline.clone(),
                    continuation: Continuation::Stream {
                        stream,
                        pending,
                        exhausted,
                    },
                    start_index: loaded_count,
                    total_count: None,
                },
            );
            cursor
        });

        let state = match next_cursor {
            Some(cursor) => DirectoryPageState::partial(entries.len(), total_count, cursor),
            None => DirectoryPageState::complete(entries.len(), total_count),
        }
        .with_window(start_index);
        DirectoryPageResult { entries, state }
    }

    fn finish_walked_page(
        &self,
        path: &Path,
        owner: SessionId,
        request: DirectoryPageRequest,
        pipeline: &PipelineConfig,
        continuation: Option<PagingSession>,
        mut selection: PageSelection<'_>,
    ) -> DirectoryPageResult {
        let start_index = continuation
            .as_ref()
            .map(|state| state.start_index)
            .unwrap_or(0);
        let stable_total_count = continuation
            .as_ref()
            .and_then(|state| state.total_count)
            .unwrap_or(selection.total_matches);

        // Rows the walk ordered past this page become the next page's answer.
        let taken = request.limit.min(selection.entries.len());
        let rows: VecDeque<NodeEntry> = selection.entries.split_off(taken).into();
        let page = RetainedPage {
            entries: selection.entries,
            rows,
            tail_complete: !selection.overflowed,
            retain: selection.retains,
        };
        self.finish_ordered_page(
            path,
            owner,
            request,
            pipeline,
            start_index,
            Some(stable_total_count),
            page,
        )
    }
}
