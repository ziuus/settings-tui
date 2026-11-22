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

pub struct SwayInputBackend;

impl SwayInputBackend {
    pub fn new() -> Option<Self> {
        if std::env::var("SWAYSOCK").is_ok() {
            if let Ok(out) = Command::new("swaymsg").arg("-v").output() {
                if out.status.success() {
                    return Some(Self);
                }
            }
        }
        None
    }

    pub fn parse_sway_inputs_json(json_str: &str) -> InputSettings {
        let mut settings = InputSettings::default();
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
            if let Some(arr) = parsed.as_array() {
                for dev in arr {
                    if let Some(libinput) = dev.get("libinput") {
                        if let Some(nat) = libinput.get("natural_scroll").and_then(|v| v.as_str()) {
                            settings.natural_scroll = nat == "enabled";
                        }
                        if let Some(tap) = libinput.get("tap").and_then(|v| v.as_str()) {
                            settings.tap_to_click = tap == "enabled";
                        }
                        if let Some(lh) = libinput.get("left_handed").and_then(|v| v.as_str()) {
                            settings.left_handed = lh == "enabled";
                        }
                        if let Some(accel) = libinput.get("accel_speed").and_then(|v| v.as_f64()) {
                            settings.sensitivity = accel;
                        }
                    }
                }
            }
        }
        settings
    }
}

#[async_trait]
impl InputBackend for SwayInputBackend {
    async fn get_settings(&self) -> Result<InputSettings> {
        let out = Command::new("swaymsg")
            .args(["-t", "get_inputs"])
            .output()?;
        let json_str = String::from_utf8_lossy(&out.stdout);
        Ok(Self::parse_sway_inputs_json(&json_str))
    }

    async fn set_natural_scroll(&self, enabled: bool) -> Result<()> {
        let val = if enabled { "enabled" } else { "disabled" };
        let out = Command::new("swaymsg")
            .args(["input", "type:touchpad", "natural_scroll", val])
            .output()?;
        if out.status.success() {
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to set Sway natural scroll: {}",
                String::from_utf8_lossy(&out.stderr)
            ))
        }
    }

    async fn set_tap_to_click(&self, enabled: bool) -> Result<()> {
        let val = if enabled { "enabled" } else { "disabled" };
        let out = Command::new("swaymsg")
            .args(["input", "type:touchpad", "tap", val])
            .output()?;
        if out.status.success() {
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to set Sway tap-to-click: {}",
                String::from_utf8_lossy(&out.stderr)
            ))
        }
    }

    async fn set_left_handed(&self, enabled: bool) -> Result<()> {
        let val = if enabled { "enabled" } else { "disabled" };
        let out = Command::new("swaymsg")
            .args(["input", "type:pointer", "left_handed", val])
            .output()?;
        if out.status.success() {
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to set Sway left-handed: {}",
                String::from_utf8_lossy(&out.stderr)
            ))
        }
    }

    async fn set_sensitivity(&self, sensitivity: f64) -> Result<()> {
        let clamped = sensitivity.clamp(-1.0, 1.0);
        let out = Command::new("swaymsg")
            .args([
                "input",
                "type:pointer",
                "accel_speed",
                &format!("{:.2}", clamped),
            ])
            .output()?;
        if out.status.success() {
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to set Sway pointer speed: {}",
                String::from_utf8_lossy(&out.stderr)
            ))
        }
    }
}

#[derive(Default)]
pub struct GenericSysfsInputBackend;

impl GenericSysfsInputBackend {
    pub fn new() -> Self {
        Self
    }

    fn state_file() -> std::path::PathBuf {
        if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
            if !dir.is_empty() {
                return std::path::PathBuf::from(dir).join("settings-tui/input_settings.json");
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            return std::path::PathBuf::from(home).join(".config/settings-tui/input_settings.json");
        }
        std::path::PathBuf::from("/tmp/settings-tui-input.json")
    }

    fn load_settings() -> InputSettings {
        let file = Self::state_file();
        if let Ok(content) = std::fs::read_to_string(file) {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content) {
                return InputSettings {
                    natural_scroll: parsed["natural_scroll"].as_bool().unwrap_or(false),
                    tap_to_click: parsed["tap_to_click"].as_bool().unwrap_or(true),
                    left_handed: parsed["left_handed"].as_bool().unwrap_or(false),
                    sensitivity: parsed["sensitivity"].as_f64().unwrap_or(0.0),
                };
            }
        }
        InputSettings {
            natural_scroll: false,
            tap_to_click: true,
            left_handed: false,
            sensitivity: 0.0,
        }
    }

    fn save_settings(settings: &InputSettings) -> Result<()> {
        let file = Self::state_file();
        if let Some(parent) = file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let val = serde_json::json!({
            "natural_scroll": settings.natural_scroll,
            "tap_to_click": settings.tap_to_click,
            "left_handed": settings.left_handed,
            "sensitivity": settings.sensitivity,
        });
        std::fs::write(file, serde_json::to_string_pretty(&val)?)?;
        Ok(())
    }
}

#[async_trait]
impl InputBackend for GenericSysfsInputBackend {
    async fn get_settings(&self) -> Result<InputSettings> {
        Ok(Self::load_settings())
    }

    async fn set_natural_scroll(&self, enabled: bool) -> Result<()> {
        let mut cur = Self::load_settings();
        cur.natural_scroll = enabled;
        Self::save_settings(&cur)
    }

    async fn set_tap_to_click(&self, enabled: bool) -> Result<()> {
        let mut cur = Self::load_settings();
        cur.tap_to_click = enabled;
        Self::save_settings(&cur)
    }

    async fn set_left_handed(&self, enabled: bool) -> Result<()> {
        let mut cur = Self::load_settings();
        cur.left_handed = enabled;
        Self::save_settings(&cur)
    }

    async fn set_sensitivity(&self, sensitivity: f64) -> Result<()> {
        let mut cur = Self::load_settings();
        cur.sensitivity = sensitivity.clamp(-1.0, 1.0);
        Self::save_settings(&cur)
    }
}

pub fn create_input_backend() -> Option<Box<dyn InputBackend>> {
    if let Some(hypr) = HyprlandInputBackend::new() {
        return Some(Box::new(hypr));
    }
    if let Some(sway) = SwayInputBackend::new() {
        return Some(Box::new(sway));
    }
    if let Some(gsettings) = GsettingsInputBackend::new() {
        return Some(Box::new(gsettings));
    }
    Some(Box::new(GenericSysfsInputBackend::new()))
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

    #[test]
    fn test_parse_sway_inputs_json() {
        let raw = r#"[
            {
                "identifier": "1:1:AT_Keyboard",
                "name": "Keyboard",
                "type": "keyboard"
            },
            {
                "identifier": "2:7:SynPS/2_Synaptics_TouchPad",
                "name": "TouchPad",
                "type": "touchpad",
                "libinput": {
                    "natural_scroll": "enabled",
                    "tap": "enabled",
                    "left_handed": "disabled",
                    "accel_speed": 0.35
                }
            }
        ]"#;

        let settings = SwayInputBackend::parse_sway_inputs_json(raw);
        assert!(settings.natural_scroll);
        assert!(settings.tap_to_click);
        assert!(!settings.left_handed);
        assert!((settings.sensitivity - 0.35).abs() < 1e-4);
    }
}
