use super::{DisplayBackend, Monitor};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::process::Command;

pub struct HyprlandBackend;

impl HyprlandBackend {
    pub fn new() -> Result<Self> {
        // Check if hyprctl exists
        match Command::new("hyprctl").arg("version").output() {
            Ok(output) if output.status.success() => Ok(Self),
            _ => Err(anyhow!("Hyprland not detected")),
        }
    }
}

#[async_trait]
impl DisplayBackend for HyprlandBackend {
    async fn get_monitors(&self) -> Result<Vec<Monitor>> {
        let output = Command::new("hyprctl").arg("monitors").arg("-j").output()?;
        let json_str = String::from_utf8_lossy(&output.stdout);
        let parsed: Value = serde_json::from_str(&json_str)?;

        let mut monitors = Vec::new();

        if let Some(arr) = parsed.as_array() {
            for m in arr {
                let mut supported_modes = Vec::new();
                if let Some(modes) = m["availableModes"].as_array() {
                    for mode in modes {
                        if let Some(s) = mode.as_str() {
                            supported_modes.push(s.to_string());
                        }
                    }
                }

                monitors.push(Monitor {
                    id: m["id"].as_i64().unwrap_or(0),
                    name: m["name"].as_str().unwrap_or("Unknown").to_string(),
                    description: m["description"].as_str().unwrap_or("Unknown").to_string(),
                    width: m["width"].as_i64().unwrap_or(1920) as i32,
                    height: m["height"].as_i64().unwrap_or(1080) as i32,
                    refresh_rate: m["refreshRate"].as_f64().unwrap_or(60.0),
                    scale: m["scale"].as_f64().unwrap_or(1.0),
                    active: m["disabled"].as_bool().map(|d| !d).unwrap_or(true),
                    primary: false, // hyprland doesn't explicitly track primary in the same way, assume index 0 or focused
                    supported_modes,
                });
            }
        }

        if !monitors.is_empty() {
            monitors[0].primary = true;
        }

        Ok(monitors)
    }

    async fn set_resolution(
        &self,
        name: &str,
        width: i32,
        height: i32,
        refresh: f64,
    ) -> Result<()> {
        let scale = if let Ok(monitors) = self.get_monitors().await {
            monitors
                .into_iter()
                .find(|m| m.name == name)
                .map(|m| m.scale)
                .unwrap_or(1.0)
        } else {
            1.0
        };
        let res_str = format!("{}x{}@{}", width, height, refresh);
        let status = Command::new("hyprctl")
            .arg("keyword")
            .arg("monitor")
            .arg(format!("{},{},auto,{}", name, res_str, scale))
            .status()?;

        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("Failed to set resolution"))
        }
    }

    async fn get_brightness(&self) -> Result<Option<u32>> {
        Ok(read_system_brightness())
    }

    async fn set_brightness(&self, percent: u32) -> Result<()> {
        write_system_brightness(percent)
    }
}

#[derive(Default)]
pub struct GenericDisplayBackend;

impl GenericDisplayBackend {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl DisplayBackend for GenericDisplayBackend {
    async fn get_monitors(&self) -> Result<Vec<Monitor>> {
        Ok(Vec::new())
    }

    async fn set_resolution(
        &self,
        _name: &str,
        _width: i32,
        _height: i32,
        _refresh: f64,
    ) -> Result<()> {
        Err(anyhow!("Resolution switching requires Hyprland compositor"))
    }

    async fn get_brightness(&self) -> Result<Option<u32>> {
        Ok(read_system_brightness())
    }

    async fn set_brightness(&self, percent: u32) -> Result<()> {
        write_system_brightness(percent)
    }
}

fn read_system_brightness() -> Option<u32> {
    // 1. Try brightnessctl
    if let Ok(output) = Command::new("brightnessctl").arg("g").output() {
        if output.status.success() {
            if let Ok(max_out) = Command::new("brightnessctl").arg("m").output() {
                if max_out.status.success() {
                    let cur: f64 = String::from_utf8_lossy(&output.stdout)
                        .trim()
                        .parse()
                        .unwrap_or(0.0);
                    let max: f64 = String::from_utf8_lossy(&max_out.stdout)
                        .trim()
                        .parse()
                        .unwrap_or(1.0);
                    if max > 0.0 {
                        let pct = ((cur / max) * 100.0).round() as u32;
                        return Some(pct.min(100));
                    }
                }
            }
        }
    }

    // 2. Fallback to /sys/class/backlight sysfs
    if let Ok(entries) = std::fs::read_dir("/sys/class/backlight") {
        for entry in entries.flatten() {
            let cur_path = entry.path().join("brightness");
            let max_path = entry.path().join("max_brightness");
            if let (Ok(cur_str), Ok(max_str)) = (
                std::fs::read_to_string(cur_path),
                std::fs::read_to_string(max_path),
            ) {
                if let (Ok(cur), Ok(max)) =
                    (cur_str.trim().parse::<f64>(), max_str.trim().parse::<f64>())
                {
                    if max > 0.0 {
                        let pct = ((cur / max) * 100.0).round() as u32;
                        return Some(pct.min(100));
                    }
                }
            }
        }
    }

    None
}

fn write_system_brightness(percent: u32) -> Result<()> {
    let clamped = percent.min(100);
    let output = Command::new("brightnessctl")
        .arg("s")
        .arg(format!("{}%", clamped))
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        let msg = err
            .lines()
            .next()
            .unwrap_or("Failed to set brightness")
            .trim();
        Err(anyhow!("{}", msg))
    }
}
