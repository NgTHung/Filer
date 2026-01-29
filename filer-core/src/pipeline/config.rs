//! Pipeline configuration - serializable structs for cross-process communication
//!
//! These config types are small and serializable, suitable for sending between
//! frontend (web/desktop) and core. The actual Pipeline with trait objects
//! is built from these configs inside the core.

use serde::{Deserialize, Serialize};

use crate::pipeline::sort::{SortField, SortOrder};

/// Pipeline configuration - small, serializable, sent to/from frontend
///
/// # Example
/// ```
/// use filer_core::pipeline::config::{PipelineConfig, SortConfig, FilterConfig};
/// use filer_core::pipeline::sort::{SortField, SortOrder};
///
/// let config = PipelineConfig {
///     sort: Some(SortConfig {
///         field: SortField::Name,
///         order: SortOrder::Ascending,
///         directories_first: true,
///     }),
///     filter: Some(FilterConfig {
///         show_hidden: false,
///         ..Default::default()
///     }),
///     group: None,
/// };
///
/// // Serialize for sending over WebSocket (~100-200 bytes)
/// let json = serde_json::to_string(&config).unwrap();
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PipelineConfig {
    /// Sort configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<SortConfig>,
    /// Filter configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<FilterConfig>,
    /// Group configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<GroupConfig>,
}

impl PipelineConfig {
    /// Create a new empty pipeline config
    pub fn new() -> Self {
        Self::default()
    }

    /// Create config with default sorting (name, ascending, dirs first)
    pub fn with_default_sort() -> Self {
        Self {
            sort: Some(SortConfig::default()),
            filter: Some(FilterConfig::default()),
            group: None,
        }
    }

    /// Set sort configuration
    pub fn sort(mut self, field: SortField, order: SortOrder, directories_first: bool) -> Self {
        self.sort = Some(SortConfig {
            field,
            order,
            directories_first,
        });
        self
    }

    /// Set filter configuration
    pub fn filter(mut self, filter: FilterConfig) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Set show hidden files
    pub fn show_hidden(mut self, show: bool) -> Self {
        let filter = self.filter.get_or_insert_with(FilterConfig::default);
        filter.show_hidden = show;
        self
    }

    /// Set group configuration
    pub fn group_by(mut self, by: GroupBy) -> Self {
        self.group = Some(GroupConfig { by });
        self
    }
}

/// Sort configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortConfig {
    /// Field to sort by
    pub field: SortField,
    /// Sort order (ascending/descending)
    pub order: SortOrder,
    /// Whether to show directories before files
    pub directories_first: bool,
}

impl Default for SortConfig {
    fn default() -> Self {
        Self {
            field: SortField::Name,
            order: SortOrder::Ascending,
            directories_first: true,
        }
    }
}

/// Filter configuration
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilterConfig {
    /// Show hidden files (dotfiles on Unix)
    #[serde(default)]
    pub show_hidden: bool,

    /// Only show files with these extensions (empty = show all)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include_extensions: Vec<String>,

    /// Hide files with these extensions
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude_extensions: Vec<String>,

    /// Minimum file size in bytes (None = no limit)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_size: Option<u64>,

    /// Maximum file size in bytes (None = no limit)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_size: Option<u64>,

    /// Only show files matching this name pattern (glob)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_pattern: Option<String>,
}

impl FilterConfig {
    /// Create filter that shows only specified extensions
    pub fn only_extensions(extensions: Vec<String>) -> Self {
        Self {
            include_extensions: extensions,
            ..Default::default()
        }
    }

    /// Create filter that hides specified extensions
    pub fn exclude_extensions(extensions: Vec<String>) -> Self {
        Self {
            exclude_extensions: extensions,
            ..Default::default()
        }
    }
}

/// Group configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupConfig {
    /// How to group files
    pub by: GroupBy,
}

/// Grouping strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
    /// No grouping
    #[default]
    None,
    /// Group by file extension
    Extension,
    /// Group by modification date (today, yesterday, this week, etc.)
    Date,
    /// Group by size category (tiny, small, medium, large, huge)
    Size,
    /// Group by first letter of name
    FirstLetter,
    /// Group by MIME type category (images, documents, videos, etc.)
    Type,
}
