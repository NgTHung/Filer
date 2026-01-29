use serde::{Deserialize, Serialize};

use crate::model::node::FileNode;
use crate::pipeline::{PipelineData, Stage};

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
    field: SortField,
    order: SortOrder,
    directories_first: bool,
}

impl SortBy {
    pub fn new(field: SortField, order: SortOrder, directories_first: bool) -> Self {
        Self {
            field,
            order,
            directories_first,
        }
    }
    
    fn handle_order(&self, ord: std::cmp::Ordering) -> std::cmp::Ordering {
        match self.order {
            SortOrder::Ascending => ord,
            SortOrder::Descending => ord.reverse(),
        }
    }
    
    fn sort_nodes(&self, mut nodes: Vec<FileNode>) -> Vec<FileNode> {
        nodes.sort_by(|a, b| {
            if self.directories_first && a.is_dir() != b.is_dir() {
                if a.is_dir() {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                }
            }
            else {
                match self.field {
                    SortField::Name => self.handle_order(a.name.cmp(&b.name)),
                    SortField::Size => self.handle_order(a.size.cmp(&b.size)),
                    SortField::Modified => self.handle_order(a.modified.cmp(&b.modified)),
                    SortField::Created => self.handle_order(a.created.cmp(&b.created)),
                    SortField::Extension => {
                        let exta = a.extension();
                        let extb = b.extension();
                        if exta.is_some() != extb.is_some() {
                            if exta.is_some(){
                                self.handle_order(std::cmp::Ordering::Less)
                            }
                            else{
                                self.handle_order(std::cmp::Ordering::Greater)
                            }
                        }
                        else if exta.is_none() {
                            std::cmp::Ordering::Equal
                        }
                        else {
                            let exta = exta.unwrap();
                            let extb = extb.unwrap();
                            self.handle_order(exta.cmp(extb))
                        }
                    },
                    SortField::Type => self.handle_order(a.name.cmp(&b.name)),
                }
            }
        });
        nodes
    }
}

impl Stage for SortBy {
    fn process(&self, input: PipelineData) -> PipelineData {
        match input {
            PipelineData::Flat(nodes) => {
                PipelineData::Flat(self.sort_nodes(nodes))
            }
            PipelineData::Grouped(mut grouped) => {
                // Sort within each group
                for group in &mut grouped.groups {
                    group.nodes = self.sort_nodes(group.nodes.clone());
                }
                PipelineData::Grouped(grouped)
            }
        }
    }

    fn name(&self) -> &'static str {
        "sort_by"
    }
}
