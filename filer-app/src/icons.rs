use filer_core::MimeCategory;
use filer_core::model::node::NodeKind;
use filer_core::FileNode;

/// Return a text icon for a given MIME category.
/// Characters are from the Geometric Shapes Unicode block (U+25xx) which
/// are present in DejaVu Sans, Liberation Sans, and all other standard
/// Linux system fonts — no color emoji font required.
pub fn for_category(cat: &MimeCategory) -> &'static str {
    match cat {
        MimeCategory::Image    => "◉",
        MimeCategory::Audio    => "♪",
        MimeCategory::Video    => "▶",
        MimeCategory::Document => "▤",
        MimeCategory::Archive  => "◫",
        MimeCategory::Binary   => "▪",
        MimeCategory::Text     => "≡",
        MimeCategory::Unknown  => "·",
    }
}

/// Return a text icon for a file or directory node.
pub fn for_node(node: &FileNode) -> &'static str {
    match &node.kind {
        NodeKind::Directory { .. } => "▸",  // right-pointing triangle → navigate
        NodeKind::Symlink { .. }   => "↪",
        NodeKind::File { extension } => match extension.as_deref() {
            Some("rs" | "py" | "js" | "ts" | "jsx" | "tsx"
               | "c" | "cpp" | "h" | "hpp" | "go" | "java"
               | "rb" | "php" | "cs" | "swift" | "kt")  => "≡",  // code
            Some("png" | "jpg" | "jpeg" | "gif" | "svg"
               | "webp" | "bmp" | "ico" | "tiff")        => "◉",  // image
            Some("mp3" | "flac" | "ogg" | "wav"
               | "aac" | "m4a" | "opus")                 => "♪",  // audio
            Some("mp4" | "mkv" | "avi" | "mov"
               | "webm" | "flv" | "wmv")                 => "▶",  // video
            Some("pdf" | "doc" | "docx" | "odt"
               | "rtf" | "epub")                         => "▤",  // document
            Some("zip" | "tar" | "gz" | "bz2"
               | "xz" | "7z" | "rar" | "zst")           => "◫",  // archive
            Some("md" | "txt" | "rst" | "log")           => "≡",  // text
            Some("toml" | "yaml" | "yml" | "json"
               | "xml" | "ini" | "env")                  => "◈",  // config
            Some("exe" | "so" | "dll" | "bin" | "out")  => "▪",  // binary
            Some("ttf" | "otf" | "woff" | "woff2")      => "Aa",  // font
            Some("html" | "htm" | "css")                 => "◈",  // web
            _                                            => "·",  // generic file
        },
    }
}
