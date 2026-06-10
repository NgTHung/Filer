//! Tests for structured error types

use crate::api::events::Event;
use crate::errors::{CoreError, ErrorCode, ErrorContext, ErrorKind, ErrorTarget};
use crate::model::capability::LocationCapabilityError;
use crate::model::location::{Location, LocationRef, ProviderRef};
use crate::model::operation::OperationId;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use std::path::PathBuf;

#[test]
fn test_error_constructor_sets_formal_fields() {
    let path = PathBuf::from("/tmp/file.txt");
    let error = CoreError::io(path.clone(), "Failed to read");

    assert_eq!(error.kind(), ErrorKind::Io);
    assert_eq!(error.code(), ErrorCode::IoFailed);
    assert_eq!(error.target(), Some(&ErrorTarget::Path(path)));
    assert_eq!(error.message, "Failed to read");
    assert!(!error.recoverable());
}

#[test]
fn test_path_error_constructors_set_targets() {
    let missing = PathBuf::from("/missing");
    let denied = PathBuf::from("/root/secret");

    let not_found = CoreError::not_found(missing.clone());
    assert_eq!(not_found.kind(), ErrorKind::NotFound);
    assert_eq!(not_found.code(), ErrorCode::PathNotFound);
    assert_eq!(not_found.target(), Some(&ErrorTarget::Path(missing)));
    assert!(not_found.recoverable());

    let permission = CoreError::permission_denied(denied.clone());
    assert_eq!(permission.kind(), ErrorKind::PermissionDenied);
    assert_eq!(permission.code(), ErrorCode::PermissionDenied);
    assert_eq!(permission.target(), Some(&ErrorTarget::Path(denied)));
    assert!(permission.recoverable());
}

#[test]
fn test_error_display_uses_message() {
    let error = CoreError::invalid_path("Invalid path format");
    assert_eq!(format!("{error}"), "Invalid path format");
}

#[test]
fn test_error_is_error_trait() {
    let error = CoreError::cancelled();
    let _: &dyn std::error::Error = &error;
}

#[test]
fn test_error_code_kind_mapping() {
    let cases = [
        (ErrorCode::IoFailed, ErrorKind::Io),
        (ErrorCode::PathNotFound, ErrorKind::NotFound),
        (ErrorCode::PermissionDenied, ErrorKind::PermissionDenied),
        (ErrorCode::ReadOnly, ErrorKind::PermissionDenied),
        (ErrorCode::InvalidPath, ErrorKind::InvalidPath),
        (ErrorCode::LocationUnresolved, ErrorKind::InvalidLocation),
        (
            ErrorCode::LocationSegmentedUnsupported,
            ErrorKind::InvalidLocation,
        ),
        (ErrorCode::UnsupportedProvider, ErrorKind::Unsupported),
        (ErrorCode::ChannelClosed, ErrorKind::ChannelClosed),
        (ErrorCode::Cancelled, ErrorKind::Cancelled),
        (ErrorCode::TimedOut, ErrorKind::Timeout),
        (ErrorCode::ActorFailed, ErrorKind::Actor),
        (ErrorCode::NetworkFailed, ErrorKind::Network),
        (ErrorCode::DataInvalid, ErrorKind::InvalidData),
        (ErrorCode::InputInvalid, ErrorKind::InvalidInput),
        (ErrorCode::Collision, ErrorKind::Conflict),
        (ErrorCode::StaleRequest, ErrorKind::InvalidInput),
        (ErrorCode::SessionUnknown, ErrorKind::InvalidInput),
        (ErrorCode::NavigationUnavailable, ErrorKind::InvalidInput),
        (ErrorCode::UnsupportedOperation, ErrorKind::Unsupported),
        (
            ErrorCode::ProviderCapabilityUnavailable,
            ErrorKind::Unsupported,
        ),
        (ErrorCode::Unknown, ErrorKind::Unknown),
    ];

    for (code, kind) in cases {
        assert_eq!(code.kind(), kind);
    }
}

#[test]
fn test_error_code_recoverability() {
    for code in [
        ErrorCode::PathNotFound,
        ErrorCode::PermissionDenied,
        ErrorCode::ReadOnly,
        ErrorCode::InvalidPath,
        ErrorCode::LocationUnresolved,
        ErrorCode::LocationSegmentedUnsupported,
        ErrorCode::UnsupportedProvider,
        ErrorCode::Cancelled,
        ErrorCode::TimedOut,
        ErrorCode::NetworkFailed,
        ErrorCode::Collision,
        ErrorCode::StaleRequest,
        ErrorCode::SessionUnknown,
        ErrorCode::NavigationUnavailable,
        ErrorCode::UnsupportedOperation,
        ErrorCode::ProviderCapabilityUnavailable,
    ] {
        assert!(code.is_recoverable(), "{code:?} should be recoverable");
    }

    for code in [
        ErrorCode::IoFailed,
        ErrorCode::ChannelClosed,
        ErrorCode::ActorFailed,
        ErrorCode::DataInvalid,
        ErrorCode::InputInvalid,
        ErrorCode::Unknown,
    ] {
        assert!(!code.is_recoverable(), "{code:?} should not be recoverable");
    }
}

