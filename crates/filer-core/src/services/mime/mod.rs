mod detector;
pub(crate) mod table;

pub use detector::{
    AMBIGUOUS_EXTENSIONS, DetectionConfidence, DetectionStrategy, MAGIC_BYTE_WINDOW, MimeCategory,
    MimeDetector, MimeInfo,
};
