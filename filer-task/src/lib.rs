//! # Filer Task
//!
//! `filer-task` opens an explicit project policy, reads task files from
//! `.tasks/`, and validates the metadata that drives Filer development work.
//! Project discovery stays separate so library code never depends on process
//! state.
//!
//! ```
//! use filer_task::{project::TaskProject, repo::discover_project_root};
//!
//! let root = discover_project_root(std::env::current_dir()?)?;
//! let project = TaskProject::open(root)?;
//! assert!(project.policy().domain("core").is_some());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod agent_context;
pub mod cli;
pub mod error;
pub mod frontmatter;
pub mod lifecycle;
pub mod markdown;
pub mod model;
mod output;
pub mod project;
pub mod repo;
pub mod validate;
