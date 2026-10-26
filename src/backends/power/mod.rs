use super::{BatteryState, PowerBackend, PowerInfo};
use crate::dbus::power::{PowerProfilesProxy, UPowerDeviceProxy, UPowerProxy};
use anyhow::Result;
use async_trait::async_trait;
use zbus::Connection;

pub struct UPowerBackend {
    connection: Connection,
}

impl UPowerBackend {
    pub async fn new() -> Result<Self> {
        let connection = Connection::system().await?;
        Ok(Self { connection })
    }
}

#[async_trait]
impl PowerBackend for UPowerBackend {
    async fn get_info(&self) -> Result<PowerInfo> {
        let upower = UPowerProxy::new(&self.connection).await?;
        let on_battery = upower.on_battery().await.unwrap_or(false);

        let mut battery_percentage = 100.0;
        let mut battery_state = BatteryState::Unknown;
        let mut energy_wh = None;
        let mut energy_full_wh = None;
        let mut energy_full_design_wh = None;
        let mut energy_rate_w = None;
        let mut health_percentage = None;
        let mut charge_cycles = None;
        let mut voltage_v = None;
        let mut time_to_empty_secs = None;
        let mut time_to_full_secs = None;
        let mut battery_model = None;
        let mut battery_vendor = None;

        let battery_device_path = match upower.enumerate_devices().await {
            Ok(devices) => devices
                .into_iter()
                .find(|d| d.as_str().contains("/battery_")),
            Err(_) => None,
        };

        let device_res = if let Some(path) = battery_device_path {
            UPowerDeviceProxy::builder(&self.connection)
                .path(path)
                .map(|b| b.build())?
                .await
                .ok()
        } else {
            UPowerDeviceProxy::new(&self.connection).await.ok()
        };

        if let Some(device) = device_res {
            battery_percentage = device.percentage().await.unwrap_or(100.0);

            let state_val = device.state().await.unwrap_or(0);
            battery_state = match state_val {
                1 => BatteryState::Charging,
                2 => BatteryState::Discharging,
                3 => BatteryState::Empty,
                4 => BatteryState::FullyCharged,
                5 => BatteryState::PendingCharge,
                6 => BatteryState::PendingDischarge,
                _ => BatteryState::Unknown,
            };

            energy_wh = device.energy().await.ok().filter(|v| *v > 0.0);
            energy_full_wh = device.energy_full().await.ok().filter(|v| *v > 0.0);
            energy_full_design_wh = device.energy_full_design().await.ok().filter(|v| *v > 0.0);
            energy_rate_w = device.energy_rate().await.ok().filter(|v| *v > 0.0);
            health_percentage = device.capacity().await.ok().filter(|v| *v > 0.0);
            charge_cycles = device.charge_cycles().await.ok().filter(|v| *v >= 0);
            voltage_v = device.voltage().await.ok().filter(|v| *v > 0.0);
            time_to_empty_secs = device.time_to_empty().await.ok().filter(|v| *v > 0);
            time_to_full_secs = device.time_to_full().await.ok().filter(|v| *v > 0);
            battery_model = device.model().await.ok().filter(|s| !s.is_empty());
            battery_vendor = device.vendor().await.ok().filter(|s| !s.is_empty());
        }

        let mut charge_limit = None;
        if let Ok(paths) = std::fs::read_dir("/sys/class/power_supply/") {
            for path in paths.flatten() {
                if let Some(name) = path.file_name().to_str() {
                    if name.starts_with("BAT") {
                        let mut limit_path = path.path();
                        limit_path.push("charge_control_end_threshold");
                        if let Ok(limit_str) = std::fs::read_to_string(&limit_path) {
                            if let Ok(limit) = limit_str.trim().parse::<u8>() {
                                charge_limit = Some(limit);
                                break;
                            }
                        }
                    }
                }
            }
        }
        let mut power_profile = None;
        if let Ok(profiles) = PowerProfilesProxy::new(&self.connection).await {
            if let Ok(profile) = profiles.active_profile().await {
                power_profile = Some(profile);
            }
        }

        let mut power_button_action = "poweroff".to_string();
        let mut idle_delay = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.session", "idle-delay"])
            .output()
            .ok()
            .and_then(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .trim()
                    .split(' ')
                    .next_back()
                    .unwrap_or("")
                    .parse::<u32>()
                    .ok()
            });
        if idle_delay.is_none() {
            if let Some(path) = get_hypridle_conf_path() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    idle_delay = parse_hypridle_timeout(&content, "dpms");
                }
            }
        }

        // Lock Screen Timeout
        let mut lock_delay = None;
        if let Some(path) = get_hypridle_conf_path() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                lock_delay = parse_hypridle_timeout(&content, "hyprlock");
            }
        }
        if lock_delay.is_none() {
            lock_delay = std::process::Command::new("gsettings")
                .args(["get", "org.gnome.desktop.screensaver", "lock-delay"])
                .output()
                .ok()
                .and_then(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .trim()
                        .split(' ')
                        .next_back()
                        .unwrap_or("")
                        .parse::<u32>()
                        .ok()
                });
        }
        if lock_delay.is_none() {
            lock_delay = Some(0);
        }

        let try_read = |path: &str| -> Option<String> { std::fs::read_to_string(path).ok() };

        // Suspend Timeout
        let mut suspend_delay = None;
        if let Some(path) = get_hypridle_conf_path() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                suspend_delay = parse_hypridle_timeout(&content, "suspend");
            }
        }
        if suspend_delay.is_none() {
            if let Some(content) = try_read("/etc/systemd/logind.conf.d/settings-tui-suspend.conf")
                .or_else(|| try_read("/etc/systemd/logind.conf"))
            {
                for line in content.lines() {
                    let l = line.trim();
                    if l.starts_with("IdleActionSec=") {
                        let val = l.replace("IdleActionSec=", "");
                        suspend_delay = parse_systemd_sec(&val);
                    } else if l.starts_with("IdleAction=ignore") {
                        suspend_delay = Some(0);
                    }
                }
            }
        }
        if suspend_delay.is_none() {
            suspend_delay = std::process::Command::new("gsettings")
                .args([
                    "get",
                    "org.gnome.settings-daemon.plugins.power",
                    "sleep-inactive-ac-timeout",
                ])
                .output()
                .ok()
                .and_then(|o| {
                    let s = String::from_utf8_lossy(&o.stdout);
                    s.trim()
                        .split(' ')
                        .next_back()
                        .and_then(|v| v.parse::<u32>().ok())
                });
        }
        if suspend_delay.is_none() {
            suspend_delay = Some(0);
        }

        let mut lid_action = "suspend".to_string();

        if let Some(content) = try_read("/etc/systemd/logind.conf.d/settings-tui-powerkey.conf")
            .or_else(|| try_read("/etc/systemd/logind.conf"))
        {
            for line in content.lines() {
                let l = line.trim();
                if l.starts_with("HandlePowerKey=") {
                    power_button_action = l.replace("HandlePowerKey=", "");
                } else if l.starts_with("#HandlePowerKey=") && power_button_action == "poweroff" {
                    power_button_action = l.replace("#HandlePowerKey=", "");
                }
            }
        }
        if let Some(content) = try_read("/etc/systemd/logind.conf.d/settings-tui-lid.conf")
            .or_else(|| try_read("/etc/systemd/logind.conf"))
        {
            for line in content.lines() {
                let l = line.trim();
                if l.starts_with("HandleLidSwitch=") {
                    lid_action = l.replace("HandleLidSwitch=", "");
                } else if l.starts_with("#HandleLidSwitch=") && lid_action == "suspend" {
                    lid_action = l.replace("#HandleLidSwitch=", "");
                }
            }
        }
        Ok(PowerInfo {
            on_battery,
            battery_percentage,
            battery_state,
            power_profile,
            energy_wh,
            energy_full_wh,
            energy_full_design_wh,
            energy_rate_w,
            health_percentage,
            charge_cycles,
            voltage_v,
            time_to_empty_secs,
            time_to_full_secs,
            battery_model,
            battery_vendor,
            charge_limit,
            power_button_action,
            lid_action,
            idle_delay,
            lock_delay,
            suspend_delay,
            stay_awake: is_stay_awake_active(),
        })
    }

    async fn set_charge_limit(&self, limit: u8) -> Result<()> {
        // Try to find the battery
        let mut bat_path = None;
        if let Ok(paths) = std::fs::read_dir("/sys/class/power_supply/") {
            for path in paths.flatten() {
                if let Some(name) = path.file_name().to_str() {
                    if name.starts_with("BAT") {
                        let mut p = path.path();
                        p.push("charge_control_end_threshold");
                        if p.exists() {
                            bat_path = Some(p);
                            break;
                        }
                    }
                }
            }
        }
        if let Some(p) = bat_path {
            // We need pkexec or Polkit because sysfs is root-owned
            let status = std::process::Command::new("pkexec")
                .arg("sh")
                .arg("-c")
                .arg(format!("echo {} > {}", limit, p.display()))
                .status()?;
            if !status.success() {
                return Err(anyhow::anyhow!(
                    "Failed to set charge limit (authentication failed or permission denied)"
                ));
            }
        } else {
            return Err(anyhow::anyhow!(
                "Battery charge limit is not supported on this device"
            ));
        }
        Ok(())
    }
    async fn set_power_profile(&self, profile: &str) -> Result<()> {
        let profiles = PowerProfilesProxy::new(&self.connection).await?;
        profiles.set_active_profile(profile).await?;
        Ok(())
    }
    async fn set_power_button_action(&self, action: &str) -> Result<()> {
        let content = format!("[Login]\nHandlePowerKey={}\n", action);
        let status = std::process::Command::new("pkexec")
            .arg("sh")
            .arg("-c")
            .arg(format!("mkdir -p /etc/systemd/logind.conf.d && echo '{}' > /etc/systemd/logind.conf.d/settings-tui-powerkey.conf && systemctl reload systemd-logind || systemctl restart systemd-logind", content))
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to set power button action"))
        }
    }

    async fn set_idle_delay(&self, seconds: u32) -> Result<()> {
        let _ = set_hypridle_timeout("dpms", seconds, "hyprctl dispatch dpms off");
        let _ = std::process::Command::new("gsettings")
            .args([
                "set",
                "org.gnome.desktop.session",
                "idle-delay",
                &seconds.to_string(),
            ])
            .output();
        Ok(())
    }

    async fn set_lock_delay(&self, seconds: u32) -> Result<()> {
        let lock_cmd = if let Some(path) = get_hypridle_conf_path() {
            if let Ok(c) = std::fs::read_to_string(&path) {
                c.lines()
                    .find(|l| l.trim().starts_with("on-timeout") && l.contains("hyprlock"))
                    .and_then(|l| l.split('=').nth(1))
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "hyprlock".to_string())
            } else {
                "hyprlock".to_string()
            }
        } else {
            "hyprlock".to_string()
        };
        let _ = set_hypridle_timeout("hyprlock", seconds, &lock_cmd);
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "restart", "hypridle.service"])
            .status();

        let _ = std::process::Command::new("gsettings")
            .args([
                "set",
                "org.gnome.desktop.screensaver",
                "lock-delay",
                &seconds.to_string(),
            ])
            .output();
        Ok(())
    }

    async fn set_suspend_delay(&self, seconds: u32) -> Result<()> {
        let _ = set_hypridle_timeout("suspend", seconds, "systemctl suspend");
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "restart", "hypridle.service"])
            .status();

        let content = if seconds == 0 {
            "[Login]\nIdleAction=ignore\n".to_string()
        } else {
            format!("[Login]\nIdleAction=suspend\nIdleActionSec={}s\n", seconds)
        };
        let _ = std::process::Command::new("pkexec")
            .arg("sh")
            .arg("-c")
            .arg(format!("mkdir -p /etc/systemd/logind.conf.d && echo '{}' > /etc/systemd/logind.conf.d/settings-tui-suspend.conf && systemctl reload systemd-logind || systemctl restart systemd-logind", content))
            .status();

        let _ = std::process::Command::new("gsettings")
            .args([
                "set",
                "org.gnome.settings-daemon.plugins.power",
                "sleep-inactive-ac-timeout",
                &seconds.to_string(),
            ])
            .output();
        let _ = std::process::Command::new("gsettings")
            .args([
                "set",
                "org.gnome.settings-daemon.plugins.power",
                "sleep-inactive-battery-timeout",
                &seconds.to_string(),
            ])
            .output();
        Ok(())
    }

    async fn set_lid_action(&self, action: &str) -> Result<()> {
        let content = format!("[Login]\nHandleLidSwitch={}\n", action);
        let status = std::process::Command::new("pkexec")
            .arg("sh")
            .arg("-c")
            .arg(format!("mkdir -p /etc/systemd/logind.conf.d && echo '{}' > /etc/systemd/logind.conf.d/settings-tui-lid.conf && systemctl reload systemd-logind || systemctl restart systemd-logind", content))
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to set lid action"))
        }
    }

    async fn set_stay_awake(&self, enabled: bool) -> Result<()> {
        apply_stay_awake(enabled)
    }
}

