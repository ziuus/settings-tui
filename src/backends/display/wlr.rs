use super::{
    apply_grayscale, apply_night_light, is_grayscale_active, is_night_light_active,
    read_system_brightness, write_system_brightness, DisplayBackend, Monitor,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::process::Command;

pub struct WlrRandrBackend;

impl WlrRandrBackend {
    pub fn new() -> Result<Self> {
        let output = Command::new("wlr-randr").arg("--help").output();
        match output {
            Ok(o) if o.status.success() || !o.stdout.is_empty() || !o.stderr.is_empty() => Ok(Self),
            _ => Err(anyhow!("wlr-randr binary not found or not executable")),
        }
    }

    pub fn parse_wlr_json(json_str: &str) -> Result<Vec<Monitor>> {
        let parsed: Value = serde_json::from_str(json_str)?;
        let mut monitors = Vec::new();

        if let Some(arr) = parsed.as_array() {
            for (idx, m) in arr.iter().enumerate() {
                let name = m["name"].as_str().unwrap_or("Unknown").to_string();
                let desc = m["description"].as_str().unwrap_or(&name).to_string();
                let enabled = m["enabled"].as_bool().unwrap_or(true);
                let scale = m["scale"].as_f64().unwrap_or(1.0);

                let mut width = 1920;
                let mut height = 1080;
                let mut refresh_rate = 60.0;
                let mut supported_modes = Vec::new();

                if let Some(modes) = m["modes"].as_array() {
                    for mode in modes {
                        let w = mode["width"].as_i64().unwrap_or(1920) as i32;
                        let h = mode["height"].as_i64().unwrap_or(1080) as i32;
                        let r = mode["refresh"].as_f64().unwrap_or(60.0);
                        let is_current = mode["current"].as_bool().unwrap_or(false);

                        supported_modes.push(format!("{}x{}@{:.2}", w, h, r));
                        if is_current {
                            width = w;
                            height = h;
                            refresh_rate = r;
                        }
                    }
                }

                monitors.push(Monitor {
                    id: idx as i64,
                    name,
                    description: desc,
                    width,
                    height,
                    refresh_rate,
                    scale,
                    active: enabled,
                    primary: idx == 0,
                    supported_modes,
                    mirror_of: None,
                });
            }
        }

        Ok(monitors)
    }

    pub fn parse_wlr_text(text: &str) -> Vec<Monitor> {
        let mut monitors = Vec::new();
        let mut cur_name = String::new();
        let mut cur_desc = String::new();
        let mut cur_enabled = true;
        let mut cur_scale = 1.0;
        let mut cur_width = 1920;
        let mut cur_height = 1080;
        let mut cur_refresh = 60.0;
        let mut supported_modes = Vec::new();

        let flush = |monitors: &mut Vec<Monitor>,
                     name: &mut String,
                     desc: &mut String,
                     enabled: bool,
                     scale: f64,
                     w: i32,
                     h: i32,
                     r: f64,
                     modes: &mut Vec<String>| {
            if !name.is_empty() {
                let id = monitors.len() as i64;
                monitors.push(Monitor {
                    id,
                    name: name.clone(),
                    description: if desc.is_empty() {
                        name.clone()
                    } else {
                        desc.clone()
                    },
                    width: w,
                    height: h,
                    refresh_rate: r,
                    scale,
                    active: enabled,
                    primary: id == 0,
                    supported_modes: std::mem::take(modes),
                    mirror_of: None,
                });
                name.clear();
                desc.clear();
            }
        };

        for line in text.lines() {
            let trimmed = line.trim();
            if !line.starts_with(' ') && !line.starts_with('\t') && !trimmed.is_empty() {
                // New monitor line: e.g. "eDP-1 \"BOE 0x06FB  (eDP-1)\""
                flush(
                    &mut monitors,
                    &mut cur_name,
                    &mut cur_desc,
                    cur_enabled,
                    cur_scale,
                    cur_width,
                    cur_height,
                    cur_refresh,
                    &mut supported_modes,
                );

                if let Some(first_space) = trimmed.find(' ') {
                    cur_name = trimmed[..first_space].to_string();
                    let rest = trimmed[first_space..].trim();
                    cur_desc = rest.trim_matches('"').to_string();
                } else {
                    cur_name = trimmed.to_string();
                    cur_desc = trimmed.to_string();
                }
                cur_enabled = true;
                cur_scale = 1.0;
            } else if trimmed.starts_with("Enabled:") {
                cur_enabled = trimmed.contains("yes");
            } else if trimmed.starts_with("Scale:") {
                if let Some(val) = trimmed.strip_prefix("Scale:") {
                    cur_scale = val.trim().parse::<f64>().unwrap_or(1.0);
                }
            } else if trimmed.contains("px,") && trimmed.contains("Hz") {
                // e.g. "1920x1080 px, 60.027000 Hz (preferred, current)"
                let is_current = trimmed.contains("current");
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if let Some(res) = parts.first() {
                    let dims: Vec<&str> = res.split('x').collect();
                    if dims.len() == 2 {
                        let w = dims[0].parse::<i32>().unwrap_or(1920);
                        let h = dims[1].parse::<i32>().unwrap_or(1080);
                        let hz = parts
                            .get(2)
                            .and_then(|s| s.parse::<f64>().ok())
                            .unwrap_or(60.0);
                        supported_modes.push(format!("{}x{}@{:.2}", w, h, hz));
                        if is_current {
                            cur_width = w;
                            cur_height = h;
                            cur_refresh = hz;
                        }
                    }
                }
            }
        }

        flush(
            &mut monitors,
            &mut cur_name,
            &mut cur_desc,
            cur_enabled,
            cur_scale,
            cur_width,
            cur_height,
            cur_refresh,
            &mut supported_modes,
        );

        monitors
    }
}

#[async_trait]
impl DisplayBackend for WlrRandrBackend {
    async fn get_monitors(&self) -> Result<Vec<Monitor>> {
        // Try JSON output first
        if let Ok(out) = Command::new("wlr-randr").arg("--json").output() {
            if out.status.success() {
                let json_str = String::from_utf8_lossy(&out.stdout);
                if let Ok(monitors) = Self::parse_wlr_json(&json_str) {
                    if !monitors.is_empty() {
                        return Ok(monitors);
                    }
                }
            }
        }

        // Fallback to text parsing
        let out = Command::new("wlr-randr").output()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let monitors = Self::parse_wlr_text(&text);
        Ok(monitors)
    }

    async fn set_resolution(
        &self,
        name: &str,
        width: i32,
        height: i32,
        refresh: f64,
    ) -> Result<()> {
        let mode_arg = format!("{}x{}@{:.3}Hz", width, height, refresh);
        let out = Command::new("wlr-randr")
            .arg("--output")
            .arg(name)
            .arg("--mode")
            .arg(&mode_arg)
            .output()?;

        if out.status.success() {
            Ok(())
        } else {
            let err = String::from_utf8_lossy(&out.stderr);
            Err(anyhow!(
                "wlr-randr failed to set resolution: {}",
                err.trim()
            ))
        }
    }

    async fn set_scale(&self, name: &str, scale: f64) -> Result<()> {
        let out = Command::new("wlr-randr")
            .arg("--output")
            .arg(name)
            .arg("--scale")
            .arg(format!("{:.2}", scale))
            .output()?;

        if out.status.success() {
            Ok(())
        } else {
            let err = String::from_utf8_lossy(&out.stderr);
            Err(anyhow!("wlr-randr failed to set scale: {}", err.trim()))
        }
    }

    async fn set_monitor_mode(
        &self,
        name: &str,
        mode: &str,
        _mirror_target: Option<&str>,
    ) -> Result<()> {
        let flag = if mode == "disable" || mode == "off" {
            "--off"
        } else {
            "--on"
        };
        let out = Command::new("wlr-randr")
            .arg("--output")
            .arg(name)
            .arg(flag)
            .output()?;

        if out.status.success() {
            Ok(())
        } else {
            let err = String::from_utf8_lossy(&out.stderr);
            Err(anyhow!("wlr-randr failed to set state: {}", err.trim()))
        }
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
    fn test_parse_wlr_json() {
        let raw = r#"[
            {
                "name": "eDP-1",
                "description": "BOE Display",
                "enabled": true,
                "scale": 1.25,
                "modes": [
                    { "width": 1920, "height": 1080, "refresh": 60.027, "current": true },
                    { "width": 1280, "height": 720, "refresh": 59.94, "current": false }
                ]
            }
        ]"#;

        let monitors = WlrRandrBackend::parse_wlr_json(raw).expect("should parse");
        assert_eq!(monitors.len(), 1);
        assert_eq!(monitors[0].name, "eDP-1");
        assert_eq!(monitors[0].description, "BOE Display");
        assert_eq!(monitors[0].width, 1920);
        assert_eq!(monitors[0].height, 1080);
        assert_eq!(monitors[0].scale, 1.25);
        assert!(monitors[0].active);
        assert!(monitors[0].primary);
        assert_eq!(monitors[0].supported_modes.len(), 2);
    }

    #[test]
    fn test_parse_wlr_text() {
        let raw = r#"eDP-1 "BOE 0x06FB (eDP-1)"
  Make: BOE
  Model: 0x06FB
  Physical size: 340x190 mm
  Enabled: yes
  Modes:
    1920x1080 px, 60.027000 Hz (preferred, current)
    1280x720 px, 60.000000 Hz
  Position: 0,0
  Transform: normal
  Scale: 1.000000
"#;
        let monitors = WlrRandrBackend::parse_wlr_text(raw);
        assert_eq!(monitors.len(), 1);
        assert_eq!(monitors[0].name, "eDP-1");
        assert_eq!(monitors[0].width, 1920);
        assert_eq!(monitors[0].height, 1080);
        assert_eq!(monitors[0].scale, 1.0);
        assert!(monitors[0].active);
        assert_eq!(monitors[0].supported_modes.len(), 2);
    }
}
