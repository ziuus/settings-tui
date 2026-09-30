use crate::app::AppEvent;
use crate::backends::{AudioBackend, NetworkBackend, PowerBackend};
use futures::StreamExt;
use tokio::sync::mpsc;

/// Subscribes to both WirelessEnabled and Connectivity changes.
/// Falls back gracefully if NetworkManager is unavailable.
pub async fn spawn_network_listener(tx: mpsc::Sender<AppEvent>) {
    tokio::spawn(async move {
        let Ok(conn) = zbus::Connection::system().await else {
            return;
        };
        let Ok(proxy) = crate::dbus::network_manager::NetworkManagerProxy::new(&conn).await else {
            return;
        };
        let Ok(net) = crate::backends::network::NetworkManagerBackend::new().await else {
            return;
        };

        // Subscribe to BOTH wireless_enabled AND connectivity property changes.
        let mut wifi_stream = proxy.receive_wireless_enabled_changed().await;
        let mut conn_stream = proxy.receive_connectivity_changed().await;

        loop {
            tokio::select! {
                item = wifi_stream.next() => {
                    if item.is_none() { break; }
                    if let (Ok(enabled), Ok(nets)) =
                        (net.wifi_enabled().await, net.networks().await)
                    {
                        let _ = tx.send(AppEvent::UpdateNetworks(enabled, nets)).await;
                    }
                }
                item = conn_stream.next() => {
                    if item.is_none() { break; }
                    // Connectivity changed: re-fetch network list (active APs updated)
                    if let (Ok(enabled), Ok(nets)) =
                        (net.wifi_enabled().await, net.networks().await)
                    {
                        let _ = tx.send(AppEvent::UpdateNetworks(enabled, nets)).await;
                    }
                }
            }
        }
    });
}

/// Subscribes to UPower battery state AND percentage property changes.
pub async fn spawn_power_listener(tx: mpsc::Sender<AppEvent>) {
    tokio::spawn(async move {
        let Ok(conn) = zbus::Connection::system().await else {
            return;
        };
        let Ok(proxy) = crate::dbus::power::UPowerDeviceProxy::new(&conn).await else {
            return;
        };
        let Ok(power) = crate::backends::power::UPowerBackend::new().await else {
            return;
        };

        let mut state_stream = proxy.receive_state_changed().await;
        let mut pct_stream = proxy.receive_percentage_changed().await;

        loop {
            tokio::select! {
                item = state_stream.next() => {
                    if item.is_none() { break; }
                    if let Ok(info) = power.get_info().await {
                        let _ = tx.send(AppEvent::UpdatePower(info)).await;
                    }
                }
                item = pct_stream.next() => {
                    if item.is_none() { break; }
                    if let Ok(info) = power.get_info().await {
                        let _ = tx.send(AppEvent::UpdatePower(info)).await;
                    }
                }
            }
        }
    });
}

/// Watches PipeWire events via pw-mon with 500ms debounce.
/// pw-mon is part of pipewire-utils. If unavailable, falls back silently —
/// audio state will still update via the 5-second poll in the main loop.
pub async fn spawn_audio_listener(tx: mpsc::Sender<AppEvent>) {
    tokio::spawn(async move {
        let child = tokio::process::Command::new("pw-mon")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn();

        if let Ok(mut child) = child {
            if let Some(stdout) = child.stdout.take() {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let mut reader = BufReader::new(stdout);
                let mut line = String::new();

                let audio = crate::backends::audio::WpctlBackend::new();
                let mut last_update = tokio::time::Instant::now();

                while let Ok(bytes) = reader.read_line(&mut line).await {
                    if bytes == 0 {
                        break;
                    }

                    if line.contains("changed:")
                        || line.contains("added:")
                        || line.contains("removed:")
                    {
                        let now = tokio::time::Instant::now();
                        if now.duration_since(last_update).as_millis() > 500 {
                            if let (Ok(sinks), Ok(sources), Ok(streams)) = (
                                audio.get_sinks().await,
                                audio.get_sources().await,
                                audio.get_streams().await,
                            ) {
                                let _ = tx
                                    .send(AppEvent::UpdateAudio(sinks, sources, streams))
                                    .await;
                            }
                            last_update = now;
                        }
                    }
                    line.clear();
                }
                // pw-mon exited — let the main poll loop handle future updates.
                let _ = child.wait().await;
            }
        }
        // pw-mon not found or exited: audio will update via 5s poll.
    });
}

/// Polls Bluetooth device state every 3 seconds.
/// BlueZ InterfacesAdded/Removed signals require complex proxy lifetime management;
/// a 3-second poll is reliable, safe, and low-overhead for BT (changes are rare).
pub async fn spawn_bluetooth_listener(tx: mpsc::Sender<AppEvent>) {
    use crate::backends::BluetoothBackend;

    tokio::spawn(async move {
        let Ok(bt) = crate::backends::bluetooth::BlueZBackend::new().await else {
            return;
        };

        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

            if let Ok(devices) = bt.devices().await {
                let _ = tx.send(AppEvent::UpdateBluetooth(devices)).await;
            }
            if let Ok(adapter) = bt.get_adapter().await {
                let _ = tx.send(AppEvent::UpdateBluetoothAdapter(adapter)).await;
            }
        }
    });
}
