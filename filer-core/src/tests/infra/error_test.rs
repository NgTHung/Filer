//! Tests for error types

use crate::api::events::Event;
use crate::errors::{CoreError, ErrorKind};
use crate::model::session::SessionId;
use std::path::PathBuf;

#[test]
fn test_error_io_variant() {
    let error = CoreError::Io {
        path: PathBuf::from("/tmp/file.txt"),
        message: "Failed to read".to_string(),
    };

    match error {
        CoreError::Io { path, message } => {
            assert_eq!(path, PathBuf::from("/tmp/file.txt"));
            assert_eq!(message, "Failed to read");
        }
        _ => panic!("Expected Io variant"),
    }
}

#[test]
fn test_error_not_found_variant() {
    let error = CoreError::NotFound(PathBuf::from("/nonexistent/path"));

    match error {
        CoreError::NotFound(path) => {
            assert_eq!(path, PathBuf::from("/nonexistent/path"));
        }
        _ => panic!("Expected NotFound variant"),
    }
}

#[test]
fn test_error_permission_denied_variant() {
    let error = CoreError::PermissionDenied(PathBuf::from("/root/secret"));

    match error {
        CoreError::PermissionDenied(path) => {
            assert_eq!(path, PathBuf::from("/root/secret"));
        }
        _ => panic!("Expected PermissionDenied variant"),
    }
}

#[test]
fn test_error_invalid_path_variant() {
    let error = CoreError::InvalidPath("Invalid path format".to_string());

    match error {
        CoreError::InvalidPath(msg) => {
            assert_eq!(msg, "Invalid path format");
        }
        _ => panic!("Expected InvalidPath variant"),
    }
}

#[test]
fn test_error_channel_closed_variant() {
    let error = CoreError::ChannelClosed("test channel".into());

    match error {
        CoreError::ChannelClosed(detail) => {
            assert_eq!(detail, "test channel");
        }
        _ => panic!("Expected ChannelClosed variant"),
    }
}

#[test]
fn test_error_cancelled_variant() {
    let error = CoreError::Cancelled;

    match error {
        CoreError::Cancelled => {
            // Expected variant
        }
        _ => panic!("Expected Cancelled variant"),
    }
}

#[test]
fn test_error_actor_error_variant() {
    let error = CoreError::ActorError {
        actor: "Navigator",
        message: "Failed to navigate".to_string(),
    };

    match error {
        CoreError::ActorError { actor, message } => {
            assert_eq!(actor, "Navigator");
            assert_eq!(message, "Failed to navigate");
        }
        _ => panic!("Expected ActorError variant"),
    }
}

#[test]
fn test_error_display_io() {
    let error = CoreError::Io {
        path: PathBuf::from("/tmp/file.txt"),
        message: "Failed to read".to_string(),
    };

    let display = format!("{}", error);
    assert!(display.contains("/tmp/file.txt"));
    assert!(display.contains("Failed to read"));
}

#[test]
fn test_error_display_not_found() {
    let error = CoreError::NotFound(PathBuf::from("/nonexistent/path"));

    let display = format!("{}", error);
    assert!(display.contains("/nonexistent/path"));
    assert!(display.contains("not found") || display.contains("Not found"));
}

#[test]
fn test_error_display_permission_denied() {
    let error = CoreError::PermissionDenied(PathBuf::from("/root/secret"));

    let display = format!("{}", error);
    assert!(display.contains("/root/secret"));
    assert!(display.contains("permission") || display.contains("Permission"));
}

#[test]
fn test_error_display_invalid_path() {
    let error = CoreError::InvalidPath("Invalid path format".to_string());

    let display = format!("{}", error);
    assert!(display.contains("Invalid path format"));
}

#[test]
fn test_error_display_channel_closed() {
    let error = CoreError::ChannelClosed("command bus".into());

    let display = format!("{}", error);
    assert!(
        display.contains("hannel"),
        "should mention channel: {}",
        display
    );
    assert!(
        display.contains("command bus"),
        "should contain detail: {}",
        display
    );
}

#[test]
fn test_error_display_cancelled() {
    let error = CoreError::Cancelled;

    let display = format!("{}", error);
    assert!(display.contains("cancel") || display.contains("Cancel"));
}

#[test]
fn test_error_display_actor_error() {
    let error = CoreError::ActorError {
        actor: "Navigator",
        message: "Failed to navigate".to_string(),
    };

    let display = format!("{}", error);
    assert!(display.contains("Navigator"));
    assert!(display.contains("Failed to navigate"));
}

#[test]
fn test_error_debug() {
    let error = CoreError::NotFound(PathBuf::from("/test"));
    let debug = format!("{:?}", error);
    assert!(debug.contains("NotFound"));
    assert!(debug.contains("/test"));
}

#[test]
fn test_error_is_error_trait() {
    let error = CoreError::Cancelled;
    // This ensures CoreError implements std::error::Error
    let _: &dyn std::error::Error = &error;
}

