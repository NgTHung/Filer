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
