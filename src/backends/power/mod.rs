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
        let idle_delay = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.session", "idle-delay"])
            .output()
            .ok()
            .and_then(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .trim()
                    .split(" ")
                    .last()
                    .unwrap_or("")
                    .parse::<u32>()
                    .ok()
            });
        let mut lid_action = "suspend".to_string();

        let try_read = |path: &str| -> Option<String> { std::fs::read_to_string(path).ok() };
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
}
