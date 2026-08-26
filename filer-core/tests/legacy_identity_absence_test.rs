use std::fs;
use std::path::{Path, PathBuf};

fn collect_rust_sources(directory: &Path, sources: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_rust_sources(&path, sources);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            sources.push(path);
        }
    }
}

fn allowed_node_id_definition(path: &Path, line: &str) -> bool {
    path == Path::new("model/node.rs")
        && matches!(
            line.trim(),
            "pub struct NodeId(pub u64);" | "impl NodeId {" | "NodeId({"
        )
}

#[test]
fn production_has_no_legacy_identity_plumbing() {
    let source_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    collect_rust_sources(&source_root, &mut sources);

    let mut violations = Vec::new();
    for source_path in sources {
        let relative_path = source_path.strip_prefix(&source_root).unwrap();
        if relative_path.starts_with("tests") {
            continue;
        }
        let source = fs::read_to_string(&source_path).unwrap();
        for (line_number, line) in source.lines().enumerate() {
            if (line.contains("NodeId") || line.contains("FileNode"))
                && !allowed_node_id_definition(relative_path, line)
            {
                violations.push(format!(
                    "{}:{}: {}",
                    relative_path.display(),
                    line_number + 1,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "legacy identity references remain in production:\n{}",
        violations.join("\n")
    );
}
