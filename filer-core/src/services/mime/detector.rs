use std::path::Path;

use mime as mime_crate;

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
    "", // no extension at all
    "bin", "dat", "raw", "out", "tmp",
    "txt", // could be CSV, JSON, TOML, XML saved with the wrong extension
    "log", // could be structured JSON-lines or plain text
];

pub struct MimeDetector;

impl MimeDetector {
    /// Detect MIME type from file path (extension-based, zero I/O).
    ///
    /// Delegates to `new_mime_guess` for the extension → MIME mapping.
    ///
    /// # Confidence
    /// - `Definitive` — unambiguous extension with a known MIME type
    /// - `Probable`   — extension is in `AMBIGUOUS_EXTENSIONS` but resolves to
    ///                  a `text/*` type (e.g. `.txt` → `text/plain`)
    /// - `Unknown`    — no extension, ambiguous non-text extension, or unknown extension
    pub fn detect_from_path(path: &Path) -> MimeInfo {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let is_ambiguous = Self::is_ambiguous_extension(ext);

        // For non-ambiguous extensions, our table is authoritative and more
        // accurate than new_mime_guess (e.g. .py → text/x-python, not text/plain).
        if !is_ambiguous {
            let ext_lower = ext.to_ascii_lowercase();
            if let Some(entry) = super::table::lookup_extension(&ext_lower) {
                return MimeInfo {
                    mime_type: entry.mime_type.to_string(),
                    category: entry.category,
                    encoding: None,
                    confidence: entry.confidence,
                };
            }
        }

        match new_mime_guess::from_ext(ext).first() {
            Some(m) => {
                let mime_type = m.to_string();
                let category = Self::categorize(&mime_type);
                // Ambiguous extensions whose MIME resolves to text/* get Probable
                // (e.g. "txt" → text/plain). Anything else ambiguous is Unknown
                // because we genuinely can't tell (e.g. "bin", "dat").
                let confidence = if is_ambiguous {
                    if m.type_() == mime_crate::TEXT {
                        DetectionConfidence::Probable
                    } else {
                        DetectionConfidence::Unknown
                    }
                } else {
                    DetectionConfidence::Definitive
                };
                MimeInfo {
                    mime_type,
                    category,
                    encoding: None,
                    confidence,
                }
            }
            None => MimeInfo {
                mime_type: "application/octet-stream".to_string(),
                category: MimeCategory::Unknown,
                encoding: None,
                confidence: DetectionConfidence::Unknown,
            },
        }
    }

    /// Detect MIME type from file contents (magic bytes, most accurate).
    ///
    /// Delegates to the `infer` crate which covers ~70 formats including
    /// HEIC/HEIF, 3GP, RIFF disambiguation (WebP/WAV/AVI), OLE2, and RAR.
    ///
    /// `bytes` should be the first 512 bytes of the file.
    ///
    /// # Confidence
    /// - `Definitive` — a magic signature matched
    /// - `Unknown`    — bytes too short or no signature matched
    pub fn detect_from_bytes(bytes: &[u8]) -> MimeInfo {
        match infer::get(bytes) {
            Some(kind) => {
                let mime_type = kind.mime_type().to_string();
                let category = Self::categorize(&mime_type);
                MimeInfo {
                    mime_type,
                    category,
                    encoding: None,
                    confidence: DetectionConfidence::Definitive,
                }
            }
            None => MimeInfo {
                mime_type: "application/octet-stream".to_string(),
                category: MimeCategory::Unknown,
                encoding: None,
                confidence: DetectionConfidence::Unknown,
            },
        }
    }

    /// Detect using both path hint and content (magic bytes).
    ///
    /// Tier 1 — extension: if `Definitive`, return immediately (no I/O).
    /// This correctly handles OOXML: `.docx` is Definitive → Document wins
    /// over the ZIP magic that would otherwise give Archive.
    ///
    /// Tier 2 — magic: if extension was not Definitive, check bytes.
    /// Magic wins on disagreement; if magic is also inconclusive, extension wins.
    pub fn detect(path: &Path, bytes: &[u8]) -> MimeInfo {
        let path_info = Self::detect_from_path(path);

        if path_info.confidence == DetectionConfidence::Definitive {
            return path_info;
        }

        let magic_info = Self::detect_from_bytes(bytes);

        if magic_info.confidence == DetectionConfidence::Unknown {
            return path_info;
        }

        magic_info
    }

    /// Detect using a caller-chosen strategy, with an optional pre-read header.
    ///
    /// `header` is `None` when the caller could not read the file (e.g. a
    /// remote provider). Falls back to extension-only in that case regardless
    /// of strategy.
    ///
    /// - `ExtensionOnly`         — always returns `detect_from_path`, no bytes read
    /// - `ExtensionWithFallback` — uses magic only for absent/ambiguous extensions
    /// - `MagicBytes`            — always prefers magic when `header` is `Some`
    pub fn detect_with_strategy(
        path: &Path,
        header: Option<&[u8]>,
        strategy: DetectionStrategy,
    ) -> MimeInfo {
        match strategy {
            DetectionStrategy::ExtensionOnly => Self::detect_from_path(path),

            DetectionStrategy::ExtensionWithFallback => {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if Self::is_ambiguous_extension(ext) {
                    match header {
                        Some(bytes) => Self::detect(path, bytes),
                        None => Self::detect_from_path(path),
                    }
                } else {
                    Self::detect_from_path(path)
                }
            }

            DetectionStrategy::MagicBytes => match header {
                Some(bytes) => Self::detect(path, bytes),
                None => Self::detect_from_path(path),
            },
        }
    }

