use crate::model::node::FileNode;
use crate::pipeline::Stage;

#[derive(Debug, Clone, Copy)]
pub enum SortField {
    Name,
    Size,
    Modified,
    Extension,
}

#[derive(Debug, Clone, Copy)]
pub enum SortOrder {
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
        Self { field, order, directories_first }
    }
}

impl Stage for SortBy {
    fn process(&self, input: Vec<FileNode>) -> Vec<FileNode> {
        todo!()
    }
    
    fn name(&self) -> &'static str {
        "sort_by"
    }
}