#[test]
fn test_error_kind_for_all_core_error_variants() {
    let path = PathBuf::from("/tmp/file.txt");
    let cases = vec![
        (
            CoreError::Io {
                path: path.clone(),
                message: "io".to_string(),
            },
            ErrorKind::Io,
        ),
        (CoreError::NotFound(path.clone()), ErrorKind::NotFound),
        (
            CoreError::PermissionDenied(path.clone()),
            ErrorKind::PermissionDenied,
        ),
        (
            CoreError::InvalidPath("bad path".to_string()),
            ErrorKind::InvalidPath,
        ),
        (
            CoreError::ChannelClosed("closed".to_string()),
            ErrorKind::ChannelClosed,
        ),
        (CoreError::Cancelled, ErrorKind::Cancelled),
        (
            CoreError::ActorError {
                actor: "test",
                message: "failed".to_string(),
            },
            ErrorKind::Actor,
        ),
        (
            CoreError::NetworkError("offline".to_string()),
            ErrorKind::Network,
        ),
        (
            CoreError::InvalidData("corrupt".to_string()),
            ErrorKind::InvalidData,
        ),
        (
            CoreError::InvalidInput("bad input".to_string()),
            ErrorKind::InvalidInput,
        ),
        (
            CoreError::Other(std::io::Error::other("unexpected")),
            ErrorKind::Unknown,
        ),
    ];

    for (error, expected) in cases {
        assert_eq!(error.kind(), expected);
    }
}

#[test]
fn test_error_kind_recoverability() {
    for kind in [
        ErrorKind::NotFound,
        ErrorKind::PermissionDenied,
        ErrorKind::InvalidPath,
        ErrorKind::Cancelled,
        ErrorKind::Network,
    ] {
        assert!(kind.is_recoverable(), "{kind:?} should be recoverable");
    }

    for kind in [
        ErrorKind::Io,
        ErrorKind::ChannelClosed,
        ErrorKind::Actor,
        ErrorKind::InvalidData,
        ErrorKind::InvalidInput,
        ErrorKind::Unknown,
    ] {
        assert!(!kind.is_recoverable(), "{kind:?} should not be recoverable");
    }
}

#[test]
fn test_event_from_error_includes_kind_and_recoverability() {
    let session = SessionId::new();
    let path = PathBuf::from("/tmp/missing");

    let event = Event::from_error(CoreError::NotFound(path), session);
    match event {
        Event::Error {
            kind,
            recoverable,
            session: event_session,
            request,
            operation,
            ..
        } => {
            assert_eq!(kind, ErrorKind::NotFound);
            assert!(recoverable);
            assert_eq!(event_session, session);
            assert_eq!(request, None);
            assert_eq!(operation, None);
        }
        other => panic!("expected Error event, got {other:?}"),
    }

    let event = Event::from_error(CoreError::InvalidInput("bad input".to_string()), session);
    match event {
        Event::Error {
            kind, recoverable, ..
        } => {
            assert_eq!(kind, ErrorKind::InvalidInput);
            assert!(!recoverable);
        }
        other => panic!("expected Error event, got {other:?}"),
    }

    let event = Event::from_error(CoreError::Other(std::io::Error::other("unknown")), session);
    match event {
        Event::Error {
            kind, recoverable, ..
        } => {
            assert_eq!(kind, ErrorKind::Unknown);
            assert!(!recoverable);
        }
        other => panic!("expected Error event, got {other:?}"),
    }
}

// Conversion tests - from std::io::Error
#[test]
fn test_conversion_from_io_error() {
    use std::io::{Error as IoError, ErrorKind};

    let io_error = IoError::new(ErrorKind::NotFound, "file not found");
    let path = PathBuf::from("/test/file.txt");
    let core_error = CoreError::from_io_error(io_error, path.clone());

    match core_error {
        CoreError::NotFound(p) => assert_eq!(p, path),
        _ => panic!("Expected NotFound variant for NotFound io error"),
    }
}

#[test]
fn test_conversion_from_io_error_permission_denied() {
    use std::io::{Error as IoError, ErrorKind};

    let io_error = IoError::new(ErrorKind::PermissionDenied, "access denied");
    let path = PathBuf::from("/root/secret");
    let core_error = CoreError::from_io_error(io_error, path.clone());

    match core_error {
        CoreError::PermissionDenied(p) => assert_eq!(p, path),
        _ => panic!("Expected PermissionDenied variant for PermissionDenied io error"),
    }
}

#[test]
fn test_conversion_from_io_error_other() {
    use std::io::{Error as IoError, ErrorKind};

    let io_error = IoError::new(ErrorKind::TimedOut, "timeout");
    let path = PathBuf::from("/test/file.txt");
    let core_error = CoreError::from_io_error(io_error, path.clone());

    match core_error {
        CoreError::Io { path: p, message } => {
            assert_eq!(p, path);
            assert!(message.contains("timeout"));
        }
        _ => panic!("Expected Io variant for other io errors"),
    }
}

#[test]
fn test_error_equality_check() {
    // Test that we can match on error types
    let error1 = CoreError::ChannelClosed("test".into());
    let error2 = CoreError::Cancelled;

    assert!(matches!(error1, CoreError::ChannelClosed(_)));
    assert!(matches!(error2, CoreError::Cancelled));
    assert!(!matches!(error1, CoreError::Cancelled));
}

#[test]
fn test_error_with_empty_strings() {
    let error = CoreError::InvalidPath(String::new());
    let display = format!("{}", error);
    assert!(!display.is_empty());
}

#[test]
fn test_error_with_special_characters() {
    let error = CoreError::InvalidPath("Path with\nnewlines\tand\ttabs".to_string());
    let display = format!("{}", error);
    assert!(display.contains("Path with"));
}

#[test]
fn test_error_actor_with_various_actors() {
    let actors = ["Navigator", "Scanner", "Searcher", "Previewer"];

    for actor in actors {
        let error = CoreError::ActorError {
            actor,
            message: "Test error".to_string(),
        };
        let display = format!("{}", error);
        assert!(display.contains(actor));
    }
}
