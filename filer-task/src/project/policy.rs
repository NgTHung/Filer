//! # Project Policy Writes
//!
//! This module renders project policy as configuration JSON for project
//! initialization and later policy mutation. Rendering stays deterministic so
//! rejected mutations can prove the stored configuration did not change.
//!
//! ```
//! use filer_task::project::{InitDomain, InitProjectOptions, TaskProject};
//!
//! let root = tempfile::tempdir()?;
//! let project = TaskProject::init(
//!     root.path(),
//!     InitProjectOptions {
//!         domain: InitDomain::new("work", ["WORK"]),
//!     },
//! )?;
//! assert!(project.root().join(".tasks/config.json").is_file());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde_json::{Map, Value};

use super::{
    CONFIG_PATH, CriteriaPolicy, DomainPolicy, NameKind, ProjectPolicy, TagPolicy, TaskProject,
    TaskTypePolicy, TaskTypeRole, validate_name,
};
use crate::{error::TaskError, repo::TASK_DIR, validate::require_valid_report};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitProjectOptions {
    pub domain: InitDomain,
}

impl Default for InitProjectOptions {
    fn default() -> Self {
        Self {
            domain: InitDomain::new("default", ["WORK"]),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitDomain {
    pub name: String,
    pub prefixes: Vec<String>,
}

impl InitDomain {
    pub fn new<const N: usize>(name: impl Into<String>, prefixes: [&str; N]) -> Self {
        Self {
            name: name.into(),
            prefixes: prefixes.iter().map(|value| (*value).to_string()).collect(),
        }
    }
}

impl TaskProject {
    /// Create a task project at `root` and return an opened handle.
    ///
    /// Initialization writes `.tasks/config.json` but leaves domain directories
    /// absent until task creation needs them.
    pub fn init(root: impl AsRef<Path>, options: InitProjectOptions) -> Result<Self, TaskError> {
        let root = root
            .as_ref()
            .canonicalize()
            .map_err(|source| TaskError::Io {
                path: root.as_ref().to_path_buf(),
                source,
            })?;
        let tasks = root.join(TASK_DIR);
        if tasks.try_exists().map_err(|source| TaskError::Io {
            path: tasks.clone(),
            source,
        })? {
            return Err(TaskError::ProjectAlreadyExists { root });
        }

        let config_path = root.join(CONFIG_PATH);
        let policy = ProjectPolicy::for_init(&config_path, options)?;
        let content = policy.to_config_json()?;
        fs::create_dir(&tasks).map_err(|source| TaskError::Io {
            path: tasks.clone(),
            source,
        })?;
        let mut cleanup = InitializationCleanup::new(tasks, config_path.clone(), content.clone());
        crate::atomic_write::create(&config_path, &content).map_err(|source| {
            TaskError::ConfigIo {
                path: config_path.clone(),
                operation: "write",
                source,
            }
        })?;
        let project = Self::open(root)?;
        cleanup.disarm();
        Ok(project)
    }
}

impl ProjectPolicy {
    pub(crate) fn for_init(
        config_path: &Path,
        options: InitProjectOptions,
    ) -> Result<Self, TaskError> {
        validate_name(
            config_path,
            "$.domains",
            &options.domain.name,
            NameKind::Domain,
        )?;
        validate_prefixes(config_path, &options.domain.prefixes)?;

        let mut domains = std::collections::BTreeMap::new();
        domains.insert(
            options.domain.name,
            DomainPolicy {
                prefixes: options.domain.prefixes,
            },
        );
        let mut task_types = std::collections::BTreeMap::new();
        task_types.insert(
            "Feature".to_string(),
            TaskTypePolicy {
                criteria: CriteriaPolicy::Acceptance,
                role: None,
            },
        );

        Ok(Self {
            domains,
            task_types,
            tags: TagPolicy::Open,
            exclusive_tag_groups: BTreeMap::new(),
            compatibility: false,
        })
    }

    pub(crate) fn to_config_json(&self) -> Result<String, TaskError> {
        let mut root = Map::new();
        root.insert("version".to_string(), Value::from(super::CONFIG_VERSION));
        root.insert("domains".to_string(), self.domains_json());
        root.insert("task_types".to_string(), self.task_types_json());
        root.insert("tags".to_string(), self.tags_json());
        let mut rendered = serde_json::to_string_pretty(&Value::Object(root))?;
        rendered.push('\n');
        Ok(rendered)
    }

    fn domains_json(&self) -> Value {
        let mut domains = Map::new();
        for (name, policy) in &self.domains {
            let mut domain = Map::new();
            domain.insert(
                "prefixes".to_string(),
                Value::Array(policy.prefixes.iter().cloned().map(Value::from).collect()),
            );
            domains.insert(name.clone(), Value::Object(domain));
        }
        Value::Object(domains)
    }

    fn task_types_json(&self) -> Value {
        let mut task_types = Map::new();
        for (name, policy) in &self.task_types {
            let mut task_type = Map::new();
            let criteria = match policy.criteria {
                CriteriaPolicy::Acceptance => "acceptance",
                CriteriaPolicy::Exit => "exit",
            };
            task_type.insert("criteria".to_string(), Value::from(criteria));
            if policy.role == Some(TaskTypeRole::Milestone) {
                task_type.insert("role".to_string(), Value::from("milestone"));
            }
            task_types.insert(name.clone(), Value::Object(task_type));
        }
        Value::Object(task_types)
    }

    fn tags_json(&self) -> Value {
        let mut tags = Map::new();
        match &self.tags {
            TagPolicy::Open => {
                tags.insert("policy".to_string(), Value::from("open"));
            }
            TagPolicy::Strict { allowed } => {
                tags.insert("policy".to_string(), Value::from("strict"));
                tags.insert(
                    "allowed".to_string(),
                    Value::Array(allowed.iter().cloned().map(Value::from).collect()),
                );
            }
        }
        if !self.exclusive_tag_groups.is_empty() {
            let groups = self
                .exclusive_tag_groups
                .iter()
                .map(|(name, values)| {
                    (
                        name.clone(),
                        Value::Array(values.iter().cloned().map(Value::from).collect()),
                    )
                })
                .collect();
            tags.insert("exclusive_groups".to_string(), Value::Object(groups));
        }
        Value::Object(tags)
    }
}

struct InitializationCleanup {
    tasks: PathBuf,
    config: PathBuf,
    content: String,
    armed: bool,
}

impl InitializationCleanup {
    fn new(tasks: PathBuf, config: PathBuf, content: String) -> Self {
        Self {
            tasks,
            config,
            content,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for InitializationCleanup {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        if fs::read_to_string(&self.config).is_ok_and(|stored| stored == self.content) {
            let _ = fs::remove_file(&self.config);
        }
        let _ = fs::remove_dir(&self.tasks);
    }
}

impl TaskProject {
    /// Add a configured domain and atomically write the updated policy.
    pub fn add_domain(
        &self,
        name: impl AsRef<str>,
        prefixes: &[String],
    ) -> Result<Self, TaskError> {
        self.mutate_policy(|config_path, policy| {
            let name = name.as_ref();
            validate_name(config_path, "$.domains", name, NameKind::Domain)?;
            if policy.domains.contains_key(name) {
                return Err(TaskError::ConfigDuplicate {
                    config_path: config_path.to_path_buf(),
                    path: "$.domains".to_string(),
                    value: name.to_string(),
                });
            }
            validate_prefixes(config_path, prefixes)?;
            policy.domains.insert(
                name.to_string(),
                DomainPolicy {
                    prefixes: prefixes.to_vec(),
                },
            );
            Ok(())
        })
    }

    /// Remove a configured domain if no existing task depends on it.
    pub fn remove_domain(&self, name: impl AsRef<str>) -> Result<Self, TaskError> {
        self.mutate_policy(|config_path, policy| {
            let name = name.as_ref();
            if policy.domains.remove(name).is_none() {
                return Err(TaskError::ConfigInvalidValue {
                    config_path: config_path.to_path_buf(),
                    path: "$.domains".to_string(),
                    value: name.to_string(),
                    constraint: "domain must exist before it can be removed".to_string(),
                });
            }
            if policy.domains.is_empty() {
                return Err(TaskError::ConfigInvalidValue {
                    config_path: config_path.to_path_buf(),
                    path: "$.domains".to_string(),
                    value: "{}".to_string(),
                    constraint: "must define at least one domain".to_string(),
                });
            }
            Ok(())
        })
    }

    /// Add one prefix to an existing domain.
    pub fn add_prefix(
        &self,
        domain: impl AsRef<str>,
        prefix: impl AsRef<str>,
    ) -> Result<Self, TaskError> {
        self.mutate_policy(|config_path, policy| {
            let domain = domain.as_ref();
            let prefix = prefix.as_ref();
            validate_name(config_path, "$.domains.prefixes", prefix, NameKind::Prefix)?;
            let configured = policy.domains.keys().cloned().collect();
            let domain_policy =
                policy
                    .domains
                    .get_mut(domain)
                    .ok_or_else(|| TaskError::UnknownDomain {
                        domain: domain.to_string(),
                        configured,
                        root: self.root().to_path_buf(),
                    })?;
            if domain_policy.allows_prefix(prefix) {
                return Err(TaskError::ConfigDuplicate {
                    config_path: config_path.to_path_buf(),
                    path: format!("$.domains.{domain}.prefixes"),
                    value: prefix.to_string(),
                });
            }
            domain_policy.prefixes.push(prefix.to_string());
            Ok(())
        })
    }

    /// Remove one prefix from an existing domain if stored tasks still validate.
    pub fn remove_prefix(
        &self,
        domain: impl AsRef<str>,
        prefix: impl AsRef<str>,
    ) -> Result<Self, TaskError> {
        self.mutate_policy(|config_path, policy| {
            let domain = domain.as_ref();
            let prefix = prefix.as_ref();
            let configured = policy.domains.keys().cloned().collect();
            let domain_policy =
                policy
                    .domains
                    .get_mut(domain)
                    .ok_or_else(|| TaskError::UnknownDomain {
                        domain: domain.to_string(),
                        configured,
                        root: self.root().to_path_buf(),
                    })?;
            let before = domain_policy.prefixes.len();
            domain_policy.prefixes.retain(|value| value != prefix);
            if domain_policy.prefixes.len() == before {
                return Err(TaskError::ConfigInvalidValue {
                    config_path: config_path.to_path_buf(),
                    path: format!("$.domains.{domain}.prefixes"),
                    value: prefix.to_string(),
                    constraint: "prefix must exist before it can be removed".to_string(),
                });
            }
            if domain_policy.prefixes.is_empty() {
                return Err(TaskError::ConfigInvalidValue {
                    config_path: config_path.to_path_buf(),
                    path: format!("$.domains.{domain}.prefixes"),
                    value: "[]".to_string(),
                    constraint: "must not be empty".to_string(),
                });
            }
            Ok(())
        })
    }

    /// Add a configured task type.
    pub fn add_task_type(
        &self,
        name: impl AsRef<str>,
        criteria: CriteriaPolicy,
        role: Option<TaskTypeRole>,
    ) -> Result<Self, TaskError> {
        self.mutate_policy(|config_path, policy| {
            let name = name.as_ref();
            validate_name(config_path, "$.task_types", name, NameKind::TaskType)?;
            if policy.task_types.contains_key(name) {
                return Err(TaskError::ConfigDuplicate {
                    config_path: config_path.to_path_buf(),
                    path: "$.task_types".to_string(),
                    value: name.to_string(),
                });
            }
            if role == Some(TaskTypeRole::Milestone)
                && policy
                    .task_types
                    .values()
                    .any(|policy| policy.role == Some(TaskTypeRole::Milestone))
            {
                return Err(TaskError::ConfigInvalidValue {
                    config_path: config_path.to_path_buf(),
                    path: "$.task_types".to_string(),
                    value: name.to_string(),
                    constraint: "at most one task type may use the milestone role".to_string(),
                });
            }
            policy
                .task_types
                .insert(name.to_string(), TaskTypePolicy { criteria, role });
            Ok(())
        })
    }

    /// Remove a configured task type if no existing task uses it.
    pub fn remove_task_type(&self, name: impl AsRef<str>) -> Result<Self, TaskError> {
        self.mutate_policy(|config_path, policy| {
            let name = name.as_ref();
            if policy.task_types.remove(name).is_none() {
                return Err(TaskError::ConfigInvalidValue {
                    config_path: config_path.to_path_buf(),
                    path: "$.task_types".to_string(),
                    value: name.to_string(),
                    constraint: "task type must exist before it can be removed".to_string(),
                });
            }
            if policy.task_types.is_empty() {
                return Err(TaskError::ConfigInvalidValue {
                    config_path: config_path.to_path_buf(),
                    path: "$.task_types".to_string(),
                    value: "{}".to_string(),
                    constraint: "must define at least one task type".to_string(),
                });
            }
            Ok(())
        })
    }

    /// Add one strict tag. Open tag policy becomes a strict one containing the tag.
    pub fn add_tag(&self, tag: impl AsRef<str>) -> Result<Self, TaskError> {
        self.mutate_policy(|config_path, policy| {
            let tag = tag.as_ref();
            validate_name(config_path, "$.tags.allowed", tag, NameKind::Tag)?;
            match &mut policy.tags {
                TagPolicy::Open => {
                    policy.tags = TagPolicy::Strict {
                        allowed: vec![tag.to_string()],
                    };
                }
                TagPolicy::Strict { allowed } => {
                    if allowed.iter().any(|value| value == tag) {
                        return Err(TaskError::ConfigDuplicate {
                            config_path: config_path.to_path_buf(),
                            path: "$.tags.allowed".to_string(),
                            value: tag.to_string(),
                        });
                    }
                    allowed.push(tag.to_string());
                }
            }
            Ok(())
        })
    }

    /// Remove one strict tag if no existing task uses it.
    pub fn remove_tag(&self, tag: impl AsRef<str>) -> Result<Self, TaskError> {
        self.mutate_policy(|config_path, policy| {
            let tag = tag.as_ref();
            let TagPolicy::Strict { allowed } = &mut policy.tags else {
                return Err(TaskError::ConfigInvalidValue {
                    config_path: config_path.to_path_buf(),
                    path: "$.tags.policy".to_string(),
                    value: "open".to_string(),
                    constraint: "open tag policy has no catalog entries to remove".to_string(),
                });
            };
            let before = allowed.len();
            allowed.retain(|value| value != tag);
            if allowed.len() == before {
                return Err(TaskError::ConfigInvalidValue {
                    config_path: config_path.to_path_buf(),
                    path: "$.tags.allowed".to_string(),
                    value: tag.to_string(),
                    constraint: "tag must exist before it can be removed".to_string(),
                });
            }
            Ok(())
        })
    }

    fn mutate_policy(
        &self,
        mutate: impl FnOnce(&Path, &mut ProjectPolicy) -> Result<(), TaskError>,
    ) -> Result<Self, TaskError> {
        self.with_policy_write_lock(|| {
            let config_path = self.root().join(super::CONFIG_PATH);
            let mut candidate = editable_policy(self.policy());
            mutate(&config_path, &mut candidate)?;
            require_valid_report(crate::validate::validate_policy_candidate(
                self, &candidate,
            )?)?;
            let rendered = candidate.to_config_json()?;
            crate::atomic_write::replace(&config_path, &rendered).map_err(|source| {
                TaskError::ConfigIo {
                    path: config_path.clone(),
                    operation: "write",
                    source,
                }
            })?;
            TaskProject::open(self.root())
        })
    }
}

fn editable_policy(policy: &ProjectPolicy) -> ProjectPolicy {
    let mut policy = if policy.is_compatibility() {
        let mut compatibility = ProjectPolicy::filer_compatibility();
        compatibility.compatibility = false;
        compatibility
    } else {
        policy.clone()
    };
    sort_policy(&mut policy);
    policy
}

fn sort_policy(policy: &mut ProjectPolicy) {
    policy.domains = BTreeMap::from_iter(policy.domains.clone());
    policy.task_types = BTreeMap::from_iter(policy.task_types.clone());
    policy.exclusive_tag_groups = BTreeMap::from_iter(policy.exclusive_tag_groups.clone());
    if let TagPolicy::Strict { allowed } = &mut policy.tags {
        allowed.sort();
    }
}

fn validate_prefixes(config_path: &Path, prefixes: &[String]) -> Result<(), TaskError> {
    if prefixes.is_empty() {
        return Err(TaskError::ConfigInvalidValue {
            config_path: config_path.to_path_buf(),
            path: "$.domains.prefixes".to_string(),
            value: "[]".to_string(),
            constraint: "must not be empty".to_string(),
        });
    }
    let mut seen = std::collections::HashSet::new();
    for prefix in prefixes {
        validate_name(config_path, "$.domains.prefixes", prefix, NameKind::Prefix)?;
        if !seen.insert(prefix) {
            return Err(TaskError::ConfigDuplicate {
                config_path: config_path.to_path_buf(),
                path: "$.domains.prefixes".to_string(),
                value: prefix.clone(),
            });
        }
    }
    Ok(())
}
