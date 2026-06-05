use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use crate::{
    error::{TaskError, ValidationError},
    frontmatter::parse_metadata,
    model::{Priority, SortBy, Task, TaskStatus},
    repo::{DOMAINS, TASK_DIR, find_repo_root, read_task_files},
};

const CORE_PREFIXES: &[&str] = &[
    "CORE", "ACTORS", "API", "MODULES", "PIPELINE", "SERVICES", "UTILS", "VFS",
];
const APP_PREFIXES: &[&str] = &["UI", "EXPL", "SETS", "SRCH", "MEDIA", "NAV", "PERF", "A11Y"];
const ECOSYSTEM_PREFIXES: &[&str] = &["PLUG"];

#[derive(Debug, Clone, Default)]
pub struct TaskFilter {
    pub status: Option<TaskStatus>,
    pub priority: Option<Priority>,
    pub domain: Option<String>,
    pub parent: Option<String>,
    pub tag: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub tasks: Vec<Task>,
    pub errors: Vec<ValidationError>,
}

pub fn validate_current_repo() -> Result<ValidationReport, TaskError> {
    let cwd = std::env::current_dir().map_err(|source| TaskError::Io {
        path: PathBuf::from("."),
        source,
    })?;
    let root = find_repo_root(cwd)?;
    validate_repo(&root)
}

pub fn validate_repo(root: &Path) -> Result<ValidationReport, TaskError> {
    let paths = read_task_files(root)?;
    let mut tasks = Vec::new();
    let mut errors = Vec::new();

    for path in paths {
        let content = fs::read_to_string(&path).map_err(|source| TaskError::Io {
            path: path.clone(),
            source,
        })?;

        match parse_metadata(&path, &content) {
            Ok(metadata) => {
                let domain = domain_for_path(root, &path).unwrap_or_else(|| "unknown".to_string());
                let task = Task {
                    path: path.clone(),
                    domain,
                    metadata,
                };
                validate_single_task(root, &task, &mut errors);
                tasks.push(task);
            }
            Err(error) => errors.push(error),
        }
    }

    validate_cross_references(&tasks, &mut errors);
    tasks.sort_by(|left, right| left.metadata.id.cmp(&right.metadata.id));

    Ok(ValidationReport { tasks, errors })
}

pub fn require_valid_report(report: ValidationReport) -> Result<Vec<Task>, TaskError> {
    if report.errors.is_empty() {
        Ok(report.tasks)
    } else {
        Err(TaskError::Validation(report.errors))
    }
}

pub fn filter_tasks(mut tasks: Vec<Task>, filter: &TaskFilter, sort_by: SortBy) -> Vec<Task> {
    tasks.retain(|task| {
        filter
            .status
            .is_none_or(|status| task.metadata.status == status)
            && filter
                .priority
                .is_none_or(|priority| task.metadata.priority == priority)
            && filter
                .domain
                .as_ref()
                .is_none_or(|domain| &task.domain == domain)
            && filter
                .parent
                .as_ref()
                .is_none_or(|parent| task.metadata.parent.as_ref() == Some(parent))
            && filter
                .tag
                .as_ref()
                .is_none_or(|tag| task.metadata.tags.iter().any(|value| value == tag))
    });

    match sort_by {
        SortBy::Status => tasks.sort_by_key(|task| status_order(task.metadata.status)),
        SortBy::Priority => tasks.sort_by_key(|task| priority_order(task.metadata.priority)),
        SortBy::Id => tasks.sort_by(|left, right| left.metadata.id.cmp(&right.metadata.id)),
        SortBy::Domain => tasks.sort_by(|left, right| {
            left.domain
                .cmp(&right.domain)
                .then_with(|| left.metadata.id.cmp(&right.metadata.id))
        }),
    }

    tasks
}

fn validate_single_task(root: &Path, task: &Task, errors: &mut Vec<ValidationError>) {
    let path = &task.path;
    let metadata = &task.metadata;

    if !is_valid_task_id(&metadata.id) {
        errors.push(ValidationError::at(
            path,
            format!("invalid id {}; expected PREFIX-NUMBER", metadata.id),
        ));
    }

    if metadata.title.chars().count() < 5 {
        errors.push(ValidationError::at(
            path,
            "title must be at least 5 characters",
        ));
    }

    if let Some(parent) = &metadata.parent {
        if !is_valid_task_id(parent) {
            errors.push(ValidationError::at(
                path,
                format!("invalid parent id {parent}; expected PREFIX-NUMBER"),
            ));
        }
    }

    if let Some(last_updated) = &metadata.last_updated {
        if !is_valid_iso_date(last_updated) {
            errors.push(ValidationError::at(
                path,
                "last_updated must use YYYY-MM-DD format",
            ));
        }
    }

    validate_path(root, task, errors);
}

