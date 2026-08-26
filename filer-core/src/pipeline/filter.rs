use crate::model::node::NodeEntry;
use crate::pipeline::{PipelineData, Stage};

pub struct FilterHidden {
    show_hidden: bool,
}

impl FilterHidden {
    pub fn new(show_hidden: bool) -> Self {
        Self { show_hidden }
    }

    fn filter_nodes(&self, nodes: Vec<NodeEntry>) -> Vec<NodeEntry> {
        nodes
            .into_iter()
            .filter(|entry| self.show_hidden || !entry.meta.hidden)
            .collect()
    }
}

impl Stage for FilterHidden {
    fn process(&self, input: PipelineData) -> PipelineData {
        match input {
            PipelineData::Flat(nodes) => PipelineData::Flat(self.filter_nodes(nodes)),
            PipelineData::Grouped(mut grouped) => {
                // Filter within each group
                for group in &mut grouped.groups {
                    group.nodes = self.filter_nodes(std::mem::take(&mut group.nodes));
                }
                // Remove empty groups and recalculate total
                grouped.groups.retain(|g| !g.nodes.is_empty());
                grouped.total_count = grouped.groups.iter().map(|g| g.nodes.len()).sum();
                PipelineData::Grouped(grouped)
            }
        }
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

    fn filter_nodes(&self, nodes: Vec<NodeEntry>) -> Vec<NodeEntry> {
        nodes
            .into_iter()
            .filter(|f| {
                let has_ext = self
                    .extensions
                    .iter()
                    .any(|extension| f.extension().is_some_and(|actual| actual == extension));
                if self.exclusion { !has_ext } else { has_ext }
            })
            .collect()
    }
}

impl Stage for FilterByExtension {
    fn process(&self, input: PipelineData) -> PipelineData {
        match input {
            PipelineData::Flat(nodes) => PipelineData::Flat(self.filter_nodes(nodes)),
            PipelineData::Grouped(mut grouped) => {
                // Filter within each group
                for group in &mut grouped.groups {
                    group.nodes = self.filter_nodes(std::mem::take(&mut group.nodes));
                }
                // Remove empty groups and recalculate total
                grouped.groups.retain(|g| !g.nodes.is_empty());
                grouped.total_count = grouped.groups.iter().map(|g| g.nodes.len()).sum();
                PipelineData::Grouped(grouped)
            }
        }
    }

    fn name(&self) -> &'static str {
        "filter_by_extension"
    }
}
