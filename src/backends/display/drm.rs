use super::{
    apply_grayscale, apply_night_light, is_grayscale_active, is_night_light_active,
    read_system_brightness, write_system_brightness, DisplayBackend, Monitor,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::fs;
use std::path::Path;

pub struct DrmSysfsBackend;

impl DrmSysfsBackend {
    pub fn new() -> Result<Self> {
        let drm_path = Path::new("/sys/class/drm");
        if drm_path.is_dir() {
            Ok(Self)
        } else {
            Err(anyhow!("/sys/class/drm directory not accessible"))
        }
    }

    pub fn parse_edid_model(edid_bytes: &[u8]) -> Option<String> {
        if edid_bytes.len() < 128 {
            return None;
        }

        // EDID descriptor blocks start at offset 54, 72, 90, 108 (each 18 bytes)
        for offset in [54, 72, 90, 108] {
            if offset + 18 <= edid_bytes.len() {
                let block = &edid_bytes[offset..offset + 18];
                // Display Product Name tag is 0x00 0x00 0x00 0xFC 0x00
                if block[0] == 0 && block[1] == 0 && block[2] == 0 && block[3] == 0xFC {
                    let text = String::from_utf8_lossy(&block[5..18]);
                    let clean =
                        text.trim_matches(|c| c == '\n' || c == '\r' || c == '\0' || c == ' ');
                    if !clean.is_empty() {
                        return Some(clean.to_string());
                    }
                }
            }
        }
        None
    }

    pub fn scan_drm_monitors(base_dir: &Path) -> Vec<Monitor> {
        let mut monitors = Vec::new();

        if let Ok(entries) = fs::read_dir(base_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                // Filter connector dirs like "card0-eDP-1", "card0-HDMI-A-1", "card1-DP-1"
                if !name.starts_with("card") || !name.contains('-') {
                    continue;
                }

                let conn_path = entry.path();
                let status_path = conn_path.join("status");
                if let Ok(status) = fs::read_to_string(&status_path) {
                    if status.trim() == "connected" {
                        let connector_name = name
                            .split_once('-')
                            .map(|(_, c)| c)
                            .unwrap_or(&name)
                            .to_string();

                        let mut width = 1920;
                        let mut height = 1080;
                        let mut supported_modes = Vec::new();

                        let modes_path = conn_path.join("modes");
                        if let Ok(modes_str) = fs::read_to_string(&modes_path) {
                            for line in modes_str.lines() {
                                let trimmed = line.trim();
                                if trimmed.contains('x') {
                                    supported_modes.push(trimmed.to_string());
                                }
                            }
                        }

                        if let Some(first_mode) = supported_modes.first() {
                            let dims: Vec<&str> = first_mode.split('x').collect();
                            if dims.len() == 2 {
                                width = dims[0].parse().unwrap_or(1920);
                                height = dims[1].parse().unwrap_or(1080);
                            }
                        }

                        let mut desc = connector_name.clone();
                        let edid_path = conn_path.join("edid");
                        if let Ok(edid_bytes) = fs::read(&edid_path) {
                            if let Some(model) = Self::parse_edid_model(&edid_bytes) {
                                desc = format!("{} ({})", model, connector_name);
                            }
                        }

                        let id = monitors.len() as i64;
                        monitors.push(Monitor {
                            id,
                            name: connector_name,
                            description: desc,
                            width,
                            height,
                            refresh_rate: 60.0,
                            scale: 1.0,
                            active: true,
                            primary: id == 0,
                            supported_modes,
                            mirror_of: None,
                        });
                    }
                }
            }
        }

        monitors
    }
}

#[async_trait]
impl DisplayBackend for DrmSysfsBackend {
    async fn get_monitors(&self) -> Result<Vec<Monitor>> {
        let monitors = Self::scan_drm_monitors(Path::new("/sys/class/drm"));
        Ok(monitors)
    }

    async fn set_resolution(
        &self,
        _name: &str,
        _width: i32,
        _height: i32,
        _refresh: f64,
    ) -> Result<()> {
        Err(anyhow!(
            "Resolution switching requires an active window manager / compositor (wlr-randr, xrandr, or Hyprland)"
        ))
    }

    async fn set_scale(&self, _name: &str, _scale: f64) -> Result<()> {
        Err(anyhow!(
            "Display scaling requires an active compositor (wlr-randr, xrandr, or Hyprland)"
        ))
    }

    async fn set_monitor_mode(
        &self,
        _name: &str,
        _mode: &str,
        _mirror_target: Option<&str>,
    ) -> Result<()> {
        Err(anyhow!(
            "Monitor enable/disable requires an active compositor"
        ))
    }

    async fn get_brightness(&self) -> Result<Option<u32>> {
        Ok(read_system_brightness())
    }

    async fn set_brightness(&self, percent: u32) -> Result<()> {
        write_system_brightness(percent)
    }

    async fn is_night_light_enabled(&self) -> Result<bool> {
        is_night_light_active()
    }

    async fn set_night_light_enabled(&self, enabled: bool, temperature: u32) -> Result<()> {
        apply_night_light(enabled, temperature)
    }

    async fn is_grayscale_enabled(&self) -> Result<Option<bool>> {
        is_grayscale_active()
    }

    async fn set_grayscale_enabled(&self, enabled: bool) -> Result<()> {
        apply_grayscale(enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_edid_model_none_on_short() {
        assert_eq!(DrmSysfsBackend::parse_edid_model(&[0u8; 64]), None);
    }

    #[test]
    fn test_parse_edid_model_valid() {
        let mut edid = vec![0u8; 128];
        // Place a descriptor at offset 54
        edid[54] = 0;
        edid[55] = 0;
        edid[56] = 0;
        edid[57] = 0xFC; // Product Name
        edid[58] = 0;
        let name_bytes = b"Dell U2720Q\n  ";
        edid[59..59 + name_bytes.len()].copy_from_slice(name_bytes);

        let parsed = DrmSysfsBackend::parse_edid_model(&edid);
        assert_eq!(parsed, Some("Dell U2720Q".to_string()));
    }
}
