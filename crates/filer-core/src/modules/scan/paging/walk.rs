//! # Full Directory Walks
//!
//! Sorting and grouping need every row before the first page is correct, so an
//! ordered chain walks the whole directory into a [`PageSelection`]. The walk
//! reads the provider's listing stream when it has one, because an offset
//! cursor cannot resume `read_dir` and every provider page would re-read the
//! prefix before it. On a 10,000-entry local directory, 256-row offset pages
//! read about 200,000 entries and took 80 ms, while one stream read 10,000
//! entries in 8 ms. Providers without a stream fall back to their own pages or
//! one full listing.
//!
//! ```ignore
//! let mut selection = PageSelection::with_lookahead(limit, lookahead, None, &config);
//! let complete = walk_into(provider, path, listing, &mut selection, &cx).await?;
//! ```

use std::path::Path;

use crate::errors::CoreError;
use crate::model::directory::{DEFAULT_DIRECTORY_PAGE_SIZE, DirectoryPageRequest};
use crate::vfs::context::ProviderCx;
use crate::vfs::provider::{FsProvider, ListingOptions, ProviderPaging};

use super::PageSelection;

/// Walks every row of `path` into `selection`.
///
/// Returns `Ok(false)` when the selection saw cancellation. Provider errors,
/// including cancellation a provider reports itself, return as errors.
pub(super) async fn walk_into(
    provider: &dyn FsProvider,
    path: &Path,
    listing: ListingOptions,
    selection: &mut PageSelection<'_>,
    cx: &ProviderCx<'_>,
) -> Result<bool, CoreError> {
    let scheme = provider.scheme();
    if let Some(mut stream) = cx
        .race(scheme, provider.open_listing(path, listing, cx))
        .await?
    {
        loop {
            let batch = cx
                .race(scheme, stream.next_batch(DEFAULT_DIRECTORY_PAGE_SIZE, cx))
                .await?;
            if !selection.extend(batch.entries, cx) {
                return Ok(false);
            }
            if batch.end_of_directory {
                return Ok(true);
            }
        }
    }

    match provider.paging() {
        ProviderPaging::Fallback => {
            let entries = cx
                .race(scheme, provider.list_with_options(path, listing, cx))
                .await?;
            Ok(selection.extend(entries, cx))
        }
        ProviderPaging::Native => {
            let mut cursor = None;
            loop {
                let page = cx
                    .race(
                        scheme,
                        provider.list_page(
                            path,
                            DirectoryPageRequest {
                                listing,
                                limit: DEFAULT_DIRECTORY_PAGE_SIZE,
                                cursor,
                            },
                            cx,
                        ),
                    )
                    .await?;
                let complete = page.state.complete;
                cursor = page.state.next_cursor;
                let page_count = page.entries.len();
                if !selection.extend(page.entries, cx) {
                    return Ok(false);
                }
                if complete || cursor.is_none() || page_count == 0 {
                    return Ok(true);
                }
            }
        }
    }
}
