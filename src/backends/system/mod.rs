use super::{SystemBackend, SystemInfo};
use anyhow::Result;
use async_trait::async_trait;
use sysinfo::System;

pub struct RealSystemBackend {
    sys: std::sync::Mutex<System>,
}

impl RealSystemBackend {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        Self {
            sys: std::sync::Mutex::new(sys),
        }
    }
}

#[async_trait]
impl SystemBackend for RealSystemBackend {
    async fn get_info(&self) -> Result<SystemInfo> {
        let mut sys = self
            .sys
            .lock()
            .map_err(|_| anyhow::anyhow!("System info lock poisoned"))?;
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
        let cmd = match action {
            "poweroff" => "poweroff",
            "reboot" => "reboot",
            "suspend" => "suspend",
            "hibernate" => "hibernate",
            _ => return Err(anyhow::anyhow!("Unknown power action")),
        };

        let status = std::process::Command::new("systemctl").arg(cmd).status()?;

        if status.success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to execute {}", cmd))
        }
    }
}
