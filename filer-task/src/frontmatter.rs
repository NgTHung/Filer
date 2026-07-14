use crate::{error::ValidationError, model::TaskMetadata};

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
