use super::{InputBackend, InputSettings};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::process::Command;

pub struct HyprlandInputBackend;

impl HyprlandInputBackend {
    pub fn new() -> Option<Self> {
        match Command::new("hyprctl").arg("version").output() {
            Ok(output) if output.status.success() => Some(Self),
            _ => None,
        }
    }

    pub fn parse_bool_option(output_str: &str) -> bool {
        for line in output_str.lines() {
            let trimmed = line.trim();
            if let Some(val) = trimmed.strip_prefix("bool:") {
                return val.trim() == "true" || val.trim() == "1";
            }
        }
        false
    }

    pub fn parse_float_option(output_str: &str) -> f64 {
        for line in output_str.lines() {
            let trimmed = line.trim();
            if let Some(val) = trimmed.strip_prefix("float:") {
                if let Ok(v) = val.trim().parse::<f64>() {
                    return v;
                }
            }
        }
        0.0
    }

    fn query_option(&self, opt: &str) -> Result<String> {
        let output = Command::new("hyprctl").arg("getoption").arg(opt).output()?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(anyhow!("Failed to query hyprctl option {}", opt))
        }
    }

    fn set_keyword(&self, key: &str, val: &str) -> Result<()> {
        let output = Command::new("hyprctl")
            .arg("keyword")
            .arg(key)
            .arg(val)
            .output()?;
        if output.status.success() {
            Ok(())
        } else {
            let err = String::from_utf8_lossy(&output.stderr).to_string();
            Err(anyhow!("Failed to set {}: {}", key, err.trim()))
        }
    }
}

#[async_trait]
impl InputBackend for HyprlandInputBackend {
    async fn get_settings(&self) -> Result<InputSettings> {
        let natural_str = self
            .query_option("input:touchpad:natural_scroll")
            .unwrap_or_default();
        let tap_str = self
            .query_option("input:touchpad:tap-to-click")
            .unwrap_or_default();
        let left_str = self.query_option("input:left_handed").unwrap_or_default();
        let sens_str = self.query_option("input:sensitivity").unwrap_or_default();

        Ok(InputSettings {
            natural_scroll: Self::parse_bool_option(&natural_str),
            tap_to_click: Self::parse_bool_option(&tap_str),
            left_handed: Self::parse_bool_option(&left_str),
            sensitivity: Self::parse_float_option(&sens_str),
        })
    }

    async fn set_natural_scroll(&self, enabled: bool) -> Result<()> {
        self.set_keyword(
            "input:touchpad:natural_scroll",
            if enabled { "true" } else { "false" },
        )
    }

    async fn set_tap_to_click(&self, enabled: bool) -> Result<()> {
        self.set_keyword(
            "input:touchpad:tap-to-click",
            if enabled { "true" } else { "false" },
        )
    }

    async fn set_left_handed(&self, enabled: bool) -> Result<()> {
        self.set_keyword("input:left_handed", if enabled { "true" } else { "false" })
    }

    async fn set_sensitivity(&self, sensitivity: f64) -> Result<()> {
        let clamped = sensitivity.clamp(-1.0, 1.0);
        self.set_keyword("input:sensitivity", &format!("{:.2}", clamped))
    }
}

pub struct GsettingsInputBackend;

impl GsettingsInputBackend {
    pub fn new() -> Option<Self> {
        match Command::new("gsettings").arg("help").output() {
            Ok(output) if output.status.success() => Some(Self),
            _ => None,
        }
    }

    fn get_bool(schema: &str, key: &str) -> bool {
        if let Ok(output) = Command::new("gsettings")
            .args(["get", schema, key])
            .output()
        {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            s == "true"
        } else {
            false
        }
    }

    fn get_float(schema: &str, key: &str) -> f64 {
        if let Ok(output) = Command::new("gsettings")
            .args(["get", schema, key])
            .output()
        {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            s.parse::<f64>().unwrap_or(0.0)
        } else {
            0.0
        }
    }

    fn set_val(schema: &str, key: &str, val: &str) -> Result<()> {
        let output = Command::new("gsettings")
            .args(["set", schema, key, val])
            .output()?;
        if output.status.success() {
            Ok(())
        } else {
            let err = String::from_utf8_lossy(&output.stderr).to_string();
            Err(anyhow!(
                "Failed to set gsettings {} {}: {}",
                schema,
                key,
                err.trim()
            ))
        }
    }
}

#[async_trait]
impl InputBackend for GsettingsInputBackend {
    async fn get_settings(&self) -> Result<InputSettings> {
        Ok(InputSettings {
            natural_scroll: Self::get_bool(
                "org.gnome.desktop.peripherals.touchpad",
                "natural-scroll",
            ),
            tap_to_click: Self::get_bool("org.gnome.desktop.peripherals.touchpad", "tap-to-click"),
            left_handed: Self::get_bool("org.gnome.desktop.peripherals.mouse", "left-handed"),
            sensitivity: Self::get_float("org.gnome.desktop.peripherals.mouse", "speed"),
        })
    }

    async fn set_natural_scroll(&self, enabled: bool) -> Result<()> {
        Self::set_val(
            "org.gnome.desktop.peripherals.touchpad",
            "natural-scroll",
            if enabled { "true" } else { "false" },
        )
    }

    async fn set_tap_to_click(&self, enabled: bool) -> Result<()> {
        Self::set_val(
            "org.gnome.desktop.peripherals.touchpad",
            "tap-to-click",
            if enabled { "true" } else { "false" },
        )
    }

    async fn set_left_handed(&self, enabled: bool) -> Result<()> {
        Self::set_val(
            "org.gnome.desktop.peripherals.mouse",
            "left-handed",
            if enabled { "true" } else { "false" },
        )
    }

    async fn set_sensitivity(&self, sensitivity: f64) -> Result<()> {
        let clamped = sensitivity.clamp(-1.0, 1.0);
        Self::set_val(
            "org.gnome.desktop.peripherals.mouse",
            "speed",
            &format!("{:.2}", clamped),
        )
    }
}

pub fn create_input_backend() -> Option<Box<dyn InputBackend>> {
    if let Some(hypr) = HyprlandInputBackend::new() {
        Some(Box::new(hypr))
    } else {
        GsettingsInputBackend::new().map(|g| Box::new(g) as Box<dyn InputBackend>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hyprland_bool_option() {
        let raw = "bool: true\nset: true\n";
        assert!(HyprlandInputBackend::parse_bool_option(raw));

        let raw_false = "bool: false\nset: false\n";
        assert!(!HyprlandInputBackend::parse_bool_option(raw_false));
    }

    #[test]
    fn test_parse_hyprland_float_option() {
        let raw = "float: 0.500000\nset: true\n";
        let val = HyprlandInputBackend::parse_float_option(raw);
        assert!((val - 0.5).abs() < 1e-4);

        let raw_zero = "float: 0.000000\nset: false\n";
        assert_eq!(HyprlandInputBackend::parse_float_option(raw_zero), 0.0);
    }
}