fn validate_path(root: &Path, task: &Task, errors: &mut Vec<ValidationError>) {
    let path = &task.path;
    let relative = match path.strip_prefix(root) {
        Ok(relative) => relative,
        Err(_) => {
            errors.push(ValidationError::at(path, "task file is outside repo root"));
            return;
        }
    };

    let mut parts = relative.components();
    let task_dir = parts.next().and_then(|part| part.as_os_str().to_str());
    let domain = parts.next().and_then(|part| part.as_os_str().to_str());
    if task_dir != Some(TASK_DIR) || !domain.is_some_and(|value| DOMAINS.contains(&value)) {
        errors.push(ValidationError::at(
            path,
            "task file must live under .tasks/core, .tasks/app, or .tasks/ecosystem",
        ));
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let expected_prefix = format!("{}-", task.metadata.id);
    if !file_name.starts_with(&expected_prefix) {
        errors.push(ValidationError::at(
            path,
            format!("file name must start with {}", expected_prefix),
        ));
    }

    let prefix = task
        .metadata
        .id
        .split_once('-')
        .map(|(prefix, _)| prefix)
        .unwrap_or("");
    if !allowed_prefixes(&task.domain).contains(&prefix) {
        errors.push(ValidationError::at(
            path,
            format!("prefix {prefix} is not allowed for {} tasks", task.domain),
        ));
    }
}

fn validate_cross_references(tasks: &[Task], errors: &mut Vec<ValidationError>) {
    let mut by_id: HashMap<&str, &Task> = HashMap::new();
    let mut duplicates = HashSet::new();

    for task in tasks {
        if by_id.insert(&task.metadata.id, task).is_some() {
            duplicates.insert(task.metadata.id.as_str());
        }
    }

    for duplicate in duplicates {
        errors.push(ValidationError::new(
            None,
            format!("duplicate task id {duplicate}"),
        ));
    }

    for task in tasks {
        if let Some(parent) = &task.metadata.parent {
            if !by_id.contains_key(parent.as_str()) {
                errors.push(ValidationError::at(
                    &task.path,
                    format!("parent {parent} does not reference an existing task"),
                ));
            }
        }
    }
}

fn domain_for_path(root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()?
        .components()
        .nth(1)?
        .as_os_str()
        .to_str()
        .map(str::to_string)
}

fn allowed_prefixes(domain: &str) -> &'static [&'static str] {
    match domain {
        "core" => CORE_PREFIXES,
        "app" => APP_PREFIXES,
        "ecosystem" => ECOSYSTEM_PREFIXES,
        _ => &[],
    }
}

fn is_valid_task_id(value: &str) -> bool {
    let Some((prefix, number)) = value.split_once('-') else {
        return false;
    };
    !prefix.is_empty()
        && prefix.chars().all(|ch| ch.is_ascii_uppercase())
        && !number.is_empty()
        && number.chars().all(|ch| ch.is_ascii_digit())
}

fn is_valid_iso_date(value: &str) -> bool {
    let parts: Vec<_> = value.split('-').collect();
    if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return false;
    }

    let Ok(year) = parts[0].parse::<u16>() else {
        return false;
    };
    let Ok(month) = parts[1].parse::<u8>() else {
        return false;
    };
    let Ok(day) = parts[2].parse::<u8>() else {
        return false;
    };

    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => return false,
    };

    day >= 1 && day <= max_day
}

fn is_leap_year(year: u16) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn status_order(status: TaskStatus) -> u8 {
    match status {
        TaskStatus::Done => 0,
        TaskStatus::InProgress => 1,
        TaskStatus::ToDo => 2,
    }
}

