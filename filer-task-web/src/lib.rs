//! # Filer Task Web
//!
//! A localhost web interface over the `filer-task` library. It keeps a human in
//! the loop: browse and filter tasks, read a task's detail, and run the same
//! state transitions the CLI exposes, all from a browser. Reads and writes go
//! through `filer-task`, so validation and file layout stay authoritative here.

pub mod app;
mod dto;
pub mod error;
pub mod registry;
mod routes;