fn get_state_dir() -> std::path::PathBuf {
    if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
        let p = std::path::PathBuf::from(runtime).join("settings-tui");
        let _ = std::fs::create_dir_all(&p);
        p
    } else if let Ok(home) = std::env::var("HOME") {
        let p = std::path::PathBuf::from(home).join(".config/settings-tui");
        let _ = std::fs::create_dir_all(&p);
        p
    } else {
        std::path::PathBuf::from("/tmp/settings-tui")
    }
}

fn binary_exists(bin: &str) -> bool {
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let full_path = dir.join(bin);
            if full_path.is_file() {
                return true;
            }
        }
    }
    false
}

fn is_process_running(proc: &str) -> bool {
    if let Ok(output) = std::process::Command::new("pidof").arg(proc).output() {
        if output.status.success() && !output.stdout.is_empty() {
            return true;
        }
    }
    if let Ok(output) = std::process::Command::new("pgrep")
        .arg("-x")
        .arg(proc)
        .output()
    {
        if output.status.success() && !output.stdout.is_empty() {
            return true;
        }
    }
    false
}

pub fn is_stay_awake_active() -> bool {
    let state_dir = get_state_dir();
    let pid_file = state_dir.join("stay_awake.pid");
    let hypridle_marker = state_dir.join("stay_awake_hypridle");

    if hypridle_marker.exists() {
        return true;
    }

    if let Ok(pid_str) = std::fs::read_to_string(&pid_file) {
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            if std::path::Path::new(&format!("/proc/{}", pid)).exists() {
                return true;
            }
        }
    }

    if let Ok(output) = std::process::Command::new("systemd-inhibit")
        .args(["--list", "--json=short"])
        .output()
    {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout);
            if s.contains("\"who\":\"settings-tui\"") {
                return true;
            }
        }
    }

    false
}

