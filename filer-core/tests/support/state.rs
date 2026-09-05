//! # Shared test logs
//!
//! Cloned provider fixtures use these logs to retain calls and directory rows
//! across actor tasks.
//!
//! ```
//! let entries = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
//! assert!(entries.lock().unwrap().is_empty());
//! ```

pub(crate) type SharedLog<T> = std::sync::Arc<std::sync::Mutex<Vec<T>>>;
