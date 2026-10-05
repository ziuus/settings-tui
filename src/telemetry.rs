use crate::config::SettingsConfig;
use serde::Serialize;
use std::env;

#[derive(Serialize)]
struct TelemetryPayload {
    app: String,
    version: String,
    machine_id: String,
    os: String,
    arch: String,
}

pub async fn ping_telemetry(config: &SettingsConfig) {
    if !config.telemetry_enabled {
        return;
    }

    if env::var("SETTINGS_TUI_DO_NOT_TRACK").is_ok() {
        return;
    }

    // Set your tracking endpoint here (e.g. PostHog, Plausible, Custom API)
    let endpoint = "https://settings-tui.vercel.app/api/telemetry";

    let payload = TelemetryPayload {
        app: "settings-tui".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        machine_id: config.machine_id.clone(),
        os: env::consts::OS.to_string(),
        arch: env::consts::ARCH.to_string(),
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build();

    if let Ok(client) = client {
        // Fire and forget (do not await response or block)
        let _ = client.post(endpoint).json(&payload).send().await;
    }
}
