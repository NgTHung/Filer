pub mod api;
pub mod model;
pub mod modules;
pub mod services;
pub mod utils;

mod actors;
mod errors;
pub mod pipeline;
mod vfs;

// Re-exports
pub use api::module::Module;
pub use api::{commands::Command, events::Event, handle::FilerCore};
pub use errors::{CoreError, ErrorCode, ErrorKind, ErrorTarget};
pub use model::directory::{DirectoryLoadOptions, DirectoryLoadState};
pub use model::location::{
    Location, LocationDescriptor, LocationId, LocationRef, LocationRoute, LocationSegment,
    ProviderRef,
};
pub use model::node::{FileNode, NodeEntry, NodeMeta};
pub use model::operation::OperationId;
pub use model::request::RequestId;

// Services
pub use services::metadata::{ExtendedMetadata, MetadataRegistry};
pub use services::mime::{MimeCategory, MimeDetector, MimeInfo};
pub use services::preview::{ArchivePreviewEntry, PreviewData, PreviewOptions, PreviewRegistry};

// VFS providers
pub use vfs::local::LocalFs;
pub use vfs::provider::{Capabilities, FsProvider, ListingDetail, ListingOptions};

// Actor infrastructure
pub use actors::Actor;

// Pipeline types
pub use pipeline::{GroupedEntries, PipelineConfig, SortConfig};

#[cfg(feature = "s3")]
pub use vfs::s3::{S3Config, S3Fs};

#[cfg(feature = "webdav")]
pub use vfs::webdav::{WebDavConfig, WebDavFs};

#[cfg(any(feature = "ftp", feature = "sftp"))]
pub use vfs::ftp::{FtpConfig, FtpFs};

#[cfg(feature = "fuse")]
pub use vfs::fuse::{FuseConfig, FuseFs};

#[cfg(feature = "kubernetes")]
pub use vfs::kubernetes::{K8sConfig, K8sFs};

#[cfg(test)]
mod tests;
