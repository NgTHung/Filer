pub mod archive;
pub mod local;
pub mod provider;

#[cfg(any(feature = "s3", feature = "webdav", feature = "ftp", feature = "sftp", feature = "kubernetes"))]
pub mod remote;

// Remote providers (feature-gated)
#[cfg(feature = "s3")]
pub mod s3;

#[cfg(feature = "webdav")]
pub mod webdav;

#[cfg(any(feature = "ftp", feature = "sftp"))]
pub mod ftp;

#[cfg(feature = "fuse")]
pub mod fuse;

#[cfg(feature = "kubernetes")]
pub mod kubernetes;