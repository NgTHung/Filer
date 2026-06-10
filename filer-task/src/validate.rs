use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use crate::{
    error::{TaskError, ValidationError},
    frontmatter::parse_metadata,
    markdown::{has_section, has_unchecked_checklist_item},
    model::{Priority, SortBy, Task, TaskStatus, TaskType},
    repo::{DOMAINS, MILESTONE_DOMAIN, TASK_DIR, find_repo_root, read_task_files},
};

pub(crate) const CORE_PREFIXES: &[&str] = &[
    "CORE", "ACTORS", "API", "MODULES", "PIPELINE", "SERVICES", "UTILS", "VFS", "REL", "NAV",
    "SEARCH", "OPS", "PREVIEW", "PROVIDER", "PROTOCOL",
];
pub(crate) const APP_PREFIXES: &[&str] =
    &["UI", "EXPL", "SETS", "SRCH", "MEDIA", "NAV", "PERF", "A11Y"];
pub(crate) const ECOSYSTEM_PREFIXES: &[&str] = &["PLUG", "EXT", "THEME", "PROFILE", "PROVIDER"];
pub(crate) const RULE_IDS: &[&str] = &[
    "CORE-LIBRARY",
    "PROVIDER-ACCESS",
    "SESSION-BOUNDARY",
    "ACTOR-LONG-WORK",
    "PIPELINE-TRANSFORMS",
    "WIRE-SAFE-EXTENSIONS",
    "SEMANTIC-EXTENSION-OUTPUT",
    "CORE-MECHANICS-BUILTIN",
];

#[derive(Debug, Clone, Default)]
pub struct TaskFilter {
    pub status: Option<TaskStatus>,
    pub priority: Option<Priority>,
    pub domain: Option<String>,
    pub parent: Option<String>,
    pub milestone: Option<String>,
    pub tag: Option<String>,
    pub blocked: bool,
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
                validate_body(&task, &content, &mut errors);
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
                .milestone
                .as_ref()
                .is_none_or(|milestone| task.metadata.milestone.as_ref() == Some(milestone))
            && filter
                .tag
                .as_ref()
                .is_none_or(|tag| task.metadata.tags.iter().any(|value| value == tag))
            && (!filter.blocked || task.metadata.status == TaskStatus::Blocked)
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

    for depends_on in &metadata.depends_on {
        if !is_valid_task_id(depends_on) {
            errors.push(ValidationError::at(
                path,
                format!("invalid dependency id {depends_on}; expected PREFIX-NUMBER"),
            ));
        }
    }

    let mut seen_dependencies = HashSet::new();
    for depends_on in &metadata.depends_on {
        if !seen_dependencies.insert(depends_on) {
            errors.push(ValidationError::at(
                path,
                format!("duplicate dependency id {depends_on}"),
            ));
        }
        if depends_on == &metadata.id {
            errors.push(ValidationError::at(path, "task cannot depend on itself"));
        }
    }

    for rule in &metadata.rules {
        if !RULE_IDS.contains(&rule.as_str()) {
            errors.push(ValidationError::at(path, format!("unknown rule id {rule}")));
        }
    }

    let mut seen_rules = HashSet::new();
    for rule in &metadata.rules {
        if !seen_rules.insert(rule) {
            errors.push(ValidationError::at(
                path,
                format!("duplicate rule id {rule}"),
            ));
        }
    }

