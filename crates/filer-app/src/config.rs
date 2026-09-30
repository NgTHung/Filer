use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// User-selectable application theme mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    Auto,
    Light,
    Dark,
}

/// Persistent application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub bookmarks: Vec<PathBuf>,
    pub recent_paths: Vec<PathBuf>,
    pub preview_visible: bool,
    pub sidebar_width: u16,
    pub preview_width: u16,
    pub theme_mode: ThemeMode,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bookmarks: Vec::new(),
            recent_paths: Vec::new(),
            preview_visible: true,
            sidebar_width: 200,
            preview_width: 300,
            theme_mode: ThemeMode::Auto,
        }
    }
}

impl Config {
    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("filer").join("config.json"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        let Ok(data) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&data).unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(data) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, data);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serialize_roundtrip() {
        let mut cfg = Config::default();
        cfg.bookmarks.push(PathBuf::from("/home/user/projects"));
        cfg.recent_paths.push(PathBuf::from("/home/user/downloads"));
        cfg.preview_visible = false;
        cfg.theme_mode = ThemeMode::Dark;

        let json = serde_json::to_string(&cfg).unwrap();
        let back: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(back.bookmarks, cfg.bookmarks);
        assert_eq!(back.recent_paths, cfg.recent_paths);
        assert_eq!(back.preview_visible, cfg.preview_visible);
        assert_eq!(back.sidebar_width, cfg.sidebar_width);
        assert_eq!(back.theme_mode, cfg.theme_mode);
    }

    #[test]
    fn test_config_default() {
        let cfg = Config::default();
        assert!(cfg.preview_visible);
        assert!(cfg.bookmarks.is_empty());
        assert!(cfg.recent_paths.is_empty());
        assert_eq!(cfg.sidebar_width, 200);
        assert_eq!(cfg.theme_mode, ThemeMode::Auto);
    }
}
