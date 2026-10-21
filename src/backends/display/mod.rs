use super::{DisplayBackend, Monitor};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::path::{Path, PathBuf};
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
                    primary: false,
                    supported_modes,
                    mirror_of: m
                        .get("mirrorOf")
                        .and_then(|v| v.as_str())
                        .filter(|v| *v != "none")
                        .map(|v| v.to_string()),
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

    async fn set_monitor_mode(
        &self,
        name: &str,
        mode: &str,
        mirror_target: Option<&str>,
    ) -> Result<()> {
        let (mon_res, mon_scale) = if let Ok(monitors) = self.get_monitors().await {
            if let Some(m) = monitors.into_iter().find(|m| m.name == name) {
                (
                    format!("{}x{}@{}", m.width, m.height, m.refresh_rate),
                    m.scale,
                )
            } else {
                return Err(anyhow::anyhow!("Monitor not found"));
            }
        } else {
            return Err(anyhow::anyhow!("Failed to read monitors"));
        };

        let arg = match mode {
            "disable" => format!("{},disable", name),
            "mirror" => {
                if let Some(target) = mirror_target {
                    format!("{},{},auto,{},mirror,{}", name, mon_res, mon_scale, target)
                } else {
                    return Err(anyhow::anyhow!("No mirror target provided"));
                }
            }
            _ => format!("{},{},auto,{}", name, mon_res, mon_scale),
        };

        let status = std::process::Command::new("hyprctl")
            .arg("keyword")
            .arg("monitor")
            .arg(arg)
            .status()?;

        if status.success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("hyprctl failed to set monitor mode"))
        }
    }

    async fn set_scale(&self, name: &str, scale: f64) -> Result<()> {
        let res_str = if let Ok(monitors) = self.get_monitors().await {
            if let Some(m) = monitors.into_iter().find(|m| m.name == name) {
                format!("{}x{}@{}", m.width, m.height, m.refresh_rate)
            } else {
                return Err(anyhow::anyhow!("Monitor not found"));
            }
        } else {
            return Err(anyhow::anyhow!("Failed to read monitors"));
        };
        let status = std::process::Command::new("hyprctl")
            .arg("keyword")
            .arg("monitor")
            .arg(format!("{},{},auto,{}", name, res_str, scale))
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("hyprctl failed to set scale"))
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
        Err(anyhow::anyhow!(
            "Resolution switching requires Hyprland compositor"
        ))
    }

    async fn set_scale(&self, _name: &str, _scale: f64) -> Result<()> {
        Err(anyhow::anyhow!(
            "Display scaling is not supported on this generic backend."
        ))
    }
    async fn set_monitor_mode(
        &self,
        _name: &str,
        _mode: &str,
        _mirror_target: Option<&str>,
    ) -> Result<()> {
        Err(anyhow::anyhow!("Not supported on generic backend"))
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

fn binary_exists(bin: &str) -> bool {
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let full_path = dir.join(bin);
            if full_path.is_file() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(meta) = full_path.metadata() {
                        if meta.permissions().mode() & 0o111 != 0 {
                            return true;
                        }
                    }
                }
                #[cfg(not(unix))]
                return true;
            }
        }
    }
    false
}

const GRAYSCALE_SHADER: &str = r#"precision highp float;
varying vec2 v_texcoord;
uniform sampler2D tex;

void main() {
    vec4 pixColor = texture2D(tex, v_texcoord);
    float gray = dot(pixColor.rgb, vec3(0.299, 0.587, 0.114));
    gl_FragColor = vec4(vec3(gray), pixColor.a);
}
"#;

const NIGHT_LIGHT_SHADER: &str = r#"precision highp float;
varying vec2 v_texcoord;
uniform sampler2D tex;

void main() {
    vec4 pixColor = texture2D(tex, v_texcoord);
    pixColor.r = min(1.0, pixColor.r * 1.0);
    pixColor.g = min(1.0, pixColor.g * 0.88);
    pixColor.b = min(1.0, pixColor.b * 0.70);
    gl_FragColor = pixColor;
}
"#;

