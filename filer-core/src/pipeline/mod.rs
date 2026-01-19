pub mod filter;
pub mod group;
pub mod sort;

use crate::model::node::FileNode;

/// A stage in the processing pipeline
pub trait Stage: Send + Sync {
    fn process(&self, input: Vec<FileNode>) -> Vec<FileNode>;
    fn name(&self) -> &'static str;
}

/// Composable pipeline of transformations
pub struct Pipeline {
    stages: Vec<Box<dyn Stage>>,
}

impl Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new() }
    }
    
    pub fn add<S: Stage + 'static>(mut self, stage: S) -> Self {
        self.stages.push(Box::new(stage));
        self
    }
    
    pub fn execute(&self, mut data: Vec<FileNode>) -> Vec<FileNode> {
        for stage in &self.stages {
            data = stage.process(data);
        }
        data
    }
}