use super::{ServiceInfo, ServicesBackend};
use crate::dbus::systemd::ManagerProxy;
use anyhow::Result;
use async_trait::async_trait;
use zbus::Connection;

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
