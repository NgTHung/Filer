pub mod config;
pub mod filter;
pub mod group;
mod order;
pub mod sort;

use crate::model::directory::DirectoryLoadState;
use crate::model::node::NodeEntry;

#[allow(unused_imports)]
pub use config::{
    FilterConfig, GroupBy, GroupConfig, PipelineConfig, PipelinePagingMode, SortConfig,
};
pub use order::compare_nodes;
pub(crate) use order::effective_listing;

/// Grouped node entries with metadata.
#[derive(Debug, Clone)]
pub struct GroupedEntries {
    /// Groups in display order
    pub groups: Vec<EntryGroup>,
    /// Total count across all groups
    pub total_count: usize,
}

#[derive(Debug, Clone)]
pub struct EntryGroup {
    pub label: String,
    pub nodes: Vec<NodeEntry>,
    pub order: usize,
}

impl GroupedEntries {
    pub fn limited(self, limit: Option<usize>) -> (Self, DirectoryLoadState) {
        let total_count = self.total_count;
        let Some(limit) = limit else {
            return (self, DirectoryLoadState::complete(total_count));
        };

        let mut remaining = limit;
        let mut loaded_count = 0;
        let mut groups = Vec::new();

        for mut group in self.groups {
            if remaining == 0 {
                break;
            }

            if group.nodes.len() > remaining {
                group.nodes.truncate(remaining);
            }

            let group_count = group.nodes.len();
            if group_count > 0 {
                loaded_count += group_count;
                remaining -= group_count;
                groups.push(group);
            }
        }

        (
            Self {
                groups,
                total_count: loaded_count,
            },
            DirectoryLoadState::from_counts(loaded_count, total_count),
        )
    }
}

/// Pipeline data can be either flat or grouped
#[derive(Debug, Clone)]
pub enum PipelineData {
    Flat(Vec<NodeEntry>),
    Grouped(GroupedEntries),
}

/// A stage in the processing pipeline
pub trait Stage: Send + Sync {
    fn process(&self, input: PipelineData) -> PipelineData;
    fn name(&self) -> &'static str;
}

/// Composable pipeline of transformations
///
/// The Pipeline itself is NOT serializable (contains trait objects).
/// Use `PipelineConfig` for cross-process communication and build
/// the Pipeline in-core with `Pipeline::from_config()`.
pub struct Pipeline {
    stages: Vec<Box<dyn Stage>>,
}

impl Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new() }
    }

    /// Build a Pipeline from a serializable PipelineConfig
    ///
    /// This is the bridge between the serializable config (sent from frontend)
    /// and the actual executable pipeline (used in core).
    pub fn from_config(config: &PipelineConfig) -> Self {
        let mut pipeline = Pipeline::new();

        // Add filter stages
        if let Some(filter_config) = &config.filter {
            // Hidden files filter
            pipeline = pipeline.add(filter::FilterHidden::new(filter_config.show_hidden));

            // Extension filter (include)
            if !filter_config.include_extensions.is_empty() {
                pipeline = pipeline.add(filter::FilterByExtension::new(
                    filter_config.include_extensions.clone(),
                    false,
                ));
            }

            // TODO: Add more filter stages as implemented
            // - exclude_extensions
            if !filter_config.exclude_extensions.is_empty() {
                pipeline = pipeline.add(filter::FilterByExtension::new(
                    filter_config.exclude_extensions.clone(),
                    true,
                ));
            }

            // - min_size / max_size
            // - name_pattern
        }

        // Add order stage
        if config.sort.is_some() || config.group.is_some() {
            pipeline = pipeline.add(sort::SortBy::from_config(config.clone()));
        }

        // Add group stage
        if let Some(group_config) = &config.group {
            let group_field = match group_config.by {
                GroupBy::None => None,
                GroupBy::Extension => Some(group::GroupField::Extension),
                GroupBy::Date => Some(group::GroupField::Date),
                GroupBy::Size => Some(group::GroupField::Size),
                GroupBy::FirstLetter => Some(group::GroupField::FirstLetter),
                GroupBy::Type => Some(group::GroupField::Extension), // Map to extension for now
            };

            if let Some(field) = group_field {
                pipeline = pipeline.add(group::GroupBy::new(field));
            }
        }

        pipeline
    }

    pub fn add<S: Stage + 'static>(mut self, stage: S) -> Self {
        self.stages.push(Box::new(stage));
        self
    }

    pub fn execute(&self, data: Vec<NodeEntry>) -> PipelineData {
        let mut pipeline_data = PipelineData::Flat(data);

        for stage in &self.stages {
            pipeline_data = stage.process(pipeline_data);
        }

        pipeline_data
    }

    /// Execute the pipeline and always return `GroupedEntries`.
    ///
    /// When no grouping stage is configured the result is a single
    /// `EntryGroup` with an empty label, a degenerate flat list that
    /// the UI can render without section headers.
    pub fn execute_grouped(&self, data: Vec<NodeEntry>) -> GroupedEntries {
        match self.execute(data) {
            PipelineData::Grouped(grouped) => grouped,
            PipelineData::Flat(nodes) => {
                let total_count = nodes.len();
                GroupedEntries {
                    groups: vec![EntryGroup {
                        label: String::new(),
                        nodes,
                        order: 0,
                    }],
                    total_count,
                }
            }
        }
    }

    /// Convenience method for flat output
    pub fn execute_flat(&self, data: Vec<NodeEntry>) -> Vec<NodeEntry> {
        match self.execute(data) {
            PipelineData::Flat(nodes) => nodes,
            PipelineData::Grouped(grouped) => {
                // Flatten if needed
                grouped.groups.into_iter().flat_map(|g| g.nodes).collect()
            }
        }
    }

    /// Get number of stages
    pub fn len(&self) -> usize {
        self.stages.len()
    }

    /// Check if pipeline is empty
    pub fn is_empty(&self) -> bool {
        self.stages.is_empty()
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}
