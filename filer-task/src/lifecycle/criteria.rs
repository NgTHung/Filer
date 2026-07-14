//! # Criteria Toggles
//!
//! This module flips one checklist marker while preserving the rest of the task
//! file byte-for-byte. The updated content is validated before replacement so a
//! toggle cannot silently leave the repository invalid.
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
        let toggled = toggle_marker(&content, heading, index, &identity.to_string())?;
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

fn toggle_marker(
    content: &str,
    heading: &str,
    wanted: usize,
    identity: &str,
) -> Result<String, TaskError> {
    let matches = checklist_matches(content, heading);
    if let Some(matched) = matches.get(wanted) {
        let mut toggled = content.to_string();
        let replacement = if matched.item.checked { " " } else { "x" };
        toggled.replace_range(matched.marker.clone(), replacement);
        return Ok(toggled);
    }

    Err(TaskError::CriterionIndexOutOfRange {
        task: identity.to_string(),
        index: wanted,
        count: matches.len(),
    })
}
