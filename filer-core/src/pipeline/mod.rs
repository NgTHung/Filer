pub mod config;
pub mod filter;
pub mod group;
pub mod sort;

use crate::model::node::FileNode;

pub use config::{FilterConfig, GroupBy, GroupConfig, PipelineConfig, SortConfig};
pub use sort::{SortField, SortOrder};

/// Grouped file nodes with metadata
#[derive(Debug, Clone)]
pub struct GroupedNodes {
    /// Groups in display order
    pub groups: Vec<FileGroup>,
    /// Total count across all groups
    pub total_count: usize,
}

#[derive(Debug, Clone)]
pub struct FileGroup {
    /// Group label (e.g., "rs", "Today", "1-10 MB")
    pub label: String,
    /// Files in this group
    pub nodes: Vec<FileNode>,
    /// Sort order for display
    pub order: usize,
}

/// Pipeline data can be either flat or grouped
#[derive(Debug, Clone)]
pub enum PipelineData {
    Flat(Vec<FileNode>),
    Grouped(GroupedNodes),
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
                    false
                ));
            }
            
            // TODO: Add more filter stages as implemented
            // - exclude_extensions
            if !filter_config.exclude_extensions.is_empty() {
                pipeline = pipeline.add(filter::FilterByExtension::new(
                    filter_config.exclude_extensions.clone(),
                    true
                ));
            }
            
            // - min_size / max_size
            // - name_pattern
        }

        // Add sort stage
        if let Some(sort_config) = &config.sort {
            pipeline = pipeline.add(sort::SortBy::new(
                sort_config.field,
                sort_config.order,
                sort_config.directories_first,
            ));
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

    pub fn execute(&self, data: Vec<FileNode>) -> PipelineData {
        let mut pipeline_data = PipelineData::Flat(data);
        
        for stage in &self.stages {
            pipeline_data = stage.process(pipeline_data);
        }
        
        pipeline_data
    }

    /// Convenience method for flat output
    pub fn execute_flat(&self, data: Vec<FileNode>) -> Vec<FileNode> {
        match self.execute(data) {
            PipelineData::Flat(nodes) => nodes,
            PipelineData::Grouped(grouped) => {
                // Flatten if needed
                grouped.groups.into_iter()
                    .flat_map(|g| g.nodes)
                    .collect()
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