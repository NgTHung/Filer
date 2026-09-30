//! # Project Configuration
//!
//! This module opens an explicit task-project root, loads its immutable policy,
//! and records a content revision for configuration and task files. Clones of
//! one handle share revision state, while different roots remain isolated.
//!
//! Mutations take an operating-system lock on `.tasks/.taskroot.lock`, so
//! handles and processes that use this library serialize writes to one root.
//! Each task file is replaced atomically. If an editor or another process
//! changes project content without updating the handle, mutation returns
//! `project_stale`; call [`TaskProject::reload`] and retry with the new handle.
//! Ordinary task writes refresh every clone's shared revision. Policy writes
//! return a reopened handle because existing handles retain their immutable
//! policy and intentionally become stale after the configuration changes.
//!
//! ```
//! use taskroot::project::TaskProject;
//!
//! let root = std::env::temp_dir().join(format!(
//!     "taskroot-policy-example-{}",
//!     std::process::id()
//! ));
//! std::fs::create_dir_all(root.join(".tasks"))?;
//! let project = TaskProject::open(&root)?;
//! assert!(project.policy().domain("core").is_some());
//! std::fs::remove_dir_all(root)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};

use crate::{
    domain::{APP_PREFIXES, CORE_PREFIXES, ECOSYSTEM_PREFIXES},
    error::TaskError,
    identity::{TaskIdentity, is_valid_domain_name, is_windows_device_name, valid_hyphen_name},
};

use self::json::{JsonNode, read_json};

mod freshness;
mod json;
mod policy;

pub use policy::{InitDomain, InitProjectOptions};

use freshness::ProjectRevision;

pub const CONFIG_VERSION: u64 = 1;
pub const CONFIG_PATH: &str = ".tasks/config.json";
pub(crate) const PROJECT_LOCK_PATH: &str = ".tasks/.taskroot.lock";

const ACCEPTANCE_TYPES: &[&str] = &[
    "Feature", "Bug", "Refactor", "TechDebt", "TestDebt", "Design", "Docs",
];

#[derive(Debug, Clone)]
pub struct TaskProject {
    root: PathBuf,
    policy: ProjectPolicy,
    state: Arc<ProjectState>,
}

#[derive(Debug)]
struct ProjectState {
    revision: Mutex<ProjectRevision>,
}

impl TaskProject {
    /// Open one explicit root and resolve its policy once.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, TaskError> {
        let root = root
            .as_ref()
            .canonicalize()
            .map_err(|source| TaskError::Io {
                path: root.as_ref().to_path_buf(),
                source,
            })?;
        let mut attempts = 0;
        loop {
            let before = ProjectRevision::read(&root)?;
            let policy = load_project_policy(&root);
            let revision = ProjectRevision::read(&root)?;
            if before == revision {
                return Ok(Self {
                    root,
                    policy: policy?,
                    state: Arc::new(ProjectState {
                        revision: Mutex::new(revision),
                    }),
                });
            }
            attempts += 1;
            if attempts == 3 {
                return Err(TaskError::StaleProject { root });
            }
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn policy(&self) -> &ProjectPolicy {
        &self.policy
    }

    /// Return whether task or configuration content changed since this handle
    /// opened or last completed a mutation.
    pub fn is_stale(&self) -> Result<bool, TaskError> {
        let baseline = revision_guard(&self.state.revision);
        Ok(*baseline != ProjectRevision::read(&self.root)?)
    }

    /// Open a fresh handle for the same canonical project root.
    ///
    /// Reloading re-reads configuration and establishes a new content revision.
    /// Existing clones keep their previous policy and revision.
    pub fn reload(&self) -> Result<Self, TaskError> {
        Self::open(&self.root)
    }

    pub(crate) fn with_write_lock<T>(
        &self,
        operation: impl FnOnce() -> Result<T, TaskError>,
    ) -> Result<T, TaskError> {
        self.with_project_lock(RevisionUpdate::Refresh, operation)
    }

    fn with_policy_write_lock<T>(
        &self,
        operation: impl FnOnce() -> Result<T, TaskError>,
    ) -> Result<T, TaskError> {
        self.with_project_lock(RevisionUpdate::Keep, operation)
    }

    fn with_project_lock<T>(
        &self,
        revision_update: RevisionUpdate,
        operation: impl FnOnce() -> Result<T, TaskError>,
    ) -> Result<T, TaskError> {
        let mut baseline = revision_guard(&self.state.revision);
        let lock_path = self.root.join(PROJECT_LOCK_PATH);
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|source| TaskError::Io {
                path: lock_path.clone(),
                source,
            })?;
        lock.lock().map_err(|source| TaskError::Io {
            path: lock_path,
            source,
        })?;
        if *baseline != ProjectRevision::read(&self.root)? {
            return Err(TaskError::StaleProject {
                root: self.root.clone(),
            });
        }
        let result = operation()?;
        if matches!(revision_update, RevisionUpdate::Refresh) {
            *baseline = ProjectRevision::read(&self.root)?;
        }
        Ok(result)
    }

