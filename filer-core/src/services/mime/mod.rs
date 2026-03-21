mod detector;
mod table;

pub use detector::{
    DetectionConfidence, DetectionStrategy, MimeCategory, MimeDetector, MimeInfo,
    AMBIGUOUS_EXTENSIONS,
};