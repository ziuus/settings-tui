use std::sync::Arc;
use tokio::sync::mpsc;
use crate::app::AppEvent;
use crate::dbus::network_manager::NetworkManagerProxy;
use crate::dbus::bluez::ObjectManagerProxy;
use crate::backends::{NetworkBackend, BluetoothBackend, AppearanceBackend, AudioBackend};
use zbus::Connection;
use futures_util::StreamExt;

pub async fn spawn_dbus_listeners(
    tx: mpsc::Sender<AppEvent>,
    net_backend: Option<Arc<crate::backends::network::NetworkManagerBackend>>,
) {
    if let Ok(conn) = Connection::system().await {
        // NetworkManager properties changed
        if let Ok(proxy) = NetworkManagerProxy::new(&conn).await {
            if let Ok(mut stream) = proxy.receive_wireless_enabled_changed().await {
                let tx = tx.clone();
                let net_backend = net_backend.clone();
                tokio::spawn(async move {
                    while let Some(_) = stream.next().await {
                        if let Some(net) = &net_backend {
                            if let (Ok(enabled), Ok(nets)) = (net.wifi_enabled().await, net.networks().await) {
                                let _ = tx.send(AppEvent::UpdateNetworks(enabled, nets)).await;
                            }
                        }
                    }
                });
            }
        }
    }
}
