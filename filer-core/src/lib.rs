pub mod api;
pub mod model;
pub mod services;
pub mod utils;

mod actors;
mod bus;
mod errors;
mod pipeline;
mod vfs;

pub use api::{commands::Command as Command, events::Event as Event, handle::FilerCore as FilerCore};
pub use errors::CoreError;
pub use model::node::FileNode;

pub use services::metadata::{BasicMetadata, ExtendedMetadata, MetadataRegistry};
pub use services::mime::{MimeCategory, MimeDetector, MimeInfo};
pub use services::preview::{PreviewData, PreviewOptions, PreviewRegistry};