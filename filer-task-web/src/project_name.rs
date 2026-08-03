//! # New Project Names
//!
//! A registered project is named after its root directory, so naming a project
//! at creation time means choosing a directory name. This module keeps that one
//! rule: the name has to be a single ordinary directory name, never a path that
//! reaches somewhere else under the directory the caller chose.
//!
//! ```
//! use filer_task_web::project_name::validated;
//!
//! assert_eq!(validated("new-thing").unwrap(), "new-thing");
//! assert!(validated("../elsewhere").is_err());
//! ```

use std::path::{Component, Path};

use crate::error::WebError;

pub fn validated(name: &str) -> Result<&str, WebError> {
    let trimmed = name.trim();
    let mut components = Path::new(trimmed).components();
    let single_name = matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(_)), None)
    );
    // The separator check is not redundant: a backslash is an ordinary
    // character in a Unix path component, so only Windows rejects it as a
    // second component, and the two platforms must refuse the same names.
    if !single_name || trimmed.contains(['/', '\\']) {
        return Err(WebError::InvalidNewProjectName(name.to_string()));
    }
    Ok(trimmed)
}
