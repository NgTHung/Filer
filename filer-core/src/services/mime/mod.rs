mod detector;
pub(crate) mod table;

pub use detector::{
    DetectionConfidence, DetectionStrategy, MimeCategory, MimeDetector, MimeInfo,
    AMBIGUOUS_EXTENSIONS,
};