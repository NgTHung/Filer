use serde::{Deserialize, Serialize};

use crate::vfs::provider::ListingOptions;

/// Controls how a directory scan loads and bounds its result rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryLoadOptions {
    #[serde(default)]
    pub listing: ListingOptions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

impl DirectoryLoadOptions {
    pub const fn unbounded(listing: ListingOptions) -> Self {
        Self {
            listing,
            limit: None,
        }
    }

    pub const fn bounded(limit: usize) -> Self {
        Self {
            listing: ListingOptions::fast(),
            limit: Some(limit),
        }
    }

    pub const fn bounded_with_listing(limit: usize, listing: ListingOptions) -> Self {
        Self {
            listing,
            limit: Some(limit),
        }
    }

    pub const fn is_bounded(&self) -> bool {
        self.limit.is_some()
    }
}

impl Default for DirectoryLoadOptions {
    fn default() -> Self {
        Self {
            listing: ListingOptions::fast(),
            limit: None,
        }
    }
}

/// Describes whether a directory result represents the complete scan result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryLoadState {
    pub loaded_count: usize,
    pub total_count: Option<usize>,
    pub complete: bool,
}

impl DirectoryLoadState {
    pub const fn complete(total_count: usize) -> Self {
        Self {
            loaded_count: total_count,
            total_count: Some(total_count),
            complete: true,
        }
    }

    pub const fn from_counts(loaded_count: usize, total_count: usize) -> Self {
        Self {
            loaded_count,
            total_count: Some(total_count),
            complete: loaded_count == total_count,
        }
    }
}
