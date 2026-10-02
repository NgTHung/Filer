mod lifecycle;
mod support;
// Each test crate uses a different subset of the shared trace builders.
#[allow(dead_code, unused_imports)]
#[path = "../trace_validation/support.rs"]
mod traces;
