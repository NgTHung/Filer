use rapidhash::RapidHashMap;

use crate::model::node::NodeEntry;
use crate::pipeline::config::GroupBy as ConfigGroupBy;
use crate::pipeline::order::{GroupSortKey, group_label, group_sort_key};
use crate::pipeline::{EntryGroup, GroupedEntries, PipelineData, Stage};

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

        let by = match self.field {
            GroupField::Extension => ConfigGroupBy::Extension,
            GroupField::Date => ConfigGroupBy::Date,
            GroupField::Size => ConfigGroupBy::Size,
            GroupField::FirstLetter => ConfigGroupBy::FirstLetter,
        };
        let mut groups_map: RapidHashMap<String, (Option<GroupSortKey>, Vec<NodeEntry>)> =
            RapidHashMap::default();

        for node in nodes {
            let label = group_label(by, &node);
            groups_map
                .entry(label)
                .or_insert_with(|| (group_sort_key(by, &node), Vec::new()))
                .1
                .push(node);
        }

        let mut groups: Vec<(Option<GroupSortKey>, EntryGroup)> = groups_map
            .into_iter()
            .enumerate()
            .map(|(idx, (label, (sort_key, nodes)))| {
                (
                    sort_key,
                    EntryGroup {
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

        let mut groups: Vec<EntryGroup> = groups.into_iter().map(|(_, group)| group).collect();
        for (idx, group) in groups.iter_mut().enumerate() {
            group.order = idx;
        }

        let total_count = groups.iter().map(|g| g.nodes.len()).sum();

        PipelineData::Grouped(GroupedEntries {
            groups,
            total_count,
        })
    }

    fn name(&self) -> &'static str {
        "group_by"
    }
}
