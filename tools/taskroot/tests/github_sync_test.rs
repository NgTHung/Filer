use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::OnceLock,
};

use serde_json::{Value, json};
use tempfile::TempDir;

struct Fixture {
    root: TempDir,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join(".tasks/core")).unwrap();
        fs::write(root.path().join(".tasks/core/CORE-001-example.md"),
            "---\nid: CORE-001\ntitle: Example task\nstatus: To Do\npriority: High\ntype: Feature\n---\n\n## Summary\n\nKeep tasks portable.\n\n## Acceptance Criteria\n\n- [ ] Works\n").unwrap();
        Self { root }
    }

    fn response(&self, index: usize, response: Value) {
        fs::write(
            self.root.path().join(format!("response-{index}.json")),
            response.to_string(),
        )
        .unwrap();
    }

    fn run(&self, apply: bool) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_taskroot"));
        let mut paths = vec![fake_gh_directory().clone()];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        command
            .args([
                "github-sync",
                "--project",
                "PVT_test",
                "--repository",
                "owner/repo",
                "--format",
                "json",
                "--root",
            ])
            .arg(self.root.path())
            .env("PATH", std::env::join_paths(paths).unwrap())
            .env("FAKE_GH_DIR", self.root.path());
        if apply {
            command.arg("--apply");
        }
        command.output().unwrap()
    }

    fn request(&self, index: usize) -> Value {
        serde_json::from_slice(
            &fs::read(self.root.path().join(format!("request-{index}.json"))).unwrap(),
        )
        .unwrap()
    }
}

fn fake_gh_directory() -> &'static PathBuf {
    static DIRECTORY: OnceLock<PathBuf> = OnceLock::new();
    DIRECTORY.get_or_init(|| {
        let path = tempfile::tempdir().unwrap().keep();
        let status = Command::new("rustc")
            .arg("--edition=2024")
            .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/fake_gh.rs"))
            .arg("-o")
            .arg(path.join(format!("gh{}", std::env::consts::EXE_SUFFIX)))
            .status()
            .unwrap();
        assert!(status.success());
        path
    })
}

fn page(items: Vec<Value>, cursor: Option<&str>) -> Value {
    json!({"data":{"node":{"items":{"nodes":items,"pageInfo":{
        "hasNextPage":cursor.is_some(),"endCursor":cursor
    }}}}})
}

fn report(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn preview_lists_creations_without_writing_to_github_or_local_tasks() {
    let fixture = Fixture::new();
    fixture.response(0, page(vec![], None));
    let before = fs::read(fixture.root.path().join(".tasks/core/CORE-001-example.md")).unwrap();
    let result = report(fixture.run(false));
    assert_eq!(result["mode"], "preview");
    assert_eq!(result["create"], 1);
    assert_eq!(result["tasks"][0]["qualified_id"], "core:CORE-001");
    assert!(
        !fixture.request(0)["query"]
            .as_str()
            .unwrap()
            .contains("mutation")
    );
    assert_eq!(
        fs::read(fixture.root.path().join(".tasks/core/CORE-001-example.md")).unwrap(),
        before
    );
}

#[test]
fn apply_creates_a_draft_and_a_repeat_run_leaves_it_unchanged() {
    let fixture = Fixture::new();
    fixture.response(0, page(vec![], None));
    fixture.response(
        1,
        json!({"data":{"addProjectV2DraftIssue":{"projectItem":{"id":"ITEM_1"}}}}),
    );
    assert_eq!(report(fixture.run(true))["create"], 1);
    let created = fixture.request(1)["variables"]["input"].clone();
    assert_eq!(created["title"], "[core:CORE-001] Example task");
    assert!(
        created["body"]
            .as_str()
            .unwrap()
            .starts_with("<!-- taskroot:owner/repo:core:CORE-001 -->\n")
    );
    fixture.response(2, page(vec![json!({"id":"ITEM_1","isArchived":false,"content":{
        "__typename":"DraftIssue","id":"DRAFT_1","title":created["title"],"body":created["body"]
    }})], None));
    let repeated = report(fixture.run(true));
    assert_eq!(repeated["create"], 0);
    assert_eq!(repeated["update"], 0);
    assert_eq!(repeated["unchanged"], 1);
}

#[test]
fn preview_finds_owned_items_after_the_first_page() {
    let fixture = Fixture::new();
    fixture.response(0, page(vec![], Some("next-page")));
    fixture.response(1, page(vec![json!({"id":"ITEM_1","isArchived":false,"content":{
        "__typename":"DraftIssue","id":"DRAFT_1","title":"Old title", "body":"<!-- taskroot:owner/repo:core:CORE-001 -->"
    }})], None));
    let result = report(fixture.run(false));
    assert_eq!(result["create"], 0);
    assert_eq!(result["update"], 1);
    assert_eq!(fixture.request(1)["variables"]["cursor"], "next-page");
}
