use serde::{Deserialize, Serialize};

use crate::model::node::NodeEntry;
use crate::pipeline::{PipelineConfig, PipelineData, SortConfig, Stage, compare_nodes};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortField {
    #[default]
    Name,
    Size,
    Modified,
    Created,
    Extension,
    Type,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    #[default]
    Ascending,
    Descending,
}

pub struct SortBy {
    config: PipelineConfig,
}

impl SortBy {
    pub fn new(field: SortField, order: SortOrder, directories_first: bool) -> Self {
        Self::from_config(PipelineConfig {
            sort: Some(SortConfig {
                field,
                order,
                directories_first,
            }),
            filter: None,
            group: None,
        })
    }

    pub(crate) fn from_config(config: PipelineConfig) -> Self {
        Self { config }
    }

    fn sort_nodes(&self, mut nodes: Vec<NodeEntry>) -> Vec<NodeEntry> {
        nodes.sort_by(|a, b| compare_nodes(&self.config, a, b));
        nodes
    }
}

impl Stage for SortBy {
    fn process(&self, input: PipelineData) -> PipelineData {
        match input {
            PipelineData::Flat(nodes) => PipelineData::Flat(self.sort_nodes(nodes)),
            PipelineData::Grouped(mut grouped) => {
                for group in &mut grouped.groups {
                    group.nodes = self.sort_nodes(std::mem::take(&mut group.nodes));
                }
                PipelineData::Grouped(grouped)
            }
        }
    }

    fn name(&self) -> &'static str {
        "sort_by"
    }
}
