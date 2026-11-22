use super::{ServiceInfo, ServicesBackend};
use crate::dbus::systemd::ManagerProxy;
use anyhow::Result;
use async_trait::async_trait;
use zbus::Connection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitSystem {
    Systemd,
    OpenRC,
    Runit,
    Unknown,
}

impl InitSystem {
    pub fn detect() -> Self {
        if std::path::Path::new("/run/systemd/system").exists() {
            InitSystem::Systemd
        } else if std::path::Path::new("/run/openrc").exists()
            || crate::platform::binary_exists("rc-service")
        {
            InitSystem::OpenRC
        } else if std::path::Path::new("/run/runit").exists()
            || std::path::Path::new("/var/service").exists()
            || crate::platform::binary_exists("sv")
        {
            InitSystem::Runit
        } else {
            InitSystem::Unknown
        }
    }
}

pub struct SystemdBackend {
    connection: Connection,
}

impl SystemdBackend {
    pub async fn new() -> Result<Self> {
        // Use system bus for systemd
        let connection = Connection::system().await?;
        Ok(Self { connection })
    }
}

#[async_trait]
impl ServicesBackend for SystemdBackend {
    async fn get_services(&self) -> Result<Vec<ServiceInfo>> {
        let manager = ManagerProxy::new(&self.connection).await?;
        let units = manager.list_units().await?;

        let mut services = Vec::new();

        for (name, description, _load, active, sub, _, _, _, _, _) in units {
            if name.ends_with(".service") {
                services.push(ServiceInfo {
                    name,
                    description,
                    active_state: active,
                    sub_state: sub,
                });
            }
        }

        // Sort alphabetically
        services.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(services)
    }

    async fn start_service(&self, name: &str) -> Result<()> {
        let manager = ManagerProxy::new(&self.connection).await?;
        manager.start_unit(name, "replace").await?;
        Ok(())
    }

    async fn stop_service(&self, name: &str) -> Result<()> {
        let manager = ManagerProxy::new(&self.connection).await?;
        manager.stop_unit(name, "replace").await?;
        Ok(())
    }
}

pub struct OpenRcBackend;

impl OpenRcBackend {
    pub fn parse_rc_status(output: &str) -> Vec<ServiceInfo> {
        let mut services = Vec::new();
        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("Runlevel:")
                || trimmed.starts_with("Dynamic:")
            {
                continue;
            }
            if let Some(bracket_pos) = trimmed.find('[') {
                let name = trimmed[..bracket_pos].trim().to_string();
                let state = trimmed[bracket_pos..]
                    .trim_matches(|c| c == '[' || c == ']' || c == ' ')
                    .to_string();
                let active_state = if state == "started" {
                    "active"
                } else {
                    "inactive"
                };
                if !name.is_empty() {
                    services.push(ServiceInfo {
                        name,
                        description: "OpenRC Service".to_string(),
                        active_state: active_state.to_string(),
                        sub_state: state,
                    });
                }
            }
        }
        services.sort_by(|a, b| a.name.cmp(&b.name));
        services
    }
}

#[async_trait]
impl ServicesBackend for OpenRcBackend {
    async fn get_services(&self) -> Result<Vec<ServiceInfo>> {
        let output = tokio::process::Command::new("rc-status")
            .arg("-a")
            .output()
            .await?;
        let out = String::from_utf8_lossy(&output.stdout);
        Ok(Self::parse_rc_status(&out))
    }

    async fn start_service(&self, name: &str) -> Result<()> {
        tokio::process::Command::new("rc-service")
            .args([name, "start"])
            .status()
            .await?;
        Ok(())
    }

    async fn stop_service(&self, name: &str) -> Result<()> {
        tokio::process::Command::new("rc-service")
            .args([name, "stop"])
            .status()
            .await?;
        Ok(())
    }
}

pub struct RunitBackend;

impl RunitBackend {
    pub fn parse_sv_status(name: &str, output: &str) -> ServiceInfo {
        let out = output.trim();
        let (active_state, sub_state) = if out.starts_with("run:") {
            ("active".to_string(), "running".to_string())
        } else if out.starts_with("down:") {
            ("inactive".to_string(), "stopped".to_string())
        } else {
            ("unknown".to_string(), out.to_string())
        };

        ServiceInfo {
            name: name.to_string(),
            description: "runit Service".to_string(),
            active_state,
            sub_state,
        }
    }
}

#[async_trait]
impl ServicesBackend for RunitBackend {
    async fn get_services(&self) -> Result<Vec<ServiceInfo>> {
        let service_dir = if std::path::Path::new("/var/service").exists() {
            "/var/service"
        } else if std::path::Path::new("/run/runit/service").exists() {
            "/run/runit/service"
        } else {
            return Ok(Vec::new());
        };

        let mut services = Vec::new();
        if let Ok(entries) = std::fs::read_dir(service_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue;
                }
                let output = tokio::process::Command::new("sv")
                    .args(["status", &name])
                    .output()
                    .await;
                let out_str = output
                    .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                    .unwrap_or_default();
                services.push(Self::parse_sv_status(&name, &out_str));
            }
        }
        services.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(services)
    }

    async fn start_service(&self, name: &str) -> Result<()> {
        tokio::process::Command::new("sv")
            .args(["up", name])
            .status()
            .await?;
        Ok(())
    }

    async fn stop_service(&self, name: &str) -> Result<()> {
        tokio::process::Command::new("sv")
            .args(["down", name])
            .status()
            .await?;
        Ok(())
    }
}

pub async fn create_services_backend() -> Option<Box<dyn ServicesBackend>> {
    match InitSystem::detect() {
        InitSystem::Systemd => {
            if let Ok(b) = SystemdBackend::new().await {
                Some(Box::new(b) as Box<dyn ServicesBackend>)
            } else {
                None
            }
        }
        InitSystem::OpenRC => Some(Box::new(OpenRcBackend) as Box<dyn ServicesBackend>),
        InitSystem::Runit => Some(Box::new(RunitBackend) as Box<dyn ServicesBackend>),
        InitSystem::Unknown => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_system_detect() {
        let init = InitSystem::detect();
        // Should detect one of the variants without panic
        assert!(matches!(
            init,
            InitSystem::Systemd | InitSystem::OpenRC | InitSystem::Runit | InitSystem::Unknown
        ));
    }

    #[test]
    fn test_openrc_status_parsing() {
        let sample = "Runlevel: default\n dbus                           [  started  ]\n sshd                           [  stopped  ]\n";
        let services = OpenRcBackend::parse_rc_status(sample);
        assert_eq!(services.len(), 2);
        assert_eq!(services[0].name, "dbus");
        assert_eq!(services[0].active_state, "active");
        assert_eq!(services[1].name, "sshd");
        assert_eq!(services[1].active_state, "inactive");
    }

    #[test]
    fn test_runit_status_parsing() {
        let run_sample = "run: dbus: (pid 1234) 50s\n";
        let svc_run = RunitBackend::parse_sv_status("dbus", run_sample);
        assert_eq!(svc_run.name, "dbus");
        assert_eq!(svc_run.active_state, "active");
        assert_eq!(svc_run.sub_state, "running");

        let down_sample = "down: sshd: 10s, normally up\n";
        let svc_down = RunitBackend::parse_sv_status("sshd", down_sample);
        assert_eq!(svc_down.name, "sshd");
        assert_eq!(svc_down.active_state, "inactive");
        assert_eq!(svc_down.sub_state, "stopped");
    }
}
