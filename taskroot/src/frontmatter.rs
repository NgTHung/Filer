//! # Task Frontmatter
//!
//! This module parses and renders task metadata through one YAML-compatible
//! representation. JSON string encoding is valid YAML and keeps every scalar
//! round-trippable without maintaining a second escaping policy.
//!
//! ```
//! use taskroot::frontmatter::parse_metadata;
//! use std::path::Path;
//!
//! let content = "---\nid: \"CORE-001\"\ntitle: \"A # title\"\nstatus: \"To Do\"\npriority: \"High\"\ntype: \"Feature\"\n---\n";
//! let metadata = parse_metadata(Path::new("CORE-001-example.md"), content)?;
//! assert_eq!(metadata.title, "A # title");
//! # Ok::<(), taskroot::error::ValidationError>(())
//! ```

use crate::{
    error::{TaskError, ValidationError},
    model::TaskMetadata,
};

pub(crate) fn render_metadata(metadata: &TaskMetadata) -> Result<String, TaskError> {
    let mut content = String::from("---\n");
    push_scalar(&mut content, "id", &metadata.id)?;
    push_scalar(&mut content, "title", &metadata.title)?;
    push_scalar(&mut content, "status", &metadata.status.to_string())?;
    push_scalar(&mut content, "priority", &metadata.priority.to_string())?;
    push_scalar(&mut content, "type", metadata.task_type.as_str())?;
    push_optional_scalar(&mut content, "parent", metadata.parent.as_deref())?;
    push_optional_scalar(&mut content, "milestone", metadata.milestone.as_deref())?;
    push_array(&mut content, "depends_on", &metadata.depends_on)?;
    push_array(&mut content, "rules", &metadata.rules)?;
    if let Some(risk) = metadata.risk {
        push_scalar(&mut content, "risk", &risk.to_string())?;
    }
    push_optional_scalar(&mut content, "impact", metadata.impact.as_deref())?;
    push_array(&mut content, "tags", &metadata.tags)?;
    push_optional_scalar(&mut content, "whitepaper", metadata.whitepaper.as_deref())?;
    push_optional_scalar(
        &mut content,
        "last_updated",
        metadata.last_updated.as_deref(),
    )?;
    content.push_str("---\n");
    Ok(content)
}

fn push_scalar(content: &mut String, key: &str, value: &str) -> Result<(), TaskError> {
    content.push_str(key);
    content.push_str(": ");
    content.push_str(&serde_json::to_string(value)?);
    content.push('\n');
    Ok(())
}

fn push_optional_scalar(
    content: &mut String,
    key: &str,
    value: Option<&str>,
) -> Result<(), TaskError> {
    if let Some(value) = value {
        push_scalar(content, key, value)?;
    }
    Ok(())
}

fn push_array(content: &mut String, key: &str, values: &[String]) -> Result<(), TaskError> {
    if values.is_empty() {
        return Ok(());
    }
    content.push_str(key);
    content.push_str(": [");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            content.push_str(", ");
        }
        content.push_str(&serde_json::to_string(value)?);
    }
    content.push_str("]\n");
    Ok(())
}

pub fn parse_metadata(
    path: &std::path::Path,
    content: &str,
) -> Result<TaskMetadata, ValidationError> {
    let yaml = extract_yaml(path, content)?;
    serde_yaml::from_str::<TaskMetadata>(&yaml)
        .map_err(|error| ValidationError::at(path, format!("invalid YAML frontmatter: {error}")))
}

fn extract_yaml(path: &std::path::Path, content: &str) -> Result<String, ValidationError> {
    let mut lines = content.lines();
    match lines.next() {
        Some(first) if first.trim_end_matches('\r') == "---" => {}
        _ => return Err(ValidationError::at(path, "missing YAML frontmatter")),
    }

    let mut yaml = String::new();
    for line in lines {
        if line.trim_end_matches('\r') == "---" {
            return Ok(yaml);
        }
        yaml.push_str(line);
        yaml.push('\n');
    }

    Err(ValidationError::at(
        path,
        "YAML frontmatter is missing closing delimiter",
    ))
}

#[cfg(test)]
mod tests {
    use super::parse_metadata;
    use crate::model::{Priority, TaskStatus, TaskType};
    use std::path::Path;

    #[test]
    fn parses_valid_frontmatter() {
        let content = "---\nid: CORE-001\ntitle: Location routing\nstatus: To Do\npriority: High\ntype: Feature\n---\n";

        let metadata = parse_metadata(Path::new("CORE-001-location-routing.md"), content)
            .expect("valid frontmatter should parse");

        assert_eq!(metadata.id, "CORE-001");
        assert_eq!(metadata.status, TaskStatus::ToDo);
        assert_eq!(metadata.priority, Priority::High);
        assert_eq!(metadata.task_type, TaskType::new("Feature"));
    }

    #[test]
    fn rejects_missing_frontmatter() {
        let error = parse_metadata(Path::new("task.md"), "id: CORE-001\n")
            .expect_err("missing frontmatter should fail");

        assert_eq!(error.message, "missing YAML frontmatter");
    }

    #[test]
    fn rejects_invalid_yaml() {
        let content = "---\nid: [\n---\n";

        let error =
            parse_metadata(Path::new("task.md"), content).expect_err("invalid YAML should fail");

        assert!(error.message.starts_with("invalid YAML frontmatter:"));
    }
}
