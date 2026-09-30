use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::model::location::LocationRef;
use crate::model::operation::{OperationId, OperationKind};
use crate::model::request::RequestId;
use crate::model::session::SessionId;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProgressKind {
    Scan,
    Operation(OperationKind),
    Search,
    Preview,
    Metadata,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProgressScope {
    pub kind: ProgressKind,
    pub session: SessionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<RequestId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<OperationId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProgressStatus {
    Started,
    Running,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProgressPhase {
    CacheLookup,
    Loading,
    Registering,
    Processing,
    Emitting,
    Finalizing,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProgressUnit {
    Entry,
    Item,
    Byte,
    Step,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProgressTarget {
    Location(LocationRef),
    Path(PathBuf),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressSnapshot {
    pub status: ProgressStatus,
    pub phase: ProgressPhase,
    pub unit: ProgressUnit,
    pub done: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<ProgressTarget>,
}

impl ProgressScope {
    pub const fn scan(session: SessionId, request: RequestId) -> Self {
        Self {
            kind: ProgressKind::Scan,
            session,
            request: Some(request),
            operation: None,
        }
    }

    pub const fn operation(
        kind: OperationKind,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    ) -> Self {
        Self {
            kind: ProgressKind::Operation(kind),
            session,
            request: Some(request),
            operation: Some(operation),
        }
    }
}

impl ProgressSnapshot {
    pub const fn new(
        status: ProgressStatus,
        phase: ProgressPhase,
        unit: ProgressUnit,
        done: usize,
        total: Option<usize>,
        current: Option<ProgressTarget>,
    ) -> Self {
        Self {
            status,
            phase,
            unit,
            done,
            total,
            current,
        }
    }
}