fn priority_order(priority: Priority) -> u8 {
    match priority {
        Priority::High => 0,
        Priority::Medium => 1,
        Priority::Low => 2,
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use tempfile::TempDir;

    use super::{TaskFilter, filter_tasks, validate_repo};
    use crate::model::{Priority, SortBy, TaskStatus};

    #[test]
    fn validate_succeeds_for_minimal_task_tree() {
        let temp = task_repo();
        write_task(
            temp.path(),
            "core/CORE-001-location-routing.md",
            "CORE-001",
            "Location routing",
            "To Do",
            "High",
            "",
        );

        let report = validate_repo(temp.path()).expect("repo should validate");

        assert!(report.errors.is_empty());
        assert_eq!(report.tasks.len(), 1);
    }

    #[test]
    fn validate_rejects_duplicate_ids() {
        let temp = task_repo();
        write_task(
            temp.path(),
            "core/CORE-001-location-routing.md",
            "CORE-001",
            "Location routing",
            "To Do",
            "High",
            "",
        );
        write_task(
            temp.path(),
            "core/CORE-001-location-cache.md",
            "CORE-001",
            "Location cache",
            "Done",
            "Medium",
            "",
        );

        let report = validate_repo(temp.path()).expect("repo should scan");

        assert!(
            report
                .errors
                .iter()
                .any(|error| error.message.contains("duplicate task id CORE-001"))
        );
    }

    #[test]
    fn validate_rejects_orphaned_parent() {
        let temp = task_repo();
        write_task(
            temp.path(),
            "core/CORE-002-location-routing.md",
            "CORE-002",
            "Location routing",
            "To Do",
            "High",
            "parent: CORE-001\n",
        );

        let report = validate_repo(temp.path()).expect("repo should scan");

        assert!(report.errors.iter().any(|error| {
            error
                .message
                .contains("parent CORE-001 does not reference an existing task")
        }));
    }

    #[test]
    fn validate_rejects_filename_without_id_prefix() {
        let temp = task_repo();
        write_task(
            temp.path(),
            "core/location-routing.md",
            "CORE-001",
            "Location routing",
            "To Do",
            "High",
            "",
        );

        let report = validate_repo(temp.path()).expect("repo should scan");

        assert!(report.errors.iter().any(|error| {
            error
                .message
                .contains("file name must start with CORE-001-")
        }));
    }

    #[test]
    fn validate_rejects_invalid_last_updated() {
        let temp = task_repo();
        write_task(
            temp.path(),
            "core/CORE-001-location-routing.md",
            "CORE-001",
            "Location routing",
            "To Do",
            "High",
            "last_updated: 2026-02-30\n",
        );

        let report = validate_repo(temp.path()).expect("repo should scan");

        assert!(report.errors.iter().any(|error| {
            error
                .message
                .contains("last_updated must use YYYY-MM-DD format")
        }));
    }

    #[test]
    fn filter_matches_status_priority_domain_parent_and_tag() {
        let temp = task_repo();
        write_task(
            temp.path(),
            "core/CORE-001-location-routing.md",
            "CORE-001",
            "Location routing",
            "In Progress",
            "High",
            "tags: [location, routing]\n",
        );
        write_task(
            temp.path(),
            "core/VFS-001-provider-routing.md",
            "VFS-001",
            "Provider routing",
            "To Do",
            "Medium",
            "parent: CORE-001\ntags: [provider]\n",
        );

        let report = validate_repo(temp.path()).expect("repo should validate");
        assert!(report.errors.is_empty());

        let filtered = filter_tasks(
            report.tasks,
            &TaskFilter {
                status: Some(TaskStatus::ToDo),
                priority: Some(Priority::Medium),
                domain: Some("core".to_string()),
                parent: Some("CORE-001".to_string()),
                tag: Some("provider".to_string()),
            },
            SortBy::Id,
        );

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].metadata.id, "VFS-001");
    }

    fn task_repo() -> TempDir {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        fs::create_dir_all(temp.path().join(".tasks/core")).expect("core task dir should exist");
        fs::create_dir_all(temp.path().join(".tasks/app")).expect("app task dir should exist");
        fs::create_dir_all(temp.path().join(".tasks/ecosystem"))
            .expect("ecosystem task dir should exist");
        fs::write(temp.path().join(".tasks/task.schema.json"), "{}")
            .expect("schema should be written");
        temp
    }

    fn write_task(
        root: &Path,
        relative: &str,
        id: &str,
        title: &str,
        status: &str,
        priority: &str,
        extra: &str,
    ) {
        let path = root.join(".tasks").join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("task parent dir should exist");
        }
        fs::write(
            path,
            format!(
                "---\nid: {id}\ntitle: {title}\nstatus: {status}\npriority: {priority}\n{extra}---\n"
            ),
        )
        .expect("task should be written");
    }
}
