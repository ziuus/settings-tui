use anyhow::Result;
use async_trait::async_trait;

pub mod appearance;
pub mod applications;
pub mod audio;
pub mod bluetooth;
pub mod display;
pub mod network;
pub mod power;
pub mod system;
pub mod systemd;

// Core definitions for Audio
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AudioDevice {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub is_default: bool,
    pub volume: f64,
    pub muted: bool,
    pub is_sink: bool,
    pub form_factor: String,
}

#[derive(Debug, Clone)]
pub struct AudioStream {
    pub id: u32,
    pub application_name: String,
    pub volume: f64,
    pub muted: bool,
    pub is_sink_input: bool, // true if playing audio, false if recording
}
#[async_trait]
pub trait AudioBackend: Send + Sync {
    async fn get_sinks(&self) -> Result<Vec<AudioDevice>>;
    async fn get_sources(&self) -> Result<Vec<AudioDevice>>;
    async fn get_streams(&self) -> Result<Vec<AudioStream>>;
    async fn set_default_sink(&self, id: u32) -> Result<()>;
    async fn set_volume(&self, id: u32, volume: f64) -> Result<()>;
    #[allow(dead_code)]
    async fn set_mute(&self, id: u32, mute: bool) -> Result<()>;
    async fn toggle_mute(&self, id: u32) -> Result<()>;
}

// Core definitions for networks
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkId(pub String);

#[derive(Debug, Clone)]
pub struct Network {
    pub id: NetworkId,
    pub name: String,
    pub connected: bool,
    pub strength: u8, // 0-100
}

#[async_trait]
pub trait NetworkBackend: Send + Sync {
    async fn wifi_enabled(&self) -> Result<bool>;
    async fn set_wifi_enabled(&self, enabled: bool) -> Result<()>;
    async fn networks(&self) -> Result<Vec<Network>>;
    async fn connect(&self, network: &NetworkId) -> Result<()>;
    async fn disconnect(&self, network: &NetworkId) -> Result<()>;
}

// System Backend
#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub distro: String,
    pub kernel: String,
    pub uptime: u64,
    pub memory_total: u64,
    pub memory_used: u64,
}

#[async_trait]
pub trait SystemBackend: Send + Sync {
    async fn get_info(&self) -> Result<SystemInfo>;
    async fn power_action(&self, action: &str) -> Result<()>;
}

// Core definitions for Bluetooth
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BluetoothDeviceId(pub String);

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct BluetoothAdapter {
    pub path: String,
    pub name: String,
    pub address: String,
    pub powered: bool,
    pub discoverable: bool,
    pub discovering: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct BluetoothDevice {
    pub id: BluetoothDeviceId,
    pub address: String,
    pub name: String,
    pub icon: String,
    pub connected: bool,
    pub paired: bool,
    pub trusted: bool,
    pub blocked: bool,
    pub rssi: Option<i16>,
}

#[async_trait]
pub trait BluetoothBackend: Send + Sync {
    async fn get_adapter(&self) -> Result<Option<BluetoothAdapter>>;
    async fn set_powered(&self, powered: bool) -> Result<()>;
    #[allow(dead_code)]
    async fn set_discoverable(&self, discoverable: bool) -> Result<()>;
    async fn devices(&self) -> Result<Vec<BluetoothDevice>>;

    async fn connect_device(&self, id: &BluetoothDeviceId) -> Result<()>;
    async fn disconnect_device(&self, id: &BluetoothDeviceId) -> Result<()>;
    async fn remove_device(&self, id: &BluetoothDeviceId) -> Result<()>;
}

// Core definitions for Power
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatteryState {
    Unknown,
    Charging,
    Discharging,
    Empty,
    FullyCharged,
    PendingCharge,
    PendingDischarge,
}

#[derive(Debug, Clone)]
pub struct PowerInfo {
    pub on_battery: bool,
    pub battery_percentage: f64,
    pub battery_state: BatteryState,
    pub power_profile: Option<String>,
}

#[async_trait]
pub trait PowerBackend: Send + Sync {
    async fn get_info(&self) -> Result<PowerInfo>;
    async fn set_power_profile(&self, profile: &str) -> Result<()>;
}

// Core definitions for Services
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ServiceInfo {
    pub name: String,
    pub description: String,
    pub active_state: String, // "active", "inactive", "failed"
    pub sub_state: String,
}

#[async_trait]
pub trait ServicesBackend: Send + Sync {
    async fn get_services(&self) -> Result<Vec<ServiceInfo>>;
    async fn start_service(&self, name: &str) -> Result<()>;
    async fn stop_service(&self, name: &str) -> Result<()>;
}

// Core definitions for Display
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Monitor {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub width: i32,
    pub height: i32,
    pub refresh_rate: f64,
    pub scale: f64,
    pub active: bool,
    pub primary: bool,
    pub supported_modes: Vec<String>,
}

#[async_trait]
pub trait DisplayBackend: Send + Sync {
    async fn get_monitors(&self) -> Result<Vec<Monitor>>;
    async fn set_resolution(&self, name: &str, width: i32, height: i32, refresh: f64)
        -> Result<()>;
}

// Core definitions for Appearance
#[derive(Debug, Clone)]
pub struct AppearanceInfo {
    pub color_scheme: String, // "default", "prefer-dark", "prefer-light"
    pub gtk_theme: String,
    pub icon_theme: String,
    pub cursor_theme: String,
    pub font_name: String,
}

#[async_trait]
pub trait AppearanceBackend: Send + Sync {
    async fn get_info(&self) -> Result<AppearanceInfo>;
    async fn set_color_scheme(&self, scheme: &str) -> Result<()>;
}

// Core definitions for Applications
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub exec: String,
    pub is_flatpak: bool,
}

#[async_trait]
pub trait ApplicationsBackend: Send + Sync {
    async fn get_applications(&self) -> Result<Vec<AppEntry>>;
    async fn launch_application(&self, exec: &str) -> Result<()>;
}