#[test]
fn test_event_from_error_includes_formal_error_fields() {
    let session = SessionId::new();
    let path = PathBuf::from("/tmp/missing");

    let event = Event::from_error(CoreError::not_found(path.clone()), session);
    match event {
        Event::Error {
            kind,
            code,
            target,
            recoverable,
            session: event_session,
            request,
            operation,
            ..
        } => {
            assert_eq!(kind, ErrorKind::NotFound);
            assert_eq!(code, ErrorCode::PathNotFound);
            assert_eq!(target, Some(ErrorTarget::Path(path)));
            assert!(recoverable);
            assert_eq!(event_session, session);
            assert_eq!(request, None);
            assert_eq!(operation, None);
        }
        other => panic!("expected Error event, got {other:?}"),
    }
}

#[test]
fn test_request_and_operation_error_helpers_preserve_correlation() {
    let session = SessionId::new();
    let request = RequestId::new();
    let operation = OperationId::new();

    let event = Event::from_operation_error(
        CoreError::invalid_input("bad input"),
        session,
        request,
        operation,
    );

    match event {
        Event::Error {
            kind,
            code,
            session: event_session,
            request: event_request,
            operation: event_operation,
            ..
        } => {
            assert_eq!(kind, ErrorKind::InvalidInput);
            assert_eq!(code, ErrorCode::InputInvalid);
            assert_eq!(event_session, session);
            assert_eq!(event_request, Some(request));
            assert_eq!(event_operation, Some(operation));
        }
        other => panic!("expected Error event, got {other:?}"),
    }
}

#[test]
fn test_conversion_from_io_error() {
    use std::io::{Error as IoError, ErrorKind as IoErrorKind};

    let path = PathBuf::from("/test/file.txt");
    let core_error = CoreError::from_io_error(
        IoError::new(IoErrorKind::NotFound, "file not found"),
        path.clone(),
    );

    assert_eq!(core_error.code(), ErrorCode::PathNotFound);
    assert_eq!(core_error.target(), Some(&ErrorTarget::Path(path)));
    assert!(std::error::Error::source(&core_error).is_some());
}

#[test]
fn test_conversion_from_permission_denied() {
    use std::io::{Error as IoError, ErrorKind as IoErrorKind};

    let path = PathBuf::from("/root/secret");
    let core_error = CoreError::from_io_error(
        IoError::new(IoErrorKind::PermissionDenied, "access denied"),
        path.clone(),
    );

    assert_eq!(core_error.code(), ErrorCode::PermissionDenied);
    assert_eq!(core_error.target(), Some(&ErrorTarget::Path(path)));
}

#[test]
fn test_conversion_from_other_io_error() {
    use std::io::{Error as IoError, ErrorKind as IoErrorKind};

    let path = PathBuf::from("/test/file.txt");
    let core_error =
        CoreError::from_io_error(IoError::new(IoErrorKind::TimedOut, "timeout"), path.clone());

    assert_eq!(core_error.code(), ErrorCode::TimedOut);
    assert_eq!(core_error.kind(), ErrorKind::Timeout);
    assert_eq!(core_error.target(), None);
    assert!(core_error.message.contains("Timed out"));
}

#[test]
fn test_emit_trace_does_not_require_subscriber() {
    CoreError::invalid_input("bad input").emit_trace();
}

#[test]
fn collision_error_exposes_source_and_destination() {
    let source = ErrorTarget::Path(PathBuf::from("/tmp/source.txt"));
    let destination = ErrorTarget::Path(PathBuf::from("/tmp/destination.txt"));

    let error = CoreError::collision(source.clone(), destination.clone());

    assert_eq!(error.kind(), ErrorKind::Conflict);
    assert_eq!(error.code(), ErrorCode::Collision);
    assert!(error.recoverable());
    assert_eq!(
        error.context(),
        Some(&ErrorContext::Collision {
            source,
            destination,
        })
    );
}

#[test]
fn stale_request_error_exposes_session_and_request() {
    let session = SessionId::new();
    let request = RequestId::new();

    let error = CoreError::stale_request(session, request);

    assert_eq!(error.code(), ErrorCode::StaleRequest);
    assert_eq!(
        error.context(),
        Some(&ErrorContext::StaleRequest { session, request })
    );
}

#[test]
fn provider_capability_error_exposes_provider_location_and_capability() {
    let location = LocationRef::from_location(&Location::local("/tmp/read-only"));
    let capability = LocationCapabilityError::WriteUnsupported;

    let error =
        CoreError::provider_capability(ProviderRef::Local, location.clone(), capability.clone());

    assert_eq!(error.code(), ErrorCode::ProviderCapabilityUnavailable);
    assert_eq!(
        error.context(),
        Some(&ErrorContext::ProviderCapability {
            provider: ProviderRef::Local,
            location,
            capability,
        })
    );
}

#[test]
fn error_event_preserves_structured_context() {
    let session = SessionId::new();
    let request = RequestId::new();
    let error = CoreError::stale_request(session, request);

    match Event::from_request_error(error, session, request) {
        Event::Error {
            context: Some(context),
            ..
        } => match *context {
            ErrorContext::StaleRequest {
                session: context_session,
                request: context_request,
            } => {
                assert_eq!(context_session, session);
                assert_eq!(context_request, request);
            }
            other => panic!("expected stale-request context, got {other:?}"),
        },
        other => panic!("expected structured stale-request event, got {other:?}"),
    }
}