fn ensure_shader(filename: &str, content: &str) -> Result<PathBuf> {
    let base_dir = if let Ok(cfg) = std::env::var("XDG_CONFIG_HOME") {
        if !cfg.is_empty() {
            PathBuf::from(cfg).join("settings-tui/shaders")
        } else {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
                .join(".config/settings-tui/shaders")
        }
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config/settings-tui/shaders")
    } else {
        std::env::temp_dir().join("settings-tui-shaders")
    };

    std::fs::create_dir_all(&base_dir)?;
    let file_path = base_dir.join(filename);
    if !file_path.exists() {
        std::fs::write(&file_path, content)?;
    }
    Ok(file_path)
}

fn is_hyprland_running() -> bool {
    std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some()
        || Command::new("hyprctl")
            .arg("version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
}

fn set_hyprland_screen_shader(shader_path: Option<&Path>) -> Result<()> {
    let path_str = shader_path.and_then(|p| p.to_str()).unwrap_or("");

    // 1. Try modern Hyprland Lua parser: eval "hl.config({ decoration = { screen_shader = '...' } })"
    let lua_code = format!(
        "hl.config({{ decoration = {{ screen_shader = '{}' }} }})",
        path_str
    );
    let eval_out = Command::new("hyprctl").arg("eval").arg(&lua_code).output();

    if let Ok(out) = eval_out {
        let msg = String::from_utf8_lossy(&out.stdout);
        if out.status.success() && msg.trim() == "ok" {
            return Ok(());
        }
    }

    // 2. Fallback to legacy Hyprland keyword
    let legacy_val = if path_str.is_empty() {
        "[[EMPTY]]"
    } else {
        path_str
    };
    let kw_out = Command::new("hyprctl")
        .arg("keyword")
        .arg("decoration:screen_shader")
        .arg(legacy_val)
        .output()?;

    if kw_out.status.success() {
        Ok(())
    } else {
        Err(anyhow!("Failed to set Hyprland screen_shader"))
    }
}

fn get_hyprland_screen_shader() -> Option<String> {
    if let Ok(output) = Command::new("hyprctl")
        .arg("getoption")
        .arg("decoration:screen_shader")
        .arg("-j")
        .output()
    {
        if output.status.success() {
            if let Ok(val) = serde_json::from_slice::<Value>(&output.stdout) {
                if let Some(s) = val.get("str").and_then(|v| v.as_str()) {
                    let trimmed = s.trim();
                    if !trimmed.is_empty() && trimmed != "[[EMPTY]]" {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }
    }
    None
}

fn is_night_light_active() -> Result<bool> {
    // 1. Check Hyprland screen_shader
    if let Some(current) = get_hyprland_screen_shader() {
        if current.contains("night_light") {
            return Ok(true);
        }
    }

    // 2. Try hyprsunset process
    if let Ok(output) = Command::new("pidof").arg("hyprsunset").output() {
        if output.status.success() && !output.stdout.is_empty() {
            return Ok(true);
        }
    }

    // 3. Try wlsunset process
    if let Ok(output) = Command::new("pidof").arg("wlsunset").output() {
        if output.status.success() && !output.stdout.is_empty() {
            return Ok(true);
        }
    }

    // 4. Try gammastep process
    if let Ok(output) = Command::new("pidof").arg("gammastep").output() {
        if output.status.success() && !output.stdout.is_empty() {
            return Ok(true);
        }
    }

    // 5. Try GNOME gsettings
    if binary_exists("gsettings") {
        if let Ok(output) = Command::new("gsettings")
            .arg("get")
            .arg("org.gnome.settings-daemon.plugins.color")
            .arg("night-light-enabled")
            .output()
        {
            if output.status.success() {
                let res = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if res == "true" {
                    return Ok(true);
                }
            }
        }
    }

    Ok(false)
}

fn is_grayscale_active() -> Result<Option<bool>> {
    if !is_hyprland_running() {
        return Ok(None);
    }
    if let Some(current) = get_hyprland_screen_shader() {
        if current.contains("grayscale") {
            return Ok(Some(true));
        }
    }
    Ok(Some(false))
}

fn apply_night_light(enabled: bool, temperature: u32) -> Result<()> {
    if !enabled {
        // Kill existing background daemons
        if binary_exists("killall") {
            let _ = Command::new("killall").arg("hyprsunset").output();
            let _ = Command::new("killall").arg("wlsunset").output();
        }
        if binary_exists("gammastep") {
            let _ = Command::new("gammastep").arg("-x").output();
        }

        // GNOME
        if binary_exists("gsettings") {
            let _ = Command::new("gsettings")
                .arg("set")
                .arg("org.gnome.settings-daemon.plugins.color")
                .arg("night-light-enabled")
                .arg("false")
                .output();
        }

        // Hyprland: if grayscale is active, preserve it; otherwise clear
        if is_grayscale_active().ok().flatten().unwrap_or(false) {
            let shader = ensure_shader("grayscale.frag", GRAYSCALE_SHADER)?;
            let _ = set_hyprland_screen_shader(Some(&shader));
        } else if is_hyprland_running() {
            let _ = set_hyprland_screen_shader(None);
        }

        return Ok(());
    }

    // ENABLED:
    // 1. If on Hyprland, prefer hyprsunset if installed, else native screen shader
    if is_hyprland_running() {
        if binary_exists("hyprsunset") {
            let _ = Command::new("hyprsunset")
                .arg("-t")
                .arg(temperature.to_string())
                .spawn();
            return Ok(());
        }

        // Apply native Hyprland night light screen shader if grayscale is not active
        if !is_grayscale_active().ok().flatten().unwrap_or(false) {
            let shader = ensure_shader("night_light.frag", NIGHT_LIGHT_SHADER)?;
            set_hyprland_screen_shader(Some(&shader))?;
        }
        return Ok(());
    }

    // 2. Generic Wayland: try wlsunset
    if binary_exists("wlsunset") {
        let spawn_res = Command::new("wlsunset")
            .arg("-t")
            .arg(temperature.to_string())
            .arg("-T")
            .arg("6500")
            .arg("-S")
            .arg("00:00")
            .arg("-s")
            .arg("00:00")
            .spawn();
        if spawn_res.is_ok() {
            return Ok(());
        }
    }

    // 3. Generic Wayland / X11: try gammastep
    if binary_exists("gammastep") {
        let spawn_res = Command::new("gammastep")
            .arg("-O")
            .arg(temperature.to_string())
            .spawn();
        if spawn_res.is_ok() {
            return Ok(());
        }
    }

    // 4. GNOME gsettings
    if binary_exists("gsettings") {
        let _ = Command::new("gsettings")
            .arg("set")
            .arg("org.gnome.settings-daemon.plugins.color")
            .arg("night-light-enabled")
            .arg("true")
            .output();
        let _ = Command::new("gsettings")
            .arg("set")
            .arg("org.gnome.settings-daemon.plugins.color")
            .arg("night-light-temperature")
            .arg(temperature.to_string())
            .output();
        return Ok(());
    }

    Err(anyhow!(
        "No supported night light daemon or compositor found (install wlsunset, gammastep, or hyprsunset)"
    ))
}

fn apply_grayscale(enabled: bool) -> Result<()> {
    if !is_hyprland_running() {
        return Err(anyhow!("Grayscale mode requires Hyprland compositor"));
    }

    if enabled {
        let shader = ensure_shader("grayscale.frag", GRAYSCALE_SHADER)?;
        set_hyprland_screen_shader(Some(&shader))
    } else {
        // If night light is currently active, revert to night light shader; otherwise clear
        if is_night_light_active().unwrap_or(false) {
            let shader = ensure_shader("night_light.frag", NIGHT_LIGHT_SHADER)?;
            set_hyprland_screen_shader(Some(&shader))
        } else {
            set_hyprland_screen_shader(None)
        }
    }
}
