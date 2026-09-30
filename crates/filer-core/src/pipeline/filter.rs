use crate::model::node::NodeEntry;
use crate::model::query::QueryFilter;
use crate::pipeline::{PipelineData, Stage};

pub(crate) trait RowFilter: Send + Sync {
    fn matches(&self, entry: &NodeEntry) -> bool;
}

pub struct FilterHidden {
    show_hidden: bool,
}

impl FilterHidden {
    pub fn new(show_hidden: bool) -> Self {
        Self { show_hidden }
    }

    fn matches(&self, entry: &NodeEntry) -> bool {
        self.show_hidden || !entry.meta.hidden
    }
}

impl RowFilter for FilterHidden {
    fn matches(&self, entry: &NodeEntry) -> bool {
        self.matches(entry)
    }
}

impl Stage for FilterHidden {
    fn process(&self, input: PipelineData) -> PipelineData {
        retain_nodes(input, |entry| self.matches(entry))
    }

    fn name(&self) -> &'static str {
        "filter_hidden"
    }
}

pub struct FilterByExtension {
    extensions: Vec<String>,
    exclusion: bool,
}

impl FilterByExtension {
    pub fn new(extensions: Vec<String>, exclusion: bool) -> Self {
        Self {
            extensions,
            exclusion,
        }
    }

    fn matches(&self, entry: &NodeEntry) -> bool {
        let has_extension = self
            .extensions
            .iter()
            .any(|extension| entry.extension().is_some_and(|actual| actual == extension));
        if self.exclusion {
            !has_extension
        } else {
            has_extension
        }
    }
}

impl RowFilter for FilterByExtension {
    fn matches(&self, entry: &NodeEntry) -> bool {
        self.matches(entry)
    }
}

impl Stage for FilterByExtension {
    fn process(&self, input: PipelineData) -> PipelineData {
        retain_nodes(input, |entry| self.matches(entry))
    }

    fn name(&self) -> &'static str {
        "filter_by_extension"
    }
}

pub(crate) struct FilterByQuery {
    filters: Vec<QueryFilter>,
}

impl FilterByQuery {
    pub(crate) fn new(filters: Vec<QueryFilter>) -> Self {
        Self { filters }
    }

    fn matches(&self, entry: &NodeEntry) -> bool {
        self.filters.iter().all(|filter| filter.matches(entry))
    }
}

impl RowFilter for FilterByQuery {
    fn matches(&self, entry: &NodeEntry) -> bool {
        self.matches(entry)
    }
}

impl Stage for FilterByQuery {
    fn process(&self, input: PipelineData) -> PipelineData {
        retain_nodes(input, |entry| self.matches(entry))
    }

    fn name(&self) -> &'static str {
        "filter_by_query"
    }
}

pub(crate) fn retain_nodes(
    input: PipelineData,
    mut predicate: impl FnMut(&NodeEntry) -> bool,
) -> PipelineData {
    match input {
        PipelineData::Flat(mut nodes) => {
            nodes.retain(&mut predicate);
            PipelineData::Flat(nodes)
        }
        PipelineData::Grouped(mut grouped) => {
            for group in &mut grouped.groups {
                group.nodes.retain(&mut predicate);
            }
            grouped.groups.retain(|group| !group.nodes.is_empty());
            grouped.total_count = grouped.groups.iter().map(|group| group.nodes.len()).sum();
            PipelineData::Grouped(grouped)
        }
    }
}
