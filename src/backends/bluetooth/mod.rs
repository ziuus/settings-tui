use super::{BluetoothAdapter, BluetoothBackend, BluetoothDevice, BluetoothDeviceId};
use crate::dbus::bluez::{Adapter1Proxy, Device1Proxy, ObjectManagerProxy};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use zbus::Connection;

pub struct BlueZBackend {
    connection: Connection,
}

impl BlueZBackend {
    pub async fn new() -> Result<Self> {
        let connection = Connection::system().await?;
        Ok(Self { connection })
    }

    async fn get_adapter_path(&self) -> Result<Option<String>> {
        let manager = ObjectManagerProxy::new(&self.connection).await?;
        let objects = manager.get_managed_objects().await?;

        for (path, interfaces) in objects {
            if interfaces.contains_key("org.bluez.Adapter1") {
                return Ok(Some(path.to_string()));
            }
        }
        Ok(None)
    }
}

#[async_trait]
impl BluetoothBackend for BlueZBackend {
    async fn get_adapter(&self) -> Result<Option<BluetoothAdapter>> {
        let path = self.get_adapter_path().await?;
        if let Some(p) = path {
            let proxy = Adapter1Proxy::builder(&self.connection)
                .path(p.clone())?
                .build()
                .await?;

            let name = proxy.alias().await.unwrap_or_default();
            let address = proxy.address().await.unwrap_or_default();
            let powered = proxy.powered().await.unwrap_or(false);
            let discoverable = proxy.discoverable().await.unwrap_or(false);
            let discovering = proxy.discovering().await.unwrap_or(false);

            Ok(Some(BluetoothAdapter {
                path: p,
                name,
                address,
                powered,
                discoverable,
                discovering,
            }))
        } else {
            Ok(None)
        }
    }

    async fn set_powered(&self, powered: bool) -> Result<()> {
        let path = self.get_adapter_path().await?;
        if let Some(p) = path {
            let proxy = Adapter1Proxy::builder(&self.connection)
                .path(p)?
                .build()
                .await?;
            proxy.set_powered(powered).await?;
            Ok(())
        } else {
            Err(anyhow!("No Bluetooth adapter found"))
        }
    }

    async fn set_discoverable(&self, discoverable: bool) -> Result<()> {
        let path = self.get_adapter_path().await?;
        if let Some(p) = path {
            let proxy = Adapter1Proxy::builder(&self.connection)
                .path(p)?
                .build()
                .await?;
            proxy.set_discoverable(discoverable).await?;
            Ok(())
        } else {
            Err(anyhow!("No Bluetooth adapter found"))
        }
    }

    async fn devices(&self) -> Result<Vec<BluetoothDevice>> {
        let manager = ObjectManagerProxy::new(&self.connection).await?;
        let objects = manager.get_managed_objects().await?;
        let mut devices = Vec::new();

        for (path, interfaces) in objects {
            if interfaces.contains_key("org.bluez.Device1") {
                let proxy = Device1Proxy::builder(&self.connection)
                    .path(path.clone())?
                    .build()
                    .await?;

                let mut name = proxy.alias().await.unwrap_or_default();
                if name.is_empty() {
                    name = proxy
                        .name()
                        .await
                        .unwrap_or_else(|_| "Unknown Device".to_string());
                }

                let address = proxy.address().await.unwrap_or_default();
                let icon = proxy
                    .icon()
                    .await
                    .unwrap_or_else(|_| "bluetooth".to_string());
                let connected = proxy.connected().await.unwrap_or(false);
                let paired = proxy.paired().await.unwrap_or(false);
                let trusted = proxy.trusted().await.unwrap_or(false);
                let blocked = proxy.blocked().await.unwrap_or(false);
                let rssi = proxy.rssi().await.ok(); // Optional

                devices.push(BluetoothDevice {
                    id: BluetoothDeviceId(path.to_string()),
                    address,
                    name,
                    icon,
                    connected,
                    paired,
                    trusted,
                    blocked,
                    rssi,
                });
            }
        }
        Ok(devices)
    }

    async fn connect_device(&self, id: &BluetoothDeviceId) -> Result<()> {
        let proxy = Device1Proxy::builder(&self.connection)
            .path(id.0.clone())?
            .build()
            .await?;
        proxy.connect().await?;
        Ok(())
    }

    async fn disconnect_device(&self, id: &BluetoothDeviceId) -> Result<()> {
        let proxy = Device1Proxy::builder(&self.connection)
            .path(id.0.clone())?
            .build()
            .await?;
        proxy.disconnect().await?;
        Ok(())
    }

    async fn remove_device(&self, id: &BluetoothDeviceId) -> Result<()> {
        let adapter_path = self.get_adapter_path().await?;
        if let Some(p) = adapter_path {
            let proxy = Adapter1Proxy::builder(&self.connection)
                .path(p)?
                .build()
                .await?;

            // Adapter1 has a RemoveDevice method which takes an object path
            let path = zbus::zvariant::ObjectPath::try_from(id.0.clone())?;
            proxy.remove_device(&path).await?;
            Ok(())
        } else {
            Err(anyhow!("No Bluetooth adapter found"))
        }
    }
}
