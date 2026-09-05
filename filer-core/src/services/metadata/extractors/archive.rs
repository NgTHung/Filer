use async_trait::async_trait;
#[cfg(feature = "metadata-archive")]
use std::io::Read;
use std::path::Path;

use crate::errors::CoreError;
use crate::services::metadata::extended::ExtendedMetadata;
#[cfg(feature = "metadata-archive")]
use crate::services::metadata::extended::{ArchiveEntry, ArchiveMetadata};
use crate::services::metadata::extractor::MetadataExtractor;
use crate::services::mime::{MimeCategory, MimeInfo};
use crate::vfs::context::ProviderCx;
use crate::vfs::provider::FsProvider;

/// Archive metadata extractor (file count, sizes, compression ratio, entry listing).
///
/// Supported formats: ZIP, TAR, TAR+GZ, TAR+BZ2, TAR+XZ, TAR+ZSTD, GZ, BZ2, XZ, ZSTD, 7Z.
/// RAR requires the `metadata-archive-rar` feature (links unrar C++).
pub struct ArchiveExtractor;

impl ArchiveExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Returns `true` when the filename suffix indicates a compressed tarball.
    #[cfg(feature = "metadata-archive")]
    fn is_tarball(path: &Path) -> bool {
        path.file_name()
            .and_then(|n| n.to_str())
            .map(|name| {
                name.ends_with(".tar.gz")
                    || name.ends_with(".tgz")
                    || name.ends_with(".tar.bz2")
                    || name.ends_with(".tbz2")
                    || name.ends_with(".tar.xz")
                    || name.ends_with(".txz")
                    || name.ends_with(".tar.zst")
                    || name.ends_with(".tzst")
            })
            .unwrap_or(false)
    }

    /// Build `ArchiveMetadata` from an entry list.
    /// When `compressed_total` is 0, falls back to summing `entry.compressed_size`.
    #[cfg(feature = "metadata-archive")]
    fn build(format: &str, entries: Vec<ArchiveEntry>, compressed_total: u64) -> ArchiveMetadata {
        let file_count = entries.iter().filter(|e| !e.is_directory).count();
        let total_size: u64 = entries.iter().map(|e| e.size).sum();
        let compressed_size = if compressed_total > 0 {
            compressed_total
        } else {
            entries.iter().map(|e| e.compressed_size).sum()
        };
        let compression_ratio = if total_size > 0 && compressed_size > 0 {
            compressed_size as f32 / total_size as f32
        } else {
            0.0
        };
        ArchiveMetadata {
            format: format.to_string(),
            file_count,
            total_size,
            compressed_size,
            compression_ratio,
            entries,
        }
    }

    /// Parse a ZIP archive (needs `Read + Seek`).
    #[cfg(feature = "metadata-archive")]
    fn parse_zip<R: Read + std::io::Seek>(reader: R) -> Result<Vec<ArchiveEntry>, CoreError> {
        let mut archive = zip::ZipArchive::new(reader)
            .map_err(|e| CoreError::invalid_data(format!("Cannot open ZIP: {e}")))?;
        let mut entries = Vec::with_capacity(archive.len());
        for i in 0..archive.len() {
            let f = archive
                .by_index(i)
                .map_err(|e| CoreError::invalid_data(format!("ZIP entry {i}: {e}")))?;
            entries.push(ArchiveEntry {
                path: f.name().to_owned(),
                size: f.size(),
                compressed_size: f.compressed_size(),
                is_directory: f.is_dir(),
            });
        }
        Ok(entries)
    }

    /// Parse a TAR archive from any `Read` source (plain, or pre-wrapped decoder).
    /// Compressed sizes are not stored in TAR — they are reported as 0.
    #[cfg(feature = "metadata-archive")]
    fn parse_tar<R: Read>(reader: R) -> Result<Vec<ArchiveEntry>, CoreError> {
        let mut archive = tar::Archive::new(reader);
        let mut entries = Vec::new();
        for entry in archive
            .entries()
            .map_err(|e| CoreError::invalid_data(format!("Cannot read TAR: {e}")))?
        {
            let e = entry.map_err(|e| CoreError::invalid_data(format!("TAR entry: {e}")))?;
            let h = e.header();
            entries.push(ArchiveEntry {
                path: h
                    .path()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                size: h.size().unwrap_or(0),
                compressed_size: 0, // TAR does not store per-entry compressed sizes
                is_directory: h.entry_type().is_dir(),
            });
        }
        Ok(entries)
    }

    /// Parse a 7-zip archive (needs `Read + Seek`).
    #[cfg(feature = "metadata-archive")]
    fn parse_7z<R: Read + std::io::Seek>(reader: R) -> Result<Vec<ArchiveEntry>, CoreError> {
        use sevenz_rust2::{ArchiveReader, Password};
        let mut reader_7z = ArchiveReader::new(reader, Password::empty())
            .map_err(|e| CoreError::invalid_data(format!("Cannot open 7z: {e:?}")))?;
        let mut entries = Vec::new();
        reader_7z
            .for_each_entries(|entry, _| {
                entries.push(ArchiveEntry {
                    path: entry.name().to_owned(),
                    size: entry.size(),
                    compressed_size: entry.compressed_size,
                    is_directory: entry.is_directory(),
                });
                Ok(true)
            })
            .map_err(|e| CoreError::invalid_data(format!("7z entry error: {e:?}")))?;
        Ok(entries)
    }

    /// Decompress a single-file stream, counting bytes without allocating the
    /// full content in memory. Returns `(uncompressed_size, entry_name)`.
    #[cfg(feature = "metadata-archive")]
    fn count_decompressed<R: Read>(
        mut decoder: R,
        entry_name: String,
    ) -> Result<(u64, String), CoreError> {
        struct Sink(u64);
        impl std::io::Write for Sink {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0 += buf.len() as u64;
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut sink = Sink(0);
        std::io::copy(&mut decoder, &mut sink)
            .map_err(|e| CoreError::invalid_data(format!("Decompression error: {e}")))?;
        Ok((sink.0, entry_name))
    }
}

