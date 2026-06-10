//! # Directory Paging
//!
//! This module owns bounded pipeline paging and cursor state for the scanner.
//! It rescans provider rows for each page so core memory grows with page size,
//! while keyset cursors prevent unchanged rows from repeating between refreshes.
//!
//! ```ignore
//! let sessions = PagingSessions::new();
//! let page = sessions.load_provider(provider, path, session, request, pipeline, cancel).await?;
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex};

use crate::actors::cancel::CancellationToken;
use crate::errors::CoreError;
use crate::model::directory::{
    DEFAULT_DIRECTORY_PAGE_SIZE, DirectoryCursor, DirectoryPageRequest, DirectoryPageResult,
    DirectoryPageState,
};
use crate::model::node::FileNode;
use crate::model::session::SessionId;
use crate::pipeline::{Pipeline, PipelineConfig, compare_nodes, effective_listing};
use crate::vfs::provider::{FsProvider, ProviderPaging, validate_page_limit};

const CURSOR_PREFIX: &str = "paging:v1:";

#[derive(Clone)]
struct PagingSession {
    owner: SessionId,
    path: PathBuf,
    request: DirectoryPageRequest,
    pipeline: PipelineConfig,
    last: FileNode,
    start_index: usize,
    total_count: usize,
}

#[derive(Clone, Default)]
pub struct PagingSessions {
    sessions: Arc<Mutex<HashMap<String, PagingSession>>>,
}

pub enum PageLoad {
    Page(DirectoryPageResult),
    Cancelled,
}

impl PagingSessions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear_session(&self, session: SessionId) {
        self.sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .retain(|_, state| state.owner != session);
    }

    pub async fn load_provider(
        &self,
        provider: &dyn FsProvider,
        path: &Path,
        owner: SessionId,
        request: DirectoryPageRequest,
        pipeline_config: &PipelineConfig,
        cancel: &CancellationToken,
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

        let mut selection = PageSelection::new(
            effective_request.limit,
            continuation.as_ref().map(|state| state.last.clone()),
            pipeline_config,
        );

        match provider.paging() {
            ProviderPaging::Fallback => {
                let entries = provider
                    .list_with_options(path, effective_request.listing)
                    .await?;
                if cancel.is_cancelled() {
                    return Ok(PageLoad::Cancelled);
                }
                selection.extend(entries);
            }
            ProviderPaging::Native => {
                let mut provider_cursor = None;
                loop {
                    if cancel.is_cancelled() {
                        return Ok(PageLoad::Cancelled);
                    }
                    let raw_page = provider
                        .list_page(
                            path,
                            DirectoryPageRequest {
                                listing: effective_request.listing,
                                limit: DEFAULT_DIRECTORY_PAGE_SIZE,
                                cursor: provider_cursor,
                            },
                        )
                        .await?;
                    if cancel.is_cancelled() {
                        return Ok(PageLoad::Cancelled);
                    }
                    let complete = raw_page.state.complete;
                    provider_cursor = raw_page.state.next_cursor;
                    let page_count = raw_page.entries.len();
                    selection.extend(raw_page.entries);
                    if complete || provider_cursor.is_none() || page_count == 0 {
                        break;
                    }
                }
            }
        }

        Ok(PageLoad::Page(self.finish_page(
            path,
            owner,
            effective_request,
            pipeline_config,
            continuation,
            selection,
        )))
    }

    pub fn load_cached(
        &self,
        entries: Vec<FileNode>,
        path: &Path,
        owner: SessionId,
        request: DirectoryPageRequest,
        pipeline_config: &PipelineConfig,
    ) -> Result<DirectoryPageResult, CoreError> {
        validate_page_limit(request.limit)?;
        let effective_request = DirectoryPageRequest {
            listing: effective_listing(pipeline_config, request.listing),
            ..request
        };
        let continuation = self.continuation(path, owner, &effective_request, pipeline_config)?;
        if effective_request.cursor.is_none() {
            self.clear_session(owner);
        }
        let mut selection = PageSelection::new(
            effective_request.limit,
            continuation.as_ref().map(|state| state.last.clone()),
            pipeline_config,
        );
        selection.extend(entries);
        Ok(self.finish_page(
            path,
            owner,
            effective_request,
            pipeline_config,
            continuation,
            selection,
        ))
    }

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
        let state = self
            .sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&cursor.0)
            .cloned()
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
        self.sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&cursor.0);
        Ok(Some(state))
    }

    fn finish_page(
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
        let has_more = selection.entries.len() > request.limit;
        selection.entries.truncate(request.limit);
        let stable_total_count = continuation
            .as_ref()
            .map(|state| state.total_count)
            .unwrap_or(selection.total_matches);
        let total_count = Some(stable_total_count);
        let next_cursor = has_more
            .then(|| selection.entries.last().cloned())
            .flatten()
            .map(|last| {
                let cursor = next_cursor();
                self.sessions
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .insert(
                        cursor.0.clone(),
                        PagingSession {
                            owner,
                            path: path.to_path_buf(),
                            request: DirectoryPageRequest {
                                cursor: None,
                                ..request.clone()
                            },
                            pipeline: pipeline.clone(),
                            last,
                            start_index: start_index + selection.entries.len(),
                            total_count: stable_total_count,
                        },
                    );
                cursor
            });
        let state = match next_cursor {
            Some(cursor) => {
                DirectoryPageState::partial(selection.entries.len(), total_count, cursor)
            }
            None => DirectoryPageState::complete(selection.entries.len(), total_count),
        }
        .with_window(start_index);
        DirectoryPageResult {
            entries: selection.entries,
            state,
        }
    }
}

