use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

use crate::api::wire_commands::{WireCommand, WireCommandConversionError};
use crate::model::directory::DirectoryLoadOptions;
use crate::model::location::{Location, LocationRef};
use crate::model::node::NodeId;
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
    let node = NodeId(44);
    let other_node = NodeId(45);
    let root = location("/root");
    let target = location("/target");
    let pipeline = PipelineConfig::with_default_sort();
    let load = DirectoryLoadOptions::bounded(64);

    vec![
        (
            WireCommand::NavigatePathCompat {
                path: PathBuf::from("/root"),
                session,
                request,
            },
            "navigate_path_compat",
            "navigate.path.compat",
        ),
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
            WireCommand::NavigateNodeCompat {
                node,
                session,
                request,
            },
            "navigate_node_compat",
            "navigate.node.compat",
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
            WireCommand::SearchNodeCompat {
                query: "name:test".to_string(),
                root: node,
                session,
                request,
            },
            "search_node_compat",
            "search.node.compat",
        ),
        (
            WireCommand::SearchPathCompat {
                query: "name:test".to_string(),
                root: PathBuf::from("/root"),
                session,
                request,
            },
            "search_path_compat",
            "search.path.compat",
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
            WireCommand::LoadPreviewNodeCompat {
                id: node,
                options: Some(PreviewOptions::default()),
                session,
                request,
            },
            "load_preview_node_compat",
            "preview.load.node.compat",
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
            WireCommand::CopyNodeCompat {
                sources: vec![node],
                destination: other_node,
                session,
                request,
                operation,
            },
            "copy_node_compat",
            "ops.copy.node.compat",
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
            WireCommand::MoveNodeCompat {
                sources: vec![node],
                destination: other_node,
                session,
                request,
                operation,
            },
            "move_node_compat",
            "ops.move.node.compat",
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
            WireCommand::DeleteNodeCompat {
                nodes: vec![node],
                trash: true,
                session,
                request,
                operation,
            },
            "delete_node_compat",
            "ops.delete.node.compat",
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
            WireCommand::RenameNodeCompat {
                node,
                new_name: "renamed".to_string(),
                session,
                request,
                operation,
            },
            "rename_node_compat",
            "ops.rename.node.compat",
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
            WireCommand::CreateFolderNodeCompat {
                parent: node,
                name: "folder".to_string(),
                session,
                request,
                operation,
            },
            "create_folder_node_compat",
            "ops.create_folder.node.compat",
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
            WireCommand::CreateFileNodeCompat {
                parent: node,
                name: "file.txt".to_string(),
                session,
                request,
                operation,
            },
            "create_file_node_compat",
            "ops.create_file.node.compat",
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
            WireCommand::LoadMetadataNodeCompat {
                node,
                session,
                request,
            },
            "load_metadata_node_compat",
            "metadata.load.node.compat",
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
            WireCommand::LoadExtendedMetadataNodeCompat {
                node,
                session,
                request,
            },
            "load_extended_metadata_node_compat",
            "metadata.extended.node.compat",
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
            WireCommand::ScanPathCompat {
                path: PathBuf::from("/root"),
                session,
                pipeline: pipeline.clone(),
                load: load.clone(),
                request,
            },
            "scan_path_compat",
            "scan.path.compat",
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
            WireCommand::ScanNodeCompat {
                node,
                session,
                pipeline: pipeline.clone(),
                load,
                request,
            },
            "scan_node_compat",
            "scan.node.compat",
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
            WireCommand::WatchNodeCompat { node, session },
            "watch_node_compat",
            "watch.node.compat",
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
            WireCommand::UnwatchNodeCompat { node },
            "unwatch_node_compat",
            "watch.node.remove.compat",
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
