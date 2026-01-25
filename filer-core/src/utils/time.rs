use std::time::{SystemTime, UNIX_EPOCH, Duration};

/// Format SystemTime as human-readable string
pub fn format_time(time: SystemTime) -> String {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => {
            let secs = duration.as_secs();
            let datetime = chrono::DateTime::from_timestamp(secs as i64, 0);
            match datetime {
                Some(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
                None => "Invalid time".to_string(),
            }
        }
        Err(_) => "Invalid time".to_string(),
    }
}

/// Format SystemTime as relative string (e.g., "2 hours ago")
pub fn format_relative(time: SystemTime) -> String {
    let now = SystemTime::now();
    
    match now.duration_since(time) {
        Ok(duration) => {
            let secs = duration.as_secs();
            
            if secs < 60 {
                format!("{} seconds ago", secs)
            } else if secs < 3600 {
                let mins = secs / 60;
                if mins == 1 {
                    "1 minute ago".to_string()
                } else {
                    format!("{} minutes ago", mins)
                }
            } else if secs < 86400 {
                let hours = secs / 3600;
                if hours == 1 {
                    "1 hour ago".to_string()
                } else {
                    format!("{} hours ago", hours)
                }
            } else {
                let days = secs / 86400;
                if days == 1 {
                    "1 day ago".to_string()
                } else {
                    format!("{} days ago", days)
                }
            }
        }
        Err(_) => {
            // Time is in the future
            match time.duration_since(now) {
                Ok(duration) => {
                    let secs = duration.as_secs();
                    
                    if secs < 60 {
                        format!("in {} seconds", secs)
                    } else if secs < 3600 {
                        let mins = secs / 60;
                        if mins == 1 {
                            "in 1 minute".to_string()
                        } else {
                            format!("in {} minutes", mins)
                        }
                    } else if secs < 86400 {
                        let hours = secs / 3600;
                        if hours == 1 {
                            "in 1 hour".to_string()
                        } else {
                            format!("in {} hours", hours)
                        }
                    } else {
                        let days = secs / 86400;
                        if days == 1 {
                            "in 1 day".to_string()
                        } else {
                            format!("in {} days", days)
                        }
                    }
                }
                Err(_) => "now".to_string(),
            }
        }
    }
}

/// Format duration in seconds as human-readable string (e.g., "1:23:45")
pub fn format_duration(seconds: f64) -> String {
    let total_secs = seconds.floor() as u64;
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    
    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, mins, secs)
    } else {
        format!("{}:{:02}", mins, secs)
    }
}

/// Parse duration string to seconds (e.g., "1:23:45" -> 5025.0)
pub fn parse_duration(s: &str) -> Option<f64> {
    let parts: Vec<&str> = s.split(':').collect();
    
    match parts.len() {
        2 => {
            // MM:SS format
            let mins = parts[0].parse::<u64>().ok()?;
            let secs = parts[1].parse::<u64>().ok()?;
            Some((mins * 60 + secs) as f64)
        }
        3 => {
            // HH:MM:SS format
            let hours = parts[0].parse::<u64>().ok()?;
            let mins = parts[1].parse::<u64>().ok()?;
            let secs = parts[2].parse::<u64>().ok()?;
            Some((hours * 3600 + mins * 60 + secs) as f64)
        }
        _ => None,
    }
}
