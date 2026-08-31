//! # Directory Listing Streams
//!
//! A directory stream is a walk a caller can stop and resume later. Paged
//! listing needs this because an offset cursor cannot resume `read_dir`, which
//! has no seek: every continuation would re-read the prefix it already skipped,
//! so a full walk in pages costs work quadratic in the directory size.
//!
//! A stream owns whatever handle its provider needs and releases it on drop, so
//! a caller that abandons a page chain only has to drop the stream.
//!
//! ```ignore
//! let mut stream = provider
//!     .open_listing(path, ListingOptions::fast(), &cx)
//!     .await?
//!     .expect("provider pages natively");
//! let batch = stream.next_batch(256, &cx).await?;
//! ```

use async_trait::async_trait;

use crate::errors::CoreError;
use crate::model::node::NodeEntry;
use crate::vfs::context::ProviderCx;

/// One step of a directory walk.
///
/// `end_of_directory` is explicit rather than inferred from an empty batch,
/// because a batch can be empty when every entry in it was skipped.
#[derive(Debug, Default)]
pub struct ListingBatch {
    pub entries: Vec<NodeEntry>,
    pub end_of_directory: bool,
}

impl ListingBatch {
    pub fn partial(entries: Vec<NodeEntry>) -> Self {
        Self {
            entries,
            end_of_directory: false,
        }
    }

    pub fn final_batch(entries: Vec<NodeEntry>) -> Self {
        Self {
            entries,
            end_of_directory: true,
        }
    }
}

/// A resumable directory walk that yields entries in provider order.
#[async_trait]
pub trait DirectoryStream: Send {
    /// Yield at most `max` further entries.
    ///
    /// Returns a batch flagged `end_of_directory` once the walk is exhausted.
    /// Calling again after that flag yields an empty terminal batch.
    async fn next_batch(
        &mut self,
        max: usize,
        cx: &ProviderCx<'_>,
    ) -> Result<ListingBatch, CoreError>;
}
