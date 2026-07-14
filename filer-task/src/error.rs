use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum TaskError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    ProjectNotFound {
        start: PathBuf,
    },
    StaleProject {
        root: PathBuf,
    },
    ConfigIo {
        path: PathBuf,
        operation: &'static str,
        source: std::io::Error,
    },
    ConfigInvalidJson {
        path: PathBuf,
        line: usize,
        column: usize,
        message: String,
    },
    ConfigUnsupportedVersion {
        path: PathBuf,
        received: u64,
        supported: u64,
    },
    ConfigDuplicate {
        config_path: PathBuf,
        path: String,
        value: String,
    },
    ConfigUnknownField {
        config_path: PathBuf,
        path: String,
        field: String,
    },
    ConfigInvalidValue {
        config_path: PathBuf,
        path: String,
        value: String,
        constraint: String,
    },
    DomainRequired {
        id: String,
        candidates: Vec<String>,
        root: PathBuf,
    },
    DomainConflict {
        identity_domain: String,
        flag_domain: String,
        root: PathBuf,
    },
    InvalidReference {
        reference: String,
        constraint: String,
        root: PathBuf,
    },
    UnknownDomain {
        domain: String,
        configured: Vec<String>,
        root: PathBuf,
    },
    UnknownType(Box<TaxonomyErrorContext>),
    TagRejected(Box<TaxonomyErrorContext>),
    PrefixNotAllowed(Box<TaxonomyErrorContext>),
    Validation(Vec<ValidationError>),
    Json(serde_json::Error),
    TaskNotFound {
        reference: String,
        source_domain: Option<String>,
        root: PathBuf,
    },
    AmbiguousReference {
        reference: String,
        source_domain: Option<String>,
        candidates: Vec<String>,
        root: PathBuf,
    },
    Message(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaxonomyErrorContext {
    pub rejected_value: String,
    pub field: String,
    pub domain: Option<String>,
    pub policy: Option<String>,
    pub allowed: Vec<String>,
    pub project_root: PathBuf,
    pub task: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub path: Option<PathBuf>,
    pub message: String,
    pub code: String,
    pub context: serde_json::Value,
}

impl ValidationError {
    pub fn new(path: impl Into<Option<PathBuf>>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
            code: "validation_error".to_string(),
            context: serde_json::json!({}),
        }
    }

    pub fn at(path: impl AsRef<Path>, message: impl Into<String>) -> Self {
        Self::new(Some(path.as_ref().to_path_buf()), message)
    }

    pub fn from_task_error(path: impl AsRef<Path>, error: &TaskError) -> Self {
        Self {
            path: Some(path.as_ref().to_path_buf()),
            message: error.to_string(),
            code: error.code().to_string(),
            context: error.context(),
        }
    }
}

