//! # Compatibility Boundaries
//!
//! This module keeps legacy `NodeId` command handling at module edges. Actor
//! commands receive `LocationRef` so provider work starts from canonical
//! location identity.
//!
//! ```ignore
//! let location = resolve_node_location(registry, node)?;
//! send_or_warn(&actor_tx, ActorCommand::Run { location }, "module.key");
//! ```

use crate::api::events::Event;
use crate::errors::CoreError;
use crate::model::location::LocationRef;
use crate::model::node::NodeId;
use crate::model::operation::OperationId;
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::utils::channel::{SyncSend, send_or_warn};

pub(crate) fn resolve_node_location(
    registry: &NodeRegistry,
    node: NodeId,
) -> Result<LocationRef, CoreError> {
    registry
        .resolve_node_location(node)
        .ok_or_else(|| unresolved_node_error(node))
}

pub(crate) fn resolve_node_locations(
    registry: &NodeRegistry,
    nodes: impl IntoIterator<Item = NodeId>,
) -> Result<Vec<LocationRef>, CoreError> {
    nodes
        .into_iter()
        .map(|node| resolve_node_location(registry, node))
        .collect()
}

pub(crate) fn unresolved_node_error(node: NodeId) -> CoreError {
    CoreError::invalid_input(format!("Unable to resolve ID: {node:?}"))
}

pub(crate) fn emit_unresolved_node_request<S: SyncSend<Event>>(
    events: &S,
    node: NodeId,
    session: SessionId,
    request: RequestId,
    context: &'static str,
) {
    send_or_warn(
        events,
        Event::from_request_error(unresolved_node_error(node), session, request),
        context,
    );
}

pub(crate) fn emit_unresolved_node_operation<S: SyncSend<Event>>(
    events: &S,
    node: NodeId,
    session: SessionId,
    request: RequestId,
    operation: OperationId,
    context: &'static str,
) {
    send_or_warn(
        events,
        Event::from_operation_error(unresolved_node_error(node), session, request, operation),
        context,
    );
}

pub(crate) fn emit_unresolved_node_session<S: SyncSend<Event>>(
    events: &S,
    node: NodeId,
    session: SessionId,
    context: &'static str,
) {
    send_or_warn(
        events,
        Event::from_error(unresolved_node_error(node), session),
        context,
    );
}
