use crate::model::node::FileNode;
use crate::pipeline::Stage;

pub struct FilterHidden {
    show_hidden: bool,
}

impl FilterHidden {
    pub fn new(show_hidden: bool) -> Self {
        Self { show_hidden }
    }
}

impl Stage for FilterHidden {
    fn process(&self, input: Vec<FileNode>) -> Vec<FileNode> {
        todo!()
    }
    
    fn name(&self) -> &'static str {
        "filter_hidden"
    }
}

pub struct FilterByExtension {
    extensions: Vec<String>,
}

impl FilterByExtension {
    pub fn new(extensions: Vec<String>) -> Self {
        Self { extensions }
    }
}

impl Stage for FilterByExtension {
    fn process(&self, input: Vec<FileNode>) -> Vec<FileNode> {
        todo!()
    }
    
    fn name(&self) -> &'static str {
        "filter_by_extension"
    }
}