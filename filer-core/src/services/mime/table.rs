//! Extension lookup table for MIME detection.
//!
//! `EXT_TABLE` is a sorted `&[(&str, ExtEntry)]` slice.
//! Use `lookup_extension(ext)` for O(log n) lookup by extension string
//! (lower-case, no leading dot).
//!
//! This table is the authoritative source for common extensions.
//! It is consulted before `new_mime_guess` so that code-file extensions
//! such as `.py`, `.rs`, `.go` return the correct `text/x-*` types instead
//! of the IANA-registered `text/plain` that `new_mime_guess` emits for
//! unofficially registered extensions.

use super::detector::{DetectionConfidence, MimeCategory};

#[derive(Clone, Copy)]
pub struct ExtEntry {
    pub mime_type: &'static str,
    pub category: MimeCategory,
    pub confidence: DetectionConfidence,
}

/// Look up `ext` (lower-case, no leading dot) in `EXT_TABLE`.
///
/// Returns `Some(&ExtEntry)` when found, `None` otherwise.
pub fn lookup_extension(ext: &str) -> Option<&'static ExtEntry> {
    EXT_TABLE
        .binary_search_by_key(&ext, |&(e, _)| e)
        .ok()
        .map(|i| &EXT_TABLE[i].1)
}

//
// Rules:
//   - Sorted lexicographically by extension (required for binary_search_by_key).
//   - `Definitive`  — unambiguous, single well-known type.
//   - `Probable`    — usually this type but magic bytes could disagree.
//   - Extensions in AMBIGUOUS_EXTENSIONS are NOT in this table; the detector
//     handles them separately via is_ambiguous_extension().

