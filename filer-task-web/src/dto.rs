//! # Request Bodies
//!
//! `reason` is required, so a block/defer/obsolete request without one fails
//! deserialization and never reaches the handler.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ReasonRequest {
    pub reason: String,
}
