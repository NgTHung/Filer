use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum TaskError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    MissingRepoRoot {
        start: PathBuf,
    },
    Validation(Vec<ValidationError>),
    Json(serde_json::Error),
    NotFound {
        id: String,
    },
    Message(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub path: Option<PathBuf>,
    pub message: String,
}

impl ValidationError {
    pub fn new(path: impl Into<Option<PathBuf>>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn at(path: impl AsRef<Path>, message: impl Into<String>) -> Self {
        Self::new(Some(path.as_ref().to_path_buf()), message)
    }
}

impl fmt::Display for TaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(f, "{}: {source}", path.display())
            }
            Self::MissingRepoRoot { start } => write!(
                f,
                "could not find repo root from {}; expected .tasks/task.schema.json",
                start.display()
            ),
            Self::Validation(errors) => {
                writeln!(f, "task validation failed with {} error(s):", errors.len())?;
                for error in errors {
                    match &error.path {
                        Some(path) => writeln!(f, "- {}: {}", path.display(), error.message)?,
                        None => writeln!(f, "- {}", error.message)?,
                    }
                }
                Ok(())
            }
            Self::Json(error) => write!(f, "failed to process JSON: {error}"),
            Self::NotFound { id } => write!(f, "task {id} does not exist"),
            Self::Message(message) => write!(f, "{message}"),
        }
    }
}

impl Error for TaskError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json(error) => Some(error),
            Self::MissingRepoRoot { .. }
            | Self::Validation(_)
            | Self::NotFound { .. }
            | Self::Message(_) => None,
        }
    }
}

impl From<serde_json::Error> for TaskError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}
