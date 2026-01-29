use crate::pipeline::{FileGroup, GroupedNodes};

impl GroupedNodes{
    pub fn contain_group(&self, key: &str) -> bool{
        self.groups.iter().any(|f| {
            f.label.eq_ignore_ascii_case(key)
        })
    }
    pub fn get(&self, key: &str) -> Option<&FileGroup> {
        self.groups.iter().find(|f| {
            f.label.eq_ignore_ascii_case(key)
        })
    }
}