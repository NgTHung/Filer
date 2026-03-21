use std::path::Path;

/// How confident the detector is in its result.
///
/// Callers use this to decide whether to escalate to magic-byte detection:
/// - `Definitive` -> trust the result, skip further I/O
/// - `Probable`   -> extension matched but could be wrong; magic check optional
/// - `Unknown`    -> no usable extension or magic was inconclusive; magic needed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionConfidence {
    /// Extension + magic agree, or magic alone is unambiguous.
    Definitive,
    /// Extension matched a known type; magic bytes were not checked.
    Probable,
    /// No extension, ambiguous extension, or magic bytes inconclusive.
    Unknown,
}

/// Controls when magic-byte reading is performed.
///
/// Choose based on provider cost:
/// - Local FS  -> `MagicBytes` (single cheap `pread`, best accuracy)
/// - Remote FS -> `ExtensionOnly` (any read has network latency)
/// - Unknown   -> `ExtensionWithFallback` (read only when truly needed)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionStrategy {
    /// Never read file bytes. Fast; may misclassify renamed or extensionless files.
    ExtensionOnly,
    /// Read magic bytes only when the extension is absent or in the ambiguous set.
    ExtensionWithFallback,
    /// Always read magic bytes regardless of extension. Best accuracy.
    MagicBytes,
}

/// Detected MIME information
#[derive(Debug, Clone)]
pub struct MimeInfo {
    pub mime_type: String,
    pub category: MimeCategory,
    pub encoding: Option<String>,
    /// How reliable this result is. Callers may escalate to magic-byte
    /// detection when confidence is `Probable` or `Unknown`.
    pub confidence: DetectionConfidence,
}

/// Broad category for routing to preview providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MimeCategory {
    Text,
    Image,
    Audio,
    Video,
    Document,
    Archive,
    Binary,
    Unknown,
}

/// Extensions whose MIME type is ambiguous from the name alone.
///
/// When `DetectionStrategy::ExtensionWithFallback` is active, files with
/// these extensions always trigger a magic-byte read even if the extension
/// is technically "known". An empty string represents a missing extension
/// (e.g., `Makefile`, `Dockerfile`, `LICENSE`).
pub const AMBIGUOUS_EXTENSIONS: &[&str] = &[
    "",    // no extension at all
    "bin", "dat", "raw", "out", "tmp",
    "txt", // could be CSV, JSON, TOML, XML saved with the wrong extension
    "log", // could be structured JSON-lines or plain text
];

pub struct MimeDetector;

impl MimeDetector {
    pub fn new() -> Self {
        Self
    }

    /// Detect MIME type from file path (extension-based, zero I/O).
    ///
    /// Looks up the file extension in a built-in table and returns the
    /// corresponding MIME type and category.
    ///
    /// # Confidence
    /// - `Definitive` — extension unambiguously identifies one format (e.g., `.png`)
    /// - `Probable`   — extension is a common but multi-use type (e.g., `.txt`)
    /// - `Unknown`    — no extension, or extension is in `AMBIGUOUS_EXTENSIONS`
    ///
    /// # TODO
    /// - Build the extension -> (mime_type, MimeCategory, DetectionConfidence) table
    /// - Cover: image, audio, video, document, archive, code/text, binary categories
    /// - `Unknown` for missing extensions and all entries in `AMBIGUOUS_EXTENSIONS`
    pub fn detect_from_path(&self, path: &Path) -> MimeInfo {
        todo!()
    }

