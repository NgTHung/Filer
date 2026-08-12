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
pub use api::event_sink::{DEFAULT_EVENT_CHANNEL_CAPACITY, EventSendError, EventSink};
pub use api::module::Module;
pub use api::wire_commands::{WireCommand, WireCommandConversionError};
pub use api::{commands::Command, events::Event, handle::FilerCore};
pub use errors::{CoreError, ErrorCode, ErrorContext, ErrorKind, ErrorTarget};
pub use model::cancel::CancelSignal;
pub use model::capability::{
    LocationCapabilityError, LocationOperationCapability, LocationWatchCapability,
    LocationWatchReliability, operation_capability_for_location, watch_capability_for_location,
};
pub use model::directory::{
    DEFAULT_DIRECTORY_PAGE_SIZE, DirectoryCursor, DirectoryLoadMode, DirectoryLoadOptions,
    DirectoryLoadState, DirectoryPageRequest, DirectoryPageResult, DirectoryPageState,
};
pub use model::location::{
    Location, LocationDescriptor, LocationId, LocationRef, LocationRoute, LocationSegment,
    ProviderRef,
};
pub use model::node::{FileNode, NodeEntry, NodeEntryCapabilities, NodeMeta};
pub use model::operation::{
    OperationConflictPolicy, OperationConflictResolution, OperationId, OperationKind,
    OperationProviderGuarantee, OperationUndoMode, OperationUndoRecord,
};
pub use model::progress::{
    ProgressKind, ProgressPhase, ProgressScope, ProgressSnapshot, ProgressStatus, ProgressTarget,
    ProgressUnit,
};
pub use model::request::RequestId;

// Services
pub use services::metadata::{ExtendedMetadata, MetadataRegistry};
pub use services::mime::{MimeCategory, MimeDetector, MimeInfo};
pub use services::preview::{ArchivePreviewEntry, PreviewData, PreviewOptions, PreviewRegistry};

// VFS providers
pub use vfs::archive::ArchiveFs;
pub use vfs::context::ProviderCx;
pub use vfs::local::LocalFs;
pub use vfs::provider::{Capabilities, FsProvider, ListingDetail, ListingOptions, ProviderPaging};
pub use vfs::registry::{ProviderProfile, ProviderProfileId, ProviderRegistry};
pub use vfs::segmented::SegmentedLocationResolver;

// Actor infrastructure
pub use actors::{Actor, WorkTracker};

// Pipeline types
pub use pipeline::{GroupedEntries, PipelineConfig, SortConfig};

#[cfg(test)]
mod tests;
