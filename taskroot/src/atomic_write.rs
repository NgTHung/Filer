//! # Atomic Task Writes
//!
//! This module writes complete task content beside its destination before an
//! atomic rename, so concurrent readers see either the old file or the new
//! file.
//!
//! ```
//! use taskroot::{identity::TaskIdentity, lifecycle::start_task, project::TaskProject};
//!
//! # let root = tempfile::tempdir()?;
//! # std::fs::create_dir_all(root.path().join(".tasks/core"))?;
//! # std::fs::write(root.path().join(".tasks/core/UTILS-999-example.md"), "---\nid: UTILS-999\ntitle: Example task\nstatus: To Do\npriority: Medium\ntype: Feature\n---\n\n## Acceptance Criteria\n\n- [ ] Example\n")?;
//! let project = TaskProject::open(root.path())?;
//! start_task(&project, &TaskIdentity::new("core", "UTILS-999")?)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::{
    io::{self, Write},
    path::Path,
    thread,
    time::Duration,
};

use tempfile::NamedTempFile;

pub(crate) fn create(path: &Path, content: &str) -> io::Result<()> {
    persist(path, content, false)
}

pub(crate) fn replace(path: &Path, content: &str) -> io::Result<()> {
    persist(path, content, true)
}

fn persist(path: &Path, content: &str, overwrite: bool) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "task path has no parent directory",
        )
    })?;
    let mut temporary = NamedTempFile::new_in(parent)?;
    temporary.write_all(content.as_bytes())?;
    temporary.as_file_mut().sync_all()?;
    if overwrite {
        persist_replace(temporary, path)
    } else {
        temporary
            .persist_noclobber(path)
            .map(|_| ())
            .map_err(|error| error.error)
    }
}

fn persist_replace(mut temporary: NamedTempFile, path: &Path) -> io::Result<()> {
    let mut attempts = 0;
    loop {
        match temporary.persist(path) {
            Ok(_) => return Ok(()),
            Err(error)
                if error.error.kind() == io::ErrorKind::PermissionDenied && attempts < 49 =>
            {
                // Windows readers may briefly deny replacement even though they release the file immediately.
                temporary = error.file;
                attempts += 1;
                thread::sleep(Duration::from_millis(1));
            }
            Err(error) => return Err(error.error),
        }
    }
}
