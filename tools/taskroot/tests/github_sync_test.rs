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

fn page(mut items: Vec<Value>, cursor: Option<&str>) -> Value {
    for item in &mut items {
        if item.get("fieldValues").is_none() {
            item["fieldValues"] = json!({"nodes":[],"pageInfo":{"hasNextPage":false}});
        }
    }
    json!({"data":{"node":{"fields":{"nodes":[
        {"id":"status","name":"Status","dataType":"SINGLE_SELECT","options":[{"id":"todo","name":"To Do"}]},
        {"id":"priority","name":"Priority","dataType":"SINGLE_SELECT","options":[{"id":"high","name":"High"}]},
        {"id":"key","name":"Taskroot ID","dataType":"TEXT"},
        {"id":"domain","name":"Domain","dataType":"TEXT"},
        {"id":"milestone","name":"Taskroot Milestone","dataType":"TEXT"}
    ],"pageInfo":{"hasNextPage":false}},"items":{"nodes":items,"pageInfo":{
        "hasNextPage":cursor.is_some(),"endCursor":cursor
    }}}}})
}

fn fields_updated() -> Value {
    json!({"data":{
        "field0":{"projectV2Item":{"id":"ITEM_1"}},"field1":{"projectV2Item":{"id":"ITEM_1"}},
        "field2":{"projectV2Item":{"id":"ITEM_1"}},"field3":{"projectV2Item":{"id":"ITEM_1"}}
    }})
}

fn published_item(created: &Value) -> Value {
    json!({"id":"ITEM_1","isArchived":false,"content":{
        "__typename":"DraftIssue","id":"DRAFT_1","title":created["title"],"body":created["body"]
    },"fieldValues":{"nodes":[
        {"field":{"id":"status"},"optionId":"todo"},
        {"field":{"id":"priority"},"optionId":"high"},
        {"field":{"id":"key"},"text":"core:CORE-001"},
        {"field":{"id":"domain"},"text":"core"}
    ],"pageInfo":{"hasNextPage":false}}})
}

#[test]
fn apply_maps_task_metadata_to_project_fields() {
    let fixture = Fixture::new();
    fixture.response(0, page(vec![], None));
    fixture.response(
        1,
        json!({"data":{"addProjectV2DraftIssue":{"projectItem":{"id":"ITEM_1"}}}}),
    );
    fixture.response(2, fields_updated());
    report(fixture.run(true));
    let request = fixture.request(2);
    assert_eq!(
        request["variables"]["field0"]["value"],
        json!({"singleSelectOptionId":"todo"})
    );
    assert_eq!(
        request["variables"]["field1"]["value"],
        json!({"singleSelectOptionId":"high"})
    );
    assert_eq!(
        request["variables"]["field2"]["value"],
        json!({"text":"core:CORE-001"})
    );
    assert_eq!(
        request["variables"]["field3"]["value"],
        json!({"text":"core"})
    );
    assert!(request["variables"].get("field4").is_none());
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
    fixture.response(2, fields_updated());
    assert_eq!(report(fixture.run(true))["create"], 1);
    let created = fixture.request(1)["variables"]["input"].clone();
    assert_eq!(created["title"], "[core:CORE-001] Example task");
    assert!(
        created["body"]
            .as_str()
            .unwrap()
            .starts_with("<!-- taskroot:owner/repo:core:CORE-001 -->\n")
    );
    fixture.response(3, page(vec![published_item(&created)], None));
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

#[test]
fn invalid_local_tasks_fail_before_contacting_github() {
    let fixture = Fixture::new();
    fs::write(
        fixture.root.path().join(".tasks/core/CORE-001-example.md"),
        "invalid",
    )
    .unwrap();
    let output = fixture.run(true);
    assert!(!output.status.success());
    assert!(!fixture.root.path().join("index").exists());
}

#[test]
fn missing_project_options_fail_before_any_mutations() {
    let fixture = Fixture::new();
    let mut snapshot = page(vec![], None);
    snapshot["data"]["node"]["fields"]["nodes"][0]["options"] = json!([]);
    fixture.response(0, snapshot);
    let output = fixture.run(true);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Status needs exactly one option named To Do")
    );
    assert!(!fixture.root.path().join("request-1.json").exists());
}

#[test]
fn duplicate_owned_drafts_fail_before_any_mutations() {
    let fixture = Fixture::new();
    let item = published_item(
        &json!({"title":"Example task","body":"<!-- taskroot:owner/repo:core:CORE-001 -->"}),
    );
    fixture.response(0, page(vec![item.clone(), item], None));
    let output = fixture.run(true);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("duplicate taskroot ownership marker")
    );
    assert!(!fixture.root.path().join("request-1.json").exists());
}

