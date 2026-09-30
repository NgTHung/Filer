use std::{fs, sync::Arc, thread};

#[path = "../src/atomic_write.rs"]
mod atomic_write;

#[test]
fn creates_a_complete_new_file() {
    let temp = tempfile::tempdir().expect("temporary directory created");
    let target = temp.path().join("target.md");

    atomic_write::create(&target, "complete content").expect("new file persisted");

    assert_eq!(fs::read_to_string(target).unwrap(), "complete content");
}

#[test]
fn failed_persistence_removes_the_temporary_file() {
    let temp = tempfile::tempdir().expect("temporary directory created");
    let target = temp.path().join("target.md");
    fs::create_dir(&target).expect("conflicting target directory created");
    let before = entries(temp.path());

    let error = atomic_write::replace(&target, "complete content").unwrap_err();

    assert_ne!(error.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(entries(temp.path()), before);
    assert!(target.is_dir());
}

#[test]
fn readers_observe_only_complete_replacements() {
    let temp = tempfile::tempdir().expect("temporary directory created");
    let target = temp.path().join("target.md");
    let old = "a".repeat(512 * 1024);
    let new = "b".repeat(512 * 1024);
    fs::write(&target, &old).expect("initial file written");
    let target = Arc::new(target);
    let writer_target = Arc::clone(&target);
    let writer = thread::spawn(move || atomic_write::replace(&writer_target, &new));

    while !writer.is_finished() {
        let observed = fs::read_to_string(target.as_ref()).expect("target remains readable");
        assert!(observed == old || observed.bytes().all(|byte| byte == b'b'));
    }
    writer
        .join()
        .expect("writer thread joined")
        .expect("replacement succeeds");
    assert!(
        fs::read(target.as_ref())
            .expect("replacement read")
            .iter()
            .all(|byte| *byte == b'b')
    );
}

fn entries(path: &std::path::Path) -> Vec<std::ffi::OsString> {
    let mut entries = fs::read_dir(path)
        .expect("directory read")
        .map(|entry| entry.expect("entry read").file_name())
        .collect::<Vec<_>>();
    entries.sort();
    entries
}
