//! # Task Reference Resolution
//!
//! This module indexes exact task identities and applies the resolution policy
//! for CLI selectors, stored relationships, and task creation.
//!
//! ```
//! use filer_task::{identity::TaskIdentity, reference::IdentityIndex};
//!
//! let index = IdentityIndex::new(
//!     "/project",
//!     [TaskIdentity::new("work", "WORK-001")?],
//! );
//! assert_eq!(index.candidates("WORK-001")[0].to_string(), "work:WORK-001");
//! # Ok::<(), filer_task::identity::IdentityError>(())
//! ```

use std::{collections::BTreeMap, path::Path};

use crate::{
    error::TaskError,
    identity::{IdentityError, TaskIdentity, TaskReference},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedReference {
    pub identity: TaskIdentity,
    pub used_legacy_fallback: bool,
}

#[derive(Debug, Clone)]
pub struct IdentityIndex {
    root: std::path::PathBuf,
    by_identity: BTreeMap<TaskIdentity, ()>,
    by_local_id: BTreeMap<String, Vec<TaskIdentity>>,
}

impl IdentityIndex {
    pub fn new(root: impl AsRef<Path>, identities: impl IntoIterator<Item = TaskIdentity>) -> Self {
        let mut by_identity = BTreeMap::new();
        let mut by_local_id: BTreeMap<String, Vec<TaskIdentity>> = BTreeMap::new();
        for identity in identities {
            by_local_id
                .entry(identity.id.clone())
                .or_default()
                .push(identity.clone());
            by_identity.insert(identity, ());
        }
        for candidates in by_local_id.values_mut() {
            candidates.sort();
            candidates.dedup();
        }
        Self {
            root: root.as_ref().to_path_buf(),
            by_identity,
            by_local_id,
        }
    }

    pub fn candidates(&self, local_id: &str) -> &[TaskIdentity] {
        self.by_local_id
            .get(local_id)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn contains(&self, identity: &TaskIdentity) -> bool {
        self.by_identity.contains_key(identity)
    }

    pub fn resolve_cli(&self, value: &str) -> Result<TaskIdentity, TaskError> {
        match self.parse(value)? {
            TaskReference::Local(local_id) => Err(TaskError::DomainRequired {
                candidates: self.candidate_strings(&local_id),
                id: local_id,
                root: self.root.clone(),
            }),
            TaskReference::Qualified(identity) if self.contains(&identity) => Ok(identity),
            TaskReference::Qualified(identity) => Err(self.not_found(identity.to_string(), None)),
        }
    }

    /// Resolve a selector against every identity in the project.
    pub fn resolve_project_wide(&self, value: &str) -> Result<TaskIdentity, TaskError> {
        match self.parse(value)? {
            TaskReference::Qualified(identity) if self.contains(&identity) => Ok(identity),
            TaskReference::Qualified(identity) => Err(self.not_found(identity.to_string(), None)),
            TaskReference::Local(local_id) => match self.candidates(&local_id) {
                [identity] => Ok(identity.clone()),
                [] => Err(self.not_found(local_id, None)),
                candidates => Err(TaskError::AmbiguousReference {
                    reference: local_id,
                    source_domain: None,
                    candidates: candidates.iter().map(ToString::to_string).collect(),
                    root: self.root.clone(),
                }),
            },
        }
    }

    pub fn resolve_creation(
        &self,
        source_domain: &str,
        reference: &TaskReference,
    ) -> Result<TaskIdentity, TaskError> {
        self.resolve_strict(source_domain, reference)
    }

    pub fn resolve_configured(
        &self,
        source_domain: &str,
        reference: &TaskReference,
        compatibility: bool,
    ) -> Result<ResolvedReference, TaskError> {
        match self.resolve_strict(source_domain, reference) {
            Ok(identity) => Ok(ResolvedReference {
                identity,
                used_legacy_fallback: false,
            }),
            Err(TaskError::TaskNotFound { .. })
                if compatibility && matches!(reference, TaskReference::Local(_)) =>
            {
                let candidates = self.candidates(reference.local_id());
                match candidates {
                    [identity] => Ok(ResolvedReference {
                        identity: identity.clone(),
                        used_legacy_fallback: true,
                    }),
                    [] => {
                        Err(self.not_found(reference.to_string(), Some(source_domain.to_string())))
                    }
                    _ => Err(TaskError::AmbiguousReference {
                        reference: reference.to_string(),
                        source_domain: Some(source_domain.to_string()),
                        candidates: candidates.iter().map(ToString::to_string).collect(),
                        root: self.root.clone(),
                    }),
                }
            }
            Err(error) => Err(error),
        }
    }

    fn resolve_strict(
        &self,
        source_domain: &str,
        reference: &TaskReference,
    ) -> Result<TaskIdentity, TaskError> {
        let identity = match reference {
            TaskReference::Local(local_id) => TaskIdentity {
                domain: source_domain.to_string(),
                id: local_id.clone(),
            },
            TaskReference::Qualified(identity) => identity.clone(),
        };
        if self.contains(&identity) {
            Ok(identity)
        } else {
            Err(self.not_found(
                reference.to_string(),
                matches!(reference, TaskReference::Local(_)).then(|| source_domain.to_string()),
            ))
        }
    }

    fn parse(&self, value: &str) -> Result<TaskReference, TaskError> {
        TaskReference::parse(value).map_err(|error| self.invalid(value, &error))
    }

    fn invalid(&self, value: &str, error: &IdentityError) -> TaskError {
        TaskError::InvalidReference {
            reference: value.to_string(),
            constraint: error.constraint().to_string(),
            root: self.root.clone(),
        }
    }

    fn not_found(&self, reference: String, source_domain: Option<String>) -> TaskError {
        TaskError::TaskNotFound {
            reference,
            source_domain,
            root: self.root.clone(),
        }
    }

    fn candidate_strings(&self, local_id: &str) -> Vec<String> {
        self.candidates(local_id)
            .iter()
            .map(ToString::to_string)
            .collect()
    }
}
