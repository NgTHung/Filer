//! # Device Label
//!
//! Turns a request User-Agent into the short, human-readable name shown in the
//! session list. The label is captured once, at session creation, so a later
//! User-Agent change never rewrites it; a request that sends no recognizable
//! User-Agent still gets a stable placeholder instead of an empty cell.
//!
//! ```
//! use filer_task_web::device_label::{FALLBACK_LABEL, from_user_agent};
//!
//! assert_eq!(from_user_agent(None), FALLBACK_LABEL);
//! ```

pub const FALLBACK_LABEL: &str = "Unknown browser";
pub const RECOVERY_CLI_LABEL: &str = "Recovery CLI";

const BROWSERS: &[(&str, &str)] = &[
    ("Edg/", "Edge"),
    ("Chrome/", "Chrome"),
    ("Firefox/", "Firefox"),
    ("Safari/", "Safari"),
    ("curl/", "curl"),
];

pub fn from_user_agent(user_agent: Option<&str>) -> String {
    let Some(user_agent) = user_agent.map(str::trim).filter(|value| !value.is_empty()) else {
        return FALLBACK_LABEL.to_string();
    };
    for (token, name) in BROWSERS {
        if !user_agent.contains(token) {
            continue;
        }
        // Safari's version lives in Version/; the value after Safari/ is the
        // WebKit engine number, which would lie about the browser.
        let source = if *name == "Safari" {
            user_agent.split_once("Version/").map(|(_, rest)| rest)
        } else {
            user_agent.split_once(token).map(|(_, rest)| rest)
        };
        return match source.and_then(major_version) {
            Some(version) => format!("{name} {version}"),
            None => FALLBACK_LABEL.to_string(),
        };
    }
    FALLBACK_LABEL.to_string()
}

fn major_version(rest: &str) -> Option<&str> {
    let major = rest.trim_start().split('.').next()?;
    (!major.is_empty() && major.bytes().all(|byte| byte.is_ascii_digit())).then_some(major)
}
