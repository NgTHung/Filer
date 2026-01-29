/// Format bytes as human-readable string (e.g., "1.5 MB")
pub fn format_size(bytes: u64) -> String {
    let sf = match bytes {
        0..=256 => SizeUnit::Bytes,
        257..=262144 => SizeUnit::Kilobytes,
        262145..=268435456 => SizeUnit::Megabytes,
        268435457..=274877906944 => SizeUnit::Gigabytes,
        _ => SizeUnit::Terabytes,
    };
    let ro: f64 = (((bytes as f64) / (sf.multiplier() as f64)) * 1000.0).round() / 1000.0;
    format!("{ro} {}", sf.abbrev())
}

/// Parse human-readable size string to bytes (e.g., "1.5 MB" -> 1572864)
pub fn parse_size(s: &str) -> Option<u64> {
    let mut pos = s.to_uppercase().trim().to_string();
    while let Some(_) = pos.find("  ") {
        pos = pos.replace("  ", " ");
    }
    if let Some(fin) = pos.strip_suffix('B') {
        if let Some(fi) = fin.strip_suffix('K') {
            let fi = fi.trim();
            match fi.parse::<f64>() {
                Ok(v) => return Some((v * 1024f64) as u64),
                Err(_) => return None,
            }
        }
        if let Some(fi) = fin.strip_suffix('M') {
            let fi = fi.trim();
            match fi.parse::<f64>() {
                Ok(v) => return Some((v * 1048576f64) as u64),
                Err(_) => return None,
            }
        }
        if let Some(fi) = fin.strip_suffix('G') {
            let fi = fi.trim();
            match fi.parse::<f64>() {
                Ok(v) => return Some((v * 1073741824f64) as u64),
                Err(_) => return None,
            }
        }
        if let Some(fi) = fin.strip_suffix('T') {
            let fi = fi.trim();
            match fi.parse::<f64>() {
                Ok(v) => return Some((v * 1099511627776f64) as u64),
                Err(_) => return None,
            }
        }
        let fin = fin.trim();
        match fin.parse::<u64>() {
            Ok(v) => return Some(v),
            Err(_) => return None,
        }
    }
    match pos.parse::<u64>() {
        Ok(v) => return Some(v),
        Err(_) => return None,
    }
}

/// Size units for formatting
#[derive(Debug, Clone, Copy)]
pub enum SizeUnit {
    Bytes,
    Kilobytes,
    Megabytes,
    Gigabytes,
    Terabytes,
}

impl SizeUnit {
    /// Get the multiplier for this unit
    pub fn multiplier(&self) -> u64 {
        match self {
            SizeUnit::Bytes => 1,
            SizeUnit::Kilobytes => 1024,
            SizeUnit::Megabytes => 1024 * 1024,
            SizeUnit::Gigabytes => 1024 * 1024 * 1024,
            SizeUnit::Terabytes => 1024u64 * 1024 * 1024 * 1024,
        }
    }

    /// Get the abbreviation for this unit
    pub fn abbrev(&self) -> &'static str {
        match self {
            SizeUnit::Bytes => "B",
            SizeUnit::Kilobytes => "KB",
            SizeUnit::Megabytes => "MB",
            SizeUnit::Gigabytes => "GB",
            SizeUnit::Terabytes => "TB",
        }
    }
}

/// Size group categories for file grouping
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SizeGroup {
    /// Empty files (0 bytes)
    Empty,
    /// Tiny files (1 B - 10 KB)
    Tiny,
    /// Small files (10 KB - 1 MB)
    Small,
    /// Medium files (1 MB - 100 MB)
    Medium,
    /// Large files (100 MB - 1 GB)
    Large,
    /// Huge files (1 GB - 10 GB)
    Huge,
    /// Massive files (> 10 GB)
    Massive,
}

impl SizeGroup {
    /// Get display name for the size group
    pub fn display_name(&self) -> &'static str {
        match self {
            SizeGroup::Empty => "Empty",
            SizeGroup::Tiny => "Tiny (< 10 KB)",
            SizeGroup::Small => "Small (10 KB - 1 MB)",
            SizeGroup::Medium => "Medium (1 MB - 100 MB)",
            SizeGroup::Large => "Large (100 MB - 1 GB)",
            SizeGroup::Huge => "Huge (1 GB - 10 GB)",
            SizeGroup::Massive => "Massive (> 10 GB)",
        }
    }

    /// Get short display name
    pub fn short_name(&self) -> &'static str {
        match self {
            SizeGroup::Empty => "Empty",
            SizeGroup::Tiny => "Tiny",
            SizeGroup::Small => "Small",
            SizeGroup::Medium => "Medium",
            SizeGroup::Large => "Large",
            SizeGroup::Huge => "Huge",
            SizeGroup::Massive => "Massive",
        }
    }

    /// Get sort order (lower = smaller)
    pub fn sort_order(&self) -> u8 {
        match self {
            SizeGroup::Empty => 0,
            SizeGroup::Tiny => 1,
            SizeGroup::Small => 2,
            SizeGroup::Medium => 3,
            SizeGroup::Large => 4,
            SizeGroup::Huge => 5,
            SizeGroup::Massive => 6,
        }
    }

    /// Get the minimum size (inclusive) for this group
    pub fn min_bytes(&self) -> u64 {
        match self {
            SizeGroup::Empty => 0,
            SizeGroup::Tiny => 1,
            SizeGroup::Small => 10 * 1024,                    // 10 KB
            SizeGroup::Medium => 1024 * 1024,                 // 1 MB
            SizeGroup::Large => 100 * 1024 * 1024,            // 100 MB
            SizeGroup::Huge => 1024 * 1024 * 1024,            // 1 GB
            SizeGroup::Massive => 10 * 1024 * 1024 * 1024,    // 10 GB
        }
    }

    /// Get the maximum size (exclusive) for this group, None for Massive
    pub fn max_bytes(&self) -> Option<u64> {
        match self {
            SizeGroup::Empty => Some(1),
            SizeGroup::Tiny => Some(10 * 1024),
            SizeGroup::Small => Some(1024 * 1024),
            SizeGroup::Medium => Some(100 * 1024 * 1024),
            SizeGroup::Large => Some(1024 * 1024 * 1024),
            SizeGroup::Huge => Some(10 * 1024 * 1024 * 1024),
            SizeGroup::Massive => None,
        }
    }
}

/// Group a file size into a size category
pub fn size_group(bytes: u64) -> SizeGroup {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    
    match bytes {
        0 => SizeGroup::Empty,
        1..=10239 => SizeGroup::Tiny,                           // < 10 KB
        10240..=1048575 => SizeGroup::Small,                    // 10 KB - 1 MB
        1048576..=104857599 => SizeGroup::Medium,               // 1 MB - 100 MB
        104857600..=1073741823 => SizeGroup::Large,             // 100 MB - 1 GB
        1073741824..=10737418239 => SizeGroup::Huge,            // 1 GB - 10 GB
        _ => SizeGroup::Massive,                                // > 10 GB
    }
}

/// Group a file size into a size category, returning the display name
pub fn size_group_name(bytes: u64) -> &'static str {
    size_group(bytes).display_name()
}

/// Group a file size into a size category, returning the short name
pub fn size_group_short_name(bytes: u64) -> &'static str {
    size_group(bytes).short_name()
}
