//! # Task Edits
//!
//! This module applies partial task edits without bypassing validation. Local
//! fields validate only the in-memory target, while relationship fields validate
//! that candidate with every unchanged task in the real project.
//!
//! ```
//! use taskroot::{
//!     identity::TaskIdentity,
//!     lifecycle::{TaskPatch, edit_task},
//!     project::TaskProject,
//! };
//!
//! # let root = tempfile::tempdir()?;
//! # std::fs::create_dir_all(root.path().join(".tasks/core"))?;
//! # std::fs::write(root.path().join(".tasks/core/UTILS-998-example.md"), "---\nid: UTILS-998\ntitle: Example task\nstatus: To Do\npriority: Medium\ntype: Feature\n---\n\n## Summary\n\nOld.\n\n## Acceptance Criteria\n\n- [ ] Works\n")?;
//! let project = TaskProject::open(root.path())?;
//! edit_task(
//!     &project,
//!     &TaskIdentity::new("core", "UTILS-998")?,
//!     TaskPatch {
//!         title: Some("Updated example task".to_string()),
//!         ..TaskPatch::default()
//!     },
//! )?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::{collections::BTreeMap, fs, path::PathBuf};

use crate::{
    atomic_write,
    error::{TaskError, TaxonomyErrorContext, ValidationError},
    frontmatter::{parse_metadata, render_metadata},
    identity::TaskIdentity,
    markdown::replace_or_append_section,
    model::{Risk, TaskMetadata},
    project::TaskProject,
    validate::{CandidateScope, require_valid_report, validate_task_candidate},
};

use super::write::today;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum FieldPatch<T> {
    #[default]
    Keep,
    Set(T),
    Clear,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskPatch {
    pub title: Option<String>,
    pub summary: Option<String>,
    pub sections: BTreeMap<String, String>,
    pub risk: FieldPatch<Risk>,
    pub impact: FieldPatch<String>,
    pub tags: Option<Vec<String>>,
    pub milestone: FieldPatch<String>,
    pub parent: FieldPatch<String>,
    pub depends_on: Option<Vec<String>>,
}

impl TaskPatch {
    fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.summary.is_none()
            && self.sections.is_empty()
            && self.risk == FieldPatch::Keep
            && self.impact == FieldPatch::Keep
            && self.tags.is_none()
            && self.milestone == FieldPatch::Keep
            && self.parent == FieldPatch::Keep
            && self.depends_on.is_none()
    }

    fn validation_scope(&self) -> CandidateScope {
        if self.milestone != FieldPatch::Keep
            || self.parent != FieldPatch::Keep
            || self.depends_on.is_some()
        {
            CandidateScope::Repository
        } else {
            CandidateScope::Target
        }
    }
}

/// Apply a partial edit to one existing task and atomically replace the file.
pub fn edit_task(
    project: &TaskProject,
    identity: &TaskIdentity,
    patch: TaskPatch,
) -> Result<PathBuf, TaskError> {
    edit_task_with_group_value(project, identity, patch, None)
}

/// Set or clear the selected value in one configured exclusive tag group.
pub fn set_exclusive_tag_group_value(
    project: &TaskProject,
    identity: &TaskIdentity,
    group: &str,
    value: Option<&str>,
) -> Result<PathBuf, TaskError> {
    edit_task_with_group_value(
        project,
        identity,
        TaskPatch::default(),
        Some((group, value)),
    )
}

