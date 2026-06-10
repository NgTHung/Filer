use serde::{Deserialize, Serialize};

use crate::vfs::provider::ListingOptions;

pub const DEFAULT_DIRECTORY_PAGE_SIZE: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DirectoryCursor(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DirectoryLoadMode {
    Snapshot {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<usize>,
    },
    Page {
        limit: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cursor: Option<DirectoryCursor>,
    },
}

/// Controls how a directory scan loads and bounds its result rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryLoadOptions {
    #[serde(default)]
    pub listing: ListingOptions,
    #[serde(default)]
    pub mode: DirectoryLoadMode,
}

impl DirectoryLoadOptions {
    pub fn unbounded(listing: ListingOptions) -> Self {
        Self {
            listing,
            mode: DirectoryLoadMode::Snapshot { limit: None },
        }
    }

    pub fn bounded(limit: usize) -> Self {
        Self {
            listing: ListingOptions::fast(),
            mode: DirectoryLoadMode::Snapshot { limit: Some(limit) },
        }
    }

    pub fn bounded_with_listing(limit: usize, listing: ListingOptions) -> Self {
        Self {
            listing,
            mode: DirectoryLoadMode::Snapshot { limit: Some(limit) },
        }
    }

    pub fn page(limit: usize) -> Self {
        Self {
            listing: ListingOptions::fast(),
            mode: DirectoryLoadMode::Page {
                limit,
                cursor: None,
            },
        }
    }

    pub fn page_after(limit: usize, cursor: DirectoryCursor) -> Self {
        Self {
            listing: ListingOptions::fast(),
            mode: DirectoryLoadMode::Page {
                limit,
                cursor: Some(cursor),
            },
        }
    }

    pub fn is_bounded(&self) -> bool {
        matches!(self.mode, DirectoryLoadMode::Snapshot { limit: Some(_) })
    }

    pub fn is_paged(&self) -> bool {
        matches!(self.mode, DirectoryLoadMode::Page { .. })
    }

    pub fn snapshot_limit(&self) -> Option<usize> {
        match self.mode {
            DirectoryLoadMode::Snapshot { limit } => limit,
            DirectoryLoadMode::Page { .. } => None,
        }
    }

    pub fn page_request(&self) -> Option<DirectoryPageRequest> {
        match &self.mode {
            DirectoryLoadMode::Page { limit, cursor } => Some(DirectoryPageRequest {
                listing: self.listing,
                limit: *limit,
                cursor: cursor.clone(),
            }),
            DirectoryLoadMode::Snapshot { .. } => None,
        }
    }
}

impl Default for DirectoryLoadOptions {
    fn default() -> Self {
        Self {
            listing: ListingOptions::fast(),
            mode: DirectoryLoadMode::Page {
                limit: DEFAULT_DIRECTORY_PAGE_SIZE,
                cursor: None,
            },
        }
    }
}

impl Default for DirectoryLoadMode {
    fn default() -> Self {
        Self::Page {
            limit: DEFAULT_DIRECTORY_PAGE_SIZE,
            cursor: None,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryPageRequest {
    pub listing: ListingOptions,
    pub limit: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<DirectoryCursor>,
}

#[derive(Debug, Clone)]
pub struct DirectoryPageResult {
    pub entries: Vec<crate::model::node::FileNode>,
    pub state: DirectoryPageState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryPageState {
    pub page_count: usize,
    pub total_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<DirectoryCursor>,
    pub complete: bool,
    /// Zero-based index of the first row in this page.
    #[serde(default)]
    pub start_index: usize,
    /// Number of rows loaded through the end of this page.
    #[serde(default)]
    pub loaded_count: usize,
}

impl DirectoryPageState {
    pub fn partial(
        page_count: usize,
        total_count: Option<usize>,
        next_cursor: DirectoryCursor,
    ) -> Self {
        Self {
            page_count,
            total_count,
            next_cursor: Some(next_cursor),
            complete: false,
            start_index: 0,
            loaded_count: page_count,
        }
    }

    pub fn complete(page_count: usize, total_count: Option<usize>) -> Self {
        Self {
            page_count,
            total_count,
            next_cursor: None,
            complete: true,
            start_index: 0,
            loaded_count: page_count,
        }
    }

    pub const fn with_window(mut self, start_index: usize) -> Self {
        self.start_index = start_index;
        self.loaded_count = start_index.saturating_add(self.page_count);
        self
    }
}
