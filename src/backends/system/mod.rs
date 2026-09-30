use super::{SystemBackend, SystemDiskInfo, SystemInfo};
use crate::dbus::systemd::{HostnameManagerProxy, LogindManagerProxy, TimedateManagerProxy};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use sysinfo::{Disks, System};
use zbus::Connection;

pub struct RealSystemBackend {
    sys: std::sync::Mutex<System>,
    connection: Option<Connection>,
}

impl RealSystemBackend {
    pub async fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        let connection = Connection::system().await.ok();
        Self {
            sys: std::sync::Mutex::new(sys),
            connection,
        }
    }

    #[cfg(test)]
    pub fn new_with_connection(connection: Option<Connection>) -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        Self {
            sys: std::sync::Mutex::new(sys),
            connection,
        }
    }
}

#[async_trait]
impl SystemBackend for RealSystemBackend {
    async fn get_info(&self) -> Result<SystemInfo> {
        let (memory_total, memory_used, uptime, cpu_model, cpu_cores) = {
            let mut sys = self
                .sys
                .lock()
                .map_err(|_| anyhow!("System info lock poisoned"))?;
            sys.refresh_memory();
            sys.refresh_cpu();
            let cpu_brand = sys
                .cpus()
                .first()
                .map(|c| c.brand().trim().to_string())
                .unwrap_or_else(|| "Unknown Processor".to_string());
            let cores = sys.cpus().len();
            (
                sys.total_memory(),
                sys.used_memory(),
                System::uptime(),
                cpu_brand,
                cores,
            )
        };

        let mut hostname = System::host_name().unwrap_or_else(|| "localhost".to_string());
        let mut chassis = "system".to_string();
        let mut timezone = "UTC".to_string();
        let mut ntp_active = false;

        if let Some(conn) = &self.connection {
            if let Ok(host_proxy) = HostnameManagerProxy::new(conn).await {
                if let Ok(h) = host_proxy.hostname().await {
                    if !h.is_empty() {
                        hostname = h;
                    }
                }
                if let Ok(c) = host_proxy.chassis().await {
                    if !c.is_empty() {
                        chassis = c;
                    }
                }
            }
            if let Ok(time_proxy) = TimedateManagerProxy::new(conn).await {
                if let Ok(tz) = time_proxy.timezone().await {
                    timezone = tz;
                }
                if let Ok(ntp) = time_proxy.ntp().await {
                    ntp_active = ntp;
                }
            }
        }

        let mut disks = Vec::new();
        let sys_disks = Disks::new_with_refreshed_list();
        for disk in &sys_disks {
            let mount = disk.mount_point().to_string_lossy().to_string();
            if mount.starts_with("/sys") || mount.starts_with("/proc") || mount.starts_with("/dev")
            {
                continue;
            }
            let total = disk.total_space();
            if total > 0 {
                disks.push(SystemDiskInfo {
                    mount_point: mount,
                    total_bytes: total,
                    available_bytes: disk.available_space(),
                    fs_type: disk.file_system().to_string_lossy().to_string(),
                });
            }
        }
        disks.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));

        let distro = System::name().unwrap_or_else(|| "Unknown".to_string())
            + " "
            + &System::os_version().unwrap_or_default();
        let kernel = System::kernel_version().unwrap_or_else(|| "Unknown".to_string());

        Ok(SystemInfo {
            hostname,
            chassis,
            distro,
            kernel,
            uptime,
            memory_total,
            memory_used,
            timezone,
            ntp_active,
            disks,
            cpu_model,
            cpu_cores,
        })
    }

    async fn set_ntp(&self, active: bool) -> Result<()> {
        if let Some(conn) = &self.connection {
            let proxy = TimedateManagerProxy::new(conn).await?;
            proxy.set_ntp(active, true).await?;
            Ok(())
        } else {
            Err(anyhow!("systemd-timedated service unavailable"))
        }
    }

    async fn power_action(&self, action: &str) -> Result<()> {
        let cmd = crate::security::sanitize_systemctl_action(action)
            .ok_or_else(|| anyhow!("Unauthorized or invalid power action: '{}'", action))?;

        if let Some(conn) = &self.connection {
            if let Ok(proxy) = LogindManagerProxy::new(conn).await {
                let can_res = match cmd {
                    "poweroff" => proxy.can_power_off().await,
                    "reboot" => proxy.can_reboot().await,
                    "suspend" => proxy.can_suspend().await,
                    "hibernate" => proxy.can_hibernate().await,
                    _ => Ok("na".to_string()),
                };

                if let Ok(verdict) = can_res {
                    match verdict.as_str() {
                        "na" => {
                            return Err(anyhow!(
                                "Power action '{}' is not supported on this hardware/kernel configuration.",
                                action
                            ));
                        }
                        "no" => {
                            return Err(anyhow!(
                                "Power action '{}' is disallowed by system security policy.",
                                action
                            ));
                        }
                        _ => {} // "yes" or "challenge" - proceed
                    }
                }

                let call_res = match cmd {
                    "poweroff" => proxy.power_off(true).await,
                    "reboot" => proxy.reboot(true).await,
                    "suspend" => proxy.suspend(true).await,
                    "hibernate" => proxy.hibernate(true).await,
                    _ => Err(zbus::Error::Failure("Unsupported action".to_string())),
                };

                match call_res {
                    Ok(_) => return Ok(()),
                    Err(zbus_err) => {
                        let err_str = zbus_err.to_string();
                        if !err_str.contains("UnknownMethod") {
                            return Err(anyhow!(
                                "System logind rejected '{}': {}",
                                action,
                                err_str
                            ));
                        }
                    }
                }
            }
        }

        // Fallback to systemctl command execution with full stderr diagnostics
        let output = std::process::Command::new("systemctl").arg(cmd).output()?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let msg = stderr
                .lines()
                .next()
                .unwrap_or("Failed to execute power action")
                .trim();
            Err(anyhow!("{}", msg))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_system_backend_info() {
        let backend = RealSystemBackend::new_with_connection(None);
        let info = backend.get_info().await.expect("Failed to get system info");
        assert!(!info.distro.is_empty());
        assert!(!info.kernel.is_empty());
        assert!(info.memory_total > 0);
    }

    #[tokio::test]
    async fn test_power_action_rejects_unauthorized_actions() {
        let backend = RealSystemBackend::new_with_connection(None);
        assert!(backend.power_action("destroy").await.is_err());
        assert!(backend.power_action("rm -rf /").await.is_err());
        assert!(backend.power_action("").await.is_err());
        assert!(backend.power_action("poweroff; reboot").await.is_err());
    }

    #[tokio::test]
    async fn test_system_backend_disks_and_host_info() {
        let backend = RealSystemBackend::new_with_connection(None);
        let info = backend.get_info().await.expect("Failed to get system info");
        assert!(!info.hostname.is_empty(), "Hostname should be populated");
        assert!(!info.chassis.is_empty(), "Chassis should be populated");
        assert!(!info.timezone.is_empty(), "Timezone should be populated");
        assert!(
            !info.disks.is_empty(),
            "Disks list should contain root filesystem"
        );
        let root = info.disks.iter().find(|d| d.mount_point == "/");
        assert!(root.is_some(), "Root mount point must be detected");
        assert!(
            root.unwrap().total_bytes > 0,
            "Root disk space must be positive"
        );
    }
}