pub(crate) static EXT_TABLE: &[(&str, ExtEntry)] = &[
    (
        "7z",
        ExtEntry {
            mime_type: "application/x-7z-compressed",
            category: MimeCategory::Archive,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "aac",
        ExtEntry {
            mime_type: "audio/aac",
            category: MimeCategory::Audio,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "apng",
        ExtEntry {
            mime_type: "image/apng",
            category: MimeCategory::Image,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "asm",
        ExtEntry {
            mime_type: "text/x-asm",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "au",
        ExtEntry {
            mime_type: "audio/basic",
            category: MimeCategory::Audio,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "avi",
        ExtEntry {
            mime_type: "video/x-msvideo",
            category: MimeCategory::Video,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "avif",
        ExtEntry {
            mime_type: "image/avif",
            category: MimeCategory::Image,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "bash",
        ExtEntry {
            mime_type: "application/x-sh",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "bat",
        ExtEntry {
            mime_type: "application/x-msdos-program",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "bmp",
        ExtEntry {
            mime_type: "image/bmp",
            category: MimeCategory::Image,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "bz2",
        ExtEntry {
            mime_type: "application/x-bzip2",
            category: MimeCategory::Archive,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "c",
        ExtEntry {
            mime_type: "text/x-c",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "cc",
        ExtEntry {
            mime_type: "text/x-c",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "class",
        ExtEntry {
            mime_type: "application/java",
            category: MimeCategory::Binary,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "conf",
        ExtEntry {
            mime_type: "text/plain",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Probable,
        },
    ),
    (
        "cpp",
        ExtEntry {
            mime_type: "text/x-c",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "crt",
        ExtEntry {
            mime_type: "application/x-x509-ca-cert",
            category: MimeCategory::Binary,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "cs",
        ExtEntry {
            mime_type: "text/x-csharp",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "css",
        ExtEntry {
            mime_type: "text/css",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "csv",
        ExtEntry {
            mime_type: "text/csv",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "cxx",
        ExtEntry {
            mime_type: "text/x-c",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "deb",
        ExtEntry {
            mime_type: "application/vnd.debian.binary-package",
            category: MimeCategory::Archive,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "der",
        ExtEntry {
            mime_type: "application/x-x509-ca-cert",
            category: MimeCategory::Binary,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "dic",
        ExtEntry {
            mime_type: "text/plain",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Probable,
        },
    ),
    (
        "dir",
        ExtEntry {
            mime_type: "application/x-director",
            category: MimeCategory::Binary,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "dmg",
        ExtEntry {
            mime_type: "application/x-apple-diskimage",
            category: MimeCategory::Archive,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "dng",
        ExtEntry {
            mime_type: "image/x-adobe-dng",
            category: MimeCategory::Image,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "doc",
        ExtEntry {
            mime_type: "application/msword",
            category: MimeCategory::Document,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "docx",
        ExtEntry {
            mime_type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            category: MimeCategory::Document,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "epub",
        ExtEntry {
            mime_type: "application/epub+zip",
            category: MimeCategory::Document,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "exe",
        ExtEntry {
            mime_type: "application/vnd.microsoft.portable-executable",
            category: MimeCategory::Binary,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "flac",
        ExtEntry {
            mime_type: "audio/flac",
            category: MimeCategory::Audio,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "flv",
        ExtEntry {
            mime_type: "video/x-flv",
            category: MimeCategory::Video,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "gif",
        ExtEntry {
            mime_type: "image/gif",
            category: MimeCategory::Image,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "go",
        ExtEntry {
            mime_type: "text/x-go",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "gz",
        ExtEntry {
            mime_type: "application/gzip",
            category: MimeCategory::Archive,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "h",
        ExtEntry {
            mime_type: "text/x-c",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "heic",
        ExtEntry {
            mime_type: "image/heic",
            category: MimeCategory::Image,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "heif",
        ExtEntry {
            mime_type: "image/heif",
            category: MimeCategory::Image,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "hpp",
        ExtEntry {
            mime_type: "text/x-c",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "htm",
        ExtEntry {
            mime_type: "text/html",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "html",
        ExtEntry {
            mime_type: "text/html",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "hxx",
        ExtEntry {
            mime_type: "text/x-c",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "ico",
        ExtEntry {
            mime_type: "image/x-icon",
            category: MimeCategory::Image,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "ini",
        ExtEntry {
            mime_type: "text/plain",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Probable,
        },
    ),
    (
        "iso",
        ExtEntry {
            mime_type: "application/x-iso9660-image",
            category: MimeCategory::Archive,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "jar",
        ExtEntry {
            mime_type: "application/java-archive",
            category: MimeCategory::Archive,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "java",
        ExtEntry {
            mime_type: "text/x-java",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "jfif",
        ExtEntry {
            mime_type: "image/jpeg",
            category: MimeCategory::Image,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "jpe",
        ExtEntry {
            mime_type: "image/jpeg",
            category: MimeCategory::Image,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "jpeg",
        ExtEntry {
            mime_type: "image/jpeg",
            category: MimeCategory::Image,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "jpg",
        ExtEntry {
            mime_type: "image/jpeg",
            category: MimeCategory::Image,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "js",
        ExtEntry {
            mime_type: "text/javascript",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "json",
        ExtEntry {
            mime_type: "application/json",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "jsx",
        ExtEntry {
            mime_type: "text/javascript",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "kt",
        ExtEntry {
            mime_type: "text/x-kotlin",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "lua",
        ExtEntry {
            mime_type: "text/x-lua",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "lz4",
        ExtEntry {
            mime_type: "application/x-lz4",
            category: MimeCategory::Archive,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "m",
        ExtEntry {
            mime_type: "text/x-objcsrc",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "m4a",
        ExtEntry {
            mime_type: "audio/mp4",
            category: MimeCategory::Audio,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "m4b",
        ExtEntry {
            mime_type: "audio/mp4",
            category: MimeCategory::Audio,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "m4v",
        ExtEntry {
            mime_type: "video/x-m4v",
            category: MimeCategory::Video,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "md",
        ExtEntry {
            mime_type: "text/markdown",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "mid",
        ExtEntry {
            mime_type: "audio/midi",
            category: MimeCategory::Audio,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "midi",
        ExtEntry {
            mime_type: "audio/midi",
            category: MimeCategory::Audio,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "mkv",
        ExtEntry {
            mime_type: "video/x-matroska",
            category: MimeCategory::Video,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "mobi",
        ExtEntry {
            mime_type: "application/x-mobipocket-ebook",
            category: MimeCategory::Document,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "mov",
        ExtEntry {
            mime_type: "video/quicktime",
            category: MimeCategory::Video,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "mp3",
        ExtEntry {
            mime_type: "audio/mpeg",
            category: MimeCategory::Audio,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "mp4",
        ExtEntry {
            mime_type: "video/mp4",
            category: MimeCategory::Video,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "mpeg",
        ExtEntry {
            mime_type: "video/mpeg",
            category: MimeCategory::Video,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "mpg",
        ExtEntry {
            mime_type: "video/mpeg",
            category: MimeCategory::Video,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "mpga",
        ExtEntry {
            mime_type: "audio/mpeg",
            category: MimeCategory::Audio,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "mts",
        ExtEntry {
            mime_type: "video/mp2t",
            category: MimeCategory::Video,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "nim",
        ExtEntry {
            mime_type: "text/x-nim",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "o",
        ExtEntry {
            mime_type: "application/x-executable",
            category: MimeCategory::Binary,
            confidence: DetectionConfidence::Probable,
        },
    ),
    (
        "odt",
        ExtEntry {
            mime_type: "application/vnd.oasis.opendocument.text",
            category: MimeCategory::Document,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "ogg",
        ExtEntry {
            mime_type: "audio/ogg",
            category: MimeCategory::Audio,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "opus",
        ExtEntry {
            mime_type: "audio/opus",
            category: MimeCategory::Audio,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "otf",
        ExtEntry {
            mime_type: "font/otf",
            category: MimeCategory::Binary,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "pdf",
        ExtEntry {
            mime_type: "application/pdf",
            category: MimeCategory::Document,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "pem",
        ExtEntry {
            mime_type: "application/x-x509-ca-cert",
            category: MimeCategory::Binary,
            confidence: DetectionConfidence::Probable,
        },
    ),
    (
        "php",
        ExtEntry {
            mime_type: "application/x-httpd-php",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "pkg",
        ExtEntry {
            mime_type: "application/x-xpinstall",
            category: MimeCategory::Archive,
            confidence: DetectionConfidence::Probable,
        },
    ),
    (
        "png",
        ExtEntry {
            mime_type: "image/png",
            category: MimeCategory::Image,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "ppt",
        ExtEntry {
            mime_type: "application/vnd.ms-powerpoint",
            category: MimeCategory::Document,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "pptx",
        ExtEntry {
            mime_type: "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            category: MimeCategory::Document,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "ps",
        ExtEntry {
            mime_type: "application/postscript",
            category: MimeCategory::Document,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "py",
        ExtEntry {
            mime_type: "text/x-python",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "pyc",
        ExtEntry {
            mime_type: "application/octet-stream",
            category: MimeCategory::Binary,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "pyd",
        ExtEntry {
            mime_type: "application/octet-stream",
            category: MimeCategory::Binary,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "pyo",
        ExtEntry {
            mime_type: "application/octet-stream",
            category: MimeCategory::Binary,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "rar",
        ExtEntry {
            mime_type: "application/vnd.rar",
            category: MimeCategory::Archive,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "rb",
        ExtEntry {
            mime_type: "text/x-ruby",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "rpm",
        ExtEntry {
            mime_type: "application/x-rpm",
            category: MimeCategory::Archive,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "rs",
        ExtEntry {
            mime_type: "text/x-rust",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "rtf",
        ExtEntry {
            mime_type: "application/rtf",
            category: MimeCategory::Document,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "sh",
        ExtEntry {
            mime_type: "application/x-sh",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "so",
        ExtEntry {
            mime_type: "application/x-sharedlib",
            category: MimeCategory::Binary,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "sql",
        ExtEntry {
            mime_type: "text/x-sql",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "svg",
        ExtEntry {
            mime_type: "image/svg+xml",
            category: MimeCategory::Image,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "swift",
        ExtEntry {
            mime_type: "text/x-swift",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "tar",
        ExtEntry {
            mime_type: "application/x-tar",
            category: MimeCategory::Archive,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "tex",
        ExtEntry {
            mime_type: "text/x-tex",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "tif",
        ExtEntry {
            mime_type: "image/tiff",
            category: MimeCategory::Image,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "tiff",
        ExtEntry {
            mime_type: "image/tiff",
            category: MimeCategory::Image,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "toml",
        ExtEntry {
            mime_type: "application/toml",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "torrent",
        ExtEntry {
            mime_type: "application/x-bittorrent",
            category: MimeCategory::Binary,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "ts",
        ExtEntry {
            mime_type: "text/typescript",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "tsx",
        ExtEntry {
            mime_type: "text/typescript",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "ttf",
        ExtEntry {
            mime_type: "font/ttf",
            category: MimeCategory::Binary,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "wasm",
        ExtEntry {
            mime_type: "application/wasm",
            category: MimeCategory::Binary,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "wav",
        ExtEntry {
            mime_type: "audio/wav",
            category: MimeCategory::Audio,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "webm",
        ExtEntry {
            mime_type: "video/webm",
            category: MimeCategory::Video,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "webp",
        ExtEntry {
            mime_type: "image/webp",
            category: MimeCategory::Image,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "wmv",
        ExtEntry {
            mime_type: "video/x-ms-wmv",
            category: MimeCategory::Video,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "woff",
        ExtEntry {
            mime_type: "font/woff",
            category: MimeCategory::Binary,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "woff2",
        ExtEntry {
            mime_type: "font/woff2",
            category: MimeCategory::Binary,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "xls",
        ExtEntry {
            mime_type: "application/vnd.ms-excel",
            category: MimeCategory::Document,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "xlsx",
        ExtEntry {
            mime_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            category: MimeCategory::Document,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "xml",
        ExtEntry {
            mime_type: "text/xml",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "xz",
        ExtEntry {
            mime_type: "application/x-xz",
            category: MimeCategory::Archive,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "yaml",
        ExtEntry {
            mime_type: "text/yaml",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "yml",
        ExtEntry {
            mime_type: "text/yaml",
            category: MimeCategory::Text,
            confidence: DetectionConfidence::Definitive,
        },
    ),
    (
        "zst",
        ExtEntry {
            mime_type: "application/zstd",
            category: MimeCategory::Archive,
            confidence: DetectionConfidence::Definitive,
        },
    ),
];
