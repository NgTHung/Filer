//! # Project Configuration
//!
//! This module opens an explicit task-project root and loads its immutable
//! policy from `.tasks/config.json`. It never discovers ancestors or reads the
//! process working directory, so one process can safely keep several projects.
//!
//! ```
//! use filer_task::project::TaskProject;
//!
//! let root = std::env::temp_dir().join(format!(
//!     "filer-task-policy-example-{}",
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
};

use crate::error::TaskError;

use self::json::{JsonNode, read_json};

mod json;

pub const CONFIG_VERSION: u64 = 1;
pub const CONFIG_PATH: &str = ".tasks/config.json";

const CORE_PREFIXES: &[&str] = &[
    "CORE", "ACTORS", "API", "MODULES", "PIPELINE", "SERVICES", "UTILS", "VFS", "REL", "NAV",
    "SEARCH", "OPS", "PREVIEW", "PROVIDER", "PROTOCOL",
];
const APP_PREFIXES: &[&str] = &["UI", "EXPL", "SETS", "SRCH", "MEDIA", "NAV", "PERF", "A11Y"];
const ECOSYSTEM_PREFIXES: &[&str] = &["PLUG", "EXT", "THEME", "PROFILE", "PROVIDER"];
const ACCEPTANCE_TYPES: &[&str] = &[
    "Feature", "Bug", "Refactor", "TechDebt", "TestDebt", "Design", "Docs",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskProject {
    root: PathBuf,
    policy: ProjectPolicy,
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
        let path = root.join(CONFIG_PATH);
        let exists = path.try_exists().map_err(|source| TaskError::ConfigIo {
            path: path.clone(),
            operation: "inspect",
            source,
        })?;
        let policy = if exists {
            load_policy(&path)?
        } else {
            ProjectPolicy::filer_compatibility()
        };
        Ok(Self { root, policy })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn policy(&self) -> &ProjectPolicy {
        &self.policy
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectPolicy {
    domains: BTreeMap<String, DomainPolicy>,
    task_types: BTreeMap<String, TaskTypePolicy>,
    tags: TagPolicy,
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
    let tags = parse_tags(config_path, required(config_path, "$", fields, "tags")?)?;
    Ok(ProjectPolicy {
        domains,
        task_types,
        tags,
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

fn parse_tags(config_path: &Path, node: &JsonNode) -> Result<TagPolicy, TaskError> {
    let path = "$.tags";
    let fields = strict_object(config_path, path, node, &["policy", "allowed"])?;
    let policy = string(
        config_path,
        "$.tags.policy",
        required(config_path, path, fields, "policy")?,
    )?;
    match policy {
        "open" => {
            if field(fields, "allowed").is_some() {
                return invalid(
                    config_path,
                    "$.tags.allowed",
                    "allowed",
                    "open tag policy must omit `allowed`",
                );
            }
            Ok(TagPolicy::Open)
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
            Ok(TagPolicy::Strict { allowed })
        }
        value => invalid(
            config_path,
            "$.tags.policy",
            value,
            "must be `open` or `strict`",
        ),
    }
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
        NameKind::Domain => valid_hyphen_name(value, 64, true),
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
    } && !is_windows_device_name(value);
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

fn valid_hyphen_name(value: &str, max: usize, starts_with_letter: bool) -> bool {
    let bytes = value.as_bytes();
    if !(1..=max).contains(&bytes.len()) {
        return false;
    }
    let first_valid = if starts_with_letter {
        bytes[0].is_ascii_lowercase()
    } else {
        bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit()
    };
    first_valid
        && (bytes[bytes.len() - 1].is_ascii_lowercase() || bytes[bytes.len() - 1].is_ascii_digit())
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        && !value.contains("--")
}

fn is_windows_device_name(value: &str) -> bool {
    let upper = value.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || upper.strip_prefix("COM").is_some_and(|number| {
            matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
        || upper.strip_prefix("LPT").is_some_and(|number| {
            matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
}
