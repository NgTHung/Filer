//! # Criteria Updates
//!
//! This module changes one checklist marker while preserving the rest of the
//! task file byte-for-byte. Conditional updates also reject stale content before
//! replacement, so an index cannot silently target a changed checklist item.
//!
//! ```
//! use filer_task::{
//!     identity::TaskIdentity,
//!     lifecycle::toggle_criterion,
//!     project::TaskProject,
//! };
//!
//! # let root = tempfile::tempdir()?;
//! # std::fs::create_dir_all(root.path().join(".tasks/core"))?;
//! # std::fs::write(root.path().join(".tasks/core/UTILS-998-example.md"), "---\nid: UTILS-998\ntitle: Example task\nstatus: To Do\npriority: Medium\ntype: Feature\n---\n\n## Acceptance Criteria\n\n- [ ] Works\n")?;
//! let project = TaskProject::open(root.path())?;
//! toggle_criterion(&project, &TaskIdentity::new("core", "UTILS-998")?, 0)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::{fs, path::PathBuf};

use crate::{
    atomic_write,
    error::TaskError,
    frontmatter::parse_metadata,
    identity::TaskIdentity,
    markdown::checklist_matches,
    project::TaskProject,
    taxonomy::criteria_heading,
    validate::{CandidateScope, require_valid_report, validate_task_candidate},
};

/// Flip one zero-based criteria item for an existing task.
pub fn toggle_criterion(
    project: &TaskProject,
    identity: &TaskIdentity,
    index: usize,
) -> Result<PathBuf, TaskError> {
    let path = project.task_path(identity)?;
    project.with_write_lock(|| {
        let content = fs::read_to_string(&path).map_err(|source| TaskError::Io {
            path: path.clone(),
            source,
        })?;
        let metadata =
            parse_metadata(&path, &content).map_err(|error| TaskError::Validation(vec![error]))?;
        let heading = criteria_heading(
            project,
            &metadata.task_type,
            Some(&identity.domain),
            Some(&identity.to_string()),
        )?;
        let Some(toggled) = update_marker(
            &content,
            heading,
            index,
            &identity.to_string(),
            CriterionUpdate::Toggle,
        )?
        else {
            return Err(TaskError::Message(
                "criterion toggle did not produce an updated marker".to_string(),
            ));
        };
        require_valid_report(validate_task_candidate(
            project,
            &path,
            &toggled,
            CandidateScope::Target,
        )?)?;
        atomic_write::replace(&path, &toggled).map_err(|source| TaskError::Io {
            path: path.clone(),
            source,
        })?;
        Ok(path)
    })
}

/// Set one zero-based criteria item when its exact source content still matches.
pub fn set_criterion_checked(
    project: &TaskProject,
    identity: &TaskIdentity,
    index: usize,
    expected_hash: &str,
    checked: bool,
) -> Result<PathBuf, TaskError> {
    let path = project.task_path(identity)?;
    project.with_write_lock(|| {
        let content = fs::read_to_string(&path).map_err(|source| TaskError::Io {
            path: path.clone(),
            source,
        })?;
        let metadata =
            parse_metadata(&path, &content).map_err(|error| TaskError::Validation(vec![error]))?;
        let heading = criteria_heading(
            project,
            &metadata.task_type,
            Some(&identity.domain),
            Some(&identity.to_string()),
        )?;
        let Some(updated) = update_marker(
            &content,
            heading,
            index,
            &identity.to_string(),
            CriterionUpdate::Set {
                expected_hash,
                checked,
            },
        )?
        else {
            return Ok(path.clone());
        };
        require_valid_report(validate_task_candidate(
            project,
            &path,
            &updated,
            CandidateScope::Target,
        )?)?;
        atomic_write::replace(&path, &updated).map_err(|source| TaskError::Io {
            path: path.clone(),
            source,
        })?;
        Ok(path.clone())
    })
}

enum CriterionUpdate<'a> {
    Toggle,
    Set {
        expected_hash: &'a str,
        checked: bool,
    },
}

fn update_marker(
    content: &str,
    heading: &str,
    wanted: usize,
    identity: &str,
    update: CriterionUpdate<'_>,
) -> Result<Option<String>, TaskError> {
    let matches = checklist_matches(content, heading);
    if let Some(matched) = matches.get(wanted) {
        let checked = match update {
            CriterionUpdate::Toggle => !matched.item.checked,
            CriterionUpdate::Set {
                expected_hash,
                checked,
            } => {
                if matched.content_hash != expected_hash {
                    return Err(TaskError::CriterionContentMismatch {
                        task: identity.to_string(),
                        index: wanted,
                        expected_hash: expected_hash.to_string(),
                        actual_hash: matched.content_hash.clone(),
                    });
                }
                checked
            }
        };
        if checked == matched.item.checked {
            return Ok(None);
        }
        let mut updated = content.to_string();
        updated.replace_range(matched.marker.clone(), if checked { "x" } else { " " });
        return Ok(Some(updated));
    }

    Err(TaskError::CriterionIndexOutOfRange {
        task: identity.to_string(),
        index: wanted,
        count: matches.len(),
    })
}
