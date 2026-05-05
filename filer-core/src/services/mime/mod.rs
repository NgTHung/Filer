mod detector;
pub(crate) mod table;

pub use detector::{
    AMBIGUOUS_EXTENSIONS, DetectionConfidence, DetectionStrategy, MimeCategory, MimeDetector,
    MimeInfo,
};
