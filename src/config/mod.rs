use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SettingsConfig {
    pub default_category: Option<String>,
    pub confirm_power_actions: bool,
    pub confirm_bluetooth_remove: bool,
    pub poll_interval_secs: u64,
    pub telemetry_enabled: bool,
    pub machine_id: String,
}

impl Default for SettingsConfig {
    fn default() -> Self {
        Self {
            default_category: None,
            confirm_power_actions: true,
            confirm_bluetooth_remove: true,
            poll_interval_secs: 5,
            telemetry_enabled: true,
            machine_id: uuid::Uuid::new_v4().to_string(),
        }
    }
}

impl SettingsConfig {
    /// Resolves the default configuration file location:
    /// $XDG_CONFIG_HOME/settings-tui/config.json or ~/.config/settings-tui/config.json
    pub fn default_path() -> Option<PathBuf> {
        if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
            if !config_home.is_empty() {
                return Some(PathBuf::from(config_home).join("settings-tui/config.json"));
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            if !home.is_empty() {
                return Some(PathBuf::from(home).join(".config/settings-tui/config.json"));
            }
        }
        None
    }

    /// Loads configuration from the specified path or the standard default path.
    /// Returns default configuration if the file does not exist or cannot be parsed.
    pub fn load(custom_path: Option<&Path>) -> Self {
        let path = match custom_path {
            Some(p) => Some(p.to_path_buf()),
            None => Self::default_path(),
        };

        if let Some(path) = path {
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(cfg) = serde_json::from_str::<SettingsConfig>(&content) {
                        return cfg;
                    }
                }
            }
        }

        Self::default()
    }

    /// Atomically persists configuration to disk using crash-safe atomic write with 0o600 permissions.
    #[allow(dead_code)]
    pub fn save(&self, custom_path: Option<&Path>) -> std::io::Result<()> {
        let path = custom_path
            .map(|p| p.to_path_buf())
            .or_else(Self::default_path)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Unable to resolve configuration directory path",
                )
            })?;

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

        crate::security::atomic_write(&path, json.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let cfg = SettingsConfig::default();
        assert!(cfg.confirm_power_actions);
        assert!(cfg.confirm_bluetooth_remove);
        assert_eq!(cfg.poll_interval_secs, 5);
        assert_eq!(cfg.default_category, None);
    }

    #[test]
    fn test_config_save_and_load_roundtrip() {
        let temp_dir =
            std::env::temp_dir().join(format!("settings_cfg_test_{}", std::process::id()));
        let config_file = temp_dir.join("test_settings.json");

        let original = SettingsConfig {
            default_category: Some("Sound".to_string()),
            poll_interval_secs: 10,
            ..Default::default()
        };

        assert!(original.save(Some(&config_file)).is_ok());
        assert!(config_file.exists());

        let loaded = SettingsConfig::load(Some(&config_file));
        assert_eq!(loaded, original);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_config_load_nonexistent_returns_default() {
        let non_existent = Path::new("/tmp/definitely_not_a_valid_config_file_path_123.json");
        let cfg = SettingsConfig::load(Some(non_existent));
        let def = SettingsConfig { machine_id: cfg.machine_id.clone(), ..Default::default() };
        assert_eq!(cfg, def);
    }
}