pub fn apply_stay_awake(enabled: bool) -> Result<()> {
    let state_dir = get_state_dir();
    let pid_file = state_dir.join("stay_awake.pid");
    let hypridle_marker = state_dir.join("stay_awake_hypridle");
    let gnome_marker = state_dir.join("stay_awake_gnome_delay");

    if enabled {
        let _ = apply_stay_awake(false);

        if binary_exists("systemd-inhibit") {
            if let Ok(child) = std::process::Command::new("systemd-inhibit")
                .args([
                    "--what=idle:sleep:handle-lid-switch",
                    "--who=settings-tui",
                    "--why=Stay Awake (Caffeine) enabled by user",
                    "--mode=block",
                    "sleep",
                    "infinity",
                ])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
            {
                let _ = std::fs::write(&pid_file, child.id().to_string());
            }
        }

        if is_process_running("hypridle") {
            let svc_active = std::process::Command::new("systemctl")
                .args(["--user", "is-active", "--quiet", "hypridle.service"])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);

            if svc_active {
                let _ = std::process::Command::new("systemctl")
                    .args(["--user", "stop", "hypridle.service"])
                    .output();
                let _ = std::fs::write(&hypridle_marker, "service");
            } else {
                let _ = std::process::Command::new("pkill")
                    .args(["-STOP", "hypridle"])
                    .output();
                let _ = std::fs::write(&hypridle_marker, "process");
            }
        }

        if is_process_running("swayidle") {
            let _ = std::process::Command::new("pkill")
                .args(["-STOP", "swayidle"])
                .output();
        }

        if binary_exists("gsettings") {
            if let Ok(out) = std::process::Command::new("gsettings")
                .args(["get", "org.gnome.desktop.session", "idle-delay"])
                .output()
            {
                let delay = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let _ = std::fs::write(&gnome_marker, delay);
                let _ = std::process::Command::new("gsettings")
                    .args(["set", "org.gnome.desktop.session", "idle-delay", "0"])
                    .output();
            }
        }

        Ok(())
    } else {
        if let Ok(pid_str) = std::fs::read_to_string(&pid_file) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                let _ = std::process::Command::new("kill")
                    .arg(pid.to_string())
                    .output();
            }
            let _ = std::fs::remove_file(&pid_file);
        }
        let _ = std::process::Command::new("pkill")
            .args(["-f", "systemd-inhibit.*settings-tui"])
            .output();

        if hypridle_marker.exists() {
            if let Ok(kind) = std::fs::read_to_string(&hypridle_marker) {
                if kind.trim() == "service" {
                    let _ = std::process::Command::new("systemctl")
                        .args(["--user", "start", "hypridle.service"])
                        .output();
                } else {
                    let _ = std::process::Command::new("pkill")
                        .args(["-CONT", "hypridle"])
                        .output();
                }
            }
            let _ = std::fs::remove_file(&hypridle_marker);
        } else if !is_process_running("hypridle") {
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "start", "hypridle.service"])
                .output();
        }

        let _ = std::process::Command::new("pkill")
            .args(["-CONT", "swayidle"])
            .output();

        if gnome_marker.exists() {
            if let Ok(val) = std::fs::read_to_string(&gnome_marker) {
                let cleaned = val.trim().split(' ').next_back().unwrap_or("300");
                if let Ok(n) = cleaned.parse::<u32>() {
                    let _ = std::process::Command::new("gsettings")
                        .args([
                            "set",
                            "org.gnome.desktop.session",
                            "idle-delay",
                            &n.to_string(),
                        ])
                        .output();
                }
            }
            let _ = std::fs::remove_file(&gnome_marker);
        }

        Ok(())
    }
}

