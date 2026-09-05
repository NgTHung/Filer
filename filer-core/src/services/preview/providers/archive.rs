use std::path::Path;

use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::mime::{MimeCategory, MimeInfo};
#[cfg(feature = "metadata-archive")]
use crate::services::preview::provider::ArchivePreviewEntry;
use crate::services::preview::provider::{
    PreviewData, PreviewOptions, PreviewProvider,
};

pub struct ArchiveProvider;

impl ArchiveProvider {
    pub fn new() -> Self {
        Self
    }

    #[cfg(feature = "metadata-archive")]
    async fn list_zip(&self, path: &Path, max_entries: usize) -> Result<PreviewData, CoreError> {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            let file = std::fs::File::open(&path)
                .map_err(|e| CoreError::from_io_error(e, path.clone()))?;
            let mut archive =
                zip::ZipArchive::new(file).map_err(|e| CoreError::invalid_data(e.to_string()))?;
            let total = archive.len();
            let mut entries = Vec::new();
            for i in 0..total.min(max_entries) {
                let entry = archive
                    .by_index(i)
                    .map_err(|e| CoreError::invalid_data(e.to_string()))?;
                entries.push(ArchivePreviewEntry {
                    path: entry.name().to_string(),
                    size: entry.size(),
                    is_directory: entry.is_dir(),
                });
            }
            Ok(PreviewData::Archive {
                entries,
                total_entries: total,
                truncated: total > max_entries,
            })
        })
        .await
        .map_err(|e| CoreError::actor("archive_provider", e.to_string()))?
    }

    #[cfg(feature = "metadata-archive")]
    async fn list_tar(&self, path: &Path, max_entries: usize) -> Result<PreviewData, CoreError> {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            let file = std::fs::File::open(&path)
                .map_err(|e| CoreError::from_io_error(e, path.clone()))?;
            let mut archive = tar::Archive::new(file);
            let mut entries = Vec::new();
            let mut total = 0usize;
            for entry in archive
                .entries()
                .map_err(|e| CoreError::invalid_data(e.to_string()))?
            {
                let entry = entry.map_err(|e| CoreError::invalid_data(e.to_string()))?;
                total += 1;
                if entries.len() < max_entries {
                    let name = entry
                        .path()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|_| "<invalid>".to_string());
                    let size = entry.header().size().unwrap_or(0);
                    let is_directory =
                        matches!(entry.header().entry_type(), tar::EntryType::Directory);
                    entries.push(ArchivePreviewEntry {
                        path: name,
                        size,
                        is_directory,
                    });
                }
            }
            Ok(PreviewData::Archive {
                truncated: total > max_entries,
                total_entries: total,
                entries,
            })
        })
        .await
        .map_err(|e| CoreError::actor("archive_provider", e.to_string()))?
    }
}

#[async_trait]
impl PreviewProvider for ArchiveProvider {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Archive]
    }

    async fn generate(
        &self,
        path: &Path,
        mime: &MimeInfo,
        options: &PreviewOptions,
    ) -> Result<PreviewData, CoreError> {
        let max_entries = (options.max_bytes / 64).max(10);

        #[cfg(feature = "metadata-archive")]
        {
            let mt = mime.mime_type.as_str();
            if mt == "application/zip" || mt == "application/x-zip-compressed" {
                return self.list_zip(path, max_entries).await;
            }
            if mt == "application/x-tar"
                || mt == "application/gzip"
                || mt == "application/x-bzip2"
                || mt == "application/x-xz"
                || mt == "application/zstd"
                || mt == "application/x-7z-compressed"
            {
                return self.list_tar(path, max_entries).await;
            }
        }

        let _ = (path, mime, max_entries);
        Ok(PreviewData::Unsupported {
            mime_type: mime.mime_type.clone(),
            reason: "Archive format not supported".to_string(),
        })
    }

    fn name(&self) -> &'static str {
        "archive"
    }
}

impl Default for ArchiveProvider {
    fn default() -> Self {
        Self::new()
    }
}
