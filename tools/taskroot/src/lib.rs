//! # Taskroot
//!
//! `taskroot` opens an explicit project policy, reads task files from
//! `.tasks/`, and validates the metadata that drives project work.
//! Project discovery stays separate so library code never depends on process
//! state.
//!
//! ```
//! use taskroot::{project::TaskProject, repo::discover_project_root};
//!
//! let root = discover_project_root(std::env::current_dir()?)?;
//! let project = TaskProject::open(root)?;
//! assert!(project.policy().domain("core").is_some());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod agent_context;
mod atomic_write;
pub mod cli;
mod domain;
pub mod error;
pub mod frontmatter;
pub mod graph;
pub mod identity;
pub mod lifecycle;
pub mod markdown;
pub mod milestone;
pub mod model;
mod output;
pub mod project;
pub mod reference;
mod reference_validation;
pub mod repo;
pub mod taxonomy;
pub mod validate;