#[async_trait]
impl MetadataExtractor for ArchiveExtractor {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Archive]
    }

    async fn extract(
        &self,
        path: &Path,
        mime: &MimeInfo,
        provider: &dyn FsProvider,
        cx: &ProviderCx<'_>,
    ) -> Result<ExtendedMetadata, CoreError> {
        #[cfg(not(feature = "metadata-archive"))]
        {
            let _ = (path, mime, provider, cx);
            Ok(ExtendedMetadata::Unavailable)
        }

        #[cfg(feature = "metadata-archive")]
        {
            use bzip2::read::BzDecoder;
            use flate2::read::GzDecoder;
            use xz2::read::XzDecoder;

            let tarball = Self::is_tarball(path);
            // Stem name for single-file compressed formats (e.g. "data.txt.gz" → "data.txt").
            let stem = || {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("file")
                    .to_owned()
            };

            let meta = match &*mime.mime_type {
                "application/zip" => {
                    let entries = Self::parse_zip(provider.open_reader(path, cx).await?)?;
                    Self::build("ZIP", entries, 0)
                }

                "application/x-tar" => {
                    let entries = Self::parse_tar(provider.open_reader(path, cx).await?)?;
                    Self::build("TAR", entries, 0)
                }

                "application/gzip" if tarball => {
                    let entries =
                        Self::parse_tar(GzDecoder::new(provider.open_reader(path, cx).await?))?;
                    Self::build("TAR+GZ", entries, 0)
                }
                "application/gzip" => {
                    let compressed = provider.read(path, cx).await?;
                    let compressed_size = compressed.len() as u64;
                    let (size, name) = Self::count_decompressed(
                        GzDecoder::new(std::io::Cursor::new(compressed)),
                        stem(),
                    )?;
                    Self::build(
                        "GZ",
                        vec![ArchiveEntry {
                            path: name,
                            size,
                            compressed_size,
                            is_directory: false,
                        }],
                        compressed_size,
                    )
                }

                "application/x-bzip2" if tarball => {
                    let entries =
                        Self::parse_tar(BzDecoder::new(provider.open_reader(path, cx).await?))?;
                    Self::build("TAR+BZ2", entries, 0)
                }
                "application/x-bzip2" => {
                    let compressed = provider.read(path, cx).await?;
                    let compressed_size = compressed.len() as u64;
                    let (size, name) = Self::count_decompressed(
                        BzDecoder::new(std::io::Cursor::new(compressed)),
                        stem(),
                    )?;
                    Self::build(
                        "BZ2",
                        vec![ArchiveEntry {
                            path: name,
                            size,
                            compressed_size,
                            is_directory: false,
                        }],
                        compressed_size,
                    )
                }

                "application/x-xz" if tarball => {
                    let entries =
                        Self::parse_tar(XzDecoder::new(provider.open_reader(path, cx).await?))?;
                    Self::build("TAR+XZ", entries, 0)
                }
                "application/x-xz" => {
                    let compressed = provider.read(path, cx).await?;
                    let compressed_size = compressed.len() as u64;
                    let (size, name) = Self::count_decompressed(
                        XzDecoder::new(std::io::Cursor::new(compressed)),
                        stem(),
                    )?;
                    Self::build(
                        "XZ",
                        vec![ArchiveEntry {
                            path: name,
                            size,
                            compressed_size,
                            is_directory: false,
                        }],
                        compressed_size,
                    )
                }

                "application/zstd" if tarball => {
                    let decoder =
                        zstd::stream::read::Decoder::new(provider.open_reader(path, cx).await?)
                            .map_err(|e| CoreError::invalid_data(format!("ZSTD: {e}")))?;
                    let entries = Self::parse_tar(decoder)?;
                    Self::build("TAR+ZSTD", entries, 0)
                }
                "application/zstd" => {
                    let compressed = provider.read(path, cx).await?;
                    let compressed_size = compressed.len() as u64;
                    let decoder =
                        zstd::stream::read::Decoder::new(std::io::Cursor::new(compressed))
                            .map_err(|e| CoreError::invalid_data(format!("ZSTD: {e}")))?;
                    let (size, name) = Self::count_decompressed(decoder, stem())?;
                    Self::build(
                        "ZSTD",
                        vec![ArchiveEntry {
                            path: name,
                            size,
                            compressed_size,
                            is_directory: false,
                        }],
                        compressed_size,
                    )
                }

                "application/x-7z-compressed" => {
                    let entries = Self::parse_7z(provider.open_reader(path, cx).await?)?;
                    Self::build("7Z", entries, 0)
                }

                #[cfg(feature = "metadata-archive-rar")]
                "application/vnd.rar" => {
                    let mut archive = unrar::Archive::new(path)
                        .open_for_listing()
                        .map_err(|e| CoreError::invalid_data(format!("Cannot open RAR: {e}")))?;
                    let mut entries = Vec::new();
                    for header in archive.by_ref() {
                        let h = header
                            .map_err(|e| CoreError::invalid_data(format!("RAR entry: {e}")))?;
                        entries.push(ArchiveEntry {
                            path: h.filename.to_string_lossy().into_owned(),
                            size: h.unpacked_size,
                            compressed_size: 0, // unrar crate does not expose packed size
                            is_directory: h.is_directory(),
                        });
                    }
                    Self::build("RAR", entries, 0)
                }

                _ => ArchiveMetadata {
                    format: "Unknown".to_string(),
                    file_count: 0,
                    total_size: 0,
                    compressed_size: 0,
                    compression_ratio: 0.0,
                    entries: Vec::new(),
                },
            };

            Ok(ExtendedMetadata::Archive(meta))
        }
    }

    fn name(&self) -> &'static str {
        "archive"
    }
}

impl Default for ArchiveExtractor {
    fn default() -> Self {
        Self::new()
    }
}
