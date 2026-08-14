use std::sync::Arc;

use serde_json::Value;

use crate::api::wire_commands::{WireCommand, WireCommandConversionError};
use crate::model::directory::DirectoryLoadOptions;
use crate::model::location::{Location, LocationRef};
use crate::model::operation::OperationId;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::pipeline::PipelineConfig;
use crate::{Command, PreviewOptions};

fn location(path: &str) -> LocationRef {
    LocationRef::from_location(&Location::local(path))
}

fn built_in_commands() -> Vec<(WireCommand, &'static str, &'static str)> {
    let session = SessionId(11);
    let request = RequestId(22);
    let operation = OperationId(33);
    let root = location("/root");
    let target = location("/target");
    let pipeline = PipelineConfig::with_default_sort();
    let load = DirectoryLoadOptions::bounded(64);

    vec![
        (
            WireCommand::Navigate {
                location: root.clone(),
                session,
                request,
            },
            "navigate",
            "navigate",
        ),
        (
            WireCommand::NavigateUp { session, request },
            "navigate_up",
            "navigate.up",
        ),
        (
            WireCommand::NavigateBack { session, request },
            "navigate_back",
            "navigate.back",
        ),
        (
            WireCommand::NavigateForward { session, request },
            "navigate_forward",
            "navigate.forward",
        ),
        (
            WireCommand::Refresh { session, request },
            "refresh",
            "navigate.refresh",
        ),
        (
            WireCommand::Search {
                query: "name:test".to_string(),
                root: root.clone(),
                session,
                request,
            },
            "search",
            "search",
        ),
        (
            WireCommand::CancelSearch { session },
            "cancel_search",
            "search.cancel",
        ),
        (
            WireCommand::LoadPreview {
                location: root.clone(),
                options: Some(PreviewOptions::default()),
                session,
                request,
            },
            "load_preview",
            "preview.load",
        ),
        (
            WireCommand::CancelPreview { session },
            "cancel_preview",
            "preview.cancel",
        ),
        (
            WireCommand::Copy {
                sources: vec![root.clone()],
                destination: target.clone(),
                session,
                request,
                operation,
            },
            "copy",
            "ops.copy",
        ),
        (
            WireCommand::Move {
                sources: vec![root.clone()],
                destination: target.clone(),
                session,
                request,
                operation,
            },
            "move",
            "ops.move",
        ),
        (
            WireCommand::Delete {
                locations: vec![root.clone()],
                trash: true,
                session,
                request,
                operation,
            },
            "delete",
            "ops.delete",
        ),
        (
            WireCommand::Rename {
                location: root.clone(),
                new_name: "renamed".to_string(),
                session,
                request,
                operation,
            },
            "rename",
            "ops.rename",
        ),
        (
            WireCommand::CreateFolder {
                parent: root.clone(),
                name: "folder".to_string(),
                session,
                request,
                operation,
            },
            "create_folder",
            "ops.create_folder",
        ),
        (
            WireCommand::CreateFile {
                parent: root.clone(),
                name: "file.txt".to_string(),
                session,
                request,
                operation,
            },
            "create_file",
            "ops.create_file",
        ),
        (
            WireCommand::LoadMetadata {
                location: root.clone(),
                session,
                request,
            },
            "load_metadata",
            "metadata.load",
        ),
        (
            WireCommand::LoadExtendedMetadata {
                location: root.clone(),
                session,
                request,
            },
            "load_extended_metadata",
            "metadata.extended",
        ),
        (
            WireCommand::Scan {
                location: root.clone(),
                session,
                pipeline: pipeline.clone(),
                load: load.clone(),
                request,
            },
            "scan",
            "scan",
        ),
        (
            WireCommand::SetPipeline {
                session,
                config: pipeline,
            },
            "set_pipeline",
            "navigate.pipeline",
        ),
        (
            WireCommand::CancelScan { session },
            "cancel_scan",
            "scan.cancel",
        ),
        (
            WireCommand::CancelOperation { session, operation },
            "cancel_operation",
            "ops.cancel",
        ),
        (
            WireCommand::Watch {
                location: root.clone(),
                session,
                request,
            },
            "watch",
            "watch",
        ),
        (
            WireCommand::Unwatch {
                location: root,
                session,
            },
            "unwatch",
            "watch.remove",
        ),
        (
            WireCommand::UnwatchSession { session },
            "unwatch_session",
            "watch.session_remove",
        ),
        (WireCommand::Handshake, "handshake", "session.handshake"),
        (
            WireCommand::DestroySession { session },
            "destroy_session",
            "session.destroy",
        ),
    ]
}