    /// Detect MIME type from file contents (magic bytes, most accurate).
    ///
    /// `bytes` should be the first 512 bytes of the file; no known format
    /// requires more than that for initial identification.
    ///
    /// # Confidence
    /// - `Definitive` — a magic-byte signature matched unambiguously
    /// - `Unknown`    — bytes are too short or no signature matched
    ///
    /// # TODO
    /// Implement magic-byte signatures for (in priority order):
    /// - Images:     PNG `\x89PNG`, JPEG `\xFF\xD8\xFF`, GIF `GIF8`, WebP `RIFF…WEBP`, BMP `BM`
    /// - Documents:  PDF `%PDF`
    /// - Archives:   ZIP `PK\x03\x04`, GZIP `\x1F\x8B`, BZIP2 `BZh`, XZ `\xFD7zXZ`
    /// - Audio:      MP3 ID3 header `ID3`, OGG `OggS`, FLAC `fLaC`
    /// - Binary/ELF: `\x7FELF`
    /// - Office:     DOCX/XLSX/PPTX are ZIP-based; detect by ZIP magic then inspect content-type
    /// - Fallback:   return `Unknown` confidence when no signature matched
    pub fn detect_from_bytes(&self, bytes: &[u8]) -> MimeInfo {
        todo!()
    }

    /// Detect using both path hint and content (magic bytes).
    ///
    /// Runs `detect_from_path` first. If the result is already `Definitive`
    /// it is returned without reading bytes. Otherwise `detect_from_bytes` is
    /// run and its result is returned. When the two disagree, magic wins.
    ///
    /// # TODO
    /// - Call `detect_from_path(path)` for Tier 1
    /// - If `confidence == Definitive` -> return early
    /// - Call `detect_from_bytes(bytes)` for Tier 2
    /// - If magic confidence is `Unknown` -> return the path result
    /// - Otherwise -> return magic result (magic wins on disagreement)
    pub fn detect(&self, path: &Path, bytes: &[u8]) -> MimeInfo {
        todo!()
    }

    /// Detect using a caller-chosen strategy, with an optional pre-read header.
    ///
    /// `header` is `None` when the caller could not read the file (e.g., a
    /// remote provider that returned `Err` on `read_header`). In that case the
    /// method falls back to extension-only detection regardless of `strategy`.
    ///
    /// # Strategy behaviour
    /// - `ExtensionOnly`          -> always returns `detect_from_path` result
    /// - `ExtensionWithFallback`  -> uses magic only when the extension is absent
    ///                              or in `AMBIGUOUS_EXTENSIONS`; otherwise extension
    /// - `MagicBytes`             -> always prefers magic when `header` is `Some`;
    ///                              falls back to extension when `header` is `None`
    ///
    /// # TODO
    /// - Match on `strategy`
    /// - `ExtensionOnly`          -> `detect_from_path(path)`
    /// - `ExtensionWithFallback`  -> check `is_ambiguous_extension(ext)`:
    ///     - ambiguous AND header is Some -> `detect(path, header)`
    ///     - otherwise -> `detect_from_path(path)`
    /// - `MagicBytes`             -> `detect(path, header)` if Some, else `detect_from_path`
    pub fn detect_with_strategy(
        &self,
        path: &Path,
        header: Option<&[u8]>,
        strategy: DetectionStrategy,
    ) -> MimeInfo {
        todo!()
    }

    /// Map a MIME type string to a broad `MimeCategory`.
    ///
    /// # TODO
    /// Match on MIME type prefixes and specific values:
    /// - `"image/*"`                  -> Image
    /// - `"audio/*"`                  -> Audio
    /// - `"video/*"`                  -> Video
    /// - `"text/*"`                   -> Text
    /// - `"application/pdf"`          -> Document
    /// - `"application/msword"` and related Office types -> Document
    /// - `"application/zip"`, `"application/gzip"`, `"application/x-tar"` etc. -> Archive
    /// - `"application/octet-stream"` -> Binary
    /// - everything else              -> Unknown
    pub fn categorize(mime_type: &str) -> MimeCategory {
        todo!()
    }

    /// Returns `true` if the extension is in the ambiguous set and magic-byte
    /// detection should be preferred even when a path-based result is available.
    ///
    /// This is the **only** non-todo function in this file.
    pub fn is_ambiguous_extension(ext: &str) -> bool {
        AMBIGUOUS_EXTENSIONS.contains(&ext)
    }
}

impl Default for MimeDetector {
    fn default() -> Self {
        Self::new()
    }
}
