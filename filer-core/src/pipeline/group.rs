use std::collections::HashMap;

use crate::model::node::FileNode;
use crate::pipeline::Stage;

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
    
    pub fn group(&self, input: Vec<FileNode>) -> HashMap<String, Vec<FileNode>> {
        todo!()
    }
}

impl Stage for GroupBy {
    fn process(&self, input: Vec<FileNode>) -> Vec<FileNode> {
        // Flatten groups back to vec for pipeline compatibility
        todo!()
    }
    
    fn name(&self) -> &'static str {
        "group_by"
    }
}