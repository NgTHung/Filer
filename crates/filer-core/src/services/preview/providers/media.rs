use std::path::Path;

use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::mime::{MimeCategory, MimeInfo};
use crate::services::preview::provider::{PreviewData, PreviewOptions, PreviewProvider};

pub struct MediaProvider;

impl MediaProvider {
    pub fn new() -> Self {
        Self
    }

    #[cfg(feature = "metadata-audio")]
    async fn extract_audio(&self, path: &Path) -> Result<PreviewData, CoreError> {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            use id3::TagLike;
            let tag = id3::Tag::read_from_path(&path).unwrap_or_default();
            let duration_secs = tag.duration().map(|d| d as f64).unwrap_or(0.0);
            Ok(PreviewData::Audio {
                waveform: None,
                album_art: None,
                duration_secs,
            })
        })
        .await
        .map_err(|e| CoreError::actor("media_provider", e.to_string()))?
    }

    #[cfg(feature = "metadata-video")]
    async fn extract_video(&self, path: &Path) -> Result<PreviewData, CoreError> {
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;

        tokio::task::spawn_blocking(move || {
            let mut cursor = std::io::Cursor::new(bytes);
            let context = mp4parse::read_mp4(&mut cursor)
                .map_err(|e| CoreError::invalid_data(format!("{e:?}")))?;

            let duration_secs = context
                .tracks
                .iter()
                .filter_map(|t| {
                    let d = t.duration?;
                    let timescale = t.timescale?.0;
                    if timescale == 0 {
                        return None;
                    }
                    Some(d.0 as f64 / timescale as f64)
                })
                .fold(0f64, f64::max);

            Ok(PreviewData::Video {
                thumbnails: vec![],
                duration_secs,
            })
        })
        .await
        .map_err(|e| CoreError::actor("media_provider", e.to_string()))?
    }
}

#[async_trait]
impl PreviewProvider for MediaProvider {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Audio, MimeCategory::Video]
    }

    async fn generate(
        &self,
        path: &Path,
        mime: &MimeInfo,
        _options: &PreviewOptions,
    ) -> Result<PreviewData, CoreError> {
        match mime.category {
            MimeCategory::Audio => {
                #[cfg(feature = "metadata-audio")]
                return self.extract_audio(path).await;
                #[cfg(not(feature = "metadata-audio"))]
                let _ = path;
            }
            MimeCategory::Video => {
                #[cfg(feature = "metadata-video")]
                return self.extract_video(path).await;
                #[cfg(not(feature = "metadata-video"))]
                let _ = path;
            }
            _ => {}
        }

        Ok(PreviewData::Unsupported {
            mime_type: mime.mime_type.clone(),
            reason: "Media feature not enabled".to_string(),
        })
    }

    fn name(&self) -> &'static str {
        "media"
    }
}

impl Default for MediaProvider {
    fn default() -> Self {
        Self::new()
    }
}
