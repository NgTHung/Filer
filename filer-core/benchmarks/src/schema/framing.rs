//! # Protocol framing
//!
//! Framing enforces one newline-terminated request and one JSON object per
//! event line before conversion applies schema and closed-value checks.

use serde::de::DeserializeOwned;

use super::event::RawEvent;
use super::request::RawRequest;
use super::validation::malformed;
use super::{Event, Request};
use crate::{ErrorCode, ErrorContext, ProtocolError};

pub fn parse_request_bytes(bytes: &[u8]) -> Result<Request, ProtocolError> {
    let raw: RawRequest = parse_frame(bytes, FrameKind::Request)?;
    super::request::convert(raw)
}

pub fn parse_event_line(bytes: &[u8]) -> Result<Event, ProtocolError> {
    let raw: RawEvent = parse_frame(bytes, FrameKind::Event)?;
    super::event::convert(raw)
}

pub fn parse_event_lines(bytes: &[u8]) -> Result<Vec<Event>, ProtocolError> {
    if std::str::from_utf8(bytes).is_err() {
        return Err(malformed("event stream is not UTF-8"));
    }
    if bytes.is_empty() || !bytes.ends_with(b"\n") {
        return Err(malformed(
            "event stream is truncated or missing its final newline",
        ));
    }
    let mut events = Vec::new();
    for (index, line) in bytes.split_inclusive(|byte| *byte == b'\n').enumerate() {
        events.push(parse_event_line(line).map_err(|error| error.with_line(index + 1))?);
    }
    Ok(events)
}

#[derive(Clone, Copy)]
enum FrameKind {
    Request,
    Event,
}

fn parse_frame<T>(bytes: &[u8], kind: FrameKind) -> Result<T, ProtocolError>
where
    T: DeserializeOwned,
{
    if std::str::from_utf8(bytes).is_err() {
        return Err(malformed("frame is not UTF-8"));
    }
    if bytes.is_empty() || !bytes.ends_with(b"\n") {
        return Err(malformed("frame is not newline terminated"));
    }
    let body = &bytes[..bytes.len() - 1];
    let body = body.strip_suffix(b"\r").unwrap_or(body);
    if body.is_empty() || body.iter().any(|byte| *byte == b'\n' || *byte == b'\r') {
        return Err(malformed("frame must contain exactly one JSON object"));
    }
    if !body.iter().any(|byte| !byte.is_ascii_whitespace()) {
        return Err(malformed("frame is empty"));
    }
    let first = body
        .iter()
        .find(|byte| !byte.is_ascii_whitespace())
        .copied();
    if matches!(kind, FrameKind::Event) && first != Some(b'{') {
        return Err(ProtocolError::new(
            ErrorCode::UnexpectedStdout,
            "standard output line is not a JSON event object",
        ));
    }
    serde_json::from_slice(body).map_err(|error| {
        let (code, message) = match error.classify() {
            serde_json::error::Category::Data => (
                ErrorCode::InvalidSchema,
                "JSON object does not match the protocol schema",
            ),
            serde_json::error::Category::Syntax | serde_json::error::Category::Eof => (
                ErrorCode::MalformedJson,
                "frame is not one valid JSON object",
            ),
            serde_json::error::Category::Io => {
                (ErrorCode::MalformedJson, "frame could not be read")
            }
        };
        ProtocolError::new(code, message).with_context(ErrorContext {
            field: Some(error.to_string()),
            ..ErrorContext::default()
        })
    })
}