    pub(crate) fn task_path(&self, identity: &TaskIdentity) -> Result<PathBuf, TaskError> {
        let baseline = revision_guard(&self.state.revision);
        let paths = baseline.task_paths(&self.root, identity);
        match paths.as_slice() {
            [path] => Ok(path.clone()),
            [] => Err(TaskError::TaskNotFound {
                reference: identity.to_string(),
                source_domain: None,
                root: self.root.clone(),
            }),
            _ => Err(TaskError::Validation(vec![
                crate::error::ValidationError::new(None, format!("duplicate task id {identity}")),
            ])),
        }
    }

    pub(crate) fn with_candidate_policy(&self, policy: ProjectPolicy) -> Self {
        Self {
            root: self.root.clone(),
            policy,
            state: self.state.clone(),
        }
    }
}

#[derive(Clone, Copy)]
enum RevisionUpdate {
    Refresh,
    Keep,
}

impl PartialEq for TaskProject {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root && self.policy == other.policy
    }
}

impl Eq for TaskProject {}

fn revision_guard(revision: &Mutex<ProjectRevision>) -> MutexGuard<'_, ProjectRevision> {
    // A panic leaves the old revision intact, so the next comparison detects any completed write.
    revision
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn load_project_policy(root: &Path) -> Result<ProjectPolicy, TaskError> {
    let path = root.join(CONFIG_PATH);
    let exists = path.try_exists().map_err(|source| TaskError::ConfigIo {
        path: path.clone(),
        operation: "inspect",
        source,
    })?;
    if exists {
        load_policy(&path)
    } else {
        Ok(ProjectPolicy::filer_compatibility())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectPolicy {
    domains: BTreeMap<String, DomainPolicy>,
    task_types: BTreeMap<String, TaskTypePolicy>,
    tags: TagPolicy,
    exclusive_tag_groups: BTreeMap<String, Vec<String>>,
    compatibility: bool,
}

impl ProjectPolicy {
    pub fn domains(&self) -> &BTreeMap<String, DomainPolicy> {
        &self.domains
    }

    pub fn domain(&self, name: &str) -> Option<&DomainPolicy> {
        self.domains.get(name)
    }

    pub fn task_types(&self) -> &BTreeMap<String, TaskTypePolicy> {
        &self.task_types
    }

    pub fn task_type(&self, name: &str) -> Option<&TaskTypePolicy> {
        self.task_types.get(name)
    }

    pub fn milestone_type(&self) -> Option<&str> {
        self.task_types.iter().find_map(|(name, policy)| {
            (policy.role == Some(TaskTypeRole::Milestone)).then_some(name.as_str())
        })
    }

    pub fn tags(&self) -> &TagPolicy {
        &self.tags
    }

    pub fn exclusive_tag_group(&self, name: &str) -> Option<&[String]> {
        self.exclusive_tag_groups.get(name).map(Vec::as_slice)
    }

    pub fn exclusive_tag_groups(&self) -> &BTreeMap<String, Vec<String>> {
        &self.exclusive_tag_groups
    }

    pub fn is_compatibility(&self) -> bool {
        self.compatibility
    }

    fn filer_compatibility() -> Self {
        let mut domains = BTreeMap::new();
        insert_domain(&mut domains, "core", CORE_PREFIXES);
        insert_domain(&mut domains, "app", APP_PREFIXES);
        insert_domain(&mut domains, "ecosystem", ECOSYSTEM_PREFIXES);
        insert_domain(&mut domains, "milestones", &["MILESTONE"]);

        let mut task_types = BTreeMap::new();
        task_types.insert(
            "Milestone".to_string(),
            TaskTypePolicy {
                criteria: CriteriaPolicy::Exit,
                role: Some(TaskTypeRole::Milestone),
            },
        );
        task_types.insert(
            "Epic".to_string(),
            TaskTypePolicy {
                criteria: CriteriaPolicy::Exit,
                role: None,
            },
        );
        for name in ACCEPTANCE_TYPES {
            task_types.insert(
                (*name).to_string(),
                TaskTypePolicy {
                    criteria: CriteriaPolicy::Acceptance,
                    role: None,
                },
            );
        }

        Self {
            domains,
            task_types,
            tags: TagPolicy::Open,
            exclusive_tag_groups: BTreeMap::new(),
            compatibility: true,
        }
    }
}

fn insert_domain(domains: &mut BTreeMap<String, DomainPolicy>, name: &str, prefixes: &[&str]) {
    domains.insert(
        name.to_string(),
        DomainPolicy {
            prefixes: prefixes.iter().map(|value| (*value).to_string()).collect(),
        },
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPolicy {
    prefixes: Vec<String>,
}

impl DomainPolicy {
    pub fn prefixes(&self) -> &[String] {
        &self.prefixes
    }

    pub fn allows_prefix(&self, prefix: &str) -> bool {
        self.prefixes.iter().any(|allowed| allowed == prefix)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CriteriaPolicy {
    Acceptance,
    Exit,
}

impl CriteriaPolicy {
    pub fn heading(self) -> &'static str {
        match self {
            Self::Acceptance => "Acceptance Criteria",
            Self::Exit => "Exit Criteria",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskTypeRole {
    Milestone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskTypePolicy {
    criteria: CriteriaPolicy,
    role: Option<TaskTypeRole>,
}

impl TaskTypePolicy {
    pub fn criteria(&self) -> CriteriaPolicy {
        self.criteria
    }

    pub fn role(&self) -> Option<TaskTypeRole> {
        self.role
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagPolicy {
    Open,
    Strict { allowed: Vec<String> },
}

impl TagPolicy {
    pub fn allows(&self, tag: &str) -> bool {
        match self {
            Self::Open => true,
            Self::Strict { allowed } => allowed.iter().any(|value| value == tag),
        }
    }
}

fn load_policy(path: &Path) -> Result<ProjectPolicy, TaskError> {
    let node = read_json(path)?;
    parse_policy(path, &node)
}

fn parse_policy(config_path: &Path, node: &JsonNode) -> Result<ProjectPolicy, TaskError> {
    let fields = strict_object(
        config_path,
        "$",
        node,
        &["version", "domains", "task_types", "tags"],
    )?;
    let version_node = required(config_path, "$", fields, "version")?;
    let version = unsigned(config_path, "$.version", version_node)?;
    if version != CONFIG_VERSION {
        return Err(TaskError::ConfigUnsupportedVersion {
            path: config_path.to_path_buf(),
            received: version,
            supported: CONFIG_VERSION,
        });
    }

    let domains = parse_domains(config_path, required(config_path, "$", fields, "domains")?)?;
    let task_types = parse_task_types(
        config_path,
        required(config_path, "$", fields, "task_types")?,
    )?;
    let (tags, exclusive_tag_groups) =
        parse_tags(config_path, required(config_path, "$", fields, "tags")?)?;
    Ok(ProjectPolicy {
        domains,
        task_types,
        tags,
        exclusive_tag_groups,
        compatibility: false,
    })
}

fn parse_domains(
    config_path: &Path,
    node: &JsonNode,
) -> Result<BTreeMap<String, DomainPolicy>, TaskError> {
    let fields = unique_object(config_path, "$.domains", node)?;
    if fields.is_empty() {
        return invalid(
            config_path,
            "$.domains",
            "{}",
            "must define at least one domain",
        );
    }
    let mut domains = BTreeMap::new();
    for (name, node) in fields {
        let path = format!("$.domains.{name}");
        validate_name(config_path, &path, name, NameKind::Domain)?;
        let values = strict_object(config_path, &path, node, &["prefixes"])?;
        let prefix_path = format!("{path}.prefixes");
        let prefixes = string_array(
            config_path,
            &prefix_path,
            required(config_path, &path, values, "prefixes")?,
            false,
        )?;
        for prefix in &prefixes {
            validate_name(config_path, &prefix_path, prefix, NameKind::Prefix)?;
        }
        domains.insert(name.clone(), DomainPolicy { prefixes });
    }
    Ok(domains)
}

fn parse_task_types(
    config_path: &Path,
    node: &JsonNode,
) -> Result<BTreeMap<String, TaskTypePolicy>, TaskError> {
    let fields = unique_object(config_path, "$.task_types", node)?;
    if fields.is_empty() {
        return invalid(
            config_path,
            "$.task_types",
            "{}",
            "must define at least one task type",
        );
    }
    let mut task_types = BTreeMap::new();
    let mut milestone_types = Vec::new();
    for (name, node) in fields {
        let path = format!("$.task_types.{name}");
        validate_name(config_path, &path, name, NameKind::TaskType)?;
        let values = strict_object(config_path, &path, node, &["criteria", "role"])?;
        let criteria_path = format!("{path}.criteria");
        let criteria = match string(
            config_path,
            &criteria_path,
            required(config_path, &path, values, "criteria")?,
        )? {
            "acceptance" => CriteriaPolicy::Acceptance,
            "exit" => CriteriaPolicy::Exit,
            value => {
                return invalid(
                    config_path,
                    &criteria_path,
                    value,
                    "must be `acceptance` or `exit`",
                );
            }
        };
        let role = match field(values, "role") {
            None => None,
            Some(node) => match string(config_path, &format!("{path}.role"), node)? {
                "milestone" => {
                    milestone_types.push(name.clone());
                    Some(TaskTypeRole::Milestone)
                }
                value => {
                    return invalid(
                        config_path,
                        &format!("{path}.role"),
                        value,
                        "must be `milestone` when present",
                    );
                }
            },
        };
        task_types.insert(name.clone(), TaskTypePolicy { criteria, role });
    }
    if milestone_types.len() > 1 {
        return invalid(
            config_path,
            "$.task_types",
            milestone_types.join(", "),
            "at most one task type may use the milestone role",
        );
    }
    Ok(task_types)
}

fn parse_tags(
    config_path: &Path,
    node: &JsonNode,
) -> Result<(TagPolicy, BTreeMap<String, Vec<String>>), TaskError> {
    let path = "$.tags";
    let fields = strict_object(
        config_path,
        path,
        node,
        &["policy", "allowed", "exclusive_groups"],
    )?;
    let policy = string(
        config_path,
        "$.tags.policy",
        required(config_path, path, fields, "policy")?,
    )?;
    let exclusive_groups = match field(fields, "exclusive_groups") {
        Some(node) => parse_exclusive_tag_groups(config_path, node)?,
        None => BTreeMap::new(),
    };
    let policy = match policy {
        "open" => {
            if field(fields, "allowed").is_some() {
                return invalid(
                    config_path,
                    "$.tags.allowed",
                    "allowed",
                    "open tag policy must omit `allowed`",
                );
            }
            TagPolicy::Open
        }
        "strict" => {
            let allowed = string_array(
                config_path,
                "$.tags.allowed",
                required(config_path, path, fields, "allowed")?,
                true,
            )?;
            for tag in &allowed {
                validate_name(config_path, "$.tags.allowed", tag, NameKind::Tag)?;
            }
            TagPolicy::Strict { allowed }
        }
        value => {
            return invalid(
                config_path,
                "$.tags.policy",
                value,
                "must be `open` or `strict`",
            );
        }
    };
    validate_exclusive_tag_groups(config_path, &policy, &exclusive_groups)?;
    Ok((policy, exclusive_groups))
}

fn validate_exclusive_tag_groups(
    config_path: &Path,
    policy: &TagPolicy,
    groups: &BTreeMap<String, Vec<String>>,
) -> Result<(), TaskError> {
    if groups.is_empty() {
        return Ok(());
    }
    let TagPolicy::Strict { allowed } = policy else {
        return invalid(
            config_path,
            "$.tags.exclusive_groups",
            "exclusive_groups",
            "exclusive tag groups require a strict allowed tag catalog",
        );
    };
    for (name, tags) in groups {
        for tag in tags {
            if !allowed.iter().any(|allowed| allowed == tag) {
                return invalid(
                    config_path,
                    &format!("$.tags.exclusive_groups.{name}"),
                    tag,
                    "must belong to the allowed tag catalog",
                );
            }
        }
    }
    Ok(())
}

fn parse_exclusive_tag_groups(
    config_path: &Path,
    node: &JsonNode,
) -> Result<BTreeMap<String, Vec<String>>, TaskError> {
    let fields = unique_object(config_path, "$.tags.exclusive_groups", node)?;
    let mut groups = BTreeMap::new();
    for (name, node) in fields {
        let path = format!("$.tags.exclusive_groups.{name}");
        validate_name(config_path, &path, name, NameKind::Tag)?;
        let tags = string_array(config_path, &path, node, false)?;
        for tag in &tags {
            validate_name(config_path, &path, tag, NameKind::Tag)?;
        }
        groups.insert(name.clone(), tags);
    }
    Ok(groups)
}

fn strict_object<'a>(
    config_path: &Path,
    path: &str,
    node: &'a JsonNode,
    known: &[&str],
) -> Result<&'a [(String, JsonNode)], TaskError> {
    let fields = unique_object(config_path, path, node)?;
    for (field, _) in fields {
        if !known.contains(&field.as_str()) {
            return Err(TaskError::ConfigUnknownField {
                config_path: config_path.to_path_buf(),
                path: path.to_string(),
                field: field.clone(),
            });
        }
    }
    Ok(fields)
}

fn unique_object<'a>(
    config_path: &Path,
    path: &str,
    node: &'a JsonNode,
) -> Result<&'a [(String, JsonNode)], TaskError> {
    let JsonNode::Object(fields) = node else {
        return invalid(
            config_path,
            path,
            node.display_value(),
            "must be a JSON object",
        );
    };
    let mut seen = HashSet::new();
    for (field, _) in fields {
        if !seen.insert(field) {
            return Err(TaskError::ConfigDuplicate {
                config_path: config_path.to_path_buf(),
                path: path.to_string(),
                value: field.clone(),
            });
        }
    }
    Ok(fields)
}

fn required<'a>(
    config_path: &Path,
    path: &str,
    fields: &'a [(String, JsonNode)],
    name: &str,
) -> Result<&'a JsonNode, TaskError> {
    field(fields, name).ok_or_else(|| TaskError::ConfigInvalidValue {
        config_path: config_path.to_path_buf(),
        path: format!("{path}.{name}"),
        value: "missing".to_string(),
        constraint: "field is required and cannot be null".to_string(),
    })
}

fn field<'a>(fields: &'a [(String, JsonNode)], name: &str) -> Option<&'a JsonNode> {
    fields
        .iter()
        .find_map(|(key, value)| (key == name).then_some(value))
}

fn string<'a>(config_path: &Path, path: &str, node: &'a JsonNode) -> Result<&'a str, TaskError> {
    match node {
        JsonNode::String(value) => Ok(value),
        _ => invalid(
            config_path,
            path,
            node.display_value(),
            "must be a JSON string and cannot be null",
        ),
    }
}

fn unsigned(config_path: &Path, path: &str, node: &JsonNode) -> Result<u64, TaskError> {
    match node {
        JsonNode::Unsigned(value) => Ok(*value),
        _ => invalid(
            config_path,
            path,
            node.display_value(),
            "must be a non-negative integer",
        ),
    }
}

fn string_array(
    config_path: &Path,
    path: &str,
    node: &JsonNode,
    empty_allowed: bool,
) -> Result<Vec<String>, TaskError> {
    let JsonNode::Array(values) = node else {
        return invalid(
            config_path,
            path,
            node.display_value(),
            "must be a JSON array and cannot be null",
        );
    };
    if values.is_empty() && !empty_allowed {
        return invalid(config_path, path, "[]", "must not be empty");
    }
    let mut result = Vec::with_capacity(values.len());
    let mut seen = HashSet::new();
    for node in values {
        let value = string(config_path, path, node)?;
        if !seen.insert(value) {
            return Err(TaskError::ConfigDuplicate {
                config_path: config_path.to_path_buf(),
                path: path.to_string(),
                value: value.to_string(),
            });
        }
        result.push(value.to_string());
    }
    Ok(result)
}

fn invalid<T>(
    config_path: &Path,
    path: &str,
    value: impl Into<String>,
    constraint: impl Into<String>,
) -> Result<T, TaskError> {
    Err(TaskError::ConfigInvalidValue {
        config_path: config_path.to_path_buf(),
        path: path.to_string(),
        value: value.into(),
        constraint: constraint.into(),
    })
}

#[derive(Clone, Copy)]
enum NameKind {
    Domain,
    Prefix,
    TaskType,
    Tag,
}

fn validate_name(
    config_path: &Path,
    path: &str,
    value: &str,
    kind: NameKind,
) -> Result<(), TaskError> {
    let valid = match kind {
        NameKind::Domain => is_valid_domain_name(value),
        NameKind::Prefix => {
            (1..=32).contains(&value.len())
                && value.as_bytes().first().is_some_and(u8::is_ascii_uppercase)
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        }
        NameKind::TaskType => {
            (1..=64).contains(&value.len())
                && value.as_bytes().first().is_some_and(u8::is_ascii_uppercase)
                && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
        }
        NameKind::Tag => valid_hyphen_name(value, 64, false),
    } && (matches!(kind, NameKind::Domain) || !is_windows_device_name(value));
    if valid {
        Ok(())
    } else {
        let constraint = match kind {
            NameKind::Domain => "must be a portable lowercase domain name",
            NameKind::Prefix => "must be a portable uppercase prefix",
            NameKind::TaskType => "must start uppercase and contain only ASCII letters or digits",
            NameKind::Tag => "must be a portable lowercase tag name",
        };
        invalid(config_path, path, value, constraint)
    }
}
