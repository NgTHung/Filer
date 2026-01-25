//! Tests for error types

use crate::errors::CoreError;
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
    let error = CoreError::ChannelClosed;
    
    match error {
        CoreError::ChannelClosed => {
            // Expected variant
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
    let error = CoreError::ChannelClosed;
    
    let display = format!("{}", error);
    assert!(display.contains("channel") || display.contains("Channel"));
    assert!(display.contains("closed") || display.contains("Closed"));
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
    let error1 = CoreError::ChannelClosed;
    let error2 = CoreError::Cancelled;
    
    assert!(matches!(error1, CoreError::ChannelClosed));
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
