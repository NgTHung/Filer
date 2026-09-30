use std::{
    io::Error,
    path::{self, Path, PathBuf},
};

/// Get file extension as lowercase string
pub fn get_extension(path: &Path) -> Option<&str> {
    path.extension().and_then(|s| s.to_str())
}

/// Get file stem (name without extension)
pub fn get_stem(path: &Path) -> Option<&str> {
    path.file_stem().and_then(|s| s.to_str())
}

/// Check if path is hidden (starts with dot on Unix)
pub fn is_hidden(path: &Path) -> bool {
    if let Some(filename) = path.file_name() {
        if filename.to_str().is_some_and(|name| name.starts_with('.')) {
            return true;
        }

        if let Some(par) = path.parent() {
            return is_hidden(par);
        }
    }
    false
}

/// Normalize path separators
pub fn normalize(path: &Path) -> Result<PathBuf, Error> {
    path::absolute(path)
}

/// Get parent directory name
pub fn parent_name(path: &Path) -> Option<&str> {
    path.parent().and_then(|s| s.to_str())
}
