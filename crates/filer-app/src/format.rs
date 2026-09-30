use std::time::SystemTime;

/// Format a byte count as a human-readable string.
pub fn size_human(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes == 0 {
        return "—".to_string();
    }
    if bytes < KB {
        format!("{bytes} B")
    } else if bytes < MB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    }
}

/// Format an optional `SystemTime` as a relative or absolute date string.
pub fn time_relative(t: Option<SystemTime>) -> String {
    let Some(t) = t else {
        return "—".to_string();
    };
    let now = SystemTime::now();
    let Ok(elapsed) = now.duration_since(t) else {
        return "—".to_string();
    };

    let secs = elapsed.as_secs();
    if secs < 60 {
        return "Just now".to_string();
    }
    if secs < 3600 {
        return format!("{} min ago", secs / 60);
    }
    if secs < 86400 {
        return format!("{} hr ago", secs / 3600);
    }
    if secs < 86400 * 2 {
        return "Yesterday".to_string();
    }
    if secs < 86400 * 7 {
        return format!("{} days ago", secs / 86400);
    }

    let unix = t
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, m, d) = epoch_to_ymd(unix / 86400);
    format!("{y}-{m:02}-{d:02}")
}

fn epoch_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let dy = if is_leap(year) { 366 } else { 365 };
        if days < dy {
            break;
        }
        days -= dy;
        year += 1;
    }
    let months = [
        31u64,
        if is_leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u64;
    for dm in &months {
        if days < *dm {
            break;
        }
        days -= dm;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_zero() {
        assert_eq!(size_human(0), "—");
    }

    #[test]
    fn test_size_bytes() {
        assert_eq!(size_human(512), "512 B");
    }

    #[test]
    fn test_size_kb() {
        assert_eq!(size_human(1536), "1.5 KB");
    }

    #[test]
    fn test_size_mb() {
        assert_eq!(size_human(2 * 1024 * 1024), "2.0 MB");
    }

    #[test]
    fn test_size_gb() {
        assert!(size_human(3 * 1024 * 1024 * 1024).contains("GB"));
    }

    #[test]
    fn test_time_none() {
        assert_eq!(time_relative(None), "—");
    }
}
