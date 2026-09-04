use crate::model::node::NodeEntry;
use crate::model::query::QueryFilter;
use crate::pipeline::{PipelineData, Stage};

pub struct FilterHidden {
    show_hidden: bool,
}

impl FilterHidden {
    pub fn new(show_hidden: bool) -> Self {
        Self { show_hidden }
    }
}

impl Stage for FilterHidden {
    fn process(&self, input: PipelineData) -> PipelineData {
        retain_nodes(input, |entry| self.show_hidden || !entry.meta.hidden)
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
}

impl Stage for FilterByExtension {
    fn process(&self, input: PipelineData) -> PipelineData {
        retain_nodes(input, |entry| {
            let has_extension = self
                .extensions
                .iter()
                .any(|extension| entry.extension().is_some_and(|actual| actual == extension));
            if self.exclusion {
                !has_extension
            } else {
                has_extension
            }
        })
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
}

impl Stage for FilterByQuery {
    fn process(&self, input: PipelineData) -> PipelineData {
        retain_nodes(input, |node| {
            self.filters.iter().all(|filter| filter.matches(node))
        })
    }

    fn name(&self) -> &'static str {
        "filter_by_query"
    }
}

fn retain_nodes(
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
