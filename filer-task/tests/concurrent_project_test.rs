use std::{
    fs::{self, OpenOptions},
    process::Command,
    sync::{Arc, Barrier, mpsc},
    thread,
    time::Duration,
};

use filer_task::{
    error::TaskError,
    identity::TaskIdentity,
    lifecycle::{block_task, start_task},
    project::TaskProject,
};

fn project_with_task(id: &str) -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("temporary project created");
    let domain = temp.path().join(".tasks/core");
    fs::create_dir_all(&domain).expect("task domain created");
    fs::write(
        domain.join(format!("{id}-task.md")),
        format!(
            "---\nid: {id}\ntitle: Concurrent task\nstatus: To Do\npriority: Medium\ntype: Feature\nlast_updated: 2026-07-14\n---\n\n## Summary\n\nExercise project coordination.\n\n## Acceptance Criteria\n\n- [ ] Works\n"
        ),
    )
    .expect("task written");
    temp
}

fn identity(id: &str) -> TaskIdentity {
    TaskIdentity::new("core", id).expect("valid identity")
}

#[test]
fn detects_external_content_changes_and_recovers_by_reloading() {
    let temp = project_with_task("UTILS-901");
    let task_path = temp.path().join(".tasks/core/UTILS-901-task.md");
    let project = TaskProject::open(temp.path()).expect("project opens");
    let original = fs::read_to_string(&task_path).expect("task read");
    let changed = original.replace("Works", "Fails");
    assert_eq!(original.len(), changed.len());

    fs::write(&task_path, changed).expect("external change written");

    assert!(project.is_stale().expect("staleness checked"));
    let error = start_task(&project, &identity("UTILS-901")).unwrap_err();
    assert!(matches!(error, TaskError::StaleProject { .. }));
    assert_eq!(error.code(), "project_stale");
    assert_eq!(
        error.context()["root"],
        serde_json::json!(temp.path().canonicalize().unwrap())
    );

    let reloaded = project.reload().expect("project reloads");
    assert!(!reloaded.is_stale().expect("reloaded project checked"));
    start_task(&reloaded, &identity("UTILS-901")).expect("fresh mutation succeeds");
}

#[test]
fn detects_external_task_and_configuration_set_changes() {
    let temp = project_with_task("UTILS-902");
    let existing = temp.path().join(".tasks/core/UTILS-902-task.md");
    let existing_content = fs::read(&existing).expect("existing task read");
    let project = TaskProject::open(temp.path()).expect("project opens");
    let added = temp.path().join(".tasks/core/UTILS-903-added.md");
    fs::write(&added, "external").expect("external task added");
    assert!(project.is_stale().expect("addition detected"));

    fs::remove_file(&added).expect("external task removed");
    assert!(!project.is_stale().expect("original state restored"));

    fs::remove_file(&existing).expect("existing task removed");
    assert!(project.is_stale().expect("deletion detected"));
    fs::write(&existing, existing_content).expect("existing task restored");
    assert!(!project.is_stale().expect("task restoration detected"));

    fs::write(temp.path().join(".tasks/config.json"), "{}").expect("config added");
    assert!(project.is_stale().expect("configuration detected"));
}

#[test]
fn detects_configuration_modification_and_removal() {
    let temp = project_with_task("UTILS-908");
    let config_path = temp.path().join(".tasks/config.json");
    let config = r#"{
        "version": 1,
        "domains": {"core": {"prefixes": ["UTILS"]}},
        "task_types": {"Feature": {"criteria": "acceptance"}},
        "tags": {"policy": "open"}
    }"#;
    fs::write(&config_path, config).expect("configuration written");
    let project = TaskProject::open(temp.path()).expect("configured project opens");

    fs::write(&config_path, config.replace("UTILS", "COREX")).expect("configuration modified");
    assert!(project.is_stale().expect("configuration change detected"));

    fs::remove_file(config_path).expect("configuration removed");
    assert!(project.is_stale().expect("configuration removal detected"));
}

#[test]
fn successful_mutation_refreshes_every_clone() {
    let temp = project_with_task("UTILS-904");
    let project = TaskProject::open(temp.path()).expect("project opens");
    let clone = project.clone();

    start_task(&clone, &identity("UTILS-904")).expect("mutation succeeds");

    assert!(!project.is_stale().expect("original handle refreshed"));
    assert!(!clone.is_stale().expect("clone refreshed"));
}

#[test]
fn independent_handles_for_one_root_serialize_and_detect_stale_state() {
    let temp = project_with_task("UTILS-905");
    let first = TaskProject::open(temp.path()).expect("first handle opens");
    let second = TaskProject::open(temp.path()).expect("second handle opens");
    let barrier = Arc::new(Barrier::new(3));

    let first_barrier = Arc::clone(&barrier);
    let first_thread = thread::spawn(move || {
        first_barrier.wait();
        start_task(&first, &identity("UTILS-905"))
    });
    let second_barrier = Arc::clone(&barrier);
    let second_thread = thread::spawn(move || {
        second_barrier.wait();
        block_task(&second, &identity("UTILS-905"), "Concurrent request")
    });
    barrier.wait();

    let results = [
        first_thread.join().expect("first thread joined"),
        second_thread.join().expect("second thread joined"),
    ];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(TaskError::StaleProject { .. })))
            .count(),
        1
    );
}

#[test]
fn a_locked_project_does_not_block_another_project() {
    let first = project_with_task("UTILS-906");
    let second = project_with_task("UTILS-907");
    let lock_path = first.path().join(".tasks/.filer-task.lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .expect("lock file opened");
    lock.lock().expect("first project locked");
    let second_project = TaskProject::open(second.path()).expect("second project opens");
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        sender
            .send(start_task(&second_project, &identity("UTILS-907")))
            .expect("result sent");
    });

    receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("second project was not blocked")
        .expect("second project mutation succeeds");
}

#[test]
fn cli_process_waits_for_the_project_filesystem_lock() {
    let temp = project_with_task("UTILS-909");
    let lock_path = temp.path().join(".tasks/.filer-task.lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .expect("lock file opened");
    lock.lock().expect("project locked");
    let mut child = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args(["start", "core:UTILS-909", "--root"])
        .arg(temp.path())
        .spawn()
        .expect("CLI process started");

    thread::sleep(Duration::from_millis(100));
    assert!(child.try_wait().expect("process state read").is_none());
    lock.unlock().expect("project unlocked");

    assert!(child.wait().expect("CLI process joined").success());
}