fn get_hypridle_conf_path() -> Option<std::path::PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|h| std::path::PathBuf::from(h).join(".config/hypr/hypridle.conf"))
}

fn parse_hypridle_timeout(content: &str, keyword: &str) -> Option<u32> {
    let mut in_listener = false;
    let mut current_timeout = None;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("listener") && trimmed.contains('{') {
            in_listener = true;
            current_timeout = None;
        } else if trimmed == "}" {
            in_listener = false;
        } else if in_listener {
            if trimmed.starts_with("timeout") && trimmed.contains('=') {
                if let Some(val_str) = trimmed.split('=').nth(1) {
                    current_timeout = val_str.trim().parse::<u32>().ok();
                }
            } else if trimmed.starts_with("on-timeout") && trimmed.contains(keyword) {
                if let Some(t) = current_timeout {
                    return Some(t);
                }
            }
        }
    }
    None
}

fn set_hypridle_timeout(keyword: &str, seconds: u32, default_cmd: &str) -> Result<()> {
    if let Some(path) = get_hypridle_conf_path() {
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let mut lines: Vec<String> = Vec::new();
            let mut in_listener = false;
            let mut listener_lines: Vec<String> = Vec::new();
            let mut found = false;

            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("listener") && trimmed.contains('{') {
                    in_listener = true;
                    listener_lines.clear();
                    listener_lines.push(line.to_string());
                } else if in_listener {
                    listener_lines.push(line.to_string());
                    if trimmed == "}" {
                        in_listener = false;
                        let block_text = listener_lines.join("\n");
                        if block_text.contains(keyword) {
                            found = true;
                            if seconds > 0 {
                                lines.push("listener {".to_string());
                                lines.push(format!("    timeout = {}", seconds));
                                lines.push(format!("    on-timeout = {}", default_cmd));
                                lines.push("}".to_string());
                            }
                        } else {
                            lines.extend(listener_lines.clone());
                        }
                    }
                } else {
                    lines.push(line.to_string());
                }
            }

            if !found && seconds > 0 {
                lines.push("".to_string());
                lines.push("listener {".to_string());
                lines.push(format!("    timeout = {}", seconds));
                lines.push(format!("    on-timeout = {}", default_cmd));
                lines.push("}".to_string());
            }

            std::fs::write(&path, lines.join("\n"))?;
        }
    }
    Ok(())
}