impl fmt::Display for TaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(f, "{}: {source}", path.display())
            }
            Self::ProjectNotFound { start } => write!(
                f,
                "could not find a filer-task project from {}; expected a .tasks directory at that path or an ancestor",
                start.display()
            ),
            Self::StaleProject { root } => write!(
                f,
                "task project {} changed on disk; reload it before retrying the mutation",
                root.display()
            ),
            Self::ConfigIo {
                path,
                operation,
                source,
            } => write!(
                f,
                "could not {operation} task configuration {}: {source}",
                path.display()
            ),
            Self::ConfigInvalidJson {
                path,
                line,
                column,
                message,
            } => write!(
                f,
                "invalid task configuration JSON in {} at line {line}, column {column}: {message}",
                path.display()
            ),
            Self::ConfigUnsupportedVersion {
                path,
                received,
                supported,
            } => write!(
                f,
                "unsupported task configuration version {received} in {}; supported version is {supported}",
                path.display()
            ),
            Self::ConfigDuplicate {
                config_path,
                path,
                value,
            } => write!(
                f,
                "duplicate value {value:?} at {path} in {}",
                config_path.display()
            ),
            Self::ConfigUnknownField {
                config_path,
                path,
                field,
            } => write!(
                f,
                "unknown field {field:?} at {path} in {}",
                config_path.display()
            ),
            Self::ConfigInvalidValue {
                config_path,
                path,
                value,
                constraint,
            } => write!(
                f,
                "invalid value {value:?} at {path} in {}: {constraint}",
                config_path.display()
            ),
            Self::DomainRequired {
                id,
                candidates,
                root,
            } => {
                write!(
                    f,
                    "domain is required for task id {id:?} in {}; use domain:{id}",
                    root.display()
                )?;
                if !candidates.is_empty() {
                    write!(f, "; matching tasks: {}", candidates.join(", "))?;
                }
                Ok(())
            }
            Self::DomainConflict {
                identity_domain,
                flag_domain,
                root,
            } => write!(
                f,
                "task identity domain {identity_domain:?} conflicts with --domain {flag_domain:?} in {}",
                root.display()
            ),
            Self::InvalidReference {
                reference,
                constraint,
                root,
            } => write!(
                f,
                "invalid task reference {reference:?} in {}: {constraint}",
                root.display()
            ),
            Self::UnknownDomain {
                domain,
                configured,
                root,
            } => write!(
                f,
                "unknown task domain {domain:?} in {}; configured domains: {}",
                root.display(),
                configured.join(", ")
            ),
            Self::UnknownType(context) => {
                write!(
                    f,
                    "unknown task type {:?} for field {}",
                    context.rejected_value, context.field
                )?;
                if let Some(domain) = &context.domain {
                    write!(f, " in domain {domain}")?;
                }
                write!(
                    f,
                    " in {}; allowed types: {}",
                    context.project_root.display(),
                    context.allowed.join(", ")
                )
            }
            Self::TagRejected(context) => {
                let policy = context.policy.as_deref().unwrap_or("configured");
                write!(
                    f,
                    "tag {:?} is rejected for field {} by the {policy} tag policy in {}",
                    context.rejected_value,
                    context.field,
                    context.project_root.display()
                )?;
                if !context.allowed.is_empty() {
                    write!(f, "; allowed tags: {}", context.allowed.join(", "))?;
                }
                Ok(())
            }
            Self::PrefixNotAllowed(context) => {
                let domain = context.domain.as_deref().unwrap_or("unknown");
                write!(
                    f,
                    "prefix {:?} is not allowed for field {} in domain {domain} in {}; allowed prefixes: {}",
                    context.rejected_value,
                    context.field,
                    context.project_root.display(),
                    context.allowed.join(", ")
                )
            }
            Self::Validation(errors) => {
                writeln!(f, "task validation failed with {} error(s):", errors.len())?;
                for error in errors {
                    match &error.path {
                        Some(path) => writeln!(f, "- {}: {}", path.display(), error.message)?,
                        None => writeln!(f, "- {}", error.message)?,
                    }
                }
                Ok(())
            }
            Self::Json(error) => write!(f, "failed to process JSON: {error}"),
            Self::TaskNotFound {
                reference,
                source_domain,
                root,
            } => {
                write!(f, "task reference {reference:?} does not exist")?;
                if let Some(domain) = source_domain {
                    write!(f, " in domain {domain}")?;
                }
                write!(f, " in {}", root.display())
            }
            Self::AmbiguousReference {
                reference,
                source_domain,
                candidates,
                root,
            } => {
                write!(f, "task reference {reference:?} is ambiguous")?;
                if let Some(domain) = source_domain {
                    write!(f, " from domain {domain}")?;
                }
                write!(
                    f,
                    " in {}; candidates: {}",
                    root.display(),
                    candidates.join(", ")
                )
            }
            Self::Message(message) => write!(f, "{message}"),
        }
    }
}

impl Error for TaskError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } | Self::ConfigIo { source, .. } => Some(source),
            Self::Json(error) => Some(error),
            Self::ProjectNotFound { .. }
            | Self::StaleProject { .. }
            | Self::ConfigInvalidJson { .. }
            | Self::ConfigUnsupportedVersion { .. }
            | Self::ConfigDuplicate { .. }
            | Self::ConfigUnknownField { .. }
            | Self::ConfigInvalidValue { .. }
            | Self::DomainRequired { .. }
            | Self::DomainConflict { .. }
            | Self::InvalidReference { .. }
            | Self::UnknownDomain { .. }
            | Self::UnknownType(_)
            | Self::TagRejected(_)
            | Self::PrefixNotAllowed(_)
            | Self::Validation(_)
            | Self::TaskNotFound { .. }
            | Self::AmbiguousReference { .. }
            | Self::Message(_) => None,
        }
    }
}

