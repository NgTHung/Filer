use std::error::Error;
use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use std::path::PathBuf;

use filer_core::api::session_manager::SendError;
use filer_core::model::query::SearchQuery;
use filer_core::model::session::SessionId;
use filer_core::{CoreError, ErrorCode, ErrorTarget, EventSendError, WireCommandConversionError};

#[test]
fn errors_without_causes_preserve_messages_and_have_no_source() {
    let cases: Vec<(Box<dyn Error>, &str)> = vec![
        (
            Box::new(CoreError::invalid_input("invalid request")),
            "invalid request",
        ),
        (
            Box::new(SendError::SessionNotFound(SessionId(7))),
            "session not found: session:7",
        ),
        (Box::new(SendError::ChannelClosed), "channel closed"),
        (Box::new(EventSendError::Closed), "event sink is closed"),
        (
            Box::new(WireCommandConversionError::ExtensionUnsupported {
                key: "git.status".to_string(),
            }),
            "extension command 'git.status' has no wire-safe payload",
        ),
        (
            Box::new(SearchQuery::parse(" ").expect_err("empty query must fail")),
            "query parse error at 0: Empty query",
        ),
        (
            Box::new(SearchQuery::parse("report name:").expect_err("empty name must fail")),
            "query parse error at 7: Empty name: value",
        ),
    ];

    for (error, message) in cases {
        assert_eq!(error.to_string(), message);
        assert!(error.source().is_none(), "{message} must have no source");
    }
}

#[test]
fn core_error_preserves_typed_sources_through_nested_errors() {
    let path = PathBuf::from("/missing/file.txt");
    let io_error = IoError::new(IoErrorKind::NotFound, "missing on disk");
    let inner = CoreError::from_io_error(io_error, path.clone());
    let outer = CoreError::actor("scanner", "scan failed").with_source(inner);

    assert_eq!(outer.to_string(), "scan failed");
    let inner = outer
        .source()
        .and_then(|source| source.downcast_ref::<CoreError>())
        .expect("the immediate cause must retain its CoreError type");
    assert_eq!(inner.code(), ErrorCode::PathNotFound);
    assert_eq!(inner.target(), Some(&ErrorTarget::Path(path)));
    assert_eq!(inner.to_string(), "Not found: /missing/file.txt");
    assert!(inner.recoverable());

    let io_error = inner
        .source()
        .and_then(|source| source.downcast_ref::<IoError>())
        .expect("the underlying I/O error must retain its type");
    assert_eq!(io_error.kind(), IoErrorKind::NotFound);
    assert_eq!(io_error.to_string(), "missing on disk");
    assert!(io_error.source().is_none());
}

#[test]
fn channel_conversion_preserves_core_classification() {
    let error = CoreError::from(flume::SendError("undelivered message"));

    assert_eq!(error.code(), ErrorCode::ChannelClosed);
    assert_eq!(error.target(), Some(&ErrorTarget::Channel("send")));
    assert_eq!(error.to_string(), "Channel closed: send");
    assert!(!error.recoverable());
    assert!(error.source().is_none());
}
