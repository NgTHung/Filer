//! # Task Graph
//!
//! This module resolves stored relationships once and exposes exact graph
//! edges keyed by domain-qualified task identity. Exact keys prevent a local
//! ID in one domain from changing another domain's hierarchy or readiness.
//!
//! ```no_run
//! use filer_task::{graph::TaskGraph, model::Task, project::TaskProject};
//!
//! # fn build<'a>(project: &TaskProject, tasks: &'a [Task])
//! #     -> Result<TaskGraph<'a>, filer_task::error::TaskError> {
//! let graph = TaskGraph::new(project, tasks)?;
//! # Ok(graph)
//! # }
//! ```

use std::collections::{HashMap, HashSet};

use crate::{
    error::TaskError,
    identity::{TaskIdentity, TaskReference},
    model::Task,
    project::TaskProject,
    reference::IdentityIndex,
};

pub struct TaskGraph<'a> {
    tasks: HashMap<TaskIdentity, &'a Task>,
    parents: HashMap<TaskIdentity, TaskIdentity>,
    children: HashMap<TaskIdentity, Vec<TaskIdentity>>,
    dependencies: HashMap<TaskIdentity, Vec<TaskIdentity>>,
    dependents: HashMap<TaskIdentity, Vec<TaskIdentity>>,
}

impl<'a> TaskGraph<'a> {
    pub fn new(project: &TaskProject, tasks: &'a [Task]) -> Result<Self, TaskError> {
        let index = IdentityIndex::new(project.root(), tasks.iter().map(Task::identity));
        let task_map = tasks
            .iter()
            .map(|task| (task.identity(), task))
            .collect::<HashMap<_, _>>();
        let mut graph = Self {
            tasks: task_map,
            parents: HashMap::new(),
            children: HashMap::new(),
            dependencies: HashMap::new(),
            dependents: HashMap::new(),
        };

        for task in tasks {
            let source = task.identity();
            if let Some(value) = &task.metadata.parent {
                let parent = resolve(project, &index, task, value)?;
                graph.parents.insert(source.clone(), parent.clone());
                graph
                    .children
                    .entry(parent)
                    .or_default()
                    .push(source.clone());
            }
            for value in &task.metadata.depends_on {
                let dependency = resolve(project, &index, task, value)?;
                graph
                    .dependencies
                    .entry(source.clone())
                    .or_default()
                    .push(dependency.clone());
                graph
                    .dependents
                    .entry(dependency)
                    .or_default()
                    .push(source.clone());
            }
        }
        for edges in graph
            .children
            .values_mut()
            .chain(graph.dependencies.values_mut())
            .chain(graph.dependents.values_mut())
        {
            edges.sort();
            edges.dedup();
        }
        Ok(graph)
    }

    pub fn task(&self, identity: &TaskIdentity) -> Option<&'a Task> {
        self.tasks.get(identity).copied()
    }

    pub fn parent(&self, identity: &TaskIdentity) -> Option<&'a Task> {
        self.parents
            .get(identity)
            .and_then(|parent| self.task(parent))
    }

    pub fn children(&self, identity: &TaskIdentity) -> Vec<&'a Task> {
        self.related(self.children.get(identity))
    }

    pub fn dependencies(&self, identity: &TaskIdentity) -> Vec<&'a Task> {
        self.related(self.dependencies.get(identity))
    }

    pub fn dependents(&self, identity: &TaskIdentity) -> Vec<&'a Task> {
        self.related(self.dependents.get(identity))
    }

    pub fn parent_identity(&self, identity: &TaskIdentity) -> Option<&TaskIdentity> {
        self.parents.get(identity)
    }

    pub fn dependency_identities(&self, identity: &TaskIdentity) -> &[TaskIdentity] {
        self.dependencies
            .get(identity)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn ancestor_identities(&self, identity: &TaskIdentity) -> AncestorChain {
        let mut ancestors = Vec::new();
        let mut visited = HashSet::new();
        let mut current = self.parents.get(identity);
        while let Some(parent) = current {
            if !visited.insert(parent.clone()) {
                return AncestorChain {
                    ancestors,
                    cycle: Some(parent.clone()),
                };
            }
            ancestors.push(parent.clone());
            current = self.parents.get(parent);
        }
        AncestorChain {
            ancestors,
            cycle: None,
        }
    }

    fn related(&self, identities: Option<&Vec<TaskIdentity>>) -> Vec<&'a Task> {
        identities
            .into_iter()
            .flatten()
            .filter_map(|identity| self.task(identity))
            .collect()
    }
}

pub struct AncestorChain {
    pub ancestors: Vec<TaskIdentity>,
    pub cycle: Option<TaskIdentity>,
}

fn resolve(
    project: &TaskProject,
    index: &IdentityIndex,
    task: &Task,
    value: &str,
) -> Result<TaskIdentity, TaskError> {
    let reference = TaskReference::parse(value).map_err(|error| TaskError::InvalidReference {
        reference: value.to_string(),
        constraint: error.constraint().to_string(),
        root: project.root().to_path_buf(),
    })?;
    index
        .resolve_configured(
            &task.domain,
            &reference,
            project.policy().is_compatibility(),
        )
        .map(|resolved| resolved.identity)
}