struct PageSelection<'a> {
    limit: usize,
    after: Option<FileNode>,
    pipeline_config: &'a PipelineConfig,
    pipeline: Pipeline,
    entries: Vec<FileNode>,
    total_matches: usize,
}

impl<'a> PageSelection<'a> {
    fn new(limit: usize, after: Option<FileNode>, pipeline_config: &'a PipelineConfig) -> Self {
        Self {
            limit,
            after,
            pipeline_config,
            pipeline: Pipeline::from_config(pipeline_config),
            entries: Vec::with_capacity(limit.saturating_add(1)),
            total_matches: 0,
        }
    }

    fn extend(&mut self, entries: Vec<FileNode>) {
        for entry in entries {
            let mut filtered = self.pipeline.execute_flat(vec![entry]);
            let Some(entry) = filtered.pop() else {
                continue;
            };
            self.total_matches += 1;
            if self
                .after
                .as_ref()
                .is_some_and(|after| compare_nodes(self.pipeline_config, &entry, after).is_le())
            {
                continue;
            }
            let index = self
                .entries
                .binary_search_by(|candidate| {
                    compare_nodes(self.pipeline_config, candidate, &entry)
                })
                .unwrap_or_else(|index| index);
            self.entries.insert(index, entry);
            if self.entries.len() > self.limit.saturating_add(1) {
                self.entries.pop();
            }
        }
    }
}

fn next_cursor() -> DirectoryCursor {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    DirectoryCursor(format!(
        "{CURSOR_PREFIX}{}",
        COUNTER.fetch_add(1, AtomicOrdering::Relaxed)
    ))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::PageSelection;
    use crate::model::node::{FileNode, NodeId, NodeKind, NodeMeta};
    use crate::pipeline::PipelineConfig;

    #[test]
    fn selection_retains_only_page_size_plus_lookahead() {
        let config = PipelineConfig::default();
        let mut selection = PageSelection::new(10, None, &config);
        let entries = (0..10_000)
            .map(|index| FileNode {
                id: NodeId(index),
                name: format!("{index:05}.txt"),
                path: PathBuf::from(format!("/tmp/{index:05}.txt")),
                kind: NodeKind::File {
                    extension: Some("txt".into()),
                },
                size: index,
                modified: None,
                created: None,
                accessed: None,
                meta: NodeMeta::default(),
            })
            .collect();

        selection.extend(entries);

        assert_eq!(selection.total_matches, 10_000);
        assert_eq!(selection.entries.len(), 11);
    }
}
