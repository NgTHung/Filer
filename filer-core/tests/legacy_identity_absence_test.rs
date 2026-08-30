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

fn legacy_identity_names() -> [String; 2] {
    [["Node", "Id"].concat(), ["File", "Node"].concat()]
}

#[test]
fn rust_sources_have_no_legacy_identity_names() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source_roots = [manifest_dir.join("src"), manifest_dir.join("tests")];
    let mut sources = Vec::new();
    for source_root in source_roots {
        collect_rust_sources(&source_root, &mut sources);
    }

    let mut violations = Vec::new();
    let forbidden_names = legacy_identity_names();
    for source_path in sources {
        let relative_path = source_path.strip_prefix(&manifest_dir).unwrap();
        let source = fs::read_to_string(&source_path).unwrap();
        for (line_number, line) in source.lines().enumerate() {
            if forbidden_names.iter().any(|name| line.contains(name)) {
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
        "legacy identity references remain in Rust sources:\n{}",
        violations.join("\n")
    );
}
