//! # Filer Task Web
//!
//! A localhost web interface over the `taskroot` library. It keeps a human in
//! the loop: browse and filter tasks, read a task's detail, and run the same
//! state transitions the CLI exposes, all from a browser. Reads and writes go
//! through `taskroot`, so validation and file layout stay authoritative here.

pub mod app;
pub mod device_label;
mod dto;
pub mod error;
pub mod identity;
pub mod project_name;
pub mod registry;
mod routes;
pub mod storage;
