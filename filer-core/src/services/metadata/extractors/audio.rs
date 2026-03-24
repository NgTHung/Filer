use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::metadata::extended::{AudioMetadata, ExtendedMetadata};
use crate::services::metadata::extractor::MetadataExtractor;
use crate::services::mime::{MimeCategory, MimeInfo};
use crate::vfs::provider::FsProvider;

#[cfg(feature = "metadata-audio")]
use crate::services::metadata::extended::AudioTags;

/// Audio metadata extractor.
///
/// Uses the `id3` crate (MP3/ID3 tags). Stream-level fields (sample_rate,
/// channels, bit_rate) require a decoder crate and are returned as `None`.
/// Duration comes from the ID3 TLEN frame when present.
pub struct AudioExtractor;

impl AudioExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Parse the ID3 tag from `path`, returning duration (from TLEN if set) and
    /// all tag fields. Returns `None` if the file has no ID3 tag.
    ///
    /// `(duration_secs, sample_rate, channels, bit_rate, tags)`
    #[cfg(feature = "metadata-audio")]
    async fn read_tag(
        &self,
        path: &Path,
        provider: &dyn FsProvider,
    ) -> Option<(f64, AudioTags)> {
        use id3::TagLike;

        let mut reader = provider.open_reader(path).await.ok()?;
        let tag = id3::Tag::read_from2(&mut *reader).ok()?;

        let duration_secs = tag.duration().map(|d| d as f64).unwrap_or(0.0);

        let album_art = tag
            .pictures()
            .find(|p| p.picture_type == id3::frame::PictureType::CoverFront)
            .or_else(|| tag.pictures().next())
            .map(|p| p.data.clone());

        let tags = AudioTags {
            title:     tag.title().map(str::to_owned),
            artist:    tag.artist().map(str::to_owned),
            album:     tag.album().map(str::to_owned),
            year:      tag.year().map(|y| y as u32),
            track:     tag.track(),
            genre:     tag.genre().map(str::to_owned),
            album_art,
        };

        Some((duration_secs, tags))
    }
}

#[async_trait]
impl MetadataExtractor for AudioExtractor {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Audio]
    }

    async fn extract(
        &self,
        path: &Path,
        mime: &MimeInfo,
        provider: &dyn FsProvider,
    ) -> Result<ExtendedMetadata, CoreError> {
        #[cfg(not(feature = "metadata-audio"))]
        return Ok(ExtendedMetadata::Unavailable);

        #[cfg(feature = "metadata-audio")]
        {
            let format = match &*mime.mime_type {
                "audio/mpeg"  => "MP3",
                "audio/ogg"   => "OGG",
                "audio/flac"  => "FLAC",
                "audio/wav"   => "WAV",
                "audio/aac"   => "AAC",
                "audio/mp4"   => "M4A",
                "audio/x-m4a" => "M4A",
                "audio/aiff"  => "AIFF",
                "audio/opus"  => "Opus",
                _             => "Unknown",
            };

            let (duration_secs, tags) = self
                .read_tag(path, provider)
                .await
                .unwrap_or((0.0, AudioTags::default()));

            Ok(ExtendedMetadata::Audio(AudioMetadata {
                duration_secs,
                sample_rate: None, // requires decoder crate (e.g. symphonia)
                channels:    None,
                bit_rate:    None,
                format:      format.to_string(),
                tags,
            }))
        }
    }

    fn name(&self) -> &'static str {
        "audio"
    }
}

impl Default for AudioExtractor {
    fn default() -> Self {
        Self::new()
    }
}
