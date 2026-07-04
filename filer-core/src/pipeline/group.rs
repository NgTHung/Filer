use rapidhash::RapidHashMap;

use crate::model::node::FileNode;
use crate::pipeline::config::{GroupBy as ConfigGroupBy, GroupConfig, PipelineConfig};
use crate::pipeline::order::{GroupSortKey, group_label, group_sort_key};
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

        let mut groups_map: RapidHashMap<String, (GroupSortKey, Vec<FileNode>)> =
            RapidHashMap::default();

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

            let config = PipelineConfig {
                group: Some(GroupConfig { by }),
                ..PipelineConfig::default()
            };
            let sort_key = group_sort_key(&config, &node);

            groups_map
                .entry(key)
                .or_insert_with(|| (sort_key, Vec::new()))
                .1
                .push(node);
        }

        let mut groups: Vec<(GroupSortKey, FileGroup)> = groups_map
            .into_iter()
            .enumerate()
            .map(|(idx, (label, (sort_key, nodes)))| {
                (
                    sort_key,
                    FileGroup {
                        label,
                        nodes,
                        order: idx,
                    },
                )
            })
            .collect();

        groups.sort_by(|(left_key, left), (right_key, right)| {
            left_key
                .cmp(right_key)
                .then_with(|| left.label.cmp(&right.label))
        });

        let mut groups: Vec<FileGroup> = groups.into_iter().map(|(_, group)| group).collect();
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
