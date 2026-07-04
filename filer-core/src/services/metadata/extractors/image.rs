use async_trait::async_trait;
use std::path::Path;

use crate::errors::CoreError;
use crate::services::metadata::ImageMetadata;
use crate::services::metadata::extended::ExtendedMetadata;
use crate::services::metadata::extractor::MetadataExtractor;
use crate::services::mime::{MimeCategory, MimeInfo};
use crate::vfs::context::ProviderCx;
use crate::vfs::provider::FsProvider;

#[cfg(feature = "metadata-image")]
use crate::services::metadata::extended::ExifData;

/// Image metadata extractor (dimensions, format, EXIF)
pub struct ImageExtractor;

impl ImageExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Extract basic image dimensions and format
    #[cfg(feature = "metadata-image")]
    async fn extract_dimensions(
        &self,
        path: &Path,
        mime: &MimeInfo,
        provider: &dyn FsProvider,
        cx: &ProviderCx<'_>,
    ) -> Result<(u32, u32, String), CoreError> {
        use imagesize::{ImageSize, blob_size};
        let data = provider.read(path, cx).await?;
        let res = blob_size(data.as_slice()).unwrap_or(ImageSize {
            height: 0,
            width: 0,
        });
        let format = match &*mime.mime_type {
            "image/png" => "PNG",
            "image/apng" => "APNG",
            "image/avif" => "AVIF",
            "image/bmp" => "BMP",
            "image/x-adobe-dng" => "DNG",
            "image/jpeg" => "JPEG",
            "image/gif" => "GIF",
            "image/webp" => "WebP",
            "image/tiff" => "TIFF",
            "image/x-icon" => "ICO",
            "image/svg+xml" => "SVG",
            "image/heic" => "HEIC",
            "image/heif" => "HEIF",
            "image/vnd.ms-photo" => "JXR",
            _ => "Unknown",
        };
        Ok((res.width as u32, res.height as u32, format.to_string()))
    }

    /// Extract EXIF data from image, along with color space and bit depth.
    ///
    /// Returns `(ExifData, color_space, bit_depth)`.
    /// `Tag::ColorSpace`: 1 = sRGB, 65535 = Uncalibrated.
    /// `Tag::BitsPerSample`: per-channel depth (e.g. 8, 16).
    #[cfg(feature = "metadata-image")]
    async fn extract_exif(
        &self,
        path: &Path,
        provider: &dyn FsProvider,
        cx: &ProviderCx<'_>,
    ) -> Result<(ExifData, Option<String>, Option<u8>), CoreError> {
        use exif::{In, Reader, Tag, Value};
        let reader = Reader::new();
        let mut io = provider.open_reader(path, cx).await?;
        let source = reader
            .read_from_container(&mut io)
            .map_err(|e| CoreError::invalid_data(format!("Unable to parse data: {}", e)))?;

        let color_space = source
            .get_field(Tag::ColorSpace, In::PRIMARY)
            .and_then(|f| match &f.value {
                Value::Short(v) => match v.first() {
                    Some(1) => Some("sRGB".to_string()),
                    Some(65535) => Some("Uncalibrated".to_string()),
                    _ => None,
                },
                _ => None,
            });

        let bit_depth = source
            .get_field(Tag::BitsPerSample, In::PRIMARY)
            .and_then(|f| match &f.value {
                Value::Short(v) => v.first().map(|&n| n as u8),
                _ => None,
            });

        Ok((
            ExifData {
                camera_make: source
                    .get_field(Tag::Make, In::PRIMARY)
                    .map(|e| e.display_value().to_string()),
                camera_model: source
                    .get_field(Tag::Model, In::PRIMARY)
                    .map(|e| e.display_value().to_string()),
                date_taken: source
                    .get_field(Tag::DateTime, In::PRIMARY)
                    .map(|e| e.display_value().to_string()),
                gps_latitude: source.get_field(Tag::GPSLatitude, In::PRIMARY).map(|v| {
                    match &v.value {
                        exif::Value::Rational(r) => {
                            r[0].to_f64() + r[1].to_f64() / 30.0 + r[2].to_f64() / 60.0
                        }
                        _ => 0.0,
                    }
                }),
                gps_longitude: source.get_field(Tag::GPSLongitude, In::PRIMARY).map(|v| {
                    match &v.value {
                        exif::Value::Rational(r) => {
                            r[0].to_f64() + r[1].to_f64() / 30.0 + r[2].to_f64() / 60.0
                        }
                        _ => 0.0,
                    }
                }),
                exposure_time: source
                    .get_field(Tag::ExposureTime, In::PRIMARY)
                    .map(|e| e.display_value().to_string()),
                f_number: source
                    .get_field(Tag::FNumber, In::PRIMARY)
                    .map(|e| e.display_value().to_string().parse().ok())
                    .flatten(),
                iso: source
                    .get_field(Tag::ISOSpeed, In::PRIMARY)
                    .map(|e| e.display_value().to_string().parse().ok())
                    .flatten(),
                focal_length: source
                    .get_field(Tag::FocalLength, In::PRIMARY)
                    .map(|e| e.display_value().to_string().parse().ok())
                    .flatten(),
                orientation: source
                    .get_field(Tag::Orientation, In::PRIMARY)
                    .map(|e| e.display_value().to_string().parse().ok())
                    .flatten(),
                raw: source
                    .fields()
                    .map(|v| (format!("{}", v.tag), v.display_value().to_string()))
                    .collect(),
            },
            color_space,
            bit_depth,
        ))
    }
}

#[async_trait]
impl MetadataExtractor for ImageExtractor {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Image]
    }

    async fn extract(
        &self,
        path: &Path,
        mime: &MimeInfo,
        provider: &dyn FsProvider,
        cx: &ProviderCx<'_>,
    ) -> Result<ExtendedMetadata, CoreError> {
        #[cfg(not(feature = "metadata-image"))]
        return Ok(ExtendedMetadata::Unavailable);

        #[cfg(feature = "metadata-image")]
        {
            let (width, height, format) = self.extract_dimensions(path, mime, provider, cx).await?;
            // EXIF is optional — missing or unreadable EXIF is not an error.
            let (exif, color_space, bit_depth) =
                match self.extract_exif(path, provider, cx).await.ok() {
                    Some((exif, cs, bd)) => (Some(exif), cs, bd),
                    None => (None, None, None),
                };
            Ok(ExtendedMetadata::Image(ImageMetadata {
                width,
                height,
                format,
                color_space,
                bit_depth,
                has_alpha: false, // requires pixel decoding (no image crate in scope)
                exif,
            }))
        }
    }

    fn name(&self) -> &'static str {
        "image"
    }
}

impl Default for ImageExtractor {
    fn default() -> Self {
        Self::new()
    }
}
