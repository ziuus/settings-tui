use super::{SystemBackend, SystemInfo};
use crate::dbus::systemd::LogindManagerProxy;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use sysinfo::System;
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
        let mut sys = self
            .sys
            .lock()
            .map_err(|_| anyhow!("System info lock poisoned"))?;
        sys.refresh_memory();

        let distro = System::name().unwrap_or_else(|| "Unknown".to_string())
            + " "
            + &System::os_version().unwrap_or_default();
        let kernel = System::kernel_version().unwrap_or_else(|| "Unknown".to_string());
        let uptime = System::uptime();
        let memory_total = sys.total_memory();
        let memory_used = sys.used_memory();

        Ok(SystemInfo {
            distro,
            kernel,
            uptime,
            memory_total,
            memory_used,
        })
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
}