    if let Some(impact) = &metadata.impact {
        if impact.chars().count() < 10 {
            errors.push(ValidationError::at(
                path,
                "impact must be at least 10 characters when present",
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

fn validate_body(task: &Task, content: &str, errors: &mut Vec<ValidationError>) {
    let metadata = &task.metadata;
    if metadata.status == TaskStatus::Blocked && !has_section(content, "Blocked Reason") {
        errors.push(ValidationError::at(
            &task.path,
            "blocked tasks must include ## Blocked Reason",
        ));
    }

    if matches!(metadata.status, TaskStatus::Deferred | TaskStatus::Obsolete) {
        if !has_section(content, "Rationale") {
            errors.push(ValidationError::at(
                &task.path,
                "deferred and obsolete tasks must include ## Rationale",
            ));
        }
        return;
    }

    match metadata.task_type {
        TaskType::Milestone | TaskType::Epic => {
            if !has_section(content, "Exit Criteria") {
                errors.push(ValidationError::at(
                    &task.path,
                    "milestone and epic tasks must include ## Exit Criteria",
                ));
            }
            if metadata.status == TaskStatus::Done
                && has_unchecked_checklist_item(content, "Exit Criteria")
            {
                errors.push(ValidationError::at(
                    &task.path,
                    "done tasks must not have unchecked ## Exit Criteria items",
                ));
            }
        }
        _ => {
            if !has_section(content, "Acceptance Criteria") {
                errors.push(ValidationError::at(
                    &task.path,
                    "tasks must include ## Acceptance Criteria",
                ));
            }
            if metadata.status == TaskStatus::Done
                && has_unchecked_checklist_item(content, "Acceptance Criteria")
            {
                errors.push(ValidationError::at(
                    &task.path,
                    "done tasks must not have unchecked ## Acceptance Criteria items",
                ));
            }
        }
    }
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
    let valid_domain =
        domain.is_some_and(|value| DOMAINS.contains(&value) || value == MILESTONE_DOMAIN);
    if task_dir != Some(TASK_DIR) || !valid_domain {
        errors.push(ValidationError::at(
            path,
            "task file must live under .tasks/core, .tasks/app, .tasks/ecosystem, or .tasks/milestones",
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
    if prefix == "MILESTONE" && task.domain != MILESTONE_DOMAIN {
        errors.push(ValidationError::at(
            path,
            "MILESTONE prefix is only allowed under .tasks/milestones",
        ));
    } else if !allowed_prefixes(&task.domain).contains(&prefix) {
        errors.push(ValidationError::at(
            path,
            format!("prefix {prefix} is not allowed for {} tasks", task.domain),
        ));
    }

    if task.metadata.task_type == TaskType::Milestone && task.domain != MILESTONE_DOMAIN {
        errors.push(ValidationError::at(
            path,
            "Milestone tasks must live under .tasks/milestones",
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

        for depends_on in &task.metadata.depends_on {
            if !by_id.contains_key(depends_on.as_str()) {
                errors.push(ValidationError::at(
                    &task.path,
                    format!("dependency {depends_on} does not reference an existing task"),
                ));
            }
        }
    }

    validate_milestone_references(tasks, errors);

    validate_dependency_cycles(tasks, errors);
}

fn validate_milestone_references(tasks: &[Task], errors: &mut Vec<ValidationError>) {
    let mut milestone_counts: HashMap<&str, usize> = HashMap::new();
    for task in tasks {
        if task.metadata.task_type == TaskType::Milestone {
            if let Some(milestone) = &task.metadata.milestone {
                *milestone_counts.entry(milestone.as_str()).or_default() += 1;
            } else {
                errors.push(ValidationError::at(
                    &task.path,
                    "milestone tasks must include milestone",
                ));
            }
        }
    }

    for task in tasks {
        let Some(milestone) = &task.metadata.milestone else {
            continue;
        };
        match milestone_counts
            .get(milestone.as_str())
            .copied()
            .unwrap_or(0)
        {
            1 => {}
            0 => errors.push(ValidationError::at(
                &task.path,
                format!("milestone {milestone} does not reference an existing milestone task"),
            )),
            _ => errors.push(ValidationError::at(
                &task.path,
                format!("milestone {milestone} references multiple milestone tasks"),
            )),
        }
    }
}

fn validate_dependency_cycles(tasks: &[Task], errors: &mut Vec<ValidationError>) {
    let by_id: HashMap<&str, &Task> = tasks
        .iter()
        .map(|task| (task.metadata.id.as_str(), task))
        .collect();
    let mut checked = HashSet::new();

    for task in tasks {
        let mut visiting = Vec::new();
        detect_cycle(
            task.metadata.id.as_str(),
            &by_id,
            &mut visiting,
            &mut checked,
            errors,
        );
    }
}

fn detect_cycle<'a>(
    id: &'a str,
    by_id: &HashMap<&'a str, &'a Task>,
    visiting: &mut Vec<&'a str>,
    checked: &mut HashSet<&'a str>,
    errors: &mut Vec<ValidationError>,
) {
    if checked.contains(id) {
        return;
    }

    if let Some(position) = visiting.iter().position(|current| *current == id) {
        let cycle = visiting[position..].join(" -> ");
        errors.push(ValidationError::new(
            None,
            format!("dependency cycle detected: {cycle} -> {id}"),
        ));
        return;
    }

    let Some(task) = by_id.get(id) else {
        return;
    };

    visiting.push(id);
    for depends_on in &task.metadata.depends_on {
        detect_cycle(depends_on, by_id, visiting, checked, errors);
    }
    visiting.pop();
    checked.insert(id);
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

pub(crate) fn allowed_prefixes(domain: &str) -> &'static [&'static str] {
    match domain {
        "core" => CORE_PREFIXES,
        "app" => APP_PREFIXES,
        "ecosystem" => ECOSYSTEM_PREFIXES,
        "milestones" => &["MILESTONE"],
        _ => &[],
    }
}

pub(crate) fn is_valid_task_id(value: &str) -> bool {
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
        TaskStatus::Blocked => 2,
        TaskStatus::ToDo => 3,
        TaskStatus::Deferred => 4,
        TaskStatus::Obsolete => 5,
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
                milestone: None,
                tag: Some("provider".to_string()),
                blocked: false,
            },
            SortBy::Id,
        );

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].metadata.id, "VFS-001");
    }

    #[test]
    fn validate_rejects_missing_type() {
        let temp = task_repo();
        fs::write(
            temp.path().join(".tasks/core/CORE-001-location-routing.md"),
            "---\nid: CORE-001\ntitle: Location routing\nstatus: To Do\npriority: High\n---\n\n## Acceptance Criteria\n\n- [ ] Works\n",
        )
        .expect("task should be written");

        let report = validate_repo(temp.path()).expect("repo should scan");

        assert!(
            report
                .errors
                .iter()
                .any(|error| { error.message.contains("missing field `type`") })
        );
    }

    #[test]
    fn validate_rejects_missing_acceptance_criteria() {
        let temp = task_repo();
        fs::write(
            temp.path().join(".tasks/core/CORE-001-location-routing.md"),
            "---\nid: CORE-001\ntitle: Location routing\nstatus: To Do\npriority: High\ntype: Feature\n---\n",
        )
        .expect("task should be written");

        let report = validate_repo(temp.path()).expect("repo should scan");

        assert!(report.errors.iter().any(|error| {
            error
                .message
                .contains("tasks must include ## Acceptance Criteria")
        }));
    }

    #[test]
    fn validate_requires_exit_criteria_for_milestones() {
        let temp = task_repo();
        fs::create_dir_all(temp.path().join(".tasks/milestones"))
            .expect("milestone task dir should exist");
        fs::write(
            temp.path().join(".tasks/milestones/MILESTONE-000-core-contracts.md"),
            "---\nid: MILESTONE-000\ntitle: Core contracts\nstatus: To Do\npriority: High\ntype: Milestone\nmilestone: \"0.3.0\"\n---\n",
        )
        .expect("task should be written");

        let report = validate_repo(temp.path()).expect("repo should scan");

        assert!(report.errors.iter().any(|error| {
            error
                .message
                .contains("milestone and epic tasks must include ## Exit Criteria")
        }));
    }

    #[test]
    fn validate_accepts_project_milestone_references() {
        let temp = task_repo();
        write_milestone(temp.path(), "MILESTONE-003", "0.3.0");
        write_task(
            temp.path(),
            "core/CORE-042-timeout-propagation.md",
            "CORE-042",
            "Timeout propagation",
            "To Do",
            "High",
            "milestone: \"0.3.0\"\n",
        );

        let report = validate_repo(temp.path()).expect("repo should validate");

        assert!(report.errors.is_empty());
    }

    #[test]
    fn validate_rejects_missing_milestone_declaration() {
        let temp = task_repo();
        write_task(
            temp.path(),
            "core/CORE-042-timeout-propagation.md",
            "CORE-042",
            "Timeout propagation",
            "To Do",
            "High",
            "milestone: \"0.3.0\"\n",
        );

        let report = validate_repo(temp.path()).expect("repo should scan");

        assert!(report.errors.iter().any(|error| {
            error
                .message
                .contains("milestone 0.3.0 does not reference an existing milestone task")
        }));
    }

    #[test]
    fn validate_rejects_duplicate_milestone_declarations() {
        let temp = task_repo();
        write_milestone(temp.path(), "MILESTONE-003", "0.3.0");
        write_milestone(temp.path(), "MILESTONE-004", "0.3.0");
        write_task(
            temp.path(),
            "core/CORE-042-timeout-propagation.md",
            "CORE-042",
            "Timeout propagation",
            "To Do",
            "High",
            "milestone: \"0.3.0\"\n",
        );

        let report = validate_repo(temp.path()).expect("repo should scan");

        assert!(report.errors.iter().any(|error| {
            error
                .message
                .contains("milestone 0.3.0 references multiple milestone tasks")
        }));
    }

    #[test]
    fn validate_rejects_milestone_prefix_outside_milestones_dir() {
        let temp = task_repo();
        fs::write(
            temp.path().join(".tasks/core/MILESTONE-003-core-contracts.md"),
            "---\nid: MILESTONE-003\ntitle: Core contracts\nstatus: To Do\npriority: High\ntype: Feature\n---\n\n## Acceptance Criteria\n\n- [ ] Works\n",
        )
        .expect("task should be written");

        let report = validate_repo(temp.path()).expect("repo should scan");

        assert!(report.errors.iter().any(|error| {
            error
                .message
                .contains("MILESTONE prefix is only allowed under .tasks/milestones")
        }));
    }

    #[test]
    fn validate_rejects_blocked_tasks_without_reason() {
        let temp = task_repo();
        write_task(
            temp.path(),
            "core/CORE-042-timeout-propagation.md",
            "CORE-042",
            "Timeout propagation",
            "Blocked",
            "High",
            "",
        );

        let report = validate_repo(temp.path()).expect("repo should scan");

        assert!(report.errors.iter().any(|error| {
            error
                .message
                .contains("blocked tasks must include ## Blocked Reason")
        }));
    }

    #[test]
    fn validate_rejects_done_tasks_with_unchecked_acceptance() {
        let temp = task_repo();
        write_task(
            temp.path(),
            "core/CORE-042-timeout-propagation.md",
            "CORE-042",
            "Timeout propagation",
            "Done",
            "High",
            "",
        );

        let report = validate_repo(temp.path()).expect("repo should scan");

        assert!(report.errors.iter().any(|error| {
            error
                .message
                .contains("done tasks must not have unchecked ## Acceptance Criteria items")
        }));
    }

    #[test]
    fn validate_requires_rationale_for_deferred_tasks() {
        let temp = task_repo();
        fs::write(
            temp.path().join(".tasks/core/CORE-001-location-routing.md"),
            "---\nid: CORE-001\ntitle: Location routing\nstatus: Deferred\npriority: High\ntype: Feature\n---\n",
        )
        .expect("task should be written");

        let report = validate_repo(temp.path()).expect("repo should scan");

        assert!(report.errors.iter().any(|error| {
            error
                .message
                .contains("deferred and obsolete tasks must include ## Rationale")
        }));
    }

    #[test]
    fn validate_rejects_dependency_problems() {
        let temp = task_repo();
        write_task(
            temp.path(),
            "core/CORE-001-location-routing.md",
            "CORE-001",
            "Location routing",
            "To Do",
            "High",
            "depends_on: [CORE-001, CORE-404, CORE-404]\n",
        );

        let report = validate_repo(temp.path()).expect("repo should scan");

        assert!(
            report
                .errors
                .iter()
                .any(|error| error.message.contains("task cannot depend on itself"))
        );
        assert!(report.errors.iter().any(|error| {
            error
                .message
                .contains("dependency CORE-404 does not reference an existing task")
        }));
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.message.contains("duplicate dependency id CORE-404"))
        );
    }

    #[test]
    fn validate_rejects_dependency_cycles() {
        let temp = task_repo();
        write_task(
            temp.path(),
            "core/CORE-001-location-routing.md",
            "CORE-001",
            "Location routing",
            "To Do",
            "High",
            "depends_on: [CORE-002]\n",
        );
        write_task(
            temp.path(),
            "core/CORE-002-location-cache.md",
            "CORE-002",
            "Location cache",
            "To Do",
            "High",
            "depends_on: [CORE-001]\n",
        );

        let report = validate_repo(temp.path()).expect("repo should scan");

        assert!(report.errors.iter().any(|error| {
            error
                .message
                .contains("dependency cycle detected: CORE-001 -> CORE-002 -> CORE-001")
        }));
    }

    #[test]
    fn validate_rejects_unknown_rule_and_short_impact() {
        let temp = task_repo();
        write_task(
            temp.path(),
            "core/CORE-001-location-routing.md",
            "CORE-001",
            "Location routing",
            "To Do",
            "High",
            "rules: [CORE-LIBRARY, UNKNOWN-RULE]\nimpact: short\n",
        );

        let report = validate_repo(temp.path()).expect("repo should scan");

        assert!(
            report
                .errors
                .iter()
                .any(|error| error.message.contains("unknown rule id UNKNOWN-RULE"))
        );
        assert!(report.errors.iter().any(|error| {
            error
                .message
                .contains("impact must be at least 10 characters when present")
        }));
    }

    #[test]
    fn validate_accepts_expanded_prefixes() {
        let temp = task_repo();
        write_task(
            temp.path(),
            "core/PROTOCOL-001-wire-envelope.md",
            "PROTOCOL-001",
            "Wire envelope",
            "To Do",
            "High",
            "",
        );
        write_task(
            temp.path(),
            "ecosystem/EXT-001-decoration-output.md",
            "EXT-001",
            "Decoration output",
            "To Do",
            "High",
            "",
        );

        let report = validate_repo(temp.path()).expect("repo should validate");

        assert!(report.errors.is_empty());
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

    fn write_milestone(root: &Path, id: &str, milestone: &str) {
        let path = root
            .join(".tasks/milestones")
            .join(format!("{id}-project-milestone.md"));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("task parent dir should exist");
        }
        fs::write(
            path,
            format!(
                "---\nid: {id}\ntitle: Project milestone\nstatus: To Do\npriority: High\ntype: Milestone\nmilestone: \"{milestone}\"\n---\n\n## Exit Criteria\n\n- [ ] Finished\n"
            ),
        )
        .expect("milestone should be written");
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
                "---\nid: {id}\ntitle: {title}\nstatus: {status}\npriority: {priority}\ntype: Feature\n{extra}---\n\n## Acceptance Criteria\n\n- [ ] Works\n"
            ),
        )
        .expect("task should be written");
    }
}