impl TaskError {
    /// Return the stable machine-readable category for this error.
    pub fn code(&self) -> &'static str {
        match self {
            Self::ProjectNotFound { .. } => "project_not_found",
            Self::StaleProject { .. } => "project_stale",
            Self::ConfigIo { .. } => "config_io",
            Self::ConfigInvalidJson { .. } => "config_invalid_json",
            Self::ConfigUnsupportedVersion { .. } => "config_unsupported_version",
            Self::ConfigDuplicate { .. } => "config_duplicate",
            Self::ConfigUnknownField { .. } => "config_unknown_field",
            Self::ConfigInvalidValue { .. } => "config_invalid_value",
            Self::DomainRequired { .. } => "domain_required",
            Self::DomainConflict { .. } => "domain_conflict",
            Self::InvalidReference { .. } => "invalid_reference",
            Self::UnknownDomain { .. } => "unknown_domain",
            Self::UnknownType(_) => "unknown_type",
            Self::TagRejected(_) => "tag_rejected",
            Self::PrefixNotAllowed(_) => "prefix_not_allowed",
            Self::Validation(_) => "validation_failed",
            Self::Io { .. } => "io",
            Self::Json(_) => "invalid_json",
            Self::TaskNotFound { .. } => "task_not_found",
            Self::AmbiguousReference { .. } => "ambiguous_reference",
            Self::Message(_) => "invalid_operation",
        }
    }

    /// Return code-specific context without requiring callers to parse the message.
    pub fn context(&self) -> serde_json::Value {
        match self {
            Self::ConfigIo {
                path, operation, ..
            } => serde_json::json!({"path": path, "operation": operation}),
            Self::ConfigInvalidJson {
                path, line, column, ..
            } => serde_json::json!({"path": path, "line": line, "column": column}),
            Self::ConfigUnsupportedVersion {
                path,
                received,
                supported,
            } => serde_json::json!({
                "path": path,
                "received": received,
                "supported": supported
            }),
            Self::ConfigDuplicate {
                config_path,
                path,
                value,
            } => serde_json::json!({
                "config_path": config_path,
                "path": path,
                "value": value
            }),
            Self::ConfigUnknownField {
                config_path,
                path,
                field,
            } => serde_json::json!({
                "config_path": config_path,
                "path": path,
                "field": field
            }),
            Self::ConfigInvalidValue {
                config_path,
                path,
                value,
                constraint,
            } => serde_json::json!({
                "config_path": config_path,
                "path": path,
                "value": value,
                "constraint": constraint
            }),
            Self::DomainRequired {
                id,
                candidates,
                root,
            } => {
                serde_json::json!({
                    "id": id,
                    "reference": id,
                    "candidates": candidates,
                    "root": root
                })
            }
            Self::DomainConflict {
                identity_domain,
                flag_domain,
                root,
            } => serde_json::json!({
                "identity_domain": identity_domain,
                "flag_domain": flag_domain,
                "root": root
            }),
            Self::InvalidReference {
                reference,
                constraint,
                root,
            } => serde_json::json!({
                "reference": reference,
                "constraint": constraint,
                "root": root
            }),
            Self::UnknownDomain {
                domain,
                configured,
                root,
            } => serde_json::json!({
                "domain": domain,
                "configured": configured,
                "root": root
            }),
            Self::UnknownType(context)
            | Self::TagRejected(context)
            | Self::PrefixNotAllowed(context) => serde_json::json!({
                "rejected_value": context.rejected_value,
                "field": context.field,
                "domain": context.domain,
                "policy": context.policy,
                "allowed": context.allowed,
                "project_root": context.project_root,
                "task": context.task
            }),
            Self::ProjectNotFound { start } => serde_json::json!({"start": start}),
            Self::StaleProject { root } => serde_json::json!({"root": root}),
            Self::Io { path, .. } => serde_json::json!({"path": path}),
            Self::TaskNotFound {
                reference,
                source_domain,
                root,
            } => serde_json::json!({
                "reference": reference,
                "source_domain": source_domain,
                "root": root
            }),
            Self::AmbiguousReference {
                reference,
                source_domain,
                candidates,
                root,
            } => serde_json::json!({
                "reference": reference,
                "source_domain": source_domain,
                "candidates": candidates,
                "root": root
            }),
            Self::Validation(issues) => serde_json::json!({
                "issues": issues.iter().map(|issue| serde_json::json!({
                    "code": issue.code,
                    "path": issue.path,
                    "message": issue.message,
                    "context": issue.context
                })).collect::<Vec<_>>()
            }),
            Self::Json(_) | Self::Message(_) => serde_json::json!({}),
        }
    }
}

impl From<serde_json::Error> for TaskError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}
