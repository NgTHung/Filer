use std::path::Path;

use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::mime::{MimeCategory, MimeInfo};
#[cfg(feature = "preview-image")]
use crate::services::preview::provider::ImageFormat;
use crate::services::preview::provider::{PreviewData, PreviewOptions, PreviewProvider};

pub struct ImageProvider;

impl ImageProvider {
    pub fn new() -> Self {
        Self
    }

    #[cfg(feature = "preview-image")]
    async fn generate_thumbnail(
        path: &Path,
        max_width: u32,
        max_height: u32,
    ) -> Result<PreviewData, CoreError> {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            let img = image::open(&path).map_err(|e| CoreError::invalid_data(e.to_string()))?;
            let (orig_w, orig_h) = (img.width(), img.height());
            let thumb = img.thumbnail(max_width, max_height);
            let (w, h) = (thumb.width(), thumb.height());
            let mut buf = Vec::new();
            thumb
                .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
                .map_err(|e| CoreError::invalid_data(e.to_string()))?;
            Ok(PreviewData::Image {
                data: buf,
                format: ImageFormat::Png,
                width: w,
                height: h,
                original_width: orig_w,
                original_height: orig_h,
            })
        })
        .await
        .map_err(|e| CoreError::actor("image_provider", e.to_string()))?
    }
}

#[async_trait]
impl PreviewProvider for ImageProvider {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Image]
    }

    async fn generate(
        &self,
        path: &Path,
        mime: &MimeInfo,
        options: &PreviewOptions,
    ) -> Result<PreviewData, CoreError> {
        #[cfg(feature = "preview-image")]
        {
            let _ = mime;
            Self::generate_thumbnail(path, options.max_width, options.max_height).await
        }

        #[cfg(not(feature = "preview-image"))]
        {
            let _ = (path, options);
            Ok(PreviewData::Unsupported {
                mime_type: mime.mime_type.clone(),
                reason: "Image preview feature not enabled".to_string(),
            })
        }
    }

    fn name(&self) -> &'static str {
        "image"
    }
}

impl Default for ImageProvider {
    fn default() -> Self {
        Self::new()
    }
}
