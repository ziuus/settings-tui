use super::{DisplayBackend, Monitor};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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
        apply_hyprland_monitor(name, &res_str, scale, None)
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

        match mode {
            "disable" => apply_hyprland_monitor(name, "disable", mon_scale, None),
            "mirror" => {
                if let Some(target) = mirror_target {
                    apply_hyprland_monitor(name, &mon_res, mon_scale, Some(target))
                } else {
                    Err(anyhow::anyhow!("No mirror target provided"))
                }
            }
            _ => apply_hyprland_monitor(name, &mon_res, mon_scale, None),
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
        apply_hyprland_monitor(name, &res_str, scale, None)
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

fn apply_hyprland_monitor(
    name: &str,
    mode_or_res: &str,
    scale: f64,
    mirror_of: Option<&str>,
) -> Result<()> {
    // 1. Try modern Hyprland Lua eval (prevents 'can't work with non-legacy parsers' config errors)
    let lua_code = if mode_or_res == "disable" {
        format!("hl.monitor({{ output = '{}', mode = 'disable' }})", name)
    } else if let Some(target) = mirror_of {
        format!(
            "hl.monitor({{ output = '{}', mode = '{}', position = 'auto', scale = {}, mirror = '{}' }})",
            name, mode_or_res, scale, target
        )
    } else {
        format!(
            "hl.monitor({{ output = '{}', mode = '{}', position = 'auto', scale = {} }})",
            name, mode_or_res, scale
        )
    };

    if let Ok(out) = Command::new("hyprctl").arg("eval").arg(&lua_code).output() {
        let msg = String::from_utf8_lossy(&out.stdout);
        if out.status.success() && msg.trim() == "ok" {
            return Ok(());
        }
    }

    // 2. Fallback to legacy Hyprland keyword monitor
    let legacy_arg = if mode_or_res == "disable" {
        format!("{},disable", name)
    } else if let Some(target) = mirror_of {
        format!("{},{},auto,{},mirror,{}", name, mode_or_res, scale, target)
    } else {
        format!("{},{},auto,{}", name, mode_or_res, scale)
    };

    let kw_out = Command::new("hyprctl")
        .arg("keyword")
        .arg("monitor")
        .arg(&legacy_arg)
        .output()?;

    if kw_out.status.success() {
        let msg = String::from_utf8_lossy(&kw_out.stdout);
        if !msg.contains("can't work with non-legacy parsers") {
            return Ok(());
        }
    }

    Err(anyhow!("Failed to apply Hyprland monitor settings"))
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

fn is_process_running(proc: &str) -> bool {
    if let Ok(output) = Command::new("pidof").arg(proc).output() {
        if output.status.success() && !output.stdout.is_empty() {
            return true;
        }
    }
    if let Ok(output) = Command::new("pgrep").arg("-x").arg(proc).output() {
        if output.status.success() && !output.stdout.is_empty() {
            return true;
        }
    }
    false
}

const GRAYSCALE_SHADER: &str = r#"#version 300 es
precision mediump float;

in vec2 v_texcoord;
layout(location = 0) out vec4 fragColor;

uniform sampler2D tex;

void main() {
    vec4 pixColor = texture(tex, v_texcoord);
    float gray = dot(pixColor.rgb, vec3(0.299, 0.587, 0.114));
    fragColor = vec4(vec3(gray), pixColor.a);
}
"#;

const NIGHT_LIGHT_SHADER: &str = r#"#version 300 es
precision mediump float;

in vec2 v_texcoord;
layout(location = 0) out vec4 fragColor;

uniform sampler2D tex;

void main() {
    vec4 pixColor = texture(tex, v_texcoord);
    pixColor[0] = min(1.0, pixColor[0] * 1.0);
    pixColor[1] = min(1.0, pixColor[1] * 0.88);
    pixColor[2] = min(1.0, pixColor[2] * 0.70);
    fragColor = pixColor;
}
"#;

const GRAYSCALE_NIGHT_LIGHT_SHADER: &str = r#"#version 300 es
precision mediump float;

in vec2 v_texcoord;
layout(location = 0) out vec4 fragColor;

uniform sampler2D tex;

void main() {
    vec4 pixColor = texture(tex, v_texcoord);
    float gray = dot(pixColor.rgb, vec3(0.299, 0.587, 0.114));
    fragColor = vec4(gray * 1.0, gray * 0.88, gray * 0.70, pixColor.a);
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
    if !file_path.exists() || std::fs::read_to_string(&file_path).unwrap_or_default() != content {
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

fn set_hyprland_screen_shader(shader_path: Option<&Path>) -> Result<()> {
    let path_str = shader_path.and_then(|p| p.to_str()).unwrap_or("");

    // 1. Try modern Hyprland Lua eval (zero config error side effects)
    let lua_code = format!(
        "hl.config({{ decoration = {{ screen_shader = '{}' }} }})",
        path_str
    );
    if let Ok(eval_out) = Command::new("hyprctl").arg("eval").arg(&lua_code).output() {
        let msg = String::from_utf8_lossy(&eval_out.stdout);
        if eval_out.status.success() && msg.trim() == "ok" {
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

    let msg = String::from_utf8_lossy(&kw_out.stdout);
    if kw_out.status.success() && !msg.contains("can't work with non-legacy parsers") {
        return Ok(());
    }

    Ok(())
}

fn is_gnome_running() -> bool {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .or_else(|_| std::env::var("DESKTOP_SESSION"))
        .unwrap_or_default()
        .to_lowercase();
    desktop.contains("gnome")
}

fn is_kde_running() -> bool {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .or_else(|_| std::env::var("DESKTOP_SESSION"))
        .unwrap_or_default()
        .to_lowercase();
    desktop.contains("kde") || desktop.contains("plasma")
}

fn is_night_light_active() -> Result<bool> {
    // 1. Hyprland screen_shader
    if is_hyprland_running() {
        if let Some(shader) = get_hyprland_screen_shader() {
            if shader.contains("night_light") {
                return Ok(true);
            }
        }
    }

    // 2. Check universal Wayland/X11 daemons
    for proc in &["wlsunset", "gammastep", "hyprsunset", "redshift"] {
        if is_process_running(proc) {
            return Ok(true);
        }
    }

    // 3. Check GNOME gsettings (only when GNOME session is active)
    if is_gnome_running() && binary_exists("gsettings") {
        if let Ok(output) = Command::new("gsettings")
            .args([
                "get",
                "org.gnome.settings-daemon.plugins.color",
                "night-light-enabled",
            ])
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

    // 4. Check KDE Plasma via qdbus (only when KDE session is active)
    if is_kde_running() && binary_exists("qdbus") {
        if let Ok(output) = Command::new("qdbus")
            .args([
                "org.kde.KWin.ColorCorrect",
                "/ColorCorrect",
                "isNightColorActive",
            ])
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

fn apply_night_light(enabled: bool, temperature: u32) -> Result<()> {
    if is_hyprland_running() {
        let gray = is_grayscale_active().ok().flatten().unwrap_or(false);
        if enabled {
            let shader = if gray {
                ensure_shader("grayscale_night_light.frag", GRAYSCALE_NIGHT_LIGHT_SHADER)?
            } else {
                ensure_shader("night_light.frag", NIGHT_LIGHT_SHADER)?
            };
            return set_hyprland_screen_shader(Some(&shader));
        } else if gray {
            let shader = ensure_shader("grayscale.frag", GRAYSCALE_SHADER)?;
            return set_hyprland_screen_shader(Some(&shader));
        } else {
            return set_hyprland_screen_shader(None);
        }
    }

    if !enabled {
        // Kill background daemons
        for proc in &["wlsunset", "gammastep", "hyprsunset", "redshift"] {
            if is_process_running(proc) {
                let _ = Command::new("pkill").arg("-x").arg(proc).output();
            }
        }

        // GNOME
        if is_gnome_running() && binary_exists("gsettings") {
            let _ = Command::new("gsettings")
                .args([
                    "set",
                    "org.gnome.settings-daemon.plugins.color",
                    "night-light-enabled",
                    "false",
                ])
                .output();
        }

        // KDE
        if is_kde_running() && binary_exists("qdbus") {
            let _ = Command::new("qdbus")
                .args([
                    "org.kde.KWin.ColorCorrect",
                    "/ColorCorrect",
                    "setNightColorActive",
                    "false",
                ])
                .output();
        }

        return Ok(());
    }

    // ENABLED (non-Hyprland environments):
    // 1. GNOME gsettings
    if is_gnome_running() && binary_exists("gsettings") {
        let status = Command::new("gsettings")
            .args([
                "set",
                "org.gnome.settings-daemon.plugins.color",
                "night-light-enabled",
                "true",
            ])
            .status();
        if let Ok(s) = status {
            if s.success() {
                let _ = Command::new("gsettings")
                    .args([
                        "set",
                        "org.gnome.settings-daemon.plugins.color",
                        "night-light-temperature",
                        &temperature.to_string(),
                    ])
                    .output();
                return Ok(());
            }
        }
    }

    // 2. KDE Plasma via qdbus
    if is_kde_running() && binary_exists("qdbus") {
        let status = Command::new("qdbus")
            .args([
                "org.kde.KWin.ColorCorrect",
                "/ColorCorrect",
                "setNightColorActive",
                "true",
            ])
            .status();
        if let Ok(s) = status {
            if s.success() {
                return Ok(());
            }
        }
    }

    // 3. Generic Wayland: wlsunset (spawned with null stdio to detach)
    if binary_exists("wlsunset") {
        for proc in &["wlsunset", "gammastep"] {
            if is_process_running(proc) {
                let _ = Command::new("pkill").arg("-x").arg(proc).output();
            }
        }
        let res = Command::new("wlsunset")
            .arg("-t")
            .arg(temperature.to_string())
            .arg("-T")
            .arg("6500")
            .arg("-S")
            .arg("00:00")
            .arg("-s")
            .arg("00:00")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if res.is_ok() {
            return Ok(());
        }
    }

    // 4. Generic Wayland / X11: gammastep
    if binary_exists("gammastep") {
        for proc in &["wlsunset", "gammastep", "redshift"] {
            if is_process_running(proc) {
                let _ = Command::new("pkill").arg("-x").arg(proc).output();
            }
        }
        let res = Command::new("gammastep")
            .arg("-O")
            .arg(temperature.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if res.is_ok() {
            return Ok(());
        }
    }

    // 5. X11 standard: redshift
    if binary_exists("redshift") {
        let _ = Command::new("pkill").arg("-x").arg("redshift").output();
        let res = Command::new("redshift")
            .arg("-O")
            .arg(temperature.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if res.is_ok() {
            return Ok(());
        }
    }

    Err(anyhow!(
        "Night Light requires a supported tool (install wlsunset, gammastep, or redshift)"
    ))
}

fn is_grayscale_active() -> Result<Option<bool>> {
    // 1. Hyprland screen_shader
    if is_hyprland_running() {
        if let Some(shader) = get_hyprland_screen_shader() {
            if shader.contains("grayscale") {
                return Ok(Some(true));
            }
        }
        return Ok(Some(false));
    }

    // 2. GNOME Desktop (only when GNOME session is active)
    if is_gnome_running() && binary_exists("gsettings") {
        if let Ok(output) = Command::new("gsettings")
            .args([
                "get",
                "org.gnome.desktop.a11y.magnifier",
                "color-saturation",
            ])
            .output()
        {
            if output.status.success() {
                let res = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if let Ok(val) = res.parse::<f64>() {
                    return Ok(Some(val <= 0.01));
                }
            }
        }
        return Ok(Some(false));
    }

    // 3. KDE Plasma via qdbus (only when KDE session is active)
    if is_kde_running() && binary_exists("qdbus") {
        if let Ok(output) = Command::new("qdbus")
            .args(["org.kde.KWin", "/Effects", "isEffectLoaded", "grayscale"])
            .output()
        {
            if output.status.success() {
                let res = String::from_utf8_lossy(&output.stdout).trim().to_string();
                return Ok(Some(res == "true"));
            }
        }
        return Ok(Some(false));
    }

    // Unsupported on current compositor/desktop environment
    Ok(None)
}

fn apply_grayscale(enabled: bool) -> Result<()> {
    if is_hyprland_running() {
        let nl = is_night_light_active().unwrap_or(false);
        if enabled {
            let shader = if nl {
                ensure_shader("grayscale_night_light.frag", GRAYSCALE_NIGHT_LIGHT_SHADER)?
            } else {
                ensure_shader("grayscale.frag", GRAYSCALE_SHADER)?
            };
            return set_hyprland_screen_shader(Some(&shader));
        } else if nl {
            let shader = ensure_shader("night_light.frag", NIGHT_LIGHT_SHADER)?;
            return set_hyprland_screen_shader(Some(&shader));
        } else {
            return set_hyprland_screen_shader(None);
        }
    }

    // GNOME Desktop
    if is_gnome_running() && binary_exists("gsettings") {
        let sat = if enabled { "0.0" } else { "1.0" };
        let status = Command::new("gsettings")
            .args([
                "set",
                "org.gnome.desktop.a11y.magnifier",
                "color-saturation",
                sat,
            ])
            .status();
        if let Ok(s) = status {
            if s.success() {
                return Ok(());
            }
        }
    }

    // KDE Plasma via qdbus
    if is_kde_running() && binary_exists("qdbus") {
        let method = if enabled {
            "loadEffect"
        } else {
            "unloadEffect"
        };
        let status = Command::new("qdbus")
            .args(["org.kde.KWin", "/Effects", method, "grayscale"])
            .status();
        if let Ok(s) = status {
            if s.success() {
                return Ok(());
            }
        }
    }

    Err(anyhow!(
        "Grayscale mode requires a desktop environment with accessibility color filter support (GNOME, KDE Plasma, or Hyprland)"
    ))
}
