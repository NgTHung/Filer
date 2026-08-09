//! Tests for the Command Router
//!
//! The Command Router is a generic dispatcher backed by HandlerRegistry.
//! It receives `Command` from the API layer and:
//! 1. Validates the session via SessionManager
//! 2. Looks up the handler by `Command::key()` in the HandlerRegistry
//! 3. Calls the handler closure (which forwards to the appropriate actor channel)
//! 4. For `DestroySession`, runs registered destroy hooks
//!
//! Session lifecycle (Handshake / DestroySession) is registered as normal
//! handlers — the Router has no special knowledge of them.
//!
//! Tests are written BEFORE implementation (TDD).

#[cfg(test)]
mod command_router_tests {
    include!("harness.rs");
    include!("register_handlers.rs");
    include!("route_compatibility.rs");
    include!("route_compatibility_operations.rs");
    include!("route_navigation.rs");
    include!("route_unwatch_session_to_watcher.rs");
    include!("route_handshake_emits_session_created.rs");
    include!("route_unresolved_node_boundaries.rs");
}
