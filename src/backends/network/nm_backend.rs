use super::{ActiveConnectionInfo, Network, NetworkBackend, NetworkId};
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

        let mut saved_names = std::collections::HashSet::new();
        if let Ok(out) = std::process::Command::new("nmcli")
            .args(["-t", "-f", "NAME,TYPE", "connection", "show"])
            .output()
        {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                for line in text.lines() {
                    let parts: Vec<&str> = line.split(':').collect();
                    if parts.len() >= 2 && parts[1] == "802-11-wireless" {
                        saved_names.insert(parts[0].to_string());
                    }
                }
            }
        }

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
                            let flags = ap_proxy.flags().await.unwrap_or(0);
                            let wpa_flags = ap_proxy.wpa_flags().await.unwrap_or(0);
                            let rsn_flags = ap_proxy.rsn_flags().await.unwrap_or(0);
                            let frequency_mhz = ap_proxy.frequency().await.unwrap_or(0);

                            let security = if rsn_flags > 0 {
                                "WPA2/WPA3".to_string()
                            } else if wpa_flags > 0 {
                                "WPA".to_string()
                            } else if flags > 0 {
                                "WEP".to_string()
                            } else {
                                "Open".to_string()
                            };

                            let connected = if let Some(active_path) = &active_ap_path {
                                active_path.as_str() == ap_path.as_str()
                            } else {
                                false
                            };

                            let saved = saved_names.contains(&name);

                            networks.push(Network {
                                id: NetworkId(name.clone()), // Use SSID as ID for nmcli
                                name,
                                connected,
                                strength,
                                saved,
                                security,
                                frequency_mhz,
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

    async fn get_active_connection(&self) -> Result<Option<ActiveConnectionInfo>> {
        let mut dns_servers = Vec::new();
        if let Ok(resolv) = std::fs::read_to_string("/etc/resolv.conf") {
            for line in resolv.lines() {
                let line = line.trim();
                if line.starts_with("nameserver") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        dns_servers.push(parts[1].to_string());
                    }
                }
            }
        }

        let output = std::process::Command::new("ip")
            .args(["route", "show", "default"])
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                for line in text.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    let mut gw = String::new();
                    let mut dev = String::new();
                    let mut src = String::new();

                    let mut i = 0;
                    while i < parts.len() {
                        if parts[i] == "via" && i + 1 < parts.len() {
                            gw = parts[i + 1].to_string();
                            i += 2;
                        } else if parts[i] == "dev" && i + 1 < parts.len() {
                            dev = parts[i + 1].to_string();
                            i += 2;
                        } else if parts[i] == "src" && i + 1 < parts.len() {
                            src = parts[i + 1].to_string();
                            i += 2;
                        } else {
                            i += 1;
                        }
                    }

                    if !dev.is_empty() {
                        return Ok(Some(ActiveConnectionInfo {
                            interface: dev,
                            ip_address: if src.is_empty() {
                                "N/A".to_string()
                            } else {
                                src
                            },
                            gateway: if gw.is_empty() { "N/A".to_string() } else { gw },
                            dns_servers,
                        }));
                    }
                }
            }
        }

        Ok(None)
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

    async fn connect_with_password(&self, network: &NetworkId, password: &str) -> Result<()> {
        let output = std::process::Command::new("nmcli")
            .arg("device")
            .arg("wifi")
            .arg("connect")
            .arg(&network.0)
            .arg("password")
            .arg(password)
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
                .unwrap_or("Connection failed with provided password")
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

    async fn forget_network(&self, network: &NetworkId) -> Result<()> {
        let output = std::process::Command::new("nmcli")
            .arg("connection")
            .arg("delete")
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
                .unwrap_or("Failed to delete network profile")
                .trim()
                .to_string();
            Err(anyhow::anyhow!("{}", msg))
        }
    }
}
