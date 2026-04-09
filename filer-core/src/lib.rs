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
pub use api::{commands::Command, events::Event, handle::FilerCore};
pub use api::module::Module;
pub use errors::CoreError;
pub use model::node::{FileNode, NodeMeta};

// Services
pub use services::metadata::{ExtendedMetadata, MetadataRegistry};
pub use services::mime::{MimeCategory, MimeDetector, MimeInfo};
pub use services::preview::{ArchivePreviewEntry, PreviewData, PreviewOptions, PreviewRegistry};

// Crypto (feature-gated)
#[cfg(feature = "crypto")]
pub use services::crypto::{Cipher, CipherAlgorithm, Vault, VaultConfig};

// VFS providers
pub use vfs::local::LocalFs;
pub use vfs::provider::{Capabilities, FsProvider};

// Actor infrastructure
pub use actors::Actor;

// Pipeline types
pub use pipeline::{PipelineConfig, SortConfig};

#[cfg(feature = "s3")]
pub use vfs::s3::{S3Fs, S3Config};

#[cfg(feature = "webdav")]
pub use vfs::webdav::{WebDavFs, WebDavConfig};

#[cfg(any(feature = "ftp", feature = "sftp"))]
pub use vfs::ftp::{FtpFs, FtpConfig};

#[cfg(feature = "fuse")]
pub use vfs::fuse::{FuseFs, FuseConfig};

#[cfg(feature = "kubernetes")]
pub use vfs::kubernetes::{K8sFs, K8sConfig};

#[cfg(test)]
mod tests;