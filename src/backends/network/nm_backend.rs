use super::{Network, NetworkBackend, NetworkId};
use crate::dbus::network_manager::{
    AccessPointProxy, DeviceProxy, NetworkManagerProxy, WirelessDeviceProxy,
};
use anyhow::Result;
use async_trait::async_trait;
use zbus::Connection;

pub struct NetworkManagerBackend {
    connection: Connection,
}

impl NetworkManagerBackend {
    pub async fn new() -> Result<Self> {
        let connection = Connection::system().await?;
        Ok(Self { connection })
    }
}

#[async_trait]
impl NetworkBackend for NetworkManagerBackend {
    async fn wifi_enabled(&self) -> Result<bool> {
        let proxy = NetworkManagerProxy::new(&self.connection).await?;
        Ok(proxy.wireless_enabled().await?)
    }

    async fn set_wifi_enabled(&self, enabled: bool) -> Result<()> {
        let proxy = NetworkManagerProxy::new(&self.connection).await?;
        proxy.set_wireless_enabled(enabled).await?;
        Ok(())
    }

    async fn networks(&self) -> Result<Vec<Network>> {
        let proxy = NetworkManagerProxy::new(&self.connection).await?;
        let devices = proxy.get_devices().await?;

        let mut networks = Vec::new();

        for dev_path in devices {
            let dev_proxy = DeviceProxy::builder(&self.connection)
                .path(dev_path.clone())?
                .build()
                .await?;
            let dev_type = dev_proxy.device_type().await.unwrap_or(0);

            // 2 is NM_DEVICE_TYPE_WIFI
            if dev_type == 2 {
                let wireless_proxy = WirelessDeviceProxy::builder(&self.connection)
                    .path(dev_path.clone())?
                    .build()
                    .await?;
                let active_ap_path = wireless_proxy.active_access_point().await.ok();

                if let Ok(aps) = wireless_proxy.get_all_access_points().await {
                    for ap_path in aps {
                        let ap_proxy = AccessPointProxy::builder(&self.connection)
                            .path(ap_path.clone())?
                            .build()
                            .await?;

                        if let Ok(ssid_bytes) = ap_proxy.ssid().await {
                            if ssid_bytes.is_empty() {
                                continue;
                            }
                            let name = String::from_utf8_lossy(&ssid_bytes).to_string();
                            let strength = ap_proxy.strength().await.unwrap_or(0);

                            let connected = if let Some(active_path) = &active_ap_path {
                                active_path.as_str() == ap_path.as_str()
                            } else {
                                false
                            };

                            networks.push(Network {
                                id: NetworkId(name.clone()), // Use SSID as ID for nmcli
                                name,
                                connected,
                                strength,
                            });
                        }
                    }
                }
            }
        }

        // Sort and deduplicate by name, preferring stronger/connected networks
        networks.sort_by(|a, b| {
            b.connected
                .cmp(&a.connected)
                .then(b.strength.cmp(&a.strength))
        });

        let mut unique_networks = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for net in networks {
            if !seen.contains(&net.name) {
                seen.insert(net.name.clone());
                unique_networks.push(net);
            }
        }

        Ok(unique_networks)
    }

    async fn connect(&self, network: &NetworkId) -> Result<()> {
        let output = std::process::Command::new("nmcli")
            .arg("device")
            .arg("wifi")
            .arg("connect")
            .arg(&network.0)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let msg = stderr
                .lines()
                .next()
                .unwrap_or("Connection failed")
                .trim()
                .to_string();
            Err(anyhow::anyhow!("{}", msg))
        }
    }

    async fn disconnect(&self, network: &NetworkId) -> Result<()> {
        let output = std::process::Command::new("nmcli")
            .arg("connection")
            .arg("down")
            .arg("id")
            .arg(&network.0)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let msg = stderr
                .lines()
                .next()
                .unwrap_or("Disconnect failed")
                .trim()
                .to_string();
            Err(anyhow::anyhow!("{}", msg))
        }
    }
}
