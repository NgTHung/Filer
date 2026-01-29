use std::collections::HashMap;
use std::time::SystemTime;

use crate::model::node::FileNode;
use crate::pipeline::{FileGroup, GroupedNodes, PipelineData, Stage};
use crate::utils;

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
                g.groups.into_iter()
                    .flat_map(|g| g.nodes)
                    .collect()
            }
        };
        
        let mut groups_map: HashMap<String, Vec<FileNode>> = HashMap::new();
        
        for node in nodes {
            let key = match self.field {
                GroupField::Extension => {
                    node.extension()
                        .unwrap_or("No extension")
                        .to_string()
                }
                GroupField::Date => {
                    utils::time_group_name(node.modified.unwrap_or(SystemTime::now())).to_string()
                }
                GroupField::Size => utils::size_group_name(node.size).to_string(),
                GroupField::FirstLetter => {
                    node.name.chars().next()
                        .unwrap_or('#')
                        .to_uppercase()
                        .to_string()
                }
            };
            
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