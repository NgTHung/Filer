//! # Filer Task
//!
//! `filer-task` reads task files from `.tasks/` and validates the task metadata
//! that drives Filer development work. The parser stays separate from the CLI so
//! command behavior can be tested without depending on terminal output.
//!
//! ```
//! use filer_task::repo::find_repo_root;
//!
//! let root = find_repo_root(std::env::current_dir()?)?;
//! assert!(root.join(".tasks").exists());
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
pub mod repo;
pub mod validate;
