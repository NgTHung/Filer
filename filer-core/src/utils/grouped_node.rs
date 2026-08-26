use crate::pipeline::{EntryGroup, GroupedEntries};

impl GroupedEntries {
    pub fn contain_group(&self, key: &str) -> bool {
        self.groups
            .iter()
            .any(|f| f.label.eq_ignore_ascii_case(key))
    }
    pub fn get(&self, key: &str) -> Option<&EntryGroup> {
        self.groups
            .iter()
            .find(|f| f.label.eq_ignore_ascii_case(key))
    }
}