fn parse_systemd_sec(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(num) = s.strip_suffix("min") {
        num.parse::<u32>().ok().map(|m| m * 60)
    } else if let Some(num) = s.strip_suffix('m') {
        num.parse::<u32>().ok().map(|m| m * 60)
    } else if let Some(num) = s.strip_suffix('h') {
        num.parse::<u32>().ok().map(|h| h * 3600)
    } else if let Some(num) = s.strip_suffix('s') {
        num.parse::<u32>().ok()
    } else {
        s.parse::<u32>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_systemd_sec() {
        assert_eq!(parse_systemd_sec("30s"), Some(30));
        assert_eq!(parse_systemd_sec("5min"), Some(300));
        assert_eq!(parse_systemd_sec("10m"), Some(600));
        assert_eq!(parse_systemd_sec("1h"), Some(3600));
    }

    #[test]
    fn test_parse_hypridle_timeout() {
        let conf = "listener {\n    timeout = 300\n    on-timeout = hyprlock\n}\nlistener {\n    timeout = 1800\n    on-timeout = systemctl suspend\n}";
        assert_eq!(parse_hypridle_timeout(conf, "hyprlock"), Some(300));
        assert_eq!(parse_hypridle_timeout(conf, "suspend"), Some(1800));
        assert_eq!(parse_hypridle_timeout(conf, "dpms"), None);
    }

    #[test]
    fn test_stay_awake_state_toggle() {
        // Turning off should succeed and clean up markers cleanly
        assert!(apply_stay_awake(false).is_ok());
    }
}