#[test]
fn built_in_commands_have_stable_wire_labels_and_dispatch_keys() {
    for (wire, expected_label, expected_key) in built_in_commands() {
        let json = serde_json::to_value(&wire).expect("wire command should serialize");
        assert_eq!(
            json.get("type"),
            Some(&Value::String(expected_label.into()))
        );

        let decoded: WireCommand =
            serde_json::from_value(json).expect("wire command should deserialize");
        assert_eq!(decoded, wire);

        let runtime = Command::from(decoded);
        assert_eq!(runtime.key(), expected_key);
        assert_eq!(
            WireCommand::try_from(runtime).expect("built-in command should convert"),
            wire
        );
    }
}

fn removed_compatibility_commands() -> Vec<Value> {
    let pipeline = serde_json::to_value(PipelineConfig::default()).expect("pipeline serializes");
    let load = serde_json::to_value(DirectoryLoadOptions::default()).expect("load serializes");

    vec![
        serde_json::json!({
            "type": "navigate_path_compat",
            "path": "/root",
            "session": 11,
            "request": 22,
        }),
        serde_json::json!({
            "type": "navigate_node_compat",
            "node": 44,
            "session": 11,
            "request": 22,
        }),
        serde_json::json!({
            "type": "search_node_compat",
            "query": "name:test",
            "root": 44,
            "session": 11,
            "request": 22,
        }),
        serde_json::json!({
            "type": "search_path_compat",
            "query": "name:test",
            "root": "/root",
            "session": 11,
            "request": 22,
        }),
        serde_json::json!({
            "type": "load_preview_node_compat",
            "id": 44,
            "options": null,
            "session": 11,
            "request": 22,
        }),
        serde_json::json!({
            "type": "copy_node_compat",
            "sources": [44],
            "destination": 45,
            "session": 11,
            "request": 22,
            "operation": 33,
        }),
        serde_json::json!({
            "type": "move_node_compat",
            "sources": [44],
            "destination": 45,
            "session": 11,
            "request": 22,
            "operation": 33,
        }),
        serde_json::json!({
            "type": "delete_node_compat",
            "nodes": [44],
            "trash": true,
            "session": 11,
            "request": 22,
            "operation": 33,
        }),
        serde_json::json!({
            "type": "rename_node_compat",
            "node": 44,
            "new_name": "renamed",
            "session": 11,
            "request": 22,
            "operation": 33,
        }),
        serde_json::json!({
            "type": "create_folder_node_compat",
            "parent": 44,
            "name": "folder",
            "session": 11,
            "request": 22,
            "operation": 33,
        }),
        serde_json::json!({
            "type": "create_file_node_compat",
            "parent": 44,
            "name": "file.txt",
            "session": 11,
            "request": 22,
            "operation": 33,
        }),
        serde_json::json!({
            "type": "load_metadata_node_compat",
            "node": 44,
            "session": 11,
            "request": 22,
        }),
        serde_json::json!({
            "type": "load_extended_metadata_node_compat",
            "node": 44,
            "session": 11,
            "request": 22,
        }),
        serde_json::json!({
            "type": "scan_path_compat",
            "path": "/root",
            "session": 11,
            "pipeline": pipeline,
            "load": load,
            "request": 22,
        }),
        serde_json::json!({
            "type": "scan_node_compat",
            "node": 44,
            "session": 11,
            "pipeline": PipelineConfig::default(),
            "load": DirectoryLoadOptions::default(),
            "request": 22,
        }),
        serde_json::json!({
            "type": "watch_node_compat",
            "node": 44,
            "session": 11,
        }),
        serde_json::json!({
            "type": "unwatch_node_compat",
            "node": 44,
        }),
    ]
}

#[test]
fn removed_compatibility_wire_commands_are_unknown_variants() {
    for payload in removed_compatibility_commands() {
        let label = payload["type"]
            .as_str()
            .unwrap_or("<missing type>")
            .to_string();
        let error = serde_json::from_value::<WireCommand>(payload)
            .expect_err("removed compatibility command must not deserialize");
        assert!(
            error.to_string().contains("unknown variant"),
            "{label} should fail as an unknown wire variant, got {error}"
        );
    }
}

#[test]
fn runtime_extension_conversion_returns_typed_error() {
    let command = Command::Extension {
        key: "git.status".to_string(),
        payload: Arc::new(()),
        session: SessionId(7),
    };

    assert_eq!(
        WireCommand::try_from(command),
        Err(WireCommandConversionError::ExtensionUnsupported {
            key: "git.status".to_string(),
        })
    );
}