fn edit_task_with_group_value(
    project: &TaskProject,
    identity: &TaskIdentity,
    patch: TaskPatch,
    group_value: Option<(&str, Option<&str>)>,
) -> Result<PathBuf, TaskError> {
    let path = project.task_path(identity)?;
    project.with_write_lock(|| {
        if patch.is_empty() && group_value.is_none() {
            return Ok(path);
        }

        let content = fs::read_to_string(&path).map_err(|source| TaskError::Io {
            path: path.clone(),
            source,
        })?;
        let mut metadata =
            parse_metadata(&path, &content).map_err(|error| TaskError::Validation(vec![error]))?;
        let validation_scope = patch.validation_scope();
        apply_metadata_patch(&mut metadata, patch.clone());
        if let Some((group, value)) = group_value {
            apply_exclusive_tag_group_value(project, identity, &mut metadata, group, value)?;
        }
        metadata.last_updated = Some(today());
        let body = patched_body(&content, &patch)?;
        let candidate = format!("{}{}", render_metadata(&metadata)?, body);
        require_valid_report(validate_task_candidate(
            project,
            &path,
            &candidate,
            validation_scope,
        )?)?;
        atomic_write::replace(&path, &candidate).map_err(|source| TaskError::Io {
            path: path.clone(),
            source,
        })?;
        Ok(path)
    })
}

fn apply_exclusive_tag_group_value(
    project: &TaskProject,
    identity: &TaskIdentity,
    metadata: &mut TaskMetadata,
    group: &str,
    value: Option<&str>,
) -> Result<(), TaskError> {
    let members = project.policy().exclusive_tag_group(group).ok_or_else(|| {
        let configured = project
            .policy()
            .exclusive_tag_groups()
            .keys()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        TaskError::Message(format!(
            "unknown exclusive tag group {group:?}; configured groups: {configured}"
        ))
    })?;
    if let Some(value) = value
        && !members.iter().any(|member| member == value)
    {
        return Err(TaskError::TagRejected(Box::new(TaxonomyErrorContext {
            rejected_value: value.to_string(),
            field: "tags".to_string(),
            domain: Some(identity.domain.clone()),
            policy: Some(format!("exclusive group {group}")),
            allowed: members.to_vec(),
            project_root: project.root().to_path_buf(),
            task: Some(identity.to_string()),
        })));
    }
    metadata.tags.retain(|tag| !members.contains(tag));
    if let Some(value) = value {
        metadata.tags.push(value.to_string());
    }
    Ok(())
}

fn apply_metadata_patch(metadata: &mut TaskMetadata, patch: TaskPatch) {
    if let Some(title) = patch.title {
        metadata.title = title;
    }
    apply_optional(&mut metadata.risk, patch.risk);
    apply_optional(&mut metadata.impact, patch.impact);
    if let Some(tags) = patch.tags {
        metadata.tags = tags;
    }
    apply_optional(&mut metadata.milestone, patch.milestone);
    apply_optional(&mut metadata.parent, patch.parent);
    if let Some(depends_on) = patch.depends_on {
        metadata.depends_on = depends_on;
    }
}

fn apply_optional<T>(field: &mut Option<T>, patch: FieldPatch<T>) {
    match patch {
        FieldPatch::Keep => {}
        FieldPatch::Set(value) => *field = Some(value),
        FieldPatch::Clear => *field = None,
    }
}

fn patched_body(content: &str, patch: &TaskPatch) -> Result<String, TaskError> {
    let mut body = body_after_frontmatter(content)?.to_string();
    if let Some(summary) = &patch.summary {
        body = replace_or_append_section(&body, "Summary", summary);
    }
    for (heading, replacement) in &patch.sections {
        body = replace_or_append_section(&body, heading, replacement);
    }
    Ok(body)
}

fn body_after_frontmatter(content: &str) -> Result<&str, TaskError> {
    let mut offset = 0;
    let mut lines = content.split_inclusive('\n');
    let Some(first) = lines.next() else {
        return invalid_frontmatter("missing YAML frontmatter");
    };
    offset += first.len();
    if first.trim_end_matches(['\r', '\n']) != "---" {
        return invalid_frontmatter("missing YAML frontmatter");
    }
    for line in lines {
        offset += line.len();
        if line.trim_end_matches(['\r', '\n']) == "---" {
            return Ok(&content[offset..]);
        }
    }
    invalid_frontmatter("YAML frontmatter is missing closing delimiter")
}

fn invalid_frontmatter<T>(message: &str) -> Result<T, TaskError> {
    Err(TaskError::Validation(vec![ValidationError::new(
        None, message,
    )]))
}
