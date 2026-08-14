use crate::actors::Actor;
use crate::model::location::{Location, LocationDescriptor, LocationRef};
// API-007 pin: navigator internals still exercise NodeId state until those
// internals are retired.
use crate::model::node::NodeId;
use crate::model::session::SessionId;
use crate::modules::navigation::navigator::{NavCommand, NavState, Navigator, NavigatorState};
use crate::pipeline::PipelineConfig;
use std::time::Duration;
use tokio::time::timeout;

include!("navigator_state_tests.rs");

include!("nav_state_serialization_tests.rs");

#[cfg(test)]
mod navigator_actor_tests {
    include!("navigator_actor_tests.rs");
    include!("navigator_multiple_sessions.rs");
}
