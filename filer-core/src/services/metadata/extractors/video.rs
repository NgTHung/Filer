use async_trait::async_trait;
use std::path::Path;

use crate::errors::CoreError;
use crate::services::metadata::extended::{ExtendedMetadata, VideoMetadata};
use crate::services::metadata::extractor::MetadataExtractor;
use crate::services::mime::{MimeCategory, MimeInfo};
use crate::vfs::context::ProviderCx;
use crate::vfs::provider::FsProvider;

/// Video metadata extractor (dimensions, duration, codecs).
///
/// Uses `mp4parse` — supports MP4/M4V/MOV containers only.
/// Non-MP4 formats (MKV, AVI, WebM) will parse with zero dimensions/duration
/// but still return the correct format string derived from the MIME type.
pub struct VideoExtractor;

impl VideoExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Map a `CodecType` to a human-readable codec name.
    #[cfg(feature = "metadata-video")]
    fn codec_name(codec: mp4parse::CodecType) -> Option<String> {
        use mp4parse::CodecType;
        match codec {
            CodecType::H264 => Some("H.264".into()),
            CodecType::AV1 => Some("AV1".into()),
            CodecType::VP9 => Some("VP9".into()),
            CodecType::VP8 => Some("VP8".into()),
            CodecType::MP4V => Some("MPEG-4".into()),
            CodecType::H263 => Some("H.263".into()),
            CodecType::AAC => Some("AAC".into()),
            CodecType::MP3 => Some("MP3".into()),
            CodecType::Opus => Some("Opus".into()),
            CodecType::FLAC => Some("FLAC".into()),
            CodecType::ALAC => Some("ALAC".into()),
            CodecType::LPCM => Some("LPCM".into()),
            CodecType::EncryptedVideo | CodecType::EncryptedAudio => Some("Encrypted".into()),
            _ => None,
        }
    }

    /// Parse an MP4/MOV container and extract all video metadata.
    ///
    /// Returns `Err` only on I/O failure; unrecognised/non-MP4 containers
    /// produce a zeroed `VideoMetadata` rather than an error.
    #[cfg(feature = "metadata-video")]
    async fn parse_mp4(
        &self,
        path: &Path,
        provider: &dyn FsProvider,
        format: &str,
        cx: &ProviderCx<'_>,
    ) -> Result<VideoMetadata, CoreError> {
        use mp4parse::{SampleEntry, TrackType, read_mp4};

        // read_mp4 requires T: Sized, so we can't pass a trait object directly.
        // Buffer the file and wrap in Cursor for a concrete Sized type.
        let bytes = provider.read(path, cx).await?;
        let mut cursor = std::io::Cursor::new(bytes);
        let context = read_mp4(&mut cursor)
            .map_err(|e| CoreError::invalid_data(format!("Cannot parse MP4: {e:?}")))?;

        let video_track = context
            .tracks
            .iter()
            .find(|t| t.track_type == TrackType::Video);
        let audio_track = context
            .tracks
            .iter()
            .find(|t| t.track_type == TrackType::Audio);

        // Dimensions + video codec from the first video sample entry.
        let (width, height, video_codec) = video_track
            .and_then(|t| t.stsd.as_ref())
            .and_then(|stsd| stsd.descriptions.first())
            .and_then(|entry| match entry {
                SampleEntry::Video(v) => Some((
                    v.width as u32,
                    v.height as u32,
                    Self::codec_name(v.codec_type),
                )),
                _ => None,
            })
            .unwrap_or((0, 0, None));

        // Audio codec from the first audio sample entry.
        let audio_codec = audio_track
            .and_then(|t| t.stsd.as_ref())
            .and_then(|stsd| stsd.descriptions.first())
            .and_then(|entry| match entry {
                SampleEntry::Audio(a) => Self::codec_name(a.codec_type),
                _ => None,
            });

        // Duration in seconds; prefer video track, fall back to audio.
        let duration_secs = video_track
            .or(audio_track)
            .and_then(|t| {
                let ticks = t.duration?.0 as f64;
                let scale = t.timescale?.0 as f64;
                if scale == 0.0 {
                    None
                } else {
                    Some(ticks / scale)
                }
            })
            .unwrap_or(0.0);

        Ok(VideoMetadata {
            width,
            height,
            duration_secs,
            frame_rate: None, // requires reading the stts sample table
            video_codec,
            audio_codec,
            format: format.to_string(),
        })
    }
}

#[async_trait]
impl MetadataExtractor for VideoExtractor {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Video]
    }

    async fn extract(
        &self,
        path: &Path,
        mime: &MimeInfo,
        provider: &dyn FsProvider,
        cx: &ProviderCx<'_>,
    ) -> Result<ExtendedMetadata, CoreError> {
        #[cfg(not(feature = "metadata-video"))]
        return Ok(ExtendedMetadata::Unavailable);

        #[cfg(feature = "metadata-video")]
        {
            let format = match &*mime.mime_type {
                "video/mp4" => "MP4",
                "video/quicktime" => "MOV",
                "video/x-m4v" => "M4V",
                "video/x-msvideo" => "AVI",
                "video/x-matroska" => "MKV",
                "video/webm" => "WebM",
                "video/ogg" => "OGV",
                "video/mpeg" => "MPEG",
                "video/3gpp" => "3GP",
                "video/3gpp2" => "3G2",
                _ => "Unknown",
            };

            // Non-MP4 containers won't parse; return zeroed metadata rather than error.
            let metadata =
                self.parse_mp4(path, provider, format, cx)
                    .await
                    .unwrap_or(VideoMetadata {
                        width: 0,
                        height: 0,
                        duration_secs: 0.0,
                        frame_rate: None,
                        video_codec: None,
                        audio_codec: None,
                        format: format.to_string(),
                    });

            Ok(ExtendedMetadata::Video(metadata))
        }
    }

    fn name(&self) -> &'static str {
        "video"
    }
}

impl Default for VideoExtractor {
    fn default() -> Self {
        Self::new()
    }
}
