//! # Lifecycle Writes
//!
//! This module renders lifecycle metadata changes and sends complete content
//! through the shared atomic writer. Keeping file replacement here gives every
//! status transition the same cleanup and timestamp behavior.
//!
//! ```
//! use filer_task::{identity::TaskIdentity, lifecycle::start_task, project::TaskProject};
//!
//! # let root = tempfile::tempdir()?;
//! # std::fs::create_dir_all(root.path().join(".tasks/core"))?;
//! # std::fs::write(root.path().join(".tasks/core/UTILS-998-example.md"), "---\nid: UTILS-998\ntitle: Example task\nstatus: To Do\npriority: Medium\ntype: Feature\n---\n\n## Acceptance Criteria\n\n- [ ] Example\n")?;
//! let project = TaskProject::open(root.path())?;
//! start_task(&project, &TaskIdentity::new("core", "UTILS-998")?)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    atomic_write, error::TaskError, markdown::replace_or_append_section, model::TaskStatus,
};

pub(super) fn write_new_task(path: &Path, content: &str) -> Result<(), TaskError> {
    if path.exists() {
        return Err(TaskError::Message(format!(
            "task file already exists: {}",
            path.display()
        )));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| TaskError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    atomic_write::create(path, content).map_err(|source| TaskError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub(super) fn write_status(
    path: &Path,
    content: &str,
    status: TaskStatus,
    section: Option<(&str, &str)>,
) -> Result<(), TaskError> {
    let mut updated = update_frontmatter(content, status);
    if let Some((heading, text)) = section {
        updated = replace_or_append_section(&updated, heading, text);
    }
    atomic_write::replace(path, &updated).map_err(|source| TaskError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub(super) fn read(path: &Path) -> Result<String, TaskError> {
    fs::read_to_string(path).map_err(|source| TaskError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub(super) fn today() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let days = (seconds / 86_400) as i64;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn update_frontmatter(content: &str, status: TaskStatus) -> String {
    let mut in_frontmatter = false;
    let mut seen_status = false;
    let mut seen_last_updated = false;
    let mut output = Vec::new();

    for (index, line) in content.lines().enumerate() {
        if index == 0 && line.trim() == "---" {
            in_frontmatter = true;
            output.push(line.to_string());
            continue;
        }
        if in_frontmatter && line.trim() == "---" {
            if !seen_status {
                output.push(format!("status: {status}"));
            }
            if !seen_last_updated {
                output.push(format!("last_updated: {}", today()));
            }
            in_frontmatter = false;
            output.push(line.to_string());
            continue;
        }
        if in_frontmatter && line.starts_with("status:") {
            output.push(format!("status: {status}"));
            seen_status = true;
        } else if in_frontmatter && line.starts_with("last_updated:") {
            output.push(format!("last_updated: {}", today()));
            seen_last_updated = true;
        } else {
            output.push(line.to_string());
        }
    }

    let mut joined = output.join("\n");
    joined.push('\n');
    joined
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year, m as u32, d as u32)
}
