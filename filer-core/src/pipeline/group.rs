use rapidhash::RapidHashMap;

use crate::model::node::FileNode;
use crate::pipeline::config::{GroupBy as ConfigGroupBy, GroupConfig, PipelineConfig};
use crate::pipeline::order::group_label;
use crate::pipeline::{FileGroup, GroupedNodes, PipelineData, Stage};

#[derive(Debug, Clone, Copy)]
pub enum GroupField {
    Extension,
    Date,
    Size,
    FirstLetter,
}

pub struct GroupBy {
    field: GroupField,
}

impl GroupBy {
    pub fn new(field: GroupField) -> Self {
        Self { field }
    }
}

impl Stage for GroupBy {
    fn process(&self, input: PipelineData) -> PipelineData {
        let nodes = match input {
            PipelineData::Flat(v) => v,
            PipelineData::Grouped(g) => {
                // Flatten existing groups if re-grouping
                g.groups.into_iter().flat_map(|g| g.nodes).collect()
            }
        };

        let mut groups_map: RapidHashMap<String, Vec<FileNode>> = RapidHashMap::default();

        for node in nodes {
            let by = match self.field {
                GroupField::Extension => ConfigGroupBy::Extension,
                GroupField::Date => ConfigGroupBy::Date,
                GroupField::Size => ConfigGroupBy::Size,
                GroupField::FirstLetter => ConfigGroupBy::FirstLetter,
            };
            let key = group_label(
                &PipelineConfig {
                    group: Some(GroupConfig { by }),
                    ..PipelineConfig::default()
                },
                &node,
            );

            groups_map.entry(key).or_default().push(node);
        }

        // Convert to ordered Vec
        let mut groups: Vec<FileGroup> = groups_map
            .into_iter()
            .enumerate()
            .map(|(idx, (label, nodes))| FileGroup {
                label,
                nodes,
                order: idx,
            })
            .collect();

        // Sort groups by label
        groups.sort_by(|a, b| a.label.cmp(&b.label));

        // Update order after sorting
        for (idx, group) in groups.iter_mut().enumerate() {
            group.order = idx;
        }

        let total_count = groups.iter().map(|g| g.nodes.len()).sum();

        PipelineData::Grouped(GroupedNodes {
            groups,
            total_count,
        })
    }

    fn name(&self) -> &'static str {
        "group_by"
    }
}