    /// Map a raw MIME type string to a broad `MimeCategory`.
    ///
    /// Top-level type prefixes (`image/`, `audio/`, `video/`, `text/`) are
    /// matched first — this covers every infer and new_mime_guess type in
    /// those families automatically.
    ///
    /// The `application/*` match is derived directly from infer's `map.rs`
    /// MIME strings. PDF, RTF, postscript, and epub are classified by infer
    /// under `MatcherType::Archive` internally but are promoted to Document
    /// here because that is the correct routing category.
    pub fn categorize(mime_type: &str) -> MimeCategory {
        if mime_type.starts_with("image/") {
            return MimeCategory::Image;
        }
        if mime_type.starts_with("audio/") {
            return MimeCategory::Audio;
        }
        if mime_type.starts_with("video/") {
            return MimeCategory::Video;
        }
        if mime_type.starts_with("text/") {
            return MimeCategory::Text;
        }

        match mime_type {
            // ── Text-like application types (new_mime_guess) ──────────────────
            "application/json"
            | "application/javascript"
            | "application/xml"
            | "application/toml"
            | "application/x-sh"
            | "application/x-httpd-php" => MimeCategory::Text,

            // ── Documents ─────────────────────────────────────────────────────
            // infer MatcherType::Doc
            "application/msword"
            | "application/vnd.ms-excel"
            | "application/vnd.ms-powerpoint"
            | "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            | "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            | "application/vnd.oasis.opendocument.text"
            | "application/vnd.oasis.opendocument.spreadsheet"
            | "application/vnd.oasis.opendocument.presentation"
            // infer MatcherType::Book
            | "application/epub+zip"
            | "application/x-mobipocket-ebook"
            // infer MatcherType::Archive — semantically documents
            | "application/pdf"
            | "application/rtf"
            | "application/postscript"
            // new_mime_guess OOXML / macro-enabled variants (no infer magic)
            | "application/vnd.openxmlformats-officedocument.wordprocessingml.template"
            | "application/vnd.openxmlformats-officedocument.spreadsheetml.template"
            | "application/vnd.openxmlformats-officedocument.presentationml.template"
            | "application/vnd.ms-word.document.macroenabled.12"
            | "application/vnd.ms-word.template.macroenabled.12"
            | "application/vnd.ms-excel.sheet.macroenabled.12"
            | "application/vnd.ms-excel.sheet.binary.macroenabled.12"
            | "application/vnd.ms-powerpoint.presentation.macroenabled.12" => MimeCategory::Document,

            // ── Archives ──────────────────────────────────────────────────────
            // infer MatcherType::Archive (browsable compressed containers)
            "application/zip"
            | "application/gzip"
            | "application/x-tar"
            | "application/vnd.rar"
            | "application/x-bzip2"
            | "application/vnd.bzip3"
            | "application/x-7z-compressed"
            | "application/x-xz"
            | "application/zstd"
            | "application/x-lz4"
            | "application/x-lzip"
            | "application/x-compress"
            | "application/x-par2"
            | "application/x-rpm"
            | "application/x-unix-archive"
            | "application/x-cpio"
            | "application/vnd.ms-cab-compressed"
            | "application/vnd.debian.binary-package"
            // new_mime_guess extras
            | "application/x-bzip"
            | "application/x-gtar"
            | "application/x-apple-diskimage" => MimeCategory::Archive,

            // ── Binary ────────────────────────────────────────────────────────
            // infer MatcherType::App
            "application/wasm"
            | "application/x-executable"              // ELF and COFF .obj
            | "application/vnd.microsoft.portable-executable" // .exe and .dll
            | "application/java"                      // .class
            | "application/x-llvm"                    // .bc (LLVM bitcode)
            | "application/x-mach-binary"             // macOS Mach-O
            | "application/vnd.android.dex"
            | "application/vnd.android.dey"
            | "application/x-x509-ca-cert"            // .der / .pem
            // infer MatcherType::Font
            | "application/font-woff"
            | "application/font-sfnt"
            // infer MatcherType::Archive — not browsable, binary payloads
            | "application/octet-stream"              // .eot font
            | "application/x-shockwave-flash"
            | "application/vnd.sqlite3"
            | "application/dicom"
            | "application/x-nintendo-nes-rom"
            | "application/x-google-chrome-extension"
            | "application/x-ole-storage"             // .msi installer
            // new_mime_guess extras
            | "application/java-archive"
            | "application/x-sharedlib" => MimeCategory::Binary,

            _ => MimeCategory::Unknown,
        }
    }

    /// Returns `true` if the extension is in the ambiguous set and magic-byte
    /// detection should be preferred even when a path-based result is available.
    ///
    /// This is the **only** non-todo function in this file.
    pub fn is_ambiguous_extension(ext: &str) -> bool {
        AMBIGUOUS_EXTENSIONS.contains(&ext)
    }
}
