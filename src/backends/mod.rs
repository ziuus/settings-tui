use anyhow::Result;
use async_trait::async_trait;

pub mod appearance;
pub mod applications;
pub mod audio;
pub mod bluetooth;
pub mod display;
pub mod input;
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
    async fn get_volume(&self, id: u32) -> Result<(f64, bool)>;
    async fn set_default_sink(&self, id: u32) -> Result<()>;
    async fn set_default_source(&self, id: u32) -> Result<()>;
    async fn set_volume(&self, id: u32, volume: f64) -> Result<()>;
    #[allow(dead_code)]
    async fn set_mute(&self, id: u32, mute: bool) -> Result<()>;
    async fn toggle_mute(&self, id: u32) -> Result<()>;
}

// Core definitions for networks
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkId(pub String);

#[derive(Debug, Clone)]
pub struct ActiveConnectionInfo {
    pub interface: String,
    pub ip_address: String,
    pub gateway: String,
    pub mac_address: String,
    pub dns_servers: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Network {
    pub id: NetworkId,
    pub name: String,
    pub connected: bool,
    pub strength: u8, // 0-100
    pub saved: bool,
    pub security: String,
    pub frequency_mhz: u32,
}

#[async_trait]
pub trait NetworkBackend: Send + Sync {
    async fn wifi_enabled(&self) -> Result<bool>;
    async fn set_wifi_enabled(&self, enabled: bool) -> Result<()>;
    async fn networks(&self) -> Result<Vec<Network>>;
    async fn get_active_connection(&self) -> Result<Option<ActiveConnectionInfo>>;
    async fn connect(&self, network: &NetworkId) -> Result<()>;
    async fn connect_with_password(&self, network: &NetworkId, password: &str) -> Result<()>;
    async fn disconnect(&self, network: &NetworkId) -> Result<()>;
    async fn forget_network(&self, network: &NetworkId) -> Result<()>;
    async fn rescan(&self) -> Result<()>;
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DefaultAppsInfo {
    pub web_browser: Option<String>,
    pub file_manager: Option<String>,
    pub mail_client: Option<String>,
    pub text_editor: Option<String>,
}

// System Backend
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemDiskInfo {
    pub mount_point: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub fs_type: String,
}

#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub hostname: String,
    pub chassis: String,
    pub distro: String,
    pub kernel: String,
    pub uptime: u64,
    pub memory_total: u64,
    pub memory_used: u64,
    pub timezone: String,
    pub ntp_active: bool,
    pub disks: Vec<SystemDiskInfo>,
    pub cpu_model: String,
    pub cpu_cores: usize,
}

#[async_trait]
pub trait SystemBackend: Send + Sync {
    async fn get_info(&self) -> Result<SystemInfo>;
    async fn power_action(&self, action: &str) -> Result<()>;
    async fn set_ntp(&self, active: bool) -> Result<()>;
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
    pub energy_wh: Option<f64>,
    pub energy_full_wh: Option<f64>,
    pub energy_full_design_wh: Option<f64>,
    pub energy_rate_w: Option<f64>,
    pub health_percentage: Option<f64>,
    pub charge_cycles: Option<i32>,
    pub voltage_v: Option<f64>,
    pub time_to_empty_secs: Option<i64>,
    pub time_to_full_secs: Option<i64>,
    pub battery_model: Option<String>,
    pub battery_vendor: Option<String>,
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
    async fn get_brightness(&self) -> Result<Option<u32>>;
    async fn set_brightness(&self, percent: u32) -> Result<()>;
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
    async fn get_default_apps(&self) -> Result<DefaultAppsInfo>;
}

// Core definitions for Input (Mouse & Touchpad)
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InputSettings {
    pub natural_scroll: bool,
    pub tap_to_click: bool,
    pub left_handed: bool,
    pub sensitivity: f64, // -1.0 to 1.0
}

#[async_trait]
pub trait InputBackend: Send + Sync {
    async fn get_settings(&self) -> Result<InputSettings>;
    async fn set_natural_scroll(&self, enabled: bool) -> Result<()>;
    async fn set_tap_to_click(&self, enabled: bool) -> Result<()>;
    async fn set_left_handed(&self, enabled: bool) -> Result<()>;
    async fn set_sensitivity(&self, sensitivity: f64) -> Result<()>;
}