#[test]
fn graphql_errors_fail_even_when_the_process_exits_successfully() {
    let fixture = Fixture::new();
    let mut snapshot = page(vec![], None);
    snapshot["errors"] = json!([{"message":"Project access denied"}]);
    fixture.response(0, snapshot);
    let output = fixture.run(true);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Project access denied"));
    assert!(!fixture.root.path().join("request-1.json").exists());
}

#[test]
fn retry_after_a_partial_field_failure_recovers_the_existing_draft() {
    let fixture = Fixture::new();
    fixture.response(0, page(vec![], None));
    fixture.response(
        1,
        json!({"data":{"addProjectV2DraftIssue":{"projectItem":{"id":"ITEM_1"}}}}),
    );
    fixture.response(2, json!({"data":{"field0":{"projectV2Item":{"id":"ITEM_1"}}},"errors":[{"message":"Rate limited"}]}));
    let failed = fixture.run(true);
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("Rate limited"));
    let created = fixture.request(1)["variables"]["input"].clone();
    let mut item = published_item(&created);
    item["fieldValues"]["nodes"] = json!([]);
    fixture.response(3, page(vec![item], None));
    fixture.response(4, fields_updated());
    let recovered = report(fixture.run(true));
    assert_eq!(recovered["create"], 0);
    assert_eq!(recovered["update"], 1);
    assert!(
        !fixture.request(4)["query"]
            .as_str()
            .unwrap()
            .contains("addProjectV2DraftIssue")
    );
}

#[test]
fn updates_use_the_draft_content_id_and_preserve_unrelated_items() {
    let fixture = Fixture::new();
    let owned = published_item(
        &json!({"title":"Old title","body":"<!-- taskroot:owner/repo:core:CORE-001 -->"}),
    );
    let unrelated = published_item(&json!({"title":"Example task","body":"A human-owned item"}));
    fixture.response(0, page(vec![owned, unrelated], None));
    fixture.response(1, json!({"data":{"draft":{"draftIssue":{"id":"DRAFT_1"}}}}));
    assert_eq!(report(fixture.run(true))["update"], 1);
    let patch = fixture.request(1)["variables"]["draft"].clone();
    assert_eq!(patch["draftIssueId"], "DRAFT_1");
    assert_eq!(patch["title"], "[core:CORE-001] Example task");
    assert!(
        patch["body"]
            .as_str()
            .unwrap()
            .contains("## Acceptance Criteria\n\n- [ ] Works")
    );
}

#[test]
fn a_removed_local_milestone_clears_its_project_field() {
    let fixture = Fixture::new();
    let mut item = published_item(
        &json!({"title":"Old title","body":"<!-- taskroot:owner/repo:core:CORE-001 -->"}),
    );
    item["fieldValues"]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"field":{"id":"milestone"},"text":"0.3.0"}));
    fixture.response(0, page(vec![item], None));
    fixture.response(1, json!({"data":{"draft":{"draftIssue":{"id":"DRAFT_1"}},"field0":{"projectV2Item":{"id":"ITEM_1"}}}}));
    report(fixture.run(true));
    let request = fixture.request(1);
    assert!(
        request["query"]
            .as_str()
            .unwrap()
            .contains("clearProjectV2ItemFieldValue")
    );
    assert_eq!(request["variables"]["field0"]["fieldId"], "milestone");
}

#[test]
fn archived_or_converted_owned_items_require_explicit_recovery() {
    for (kind, archived) in [("DraftIssue", true), ("Issue", false)] {
        let fixture = Fixture::new();
        let mut item = published_item(
            &json!({"title":"Example task","body":"<!-- taskroot:owner/repo:core:CORE-001 -->"}),
        );
        item["isArchived"] = json!(archived);
        item["content"]["__typename"] = json!(kind);
        fixture.response(0, page(vec![item], None));
        let output = fixture.run(true);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("archived or converted"));
        assert_eq!(
            fixture.request(0)["variables"]["archived"],
            json!(["ARCHIVED", "NOT_ARCHIVED"])
        );
        assert!(!fixture.root.path().join("request-1.json").exists());
    }
}

#[test]
fn inaccessible_items_fail_instead_of_risking_a_duplicate() {
    let fixture = Fixture::new();
    fixture.response(
        0,
        page(vec![json!({"type":"REDACTED","content":null})], None),
    );
    let output = fixture.run(true);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("inaccessible project item"));
}

#[test]
fn incomplete_field_update_responses_are_reported_as_failures() {
    let fixture = Fixture::new();
    fixture.response(0, page(vec![], None));
    fixture.response(
        1,
        json!({"data":{"addProjectV2DraftIssue":{"projectItem":{"id":"ITEM_1"}}}}),
    );
    fixture.response(
        2,
        json!({"data":{"field0":{"projectV2Item":{"id":"ITEM_1"}}}}),
    );
    let output = fixture.run(true);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("incomplete field update response"));
}
