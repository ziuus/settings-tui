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
        let res_str = format!("{}x{}@{}", width, height, refresh);
        let status = Command::new("hyprctl")
            .arg("keyword")
            .arg("monitor")
            .arg(format!("{},{},auto,1", name, res_str))
            .status()?;

        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("Failed to set resolution"))
        }
    }
}
