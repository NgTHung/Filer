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

/// Time group categories for file grouping
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeGroup {
    /// Within the last hour
    LastHour,
    /// Earlier today (more than 1 hour ago, but same day)
    Today,
    /// Yesterday
    Yesterday,
    /// Within the last 7 days (excluding today/yesterday)
    ThisWeek,
    /// Within the last 30 days (excluding this week)
    ThisMonth,
    /// Within the last 365 days (excluding this month)
    ThisYear,
    /// Within the last 10 years
    LastDecade,
    /// More than 10 years ago
    Older,
    /// Unknown/invalid time
    Unknown,
}

impl TimeGroup {
    /// Get display name for the time group
    pub fn display_name(&self) -> &'static str {
        match self {
            TimeGroup::LastHour => "Last hour",
            TimeGroup::Today => "Today",
            TimeGroup::Yesterday => "Yesterday",
            TimeGroup::ThisWeek => "This week",
            TimeGroup::ThisMonth => "This month",
            TimeGroup::ThisYear => "This year",
            TimeGroup::LastDecade => "Last 10 years",
            TimeGroup::Older => "Older",
            TimeGroup::Unknown => "Unknown",
        }
    }

    /// Get sort order (lower = more recent)
    pub fn sort_order(&self) -> u8 {
        match self {
            TimeGroup::LastHour => 0,
            TimeGroup::Today => 1,
            TimeGroup::Yesterday => 2,
            TimeGroup::ThisWeek => 3,
            TimeGroup::ThisMonth => 4,
            TimeGroup::ThisYear => 5,
            TimeGroup::LastDecade => 6,
            TimeGroup::Older => 7,
            TimeGroup::Unknown => 8,
        }
    }
}

/// Group a SystemTime into a time category
pub fn time_group(time: SystemTime) -> TimeGroup {
    let now = SystemTime::now();
    
    let duration = match now.duration_since(time) {
        Ok(d) => d,
        Err(_) => return TimeGroup::Unknown, // Future time
    };
    
    let secs = duration.as_secs();
    
    const HOUR: u64 = 3600;
    const DAY: u64 = 86400;
    const WEEK: u64 = 7 * DAY;
    const MONTH: u64 = 30 * DAY;
    const YEAR: u64 = 365 * DAY;
    const DECADE: u64 = 10 * YEAR;
    
    if secs < HOUR {
        TimeGroup::LastHour
    } else if secs < DAY {
        TimeGroup::Today
    } else if secs < 2 * DAY {
        TimeGroup::Yesterday
    } else if secs < WEEK {
        TimeGroup::ThisWeek
    } else if secs < MONTH {
        TimeGroup::ThisMonth
    } else if secs < YEAR {
        TimeGroup::ThisYear
    } else if secs < DECADE {
        TimeGroup::LastDecade
    } else {
        TimeGroup::Older
    }
}

/// Group a SystemTime into a time category, returning the display name
pub fn time_group_name(time: SystemTime) -> &'static str {
    time_group(time).display_name()
}

/// Group an optional SystemTime, defaulting to Unknown
pub fn time_group_opt(time: Option<SystemTime>) -> TimeGroup {
    match time {
        Some(t) => time_group(t),
        None => TimeGroup::Unknown,
    }
}
