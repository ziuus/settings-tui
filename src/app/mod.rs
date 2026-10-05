mod listeners;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event as CrosstermEvent, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{error::Error, io, sync::Arc, time::Duration};
use tokio::sync::{mpsc, Mutex};

use crate::backends::{
    system::RealSystemBackend, AppEntry, AppearanceBackend, AppearanceInfo, ApplicationsBackend,
    AudioBackend, AudioDevice, BluetoothBackend, BluetoothDevice, DefaultAppsInfo, DisplayBackend,
    InputSettings, Monitor, Network, NetworkBackend, PowerBackend, PowerInfo, ServiceInfo,
    ServicesBackend, SystemBackend, SystemInfo, VpnConnection,
};
use crate::ui;
use crate::Args;

macro_rules! execute_transaction {
    ($mutate:expr, $read:expr, $verify:expr, $max_retries:expr, $interval_ms:expr) => {{
        let mut mutation_ok = false;
        let mut mutation_error: Option<String> = None;
        match $mutate.await {
            Ok(_) => mutation_ok = true,
            Err(e) => mutation_error = Some(e.to_string()),
        }

        let mut verified = false;
        if mutation_ok {
            for _ in 0..$max_retries {
                tokio::time::sleep(Duration::from_millis($interval_ms)).await;
                if let Ok(state) = $read.await {
                    if $verify(&state) {
                        verified = true;
                        break;
                    }
                }
            }
        }

        let final_state = $read.await;
        (mutation_ok, verified, final_state, mutation_error)
    }};
}

pub enum AppEvent {
    Input(event::KeyEvent),
    Tick,
    UpdateSystemInfo(SystemInfo),
    UpdateNetworks(bool, Vec<Network>),
    UpdateBluetooth(Vec<BluetoothDevice>),
    UpdateBluetoothAdapter(Option<crate::backends::BluetoothAdapter>),
    UpdatePower(PowerInfo),
    UpdateAudio(
        Vec<AudioDevice>,
        Vec<AudioDevice>,
        Vec<crate::backends::AudioStream>,
    ),
    UpdateServices(Vec<ServiceInfo>),
    UpdateMonitors(Vec<Monitor>),
    UpdateAppearance(AppearanceInfo),
    UpdateApplications(Vec<AppEntry>, Option<DefaultAppsInfo>),
    UpdateInputSettings(InputSettings),
    UpdateCapabilities(Box<crate::platform::PlatformCapabilities>),
    UpdateBrightness(Option<u32>),
    UpdateNightLight(Option<bool>),
    UpdateFlightMode(Option<bool>),
    UpdateHotspot(Option<bool>),
    UpdateActiveConnection(Option<crate::backends::ActiveConnectionInfo>),
    UpdateVpns(Vec<VpnConnection>),
    Notification(String),
}

#[derive(Clone)]
pub enum BackendCommand {
    ToggleService(String, bool), // name, start
    ToggleBluetoothPower(bool),
    ConnectBluetooth(crate::backends::BluetoothDeviceId, bool), // id, connect?
    ToggleAudioMute(u32, bool),                                 // id, is_sink
    SetAudioVolume(u32, f64),
    ToggleColorScheme,
    CyclePowerProfile,
    CyclePowerButtonAction(bool),
    CycleLidAction(bool),
    CycleIdleDelay(bool),
    SetChargeLimit(u8),
    ToggleWifi(bool),
    ToggleFlightMode(bool),
    ToggleHotspot(bool),
    SetDisplayResolution(String, i32, i32, f64),
    SetDisplayScale(String, f64),
    SetDisplayBrightness(u32),
    ConnectNetwork(crate::backends::NetworkId, bool),
    ConnectNetworkWithPassword(crate::backends::NetworkId, String),
    ForgetNetwork(crate::backends::NetworkId),
    RescanWifi,
    SetAudioDefault(u32, bool),
    RemoveBluetoothDevice(crate::backends::BluetoothDeviceId),
    LaunchApplication(String),
    SystemPowerAction(String),
    ToggleNTP(bool),
    ToggleNaturalScroll(bool),
    ToggleTapToClick(bool),
    ToggleLeftHanded(bool),
    SetPointerSensitivity(f64),
    ToggleVpn(String, bool), // uuid, activate
    SetHostname(String),
    CycleGtkTheme(bool), // true = next, false = prev
    ToggleNightLight(bool),
    CycleCursorTheme(bool),
    SetFontName(String),
    SetWallpaper(String),
    TestAudio,
    CycleIconTheme(bool),
}

#[derive(PartialEq, Eq, Debug)]
pub enum Focus {
    Sidebar,
    Content,
}

#[derive(Clone, Debug)]
pub struct PasswordModal {
    pub network_name: String,
    pub network_id: crate::backends::NetworkId,
    pub password: String,
    pub show_password: bool,
}

#[derive(Clone, Debug)]
pub struct SearchResult {
    pub title: String,
    pub category: String,
    pub description: String,
    pub target_item_idx: usize,
    pub score: i32,
}

pub struct App {
    pub should_quit: bool,
    pub focus: Focus,
    pub selected_category: usize,
    pub selected_item: usize,
    pub categories: Vec<String>,
    pub system_info: Option<SystemInfo>,
    pub wifi_enabled: bool,
    pub flight_mode_enabled: Option<bool>,
    pub hotspot_enabled: Option<bool>,
    pub networks: Vec<crate::backends::Network>,
    pub bluetooth_devices: Vec<crate::backends::BluetoothDevice>,
    pub bluetooth_powered: bool,
    pub power_info: Option<crate::backends::PowerInfo>,
    pub audio_sinks: Vec<crate::backends::AudioDevice>,
    pub audio_sources: Vec<crate::backends::AudioDevice>,
    pub audio_streams: Vec<crate::backends::AudioStream>,
    pub services: Vec<crate::backends::ServiceInfo>,
    pub monitors: Vec<crate::backends::Monitor>,
    pub display_brightness: Option<u32>,
    pub night_light_enabled: Option<bool>,
    pub active_connection: Option<crate::backends::ActiveConnectionInfo>,
    pub appearance_info: Option<crate::backends::AppearanceInfo>,
    pub applications: Vec<crate::backends::AppEntry>,
    pub default_apps: Option<DefaultAppsInfo>,
    pub input_settings: Option<InputSettings>,
    pub vpns: Vec<VpnConnection>,
    pub cmd_tx: Option<mpsc::Sender<BackendCommand>>,
    pub notifications: Vec<String>,
    pub notification_timer: usize,
    pub is_searching: bool,
    pub search_query: String,
    pub search_results: Vec<SearchResult>,
    pub search_selected_idx: usize,
    pub confirm_action: Option<(String, BackendCommand)>,
    pub password_modal: Option<PasswordModal>,
    pub hostname_modal: Option<String>,
    pub font_modal: Option<String>,
    pub wallpaper_modal: Option<String>, // editing buffer for new hostname
    pub capabilities: crate::platform::PlatformCapabilities,
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            focus: Focus::Sidebar,
            selected_category: 0,
            selected_item: 0,
            categories: vec![
                "Network".to_string(),
                "Bluetooth".to_string(),
                "Power".to_string(),
                "Sound".to_string(),
                "Display".to_string(),
                "Appearance".to_string(),
                "Applications".to_string(),
                "Mouse & Touchpad".to_string(),
                "Services".to_string(),
                "System".to_string(),
            ],
            system_info: None,
            wifi_enabled: true,
            flight_mode_enabled: None,
            hotspot_enabled: None,
            networks: vec![],
            confirm_action: None,
            password_modal: None,
            hostname_modal: None,
            font_modal: None,
            wallpaper_modal: None,
            capabilities: crate::platform::PlatformCapabilities::detect(
                true, true, true, true, true, true, true, true,
            ),
            bluetooth_devices: vec![],
            bluetooth_powered: false,
            power_info: None,
            audio_sinks: vec![],
            audio_sources: vec![],
            audio_streams: vec![],
            services: vec![],
            monitors: vec![],
            display_brightness: None,
            night_light_enabled: None,
            active_connection: None,
            appearance_info: None,
            applications: vec![],
            default_apps: None,
            input_settings: None,
            vpns: vec![],
            cmd_tx: None,
            notifications: vec![],
            notification_timer: 0,
            is_searching: false,
            search_query: String::new(),
            search_results: vec![],
            search_selected_idx: 0,
        }
    }

    pub fn visible_categories(&self) -> Vec<String> {
        if self.search_query.is_empty() {
            self.categories.clone()
        } else {
            let q = self.search_query.to_lowercase();
            self.categories
                .iter()
                .filter(|c| c.to_lowercase().contains(&q))
                .cloned()
                .collect()
        }
    }

    pub fn update_search_results(&mut self) {
        self.search_results.clear();
        self.search_selected_idx = 0;

        if self.search_query.is_empty() {
            return;
        }

        let q = self.search_query.to_lowercase();

        // Helper to calculate score
        // Exact setting title > Exact keyword > Setting description > Category > Backend metadata
        let calc_score =
            |title: &str, category: &str, description: &str, extra: &str, query: &str| -> i32 {
                let t = title.to_lowercase();
                let c = category.to_lowercase();
                let d = description.to_lowercase();
                let e = extra.to_lowercase();

                if t == query {
                    return 100;
                }
                if t.contains(query) {
                    return 85;
                }

                if (query == "microphone" || query == "mic")
                    && (t.contains("mic") || d.contains("input"))
                {
                    return 90;
                }
                if (query == "wifi" || query == "wi-fi")
                    && (t.contains("wi-fi") || t.contains("wifi") || c.contains("network"))
                {
                    return 90;
                }
                if (query == "volume" || query == "speaker" || query == "audio" || query == "sound")
                    && c == "sound"
                {
                    return 85;
                }
                if (query == "refresh rate"
                    || query == "hz"
                    || query == "resolution"
                    || query == "display"
                    || query == "screen")
                    && c == "display"
                {
                    return 85;
                }
                if (query == "dark" || query == "light" || query == "theme" || query == "color")
                    && c == "appearance"
                {
                    return 90;
                }
                if (query == "battery" || query == "power" || query == "charge") && c == "power" {
                    return 85;
                }
                if (query == "service" || query == "systemd" || query == "daemon")
                    && c == "services"
                {
                    return 80;
                }
                if (query == "sleep"
                    || query == "restart"
                    || query == "shutdown"
                    || query == "power off"
                    || query == "reboot")
                    && c == "system"
                {
                    return 90;
                }

                if d.contains(query) {
                    return 60;
                }
                if e.contains(query) {
                    return 45;
                }
                if c.contains(query) {
                    return 30;
                }

                0
            };

        // Search Audio Sinks
        let mut audio_idx = 0;
        for dev in &self.audio_sinks {
            let score = calc_score(
                &dev.description,
                "Sound",
                "Output Device Volume",
                &dev.name,
                &q,
            );
            if score > 0 {
                self.search_results.push(SearchResult {
                    title: dev.description.clone(),
                    category: "Sound".to_string(),
                    description: "Output Device Volume".to_string(),
                    target_item_idx: audio_idx,
                    score,
                });
            }
            audio_idx += 1;
        }

        // Search Audio Sources
        for dev in &self.audio_sources {
            let score = calc_score(
                &dev.description,
                "Sound",
                "Input Device Volume / Microphone",
                &dev.name,
                &q,
            );
            if score > 0 {
                self.search_results.push(SearchResult {
                    title: dev.description.clone(),
                    category: "Sound".to_string(),
                    description: "Input Device Volume / Microphone".to_string(),
                    target_item_idx: audio_idx,
                    score,
                });
            }
            audio_idx += 1;
        }

        // Search Audio Streams
        for dev in &self.audio_streams {
            let score = calc_score(
                &dev.application_name,
                "Sound",
                "Application Audio Stream Volume",
                "",
                &q,
            );
            if score > 0 {
                self.search_results.push(SearchResult {
                    title: dev.application_name.clone(),
                    category: "Sound".to_string(),
                    description: "Application Audio Stream Volume".to_string(),
                    target_item_idx: audio_idx,
                    score,
                });
            }
            audio_idx += 1;
        }

        // Search Network
        let wifi_score = calc_score(
            "Wi-Fi Switch",
            "Network",
            "Turn Wi-Fi on or off",
            "wireless network connection",
            &q,
        );
        if wifi_score > 0 {
            self.search_results.push(SearchResult {
                title: "Wi-Fi Switch".to_string(),
                category: "Network".to_string(),
                description: "Turn Wi-Fi on or off".to_string(),
                target_item_idx: 0,
                score: wifi_score,
            });
        }

        for (i, net) in self.networks.iter().enumerate() {
            let score = calc_score(
                &net.name,
                "Network",
                "Saved/Available Wi-Fi Network",
                "ssid wifi",
                &q,
            );
            if score > 0 {
                self.search_results.push(SearchResult {
                    title: net.name.clone(),
                    category: "Network".to_string(),
                    description: "Saved/Available Wi-Fi Network".to_string(),
                    target_item_idx: i + 1,
                    score,
                });
            }
        }

        // Search Bluetooth
        let bt_score = calc_score(
            "Bluetooth Power",
            "Bluetooth",
            "Enable or disable Bluetooth adapter",
            "bt wireless pairing",
            &q,
        );
        if bt_score > 0 {
            self.search_results.push(SearchResult {
                title: "Bluetooth Power".to_string(),
                category: "Bluetooth".to_string(),
                description: "Enable or disable Bluetooth adapter".to_string(),
                target_item_idx: 0,
                score: bt_score,
            });
        }

        for (i, dev) in self.bluetooth_devices.iter().enumerate() {
            let score = calc_score(
                &dev.name,
                "Bluetooth",
                "Paired / Discovered Bluetooth Device",
                &dev.address,
                &q,
            );
            if score > 0 {
                self.search_results.push(SearchResult {
                    title: dev.name.clone(),
                    category: "Bluetooth".to_string(),
                    description: "Bluetooth Device".to_string(),
                    target_item_idx: i + 1,
                    score,
                });
            }
        }

        // Search Display
        let bright_score = calc_score(
            "Screen Brightness",
            "Display",
            "Adjust screen backlight brightness level",
            "backlight monitor brightness display screen",
            &q,
        );
        if bright_score > 0 {
            self.search_results.push(SearchResult {
                title: "Screen Brightness".to_string(),
                category: "Display".to_string(),
                description: "Adjust screen backlight brightness level".to_string(),
                target_item_idx: 0,
                score: bright_score,
            });
        }

        let monitor_idx_offset = if self.display_brightness.is_some() {
            1
        } else {
            0
        };

        for (i, mon) in self.monitors.iter().enumerate() {
            let extra = format!("refresh rate resolution {}x{} hz", mon.width, mon.height);
            let score = calc_score(
                &mon.name,
                "Display",
                "Display Resolution & Refresh Rate",
                &extra,
                &q,
            );
            if score > 0 {
                self.search_results.push(SearchResult {
                    title: mon.name.clone(),
                    category: "Display".to_string(),
                    description: format!(
                        "Resolution & Refresh Rate ({}x{}@{:.0}Hz)",
                        mon.width, mon.height, mon.refresh_rate
                    ),
                    target_item_idx: i + monitor_idx_offset,
                    score,
                });
            }
        }

        // Search Appearance
        let app_score = calc_score(
            "Dark Mode / Theming",
            "Appearance",
            "Change system visual style, GTK theme, color scheme",
            "dark light theme mode",
            &q,
        );
        if app_score > 0 {
            self.search_results.push(SearchResult {
                title: "Dark Mode".to_string(),
                category: "Appearance".to_string(),
                description: "System visual style and color scheme".to_string(),
                target_item_idx: 0,
                score: app_score,
            });
        }

        let gtk_score = calc_score(
            "GTK Theme",
            "Appearance",
            "Change desktop GTK3/GTK4 application theme",
            "gtk theme style appearance desktop",
            &q,
        );
        if gtk_score > 0 {
            self.search_results.push(SearchResult {
                title: "GTK Theme".to_string(),
                category: "Appearance".to_string(),
                description: "Select installed GTK application theme".to_string(),
                target_item_idx: 1,
                score: gtk_score,
            });
        }

        let icon_score = calc_score(
            "Icon Theme",
            "Appearance",
            "Change system icon theme and folder icons",
            "icon icons theme appearance style",
            &q,
        );
        if icon_score > 0 {
            self.search_results.push(SearchResult {
                title: "Icon Theme".to_string(),
                category: "Appearance".to_string(),
                description: "Select installed icon set".to_string(),
                target_item_idx: 2,
                score: icon_score,
            });
        }

        // Search Power
        let power_score = calc_score(
            "Power Profile & Battery",
            "Power",
            "Energy management, performance/power-saver profile, battery level",
            "battery power charge profile",
            &q,
        );
        if power_score > 0 {
            self.search_results.push(SearchResult {
                title: "Power Profile & Battery".to_string(),
                category: "Power".to_string(),
                description: "Energy management and battery status".to_string(),
                target_item_idx: 0,
                score: power_score,
            });
        }

        // Search System Settings & Actions
        let sys_actions = [
            (
                "Network Time (NTP)",
                "Synchronize system clock with network time servers",
                "ntp time clock timedate timezone",
                0,
            ),
            (
                "Suspend",
                "Suspend system to RAM (sleep mode)",
                "sleep standby",
                1,
            ),
            (
                "Hibernate",
                "Hibernate system state to disk",
                "hibernate disk",
                2,
            ),
            ("Reboot", "Restart the computer", "restart reboot", 3),
            (
                "Power Off",
                "Shut down the computer system",
                "shutdown poweroff halt power off",
                4,
            ),
        ];

        for (action, desc, extra, idx) in sys_actions {
            let score = calc_score(action, "System", desc, extra, &q);
            if score > 0 {
                self.search_results.push(SearchResult {
                    title: action.to_string(),
                    category: "System".to_string(),
                    description: desc.to_string(),
                    target_item_idx: idx,
                    score,
                });
            }
        }

        // Search Applications
        for (i, app) in self.applications.iter().enumerate() {
            let score = calc_score(&app.name, "Applications", &app.description, &app.exec, &q);
            if score > 0 {
                self.search_results.push(SearchResult {
                    title: app.name.clone(),
                    category: "Applications".to_string(),
                    description: if app.description.is_empty() {
                        "Installed Application".to_string()
                    } else {
                        app.description.clone()
                    },
                    target_item_idx: i,
                    score,
                });
            }
        }

        // Search Services
        for (i, svc) in self.services.iter().enumerate() {
            let score = calc_score(
                &svc.name,
                "Services",
                "Systemd Service",
                &svc.description,
                &q,
            );
            if score > 0 {
                self.search_results.push(SearchResult {
                    title: svc.name.clone(),
                    category: "Services".to_string(),
                    description: if svc.description.is_empty() {
                        "Systemd Service".to_string()
                    } else {
                        svc.description.clone()
                    },
                    target_item_idx: i,
                    score,
                });
            }
        }

        // Search Mouse & Touchpad
        if self.input_settings.is_some() {
            let score_nat = calc_score(
                "Natural Scrolling",
                "Mouse & Touchpad",
                "Reverse scroll direction",
                "touchpad natural scrolling",
                &q,
            );
            if score_nat > 0 {
                self.search_results.push(SearchResult {
                    title: "Natural Scrolling".to_string(),
                    category: "Mouse & Touchpad".to_string(),
                    description: "Reverse scroll direction".to_string(),
                    target_item_idx: 0,
                    score: score_nat,
                });
            }

            let score_tap = calc_score(
                "Tap to Click",
                "Mouse & Touchpad",
                "Tap touchpad for primary click",
                "touchpad tap to click",
                &q,
            );
            if score_tap > 0 {
                self.search_results.push(SearchResult {
                    title: "Tap to Click".to_string(),
                    category: "Mouse & Touchpad".to_string(),
                    description: "Tap touchpad for primary click".to_string(),
                    target_item_idx: 1,
                    score: score_tap,
                });
            }

            let score_left = calc_score(
                "Left-Handed Mouse",
                "Mouse & Touchpad",
                "Swap left and right mouse buttons",
                "mouse left handed",
                &q,
            );
            if score_left > 0 {
                self.search_results.push(SearchResult {
                    title: "Left-Handed Mode".to_string(),
                    category: "Mouse & Touchpad".to_string(),
                    description: "Swap mouse buttons".to_string(),
                    target_item_idx: 2,
                    score: score_left,
                });
            }

            let score_speed = calc_score(
                "Pointer Speed",
                "Mouse & Touchpad",
                "Pointer acceleration and sensitivity",
                "mouse touchpad sensitivity speed",
                &q,
            );
            if score_speed > 0 {
                self.search_results.push(SearchResult {
                    title: "Pointer Speed".to_string(),
                    category: "Mouse & Touchpad".to_string(),
                    description: "Pointer sensitivity".to_string(),
                    target_item_idx: 3,
                    score: score_speed,
                });
            }
        }

        // Sort by score descending
        self.search_results
            .sort_by_key(|a| std::cmp::Reverse(a.score));
    }

    pub fn on_tick(&mut self) {
        if self.notification_timer > 0 {
            self.notification_timer -= 1;
            if self.notification_timer == 0 {
                self.notifications.clear();
            }
        }
    }

    pub fn clamp_selection(&mut self) {
        let vis = self.visible_categories();
        if let Some(cat) = vis.get(self.selected_category) {
            let max_items = match cat.as_str() {
                "Network" => {
                    if self.wifi_enabled {
                        self.networks.len() + 1 + self.vpns.len()
                    } else {
                        1 + self.vpns.len()
                    }
                }
                "Bluetooth" => {
                    if self.bluetooth_powered {
                        self.bluetooth_devices.len() + 1
                    } else {
                        1
                    }
                }
                "Sound" => {
                    self.audio_sinks.len() + self.audio_sources.len() + self.audio_streams.len()
                }
                "Services" => self.services.len(),
                "Display" => self.monitors.len(),
                "Applications" => self.applications.len(),
                "Mouse & Touchpad" => 4,
                "Appearance" => 3,
                "System" => 4,
                _ => 0,
            };
            if max_items == 0 {
                self.selected_item = 0;
            } else if self.selected_item >= max_items {
                self.selected_item = max_items.saturating_sub(1);
            }
        }
    }

    pub fn handle_key(&mut self, key: event::KeyEvent) {
        if let Some(modal) = &mut self.password_modal {
            match key.code {
                KeyCode::Esc => {
                    self.password_modal = None;
                }
                KeyCode::Enter => {
                    if let Some(modal) = self.password_modal.take() {
                        if let Some(tx) = &self.cmd_tx {
                            let _ = tx.try_send(BackendCommand::ConnectNetworkWithPassword(
                                modal.network_id,
                                modal.password,
                            ));
                        }
                    }
                }
                KeyCode::Tab => {
                    modal.show_password = !modal.show_password;
                }
                KeyCode::Backspace => {
                    modal.password.pop();
                }
                KeyCode::Char(c) => {
                    modal.password.push(c);
                }
                _ => {}
            }
            return;
        }

        
        if self.wallpaper_modal.is_some() {
            match key.code {
                KeyCode::Esc => {
                    self.wallpaper_modal = None;
                }
                KeyCode::Enter => {
                    if let Some(wp) = self.wallpaper_modal.take() {
                        if !wp.is_empty() {
                            if let Some(tx) = &self.cmd_tx {
                                let _ = tx.try_send(BackendCommand::SetWallpaper(wp));
                            }
                        }
                    }
                }
                KeyCode::Char(c) => {
                    if let Some(wp) = &mut self.wallpaper_modal {
                        wp.push(c);
                    }
                }
                KeyCode::Backspace => {
                    if let Some(wp) = &mut self.wallpaper_modal {
                        wp.pop();
                    }
                }
                _ => {}
            }
            return;
        }
        if self.font_modal.is_some() {
            match key.code {
                KeyCode::Esc => {
                    self.font_modal = None;
                }
                KeyCode::Enter => {
                    if let Some(font) = self.font_modal.take() {
                        if !font.is_empty() {
                            if let Some(tx) = &self.cmd_tx {
                                let _ = tx.try_send(BackendCommand::SetFontName(font));
                            }
                        }
                    }
                }
                KeyCode::Char(c) => {
                    if let Some(font) = &mut self.font_modal {
                        font.push(c);
                    }
                }
                KeyCode::Backspace => {
                    if let Some(font) = &mut self.font_modal {
                        font.pop();
                    }
                }
                _ => {}
            }
            return;
        }
        if self.hostname_modal.is_some() {
            match key.code {
                KeyCode::Esc => {
                    self.hostname_modal = None;
                }
                KeyCode::Enter => {
                    if let Some(name) = self.hostname_modal.take() {
                        if !name.is_empty() {
                            if let Some(tx) = &self.cmd_tx {
                                let _ = tx.try_send(BackendCommand::SetHostname(name));
                            }
                        }
                    }
                }
                KeyCode::Backspace => {
                    if let Some(buf) = &mut self.hostname_modal {
                        buf.pop();
                    }
                }
                KeyCode::Char(c) => {
                    if let Some(buf) = &mut self.hostname_modal {
                        if buf.len() < 64 {
                            buf.push(c);
                        }
                    }
                }
                _ => {}
            }
            return;
        }

        if let Some((_, ref cmd)) = self.confirm_action {
            match key.code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    let cmd_to_send = cmd.clone();
                    if let Some(tx) = &self.cmd_tx {
                        let _ = tx.try_send(cmd_to_send);
                    }
                    self.confirm_action = None;
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.confirm_action = None;
                }
                _ => {}
            }
            return;
        }

        if self.is_searching {
            match key.code {
                KeyCode::Esc => {
                    self.is_searching = false;
                }
                KeyCode::Enter => {
                    if !self.search_results.is_empty()
                        && self.search_selected_idx < self.search_results.len()
                    {
                        let res = self.search_results[self.search_selected_idx].clone();
                        self.is_searching = false;
                        self.search_query.clear();

                        // Find category index
                        if let Some(cat_idx) =
                            self.categories.iter().position(|c| c == &res.category)
                        {
                            self.selected_category = cat_idx;
                            self.selected_item = res.target_item_idx;
                            self.focus = Focus::Content;
                        }
                    }
                }
                KeyCode::Down => {
                    if self.search_selected_idx < self.search_results.len().saturating_sub(1) {
                        self.search_selected_idx += 1;
                    }
                }
                KeyCode::Up => {
                    if self.search_selected_idx > 0 {
                        self.search_selected_idx -= 1;
                    }
                }
                KeyCode::Backspace => {
                    self.search_query.pop();
                    self.update_search_results();
                }
                KeyCode::Char(c) => {
                    self.search_query.push(c);
                    self.update_search_results();
                }
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('/') => self.is_searching = true,
            KeyCode::Right | KeyCode::Char('l') => {
                if self.focus == Focus::Sidebar {
                    self.focus = Focus::Content;
                    self.selected_item = 0;
                } else if self.focus == Focus::Content {
                    let vis = self.visible_categories();
                    if let Some(cat) = vis.get(self.selected_category) {
                        if cat == "Display"
                            && self.display_brightness.is_some()
                            && self.selected_item == 0
                        {
                            if let Some(cur) = self.display_brightness {
                                let new_val = cur.saturating_add(5).min(100);
                                if let Some(tx) = &self.cmd_tx {
                                    let _ =
                                        tx.try_send(BackendCommand::SetDisplayBrightness(new_val));
                                }
                            }
                        } else if cat == "Mouse & Touchpad"
                            && self.input_settings.is_some()
                            && self.selected_item == 3
                        {
                            if let Some(cur) = &self.input_settings {
                                let new_val = (cur.sensitivity + 0.05).min(1.0);
                                if let Some(tx) = &self.cmd_tx {
                                    let _ =
                                        tx.try_send(BackendCommand::SetPointerSensitivity(new_val));
                                }
                            }
                        
                        } else if cat == "Power" {
                            let mut row_idx = 1;
                            let has_limit = self.power_info.as_ref().and_then(|info| info.charge_limit).is_some();
                            if has_limit { row_idx += 1; }
                            let btn_idx = row_idx;
                            row_idx += 1;
                            let lid_idx = row_idx;
                            row_idx += 1;
                            let idle_idx = row_idx;
                            if self.selected_item == btn_idx {
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::CyclePowerButtonAction(true));
                                }
                            } else if self.selected_item == lid_idx {
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::CycleLidAction(true));
                                }
                            } else if self.selected_item == idle_idx {
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::CycleIdleDelay(true));
                                }
                            }
                        } else if cat == "Appearance" {
                            if self.selected_item == 1 {
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::CycleGtkTheme(true));
                                }
                            } else if self.selected_item == 3 {
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::CycleCursorTheme(true));
                                }
                            } else if self.selected_item == 4 {
                                let current = self.appearance_info.as_ref().map(|s| s.font_name.clone()).unwrap_or_default();
                                self.font_modal = Some(current);
                            } else if self.selected_item == 5 {
                                let current = self.appearance_info.as_ref().and_then(|s| s.wallpaper.clone()).unwrap_or_default().trim_start_matches("file://").to_string();
                                self.wallpaper_modal = Some(current);
                            } else if self.selected_item == 2 {
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::CycleIconTheme(true));
                                }
                            }
                        }
                    }
                }
            }
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Esc => {
                if self.focus == Focus::Content {
                    let vis = self.visible_categories();
                    if let Some(cat) = vis.get(self.selected_category) {
                        if (key.code == KeyCode::Left || key.code == KeyCode::Char('h'))
                            && cat == "Display"
                            && self.display_brightness.is_some()
                            && self.selected_item == 0
                        {
                            if let Some(cur) = self.display_brightness {
                                let new_val = cur.saturating_sub(5);
                                if let Some(tx) = &self.cmd_tx {
                                    let _ =
                                        tx.try_send(BackendCommand::SetDisplayBrightness(new_val));
                                }
                            }
                            return;
                        } else if (key.code == KeyCode::Left || key.code == KeyCode::Char('h'))
                            && cat == "Mouse & Touchpad"
                            && self.input_settings.is_some()
                            && self.selected_item == 3
                        {
                            if let Some(cur) = &self.input_settings {
                                let new_val = (cur.sensitivity - 0.05).max(-1.0);
                                if let Some(tx) = &self.cmd_tx {
                                    let _ =
                                        tx.try_send(BackendCommand::SetPointerSensitivity(new_val));
                                }
                            }
                            return;
                        
                        } else if (key.code == KeyCode::Left || key.code == KeyCode::Char('h')) && cat == "Power" {
                            let mut row_idx = 1;
                            let has_limit = self.power_info.as_ref().and_then(|info| info.charge_limit).is_some();
                            if has_limit { row_idx += 1; }
                            let btn_idx = row_idx;
                            row_idx += 1;
                            let lid_idx = row_idx;
                            row_idx += 1;
                            let idle_idx = row_idx;
                            if self.selected_item == btn_idx {
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::CyclePowerButtonAction(false));
                                }
                                return;
                            } else if self.selected_item == lid_idx {
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::CycleLidAction(false));
                                }
                                return;
                            } else if self.selected_item == idle_idx {
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::CycleIdleDelay(false));
                                }
                                return;
                            }
                        } else if (key.code == KeyCode::Left || key.code == KeyCode::Char('h'))
                            && cat == "Appearance"
                        {
                            if self.selected_item == 1 {
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::CycleGtkTheme(false));
                                }
                                return;
                            } else if self.selected_item == 3 {
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::CycleCursorTheme(false));
                                }
                                return;
                            } else if self.selected_item == 4 {
                                let current = self.appearance_info.as_ref().map(|s| s.font_name.clone()).unwrap_or_default();
                                self.font_modal = Some(current);
                            } else if self.selected_item == 5 {
                                let current = self.appearance_info.as_ref().and_then(|s| s.wallpaper.clone()).unwrap_or_default().trim_start_matches("file://").to_string();
                                self.wallpaper_modal = Some(current);
                                return;
                            } else if self.selected_item == 2 {
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::CycleIconTheme(false));
                                }
                                return;
                            }
                        }
                    }
                    self.focus = Focus::Sidebar;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let vis = self.visible_categories();
                if self.focus == Focus::Sidebar {
                    if self.selected_category < vis.len().saturating_sub(1) {
                        self.selected_category += 1;
                    }
                } else if self.focus == Focus::Content {
                    // Very naive max items depending on category
                    if let Some(cat) = vis.get(self.selected_category) {
                        let max_items = match cat.as_str() {
                            "Network" => {
                                if self.wifi_enabled {
                                    self.networks.len() + 1 + self.vpns.len()
                                } else {
                                    1 + self.vpns.len()
                                }
                            }
                            "Bluetooth" => {
                                if self.bluetooth_powered {
                                    self.bluetooth_devices.len() + 1
                                } else {
                                    1
                                }
                            }
                            "Sound" => {
                                self.audio_sinks.len()
                                    + self.audio_sources.len()
                                    + self.audio_streams.len()
                            }
                            "Services" => self.services.len(),
                            "Display" => {
                                let base = if self.display_brightness.is_some() {
                                    1
                                } else {
                                    0
                                };
                                base + self.monitors.len()
                            }
                            "Applications" => self.applications.len(),
                            "Appearance" => 3,
                            "System" => 5,
                            _ => 0,
                        };
                        if max_items > 0 && self.selected_item < max_items - 1 {
                            self.selected_item += 1;
                        }
                    }
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.focus == Focus::Sidebar {
                    if self.selected_category > 0 {
                        self.selected_category -= 1;
                    }
                } else if self.focus == Focus::Content && self.selected_item > 0 {
                    self.selected_item -= 1;
                }
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                // Action on toggle
                let is_enter = key.code == KeyCode::Enter;
                let vis = self.visible_categories();
                if is_enter && self.focus == Focus::Sidebar {
                    self.focus = Focus::Content;
                    self.selected_item = 0;
                } else if self.focus == Focus::Content {
                    if let Some(cat) = vis.get(self.selected_category) {
                        if cat == "Services" && self.selected_item < self.services.len() {
                            let svc = &self.services[self.selected_item];
                            let start = svc.active_state != "active";
                            if let Some(tx) = &self.cmd_tx {
                                let _ = tx.try_send(BackendCommand::ToggleService(
                                    svc.name.clone(),
                                    start,
                                ));
                            }
                        } else if cat == "Network" {
                            if self.selected_item == 0 {
                                if let Some(tx) = &self.cmd_tx {
                                    let _ =
                                        tx.try_send(BackendCommand::ToggleWifi(!self.wifi_enabled));
                                }
                            } else if self.selected_item == 1 {
                                if let Some(enabled) = self.flight_mode_enabled {
                                    if let Some(tx) = &self.cmd_tx {
                                        let _ = tx.try_send(BackendCommand::ToggleFlightMode(!enabled));
                                    }
                                }
                            } else if self.selected_item == 2 {
                                if let Some(enabled) = self.hotspot_enabled {
                                    if let Some(tx) = &self.cmd_tx {
                                        let _ = tx.try_send(BackendCommand::ToggleHotspot(!enabled));
                                    }
                                }
                            } else if self.selected_item >= 3 && self.selected_item - 3 < self.networks.len() {
                                let net = &self.networks[self.selected_item - 3];
                                if net.connected {
                                    if let Some(tx) = &self.cmd_tx {
                                        let _ = tx.try_send(BackendCommand::ConnectNetwork(
                                            net.id.clone(),
                                            false,
                                        ));
                                    }
                                } else if net.saved || net.security == "Open" {
                                    if let Some(tx) = &self.cmd_tx {
                                        let _ = tx.try_send(BackendCommand::ConnectNetwork(
                                            net.id.clone(),
                                            true,
                                        ));
                                    }
                                } else {
                                    self.password_modal = Some(PasswordModal {
                                        network_name: net.name.clone(),
                                        network_id: net.id.clone(),
                                        password: String::new(),
                                        show_password: false,
                                    });
                                }
                            } else {
                                // VPN rows come after wifi rows
                                let vpn_offset = if self.wifi_enabled {
                                    self.networks.len() + 3
                                } else {
                                    3
                                };
                                let vpn_idx = self.selected_item.saturating_sub(vpn_offset);
                                if vpn_idx < self.vpns.len() {
                                    let vpn = &self.vpns[vpn_idx];
                                    let activate = !vpn.active;
                                    if let Some(tx) = &self.cmd_tx {
                                        let _ = tx.try_send(BackendCommand::ToggleVpn(
                                            vpn.uuid.clone(),
                                            activate,
                                        ));
                                    }
                                }
                            }
                        } else if cat == "Bluetooth" {
                            if self.selected_item == 0 {
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::ToggleBluetoothPower(
                                        !self.bluetooth_powered,
                                    ));
                                }
                            } else if self.selected_item <= self.bluetooth_devices.len() {
                                let dev = &self.bluetooth_devices[self.selected_item - 1];
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::ConnectBluetooth(
                                        dev.id.clone(),
                                        !dev.connected,
                                    ));
                                }
                            }
                        } else if cat == "Sound" {
                            let sinks_len = self.audio_sinks.len();
                            let sources_len = self.audio_sources.len();
                            let streams_len = self.audio_streams.len();
                            let is_enter = key.code == KeyCode::Enter;

                            let (id, is_sink) = if self.selected_item < sinks_len {
                                (self.audio_sinks[self.selected_item].id, true)
                            } else if self.selected_item < sinks_len + sources_len {
                                (self.audio_sources[self.selected_item - sinks_len].id, false)
                            } else if self.selected_item < sinks_len + sources_len + streams_len {
                                (
                                    self.audio_streams
                                        [self.selected_item - sinks_len - sources_len]
                                        .id,
                                    false,
                                )
                            } else {
                                (0, false)
                            };

                            if id != 0 {
                                if is_enter {
                                    if self.selected_item < sinks_len + sources_len {
                                        if let Some(tx) = &self.cmd_tx {
                                            let _ = tx.try_send(BackendCommand::SetAudioDefault(
                                                id, is_sink,
                                            ));
                                        }
                                    }
                                } else {
                                    if let Some(tx) = &self.cmd_tx {
                                        let _ = tx
                                            .try_send(BackendCommand::ToggleAudioMute(id, is_sink));
                                    }
                                }
                            } else if is_enter && self.selected_item == sinks_len + sources_len {
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::TestAudio);
                                }
                            }
                        } else if cat == "Display" {
                            let has_brightness = self.display_brightness.is_some();
                            let nl_idx = if has_brightness { 1 } else { 0 };
                            let has_nl = self.night_light_enabled.is_some();
                            if has_nl && self.selected_item == nl_idx {
                                if let Some(enabled) = self.night_light_enabled {
                                    if let Some(tx) = &self.cmd_tx {
                                        let _ =
                                            tx.try_send(BackendCommand::ToggleNightLight(!enabled));
                                    }
                                }
                            }
                        } else if cat == "Appearance" {
                            if self.selected_item == 0 {
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::ToggleColorScheme);
                                }
                            } else if self.selected_item == 1 {
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::CycleGtkTheme(true));
                                }
                            } else if self.selected_item == 3 {
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::CycleCursorTheme(true));
                                }
                            } else if self.selected_item == 4 {
                                let current = self.appearance_info.as_ref().map(|s| s.font_name.clone()).unwrap_or_default();
                                self.font_modal = Some(current);
                            } else if self.selected_item == 5 {
                                let current = self.appearance_info.as_ref().and_then(|s| s.wallpaper.clone()).unwrap_or_default().trim_start_matches("file://").to_string();
                                self.wallpaper_modal = Some(current);
                            } else if self.selected_item == 2 {
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::CycleIconTheme(true));
                                }
                            }
                        
                        } else if cat == "Power" {
                            let mut row_idx = 1;
                            let has_limit = self.power_info.as_ref().and_then(|info| info.charge_limit).is_some();
                            let limit_idx = if has_limit { Some(row_idx) } else { None };
                            if has_limit { row_idx += 1; }
                            let btn_idx = row_idx;
                            row_idx += 1;
                            let lid_idx = row_idx;
                            row_idx += 1;
                            let idle_idx = row_idx;

                            if self.selected_item == 0 {
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::CyclePowerProfile);
                                }
                            } else if Some(self.selected_item) == limit_idx {
                                if let Some(info) = &self.power_info {
                                    if let Some(limit) = info.charge_limit {
                                        let target = if limit < 100 { 100 } else { 80 };
                                        if let Some(tx) = &self.cmd_tx {
                                            let _ = tx.try_send(BackendCommand::SetChargeLimit(target));
                                        }
                                    }
                                }
                            } else if self.selected_item == btn_idx {
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::CyclePowerButtonAction(true));
                                }
                            } else if self.selected_item == lid_idx {
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::CycleLidAction(true));
                                }
                            } else if self.selected_item == idle_idx {
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::CycleIdleDelay(true));
                                }
                            }
} else if cat == "Applications"
                            && self.selected_item < self.applications.len()
                        {
                            let app = &self.applications[self.selected_item];
                            if let Some(tx) = &self.cmd_tx {
                                let _ = tx
                                    .try_send(BackendCommand::LaunchApplication(app.exec.clone()));
                            }
                        } else if cat == "Mouse & Touchpad" {
                            if let Some(input) = &self.input_settings {
                                if let Some(tx) = &self.cmd_tx {
                                    match self.selected_item {
                                        0 => {
                                            let _ =
                                                tx.try_send(BackendCommand::ToggleNaturalScroll(
                                                    !input.natural_scroll,
                                                ));
                                        }
                                        1 => {
                                            let _ = tx.try_send(BackendCommand::ToggleTapToClick(
                                                !input.tap_to_click,
                                            ));
                                        }
                                        2 => {
                                            let _ = tx.try_send(BackendCommand::ToggleLeftHanded(
                                                !input.left_handed,
                                            ));
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        } else if cat == "System" {
                            if self.selected_item == 0 {
                                if let Some(sys_info) = &self.system_info {
                                    let new_state = !sys_info.ntp_active;
                                    if let Some(tx) = &self.cmd_tx {
                                        let _ = tx.try_send(BackendCommand::ToggleNTP(new_state));
                                    }
                                }
                            } else if is_enter {
                                let (action, prompt) = match self.selected_item {
                                    1 => ("suspend", "Suspend the system to RAM now?"),
                                    2 => ("hibernate", "Hibernate session to swap and power off?"),
                                    3 => (
                                        "reboot",
                                        "Restart the computer now? (Unsaved work will be lost)",
                                    ),
                                    4 => (
                                        "poweroff",
                                        "Shut down and power off the computer? (Unsaved work will be lost)",
                                    ),
                                    _ => ("", ""),
                                };
                                if !action.is_empty() {
                                    self.confirm_action = Some((
                                        prompt.to_string(),
                                        BackendCommand::SystemPowerAction(action.to_string()),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            KeyCode::Backspace | KeyCode::Delete => {
                let vis = self.visible_categories();
                if self.focus == Focus::Content {
                    if let Some(cat) = vis.get(self.selected_category) {
                        if cat == "Bluetooth"
                            && self.selected_item > 0
                            && self.selected_item <= self.bluetooth_devices.len()
                        {
                            let dev = &self.bluetooth_devices[self.selected_item - 3];
                            self.confirm_action = Some((
                                format!("Forget Bluetooth device '{}'?", dev.name),
                                BackendCommand::RemoveBluetoothDevice(dev.id.clone()),
                            ));
                        } else if cat == "Network"
                            && self.selected_item >= 3
                            && self.selected_item - 3 < self.networks.len()
                        {
                            let net = &self.networks[self.selected_item - 3];
                            self.confirm_action = Some((
                                format!("Forget Wi-Fi network profile '{}'?", net.name),
                                BackendCommand::ForgetNetwork(net.id.clone()),
                            ));
                        }
                    }
                }
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                let vis = self.visible_categories();
                if self.focus == Focus::Content {
                    if let Some(cat) = vis.get(self.selected_category) {
                        if cat == "Network" {
                            if let Some(tx) = &self.cmd_tx {
                                let _ = tx.try_send(BackendCommand::RescanWifi);
                            }
                        }
                    }
                }
            }
            KeyCode::Char('+') | KeyCode::Char('=') | KeyCode::Char('-') => {
                let increase = key.code != KeyCode::Char('-');
                let vis = self.visible_categories();
                if self.focus == Focus::Content {
                    if let Some(cat) = vis.get(self.selected_category) {
                        if cat == "Sound" {
                            let sinks_len = self.audio_sinks.len();
                            let sources_len = self.audio_sources.len();
                            let streams_len = self.audio_streams.len();

                            let (id, current_vol) = if self.selected_item < sinks_len {
                                let dev = &self.audio_sinks[self.selected_item];
                                (dev.id, dev.volume)
                            } else if self.selected_item < sinks_len + sources_len {
                                let dev = &self.audio_sources[self.selected_item - sinks_len];
                                (dev.id, dev.volume)
                            } else if self.selected_item < sinks_len + sources_len + streams_len {
                                let stream = &self.audio_streams
                                    [self.selected_item - sinks_len - sources_len];
                                (stream.id, stream.volume)
                            } else {
                                (0, 0.0)
                            };

                            if id != 0 {
                                let mut new_vol = if increase {
                                    current_vol + 0.05
                                } else {
                                    current_vol - 0.05
                                };
                                new_vol = new_vol.clamp(0.0, 1.0);
                                if let Some(tx) = &self.cmd_tx {
                                    let _ =
                                        tx.try_send(BackendCommand::SetAudioVolume(id, new_vol));
                                }
                            }
                        } else if cat == "Display" {
                            let has_brightness = self.display_brightness.is_some();
                            let nl_idx = if has_brightness { 1 } else { 0 };
                            let has_nl = self.night_light_enabled.is_some();

                            if has_brightness && self.selected_item == 0 {
                                if let Some(cur) = self.display_brightness {
                                    let new_val = if increase {
                                        cur.saturating_add(5).min(100)
                                    } else {
                                        cur.saturating_sub(5)
                                    };
                                    if let Some(tx) = &self.cmd_tx {
                                        let _ = tx.try_send(BackendCommand::SetDisplayBrightness(
                                            new_val,
                                        ));
                                    }
                                }
                            } else if has_nl && self.selected_item == nl_idx {
                                if let Some(enabled) = self.night_light_enabled {
                                    if let Some(tx) = &self.cmd_tx {
                                        let _ =
                                            tx.try_send(BackendCommand::ToggleNightLight(!enabled));
                                    }
                                }
                            } else {
                                let offset = (if has_brightness { 1 } else { 0 })
                                    + (if has_nl { 1 } else { 0 });
                                let sel_offset = self.selected_item.saturating_sub(offset);
                                let mon_idx = sel_offset / 2;
                                let is_scale = (sel_offset % 2) != 0;
                                if mon_idx < self.monitors.len() {
                                    let m = &self.monitors[mon_idx];
                                    if is_scale {
                                        let new_scale = if increase {
                                            (m.scale + 0.25).min(3.0)
                                        } else {
                                            (m.scale - 0.25).max(0.5)
                                        };
                                        if let Some(tx) = &self.cmd_tx {
                                            let _ = tx.try_send(BackendCommand::SetDisplayScale(m.name.clone(), new_scale));
                                        }
                                    } else if !m.supported_modes.is_empty() {
                                        let _current_res = format!(
                                            "{}x{}@{:.2}Hz",
                                            m.width, m.height, m.refresh_rate
                                        );
                                        let current_idx = m
                                            .supported_modes
                                            .iter()
                                            .position(|s| {
                                                s.starts_with(&format!("{}x{}", m.width, m.height))
                                            })
                                            .unwrap_or(0);
                                        let new_idx = if increase {
                                            (current_idx + 1) % m.supported_modes.len()
                                        } else if current_idx == 0 {
                                            m.supported_modes.len() - 1
                                        } else {
                                            current_idx - 1
                                        };
                                        let next_mode = &m.supported_modes[new_idx];
                                        if let Some(caps) = next_mode.split('@').next() {
                                            let parts: Vec<&str> = caps.split('x').collect();
                                            if parts.len() == 2 {
                                                if let (Ok(w), Ok(h)) = (
                                                    parts[0].parse::<i32>(),
                                                    parts[1].parse::<i32>(),
                                                ) {
                                                    let refresh = if let Some(hz) =
                                                        next_mode.split('@').nth(1)
                                                    {
                                                        hz.trim_end_matches("Hz")
                                                            .parse::<f64>()
                                                            .unwrap_or(60.0)
                                                    } else {
                                                        60.0
                                                    };

                                                    if let Some(tx) = &self.cmd_tx {
                                                        let _ = tx.try_send(
                                                            BackendCommand::SetDisplayResolution(
                                                                m.name.clone(),
                                                                w,
                                                                h,
                                                                refresh,
                                                            ),
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            KeyCode::Char('e') | KeyCode::Char('E') | KeyCode::F(2) => {
                let vis = self.visible_categories();
                if self.focus == Focus::Content {
                    if let Some(cat) = vis.get(self.selected_category) {
                        if cat == "System" {
                            // Open hostname rename modal pre-filled with current hostname
                            let current = self
                                .system_info
                                .as_ref()
                                .map(|s| s.hostname.clone())
                                .unwrap_or_default();
                            self.hostname_modal = Some(current);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

pub async fn run(args: Args) -> Result<(), Box<dyn Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut initial_app = App::new();

    // Apply configuration if specified or found
    let cfg = crate::config::SettingsConfig::load(args.config.as_deref().map(std::path::Path::new));
    if let Some(def_cat) = cfg.default_category {
        if let Some(pos) = initial_app
            .categories
            .iter()
            .position(|c| c.eq_ignore_ascii_case(&def_cat))
        {
            initial_app.selected_category = pos;
        }
    }

    // Apply CLI section argument
    if let Some(sec) = &args.section {
        if let Some(pos) = initial_app
            .categories
            .iter()
            .position(|c| c.eq_ignore_ascii_case(sec))
        {
            initial_app.selected_category = pos;
            initial_app.focus = Focus::Content;
        }
    }

    // Apply CLI search argument
    if let Some(q) = &args.search {
        initial_app.is_searching = true;
        initial_app.search_query = q.clone();
        initial_app.update_search_results();
    }

    let app = Arc::new(Mutex::new(initial_app));

    let (tx, mut rx) = mpsc::channel(100);

    // Spawn external state listeners
    listeners::spawn_network_listener(tx.clone()).await;
    listeners::spawn_power_listener(tx.clone()).await;
    listeners::spawn_audio_listener(tx.clone()).await;
    listeners::spawn_bluetooth_listener(tx.clone()).await;

    // Event loop task
    let tx_tick = tx.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(250)).await;
            if tx_tick.send(AppEvent::Tick).await.is_err() {
                break;
            }
        }
    });

    let tx_input = tx.clone();
    tokio::spawn(async move {
        loop {
            if event::poll(Duration::from_millis(50)).unwrap_or(false) {
                if let Ok(CrosstermEvent::Key(key)) = event::read() {
                    if tx_input.send(AppEvent::Input(key)).await.is_err() {
                        break;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });

    // Backend fetching task
    let tx_backend = tx.clone();
    let sys_backend = RealSystemBackend::new().await;

    let net_backend = crate::backends::network::NetworkManagerBackend::new()
        .await
        .ok();
    let bt_backend = crate::backends::bluetooth::BlueZBackend::new().await.ok();
    let power_backend = crate::backends::power::UPowerBackend::new().await.ok();
    let audio_backend = crate::backends::audio::WpctlBackend::new();
    let services_backend = crate::backends::systemd::SystemdBackend::new().await.ok();

    let display_backend: Option<Box<dyn DisplayBackend>> = {
        let is_hyprland = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok();
        let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
        if is_hyprland && session_type == "wayland" {
            crate::backends::display::HyprlandBackend::new()
                .ok()
                .map(|b| Box::new(b) as Box<dyn DisplayBackend>)
        } else {
            Some(
                Box::new(crate::backends::display::GenericDisplayBackend::new())
                    as Box<dyn DisplayBackend>,
            )
        }
    };

    let appearance_backend = crate::backends::appearance::GsettingsBackend::new();
    let apps_backend = crate::backends::applications::DesktopEntryBackend::new();
    let input_backend = crate::backends::input::create_input_backend();

    let (cmd_tx, mut cmd_rx) = mpsc::channel::<BackendCommand>(10);
    app.lock().await.cmd_tx = Some(cmd_tx);

    let svc_backend_for_cmd = crate::backends::systemd::SystemdBackend::new().await.ok();
    let bt_backend_for_cmd = crate::backends::bluetooth::BlueZBackend::new().await.ok();
    let audio_backend_for_cmd = crate::backends::audio::WpctlBackend::new();
    let appearance_backend_for_cmd = crate::backends::appearance::GsettingsBackend::new();
    let power_backend_for_cmd = crate::backends::power::UPowerBackend::new().await.ok();
    let network_backend_for_cmd = crate::backends::network::NetworkManagerBackend::new()
        .await
        .ok();
    let sys_backend_for_cmd = crate::backends::system::RealSystemBackend::new().await;
    let input_backend_for_cmd = crate::backends::input::create_input_backend();
    let display_backend_for_cmd: Option<Box<dyn DisplayBackend>> = {
        let is_hyprland = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok();
        let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
        if is_hyprland && session_type == "wayland" {
            crate::backends::display::HyprlandBackend::new()
                .ok()
                .map(|b| Box::new(b) as Box<dyn DisplayBackend>)
        } else {
            Some(
                Box::new(crate::backends::display::GenericDisplayBackend::new())
                    as Box<dyn DisplayBackend>,
            )
        }
    };
    let tx_cmd_resp = tx.clone();

    let capabilities = crate::platform::PlatformCapabilities::detect(
        net_backend.is_some(),
        bt_backend.is_some(),
        power_backend.is_some(),
        true,
        display_backend.is_some(),
        appearance_backend.is_some(),
        services_backend.is_some(),
        input_backend.is_some(),
    );
    let _ = tx_backend
        .send(AppEvent::UpdateCapabilities(Box::new(capabilities)))
        .await;

    tokio::spawn(async move {
        let mut last_poll = tokio::time::Instant::now()
            .checked_sub(Duration::from_secs(10))
            .unwrap_or_else(tokio::time::Instant::now);
        loop {
            tokio::select! {
                Some(cmd) = cmd_rx.recv() => {
                    match cmd {
                        BackendCommand::ToggleService(name, start) => {
                            if !crate::security::validate_service_name(&name) {
                                let _ = tx_cmd_resp
                                    .send(AppEvent::Notification(format!(
                                        "Invalid service unit identifier: '{}'",
                                        name
                                    )))
                                    .await;
                                continue;
                            }
                            if let Some(svc) = &svc_backend_for_cmd {
                                let mutate = if start { svc.start_service(&name) } else { svc.stop_service(&name) };

                                let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                    mutate,
                                    svc.get_services(),
                                    |svcs: &Vec<ServiceInfo>| {
                                        if let Some(s) = svcs.iter().find(|s| s.name == name) {
                                            if start { s.active_state == "active" || s.active_state == "activating" }
                                            else { s.active_state == "inactive" || s.active_state == "deactivating" }
                                        } else { false }
                                    },
                                    20, 100 // up to 2 seconds for service
                                );

                                if mutation_ok {
                                    if verified {
                                        let _ = tx_cmd_resp.send(AppEvent::Notification(format!("Service {} {}", name, if start { "started" } else { "stopped" }))).await;
                                    } else {
                                        let _ = tx_cmd_resp.send(AppEvent::Notification(format!("Service {} change could not be verified.", name))).await;
                                    }
                                } else {
                                    let err_msg = mutation_error.unwrap_or_else(|| format!("Failed to {} service {}.", if start { "start" } else { "stop" }, name));
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                }

                                if let Ok(svcs) = final_state {
                                    let _ = tx_cmd_resp.send(AppEvent::UpdateServices(svcs)).await;
                                }
                            }
                        }
                        BackendCommand::ToggleBluetoothPower(target_state) => {
                            if let Some(bt) = &bt_backend_for_cmd {
                                let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                    bt.set_powered(target_state),
                                    bt.get_adapter(),
                                    |adapter: &Option<crate::backends::BluetoothAdapter>| {
                                        if let Some(a) = adapter {
                                            a.powered == target_state
                                        } else {
                                            false
                                        }
                                    },
                                    10, 100 // up to 1 second
                                );

                                if mutation_ok {
                                    if verified {
                                        let _ = tx_cmd_resp.send(AppEvent::Notification(format!("Bluetooth {}", if target_state { "enabled" } else { "disabled" }))).await;
                                    } else {
                                        let _ = tx_cmd_resp.send(AppEvent::Notification("Bluetooth change could not be verified.".to_string())).await;
                                    }
                                } else {
                                    let err_msg = mutation_error.unwrap_or_else(|| "Failed to toggle Bluetooth.".to_string());
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                }

                                if let Ok(adapter) = final_state {
                                    let _ = tx_cmd_resp.send(AppEvent::UpdateBluetoothAdapter(adapter)).await;
                                }
                            }
                        }
                        BackendCommand::ConnectBluetooth(id, connect) => {
                            if let Some(bt) = &bt_backend_for_cmd {
                                let mutate = if connect { bt.connect_device(&id) } else { bt.disconnect_device(&id) };
                                let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                    mutate,
                                    bt.devices(),
                                    |devices: &Vec<crate::backends::BluetoothDevice>| {
                                        if let Some(d) = devices.iter().find(|d| d.id == id) {
                                            d.connected == connect
                                        } else {
                                            false
                                        }
                                    },
                                    20, 200 // Bluetooth connection can take a few seconds
                                );

                                if mutation_ok {
                                    if verified {
                                        let _ = tx_cmd_resp.send(AppEvent::Notification(format!("Bluetooth device {}", if connect { "connected" } else { "disconnected" }))).await;
                                    } else {
                                        let _ = tx_cmd_resp.send(AppEvent::Notification("Bluetooth change could not be verified.".to_string())).await;
                                    }
                                } else {
                                    let err_msg = mutation_error.unwrap_or_else(|| format!("Failed to {} Bluetooth device.", if connect { "connect" } else { "disconnect" }));
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                }

                                if let Ok(devices) = final_state {
                                    let _ = tx_cmd_resp.send(AppEvent::UpdateBluetooth(devices)).await;
                                }
                            }
                        }
                        BackendCommand::ToggleAudioMute(id, _is_sink) => {
                            let initial_muted = audio_backend_for_cmd
                                .get_volume(id)
                                .await
                                .map(|(_, m)| m)
                                .unwrap_or(false);
                            let target_muted = !initial_muted;

                            let (mutation_ok, verified, _final_state, mutation_error) = execute_transaction!(
                                audio_backend_for_cmd.toggle_mute(id),
                                audio_backend_for_cmd.get_volume(id),
                                |(_, muted): &(f64, bool)| *muted == target_muted,
                                10, 50
                            );
                            if mutation_ok {
                                if verified {
                                    let _ = tx_cmd_resp
                                        .send(AppEvent::Notification(format!(
                                            "Audio device {} {}",
                                            id,
                                            if target_muted { "muted" } else { "unmuted" }
                                        )))
                                        .await;
                                } else {
                                    let _ = tx_cmd_resp
                                        .send(AppEvent::Notification(
                                            "Audio mute state change could not be verified."
                                                .to_string(),
                                        ))
                                        .await;
                                }
                            } else {
                                let err_msg = mutation_error.unwrap_or_else(|| format!("Failed to toggle mute for audio device {}", id));
                                let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                            }
                            if let Ok(sinks) = audio_backend_for_cmd.get_sinks().await {
                                if let Ok(sources) = audio_backend_for_cmd.get_sources().await {
                                    if let Ok(streams) = audio_backend_for_cmd.get_streams().await {
                                        let _ = tx_cmd_resp
                                            .send(AppEvent::UpdateAudio(sinks, sources, streams))
                                            .await;
                                    }
                                }
                            }
                        }
                        BackendCommand::SetAudioVolume(id, vol) => {
                            let (mutation_ok, verified, _final_state, mutation_error) = execute_transaction!(
                                audio_backend_for_cmd.set_volume(id, vol),
                                audio_backend_for_cmd.get_volume(id),
                                |(cur_vol, _): &(f64, bool)| (*cur_vol - vol).abs() < 0.03,
                                10, 50
                            );
                            if !mutation_ok {
                                let err_msg = mutation_error.unwrap_or_else(|| format!("Failed to set volume for device {}", id));
                                let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                            } else if !verified {
                                let _ = tx_cmd_resp
                                    .send(AppEvent::Notification(
                                        "Volume change could not be verified within threshold."
                                            .to_string(),
                                    ))
                                    .await;
                            }
                            if let Ok(sinks) = audio_backend_for_cmd.get_sinks().await {
                                if let Ok(sources) = audio_backend_for_cmd.get_sources().await {
                                    if let Ok(streams) = audio_backend_for_cmd.get_streams().await {
                                        let _ = tx_cmd_resp
                                            .send(AppEvent::UpdateAudio(sinks, sources, streams))
                                            .await;
                                    }
                                }
                            }
                        }
                        BackendCommand::SetDisplayScale(name, scale) => {
                            if let Some(disp) = &display_backend_for_cmd {
                                let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                    disp.set_scale(&name, scale),
                                    disp.get_monitors(),
                                    |monitors: &Vec<crate::backends::Monitor>| {
                                        if let Some(m) = monitors.iter().find(|m| m.name == name) {
                                            (m.scale - scale).abs() < 0.01
                                        } else {
                                            false
                                        }
                                    },
                                    10, 100
                                );
                                if mutation_ok {
                                    if !verified {
                                        let _ = tx_cmd_resp.send(AppEvent::Notification("Scale change could not be verified.".to_string())).await;
                                    }
                                } else {
                                    let err_msg = mutation_error.unwrap_or_else(|| "Failed to set display scale.".to_string());
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                }
                                if let Ok(actual_mons) = final_state {
                                    let _ = tx_cmd_resp.send(AppEvent::UpdateMonitors(actual_mons)).await;
                                }
                            }
                        }
                        BackendCommand::SetDisplayResolution(name, width, height, refresh) => {
                            if let Some(disp) = &display_backend_for_cmd {
                                let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                    disp.set_resolution(&name, width, height, refresh),
                                    disp.get_monitors(),
                                    |monitors: &Vec<crate::backends::Monitor>| {
                                        if let Some(m) = monitors.iter().find(|m| m.name == name) {
                                            m.width == width && m.height == height
                                        } else {
                                            false
                                        }
                                    },
                                    20, 100 // Displays can take time to apply changes
                                );

                                if mutation_ok {
                                    if verified {
                                        let _ = tx_cmd_resp.send(AppEvent::Notification(format!("Display resolution set to {}x{}", width, height))).await;
                                    } else {
                                        let _ = tx_cmd_resp.send(AppEvent::Notification("Display change could not be verified.".to_string())).await;
                                    }
                                } else {
                                    let err_msg = mutation_error.unwrap_or_else(|| "Failed to set display resolution.".to_string());
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                }

                                if let Ok(monitors) = final_state {
                                    let _ = tx_cmd_resp.send(AppEvent::UpdateMonitors(monitors)).await;
                                }
                            }
                        }
                        BackendCommand::ToggleColorScheme => {
                            if let Some(appr) = &appearance_backend_for_cmd {
                                if let Ok(info) = appr.get_info().await {
                                    let new_scheme = if info.color_scheme.contains("dark") {
                                        "prefer-light"
                                    } else {
                                        "prefer-dark"
                                    };
                                    let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                        appr.set_color_scheme(new_scheme),
                                        appr.get_info(),
                                        |info: &AppearanceInfo| info.color_scheme == new_scheme,
                                        10, 50 // up to 500ms
                                    );

                                    if mutation_ok {
                                        if verified {
                                            let _ = tx_cmd_resp.send(AppEvent::Notification(format!("Color scheme set to {}", new_scheme))).await;
                                        } else {
                                            let _ = tx_cmd_resp.send(AppEvent::Notification("Change could not be verified.".to_string())).await;
                                        }
                                    } else {
                                        let err_msg = mutation_error.unwrap_or_else(|| "Failed to set color scheme.".to_string());
                                        let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                    }

                                    if let Ok(actual_info) = final_state {
                                        let _ = tx_cmd_resp.send(AppEvent::UpdateAppearance(actual_info)).await;
                                    }
                                }
                            }
                        }
                        BackendCommand::SetChargeLimit(limit) => {
                            if let Some(pow) = &power_backend_for_cmd {
                                let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                    pow.set_charge_limit(limit),
                                    pow.get_info(),
                                    |info: &crate::backends::PowerInfo| info.charge_limit == Some(limit),
                                    10, 50
                                );
                                if mutation_ok {
                                    if verified {
                                        let _ = tx_cmd_resp.send(AppEvent::Notification(format!("Battery charge limit set to {}%", limit))).await;
                                    } else {
                                        let _ = tx_cmd_resp.send(AppEvent::Notification("Change could not be verified (might require reboot or replug).".to_string())).await;
                                    }
                                } else {
                                    let err_msg = mutation_error.unwrap_or_else(|| "Failed to set charge limit.".to_string());
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                }
                                if let Ok(actual_info) = final_state {
                                    let _ = tx_cmd_resp.send(AppEvent::UpdatePower(actual_info)).await;
                                }
                            }
                        }
                        
                        BackendCommand::CyclePowerButtonAction(forward) => {
                            if let Some(pwr) = &power_backend_for_cmd {
                                if let Ok(info) = pwr.get_info().await {
                                    let actions = vec!["ignore", "poweroff", "suspend", "hibernate", "interactive"];
                                    let current_idx = actions.iter().position(|a| a == &info.power_button_action).unwrap_or(1);
                                    let next_idx = if forward {
                                        (current_idx + 1) % actions.len()
                                    } else {
                                        if current_idx == 0 { actions.len() - 1 } else { current_idx - 1 }
                                    };
                                    let next_action = actions[next_idx];
                                    let _ = execute_transaction!(
                                        pwr.set_power_button_action(next_action),
                                        pwr.get_info(),
                                        |new_info: &crate::backends::PowerInfo| new_info.power_button_action == next_action,
                                        5, 100
                                    );
                                    if let Ok(new_info) = pwr.get_info().await {
                                        let _ = tx_cmd_resp.send(AppEvent::UpdatePower(new_info)).await;
                                    }
                                }
                            }
                        }
                                                BackendCommand::CycleIdleDelay(forward) => {
                            if let Some(pwr) = &power_backend_for_cmd {
                                if let Ok(info) = pwr.get_info().await {
                                    let delays: Vec<u32> = vec![0, 60, 120, 300, 600, 900, 1800, 3600];
                                    let current = info.idle_delay.unwrap_or(300);
                                    let current_idx = delays.iter().position(|&a| a == current).unwrap_or(3);
                                    let next_idx = if forward {
                                        (current_idx + 1) % delays.len()
                                    } else {
                                        if current_idx == 0 { delays.len() - 1 } else { current_idx - 1 }
                                    };
                                    let next_delay = delays[next_idx];
                                    let _ = execute_transaction!(
                                        pwr.set_idle_delay(next_delay),
                                        pwr.get_info(),
                                        |new_info: &crate::backends::PowerInfo| new_info.idle_delay == Some(next_delay),
                                        5, 100
                                    );
                                    if let Ok(new_info) = pwr.get_info().await {
                                        let _ = tx_cmd_resp.send(AppEvent::UpdatePower(new_info)).await;
                                    }
                                }
                            }
                        }
                        BackendCommand::CycleLidAction(forward) => {
                            if let Some(pwr) = &power_backend_for_cmd {
                                if let Ok(info) = pwr.get_info().await {
                                    let actions = vec!["ignore", "suspend", "hibernate"];
                                    let current_idx = actions.iter().position(|a| a == &info.lid_action).unwrap_or(1);
                                    let next_idx = if forward {
                                        (current_idx + 1) % actions.len()
                                    } else {
                                        if current_idx == 0 { actions.len() - 1 } else { current_idx - 1 }
                                    };
                                    let next_action = actions[next_idx];
                                    let _ = execute_transaction!(
                                        pwr.set_lid_action(next_action),
                                        pwr.get_info(),
                                        |new_info: &crate::backends::PowerInfo| new_info.lid_action == next_action,
                                        5, 100
                                    );
                                    if let Ok(new_info) = pwr.get_info().await {
                                        let _ = tx_cmd_resp.send(AppEvent::UpdatePower(new_info)).await;
                                    }
                                }
                            }
                        }
                        BackendCommand::CyclePowerProfile => {
                            if let Some(power) = &power_backend_for_cmd {
                                if let Ok(info) = power.get_info().await {
                                    if let Some(prof) = info.power_profile {
                                        let next = match prof.as_str() {
                                            "power-saver" => "balanced",
                                            "balanced" => "performance",
                                            "performance" => "power-saver",
                                            _ => "balanced"
                                        };
                                        let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                            power.set_power_profile(next),
                                            power.get_info(),
                                            |info: &PowerInfo| info.power_profile.as_deref() == Some(next),
                                            10, 50 // up to 500ms
                                        );

                                        if mutation_ok {
                                            if verified {
                                                let _ = tx_cmd_resp.send(AppEvent::Notification(format!("Power profile set to {}", next))).await;
                                            } else {
                                                let _ = tx_cmd_resp.send(AppEvent::Notification("Change could not be verified.".to_string())).await;
                                            }
                                        } else {
                                            let err_msg = mutation_error.unwrap_or_else(|| "Failed to set power profile.".to_string());
                                            let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                        }

                                        if let Ok(actual_info) = final_state {
                                            let _ = tx_cmd_resp.send(AppEvent::UpdatePower(actual_info)).await;
                                        }
                                    }
                                }
                            }
                        }
                        BackendCommand::ToggleFlightMode(target_state) => {
                            if let Some(net) = &network_backend_for_cmd {
                                let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                    net.set_flight_mode_enabled(target_state),
                                    net.flight_mode_enabled(),
                                    |state: &bool| *state == target_state,
                                    10, 50 // up to 500ms
                                );

                                if mutation_ok {
                                    if verified {
                                        let _ = tx_cmd_resp.send(AppEvent::Notification(format!("Flight mode {}", if target_state { "enabled" } else { "disabled" }))).await;
                                    } else {
                                        let _ = tx_cmd_resp.send(AppEvent::Notification("Change could not be verified.".to_string())).await;
                                    }
                                } else {
                                    let err_msg = mutation_error.unwrap_or_else(|| "Failed to execute flight mode command.".to_string());
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                }

                                if let Ok(actual_state) = final_state {
                                    let _ = tx_cmd_resp.send(AppEvent::UpdateFlightMode(Some(actual_state))).await;
                                }
                            }
                        }
                        BackendCommand::ToggleHotspot(target_state) => {
                            if let Some(net) = &network_backend_for_cmd {
                                let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                    net.set_hotspot_enabled(target_state),
                                    net.hotspot_enabled(),
                                    |state: &bool| *state == target_state,
                                    15, 100 // hotspots take time
                                );

                                if mutation_ok {
                                    if verified {
                                        let _ = tx_cmd_resp.send(AppEvent::Notification(format!("Hotspot {}", if target_state { "enabled" } else { "disabled" }))).await;
                                    } else {
                                        let _ = tx_cmd_resp.send(AppEvent::Notification("Change could not be verified.".to_string())).await;
                                    }
                                } else {
                                    let err_msg = mutation_error.unwrap_or_else(|| "Failed to execute hotspot command.".to_string());
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                }

                                if let Ok(actual_state) = final_state {
                                    let _ = tx_cmd_resp.send(AppEvent::UpdateHotspot(Some(actual_state))).await;
                                }
                            }
                        }
                        BackendCommand::ToggleWifi(target_state) => {
                            if let Some(net) = &network_backend_for_cmd {
                                let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                    net.set_wifi_enabled(target_state),
                                    net.wifi_enabled(),
                                    |state: &bool| *state == target_state,
                                    10, 50 // up to 500ms
                                );

                                if mutation_ok {
                                    if verified {
                                        let _ = tx_cmd_resp.send(AppEvent::Notification(format!("Wi-Fi {}", if target_state { "enabled" } else { "disabled" }))).await;
                                    } else {
                                        let _ = tx_cmd_resp.send(AppEvent::Notification("Change could not be verified.".to_string())).await;
                                    }
                                } else {
                                    let err_msg = mutation_error.unwrap_or_else(|| "Failed to execute Wi-Fi command.".to_string());
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                }

                                if let Ok(actual_state) = final_state {
                                    if let Ok(nets) = net.networks().await {
                                        let _ = tx_cmd_resp.send(AppEvent::UpdateNetworks(actual_state, nets)).await;
                                    }
                                }
                            }
                        }
                        BackendCommand::ConnectNetwork(id, connect) => {
                            if let Some(net) = &network_backend_for_cmd {
                                let mutate = if connect { net.connect(&id) } else { net.disconnect(&id) };
                                let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                    mutate,
                                    net.networks(),
                                    |networks: &Vec<crate::backends::Network>| {
                                        if let Some(n) = networks.iter().find(|n| n.id == id) {
                                            n.connected == connect
                                        } else {
                                            false
                                        }
                                    },
                                    20, 200 // Network connection can take a few seconds
                                );

                                if mutation_ok {
                                    if verified {
                                        let _ = tx_cmd_resp.send(AppEvent::Notification(format!("Network {}", if connect { "connected" } else { "disconnected" }))).await;
                                    } else {
                                        let _ = tx_cmd_resp.send(AppEvent::Notification("Network change could not be verified.".to_string())).await;
                                    }
                                } else {
                                    let err_msg = mutation_error.unwrap_or_else(|| format!("Failed to {} network.", if connect { "connect" } else { "disconnect" }));
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                }

                                if let Ok(networks) = final_state {
                                    if let Ok(enabled) = net.wifi_enabled().await {
                                        let _ = tx_cmd_resp.send(AppEvent::UpdateNetworks(enabled, networks)).await;
                                    }
                                }
                            }
                        }
                        BackendCommand::ConnectNetworkWithPassword(id, password) => {
                            if let Some(net) = &network_backend_for_cmd {
                                let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                    net.connect_with_password(&id, &password),
                                    net.networks(),
                                    |networks: &Vec<crate::backends::Network>| {
                                        if let Some(n) = networks.iter().find(|n| n.id == id) {
                                            n.connected
                                        } else {
                                            false
                                        }
                                    },
                                    25, 200
                                );

                                if mutation_ok {
                                    if verified {
                                        let _ = tx_cmd_resp.send(AppEvent::Notification(format!("Connected to '{}'.", id.0))).await;
                                    } else {
                                        let _ = tx_cmd_resp.send(AppEvent::Notification("Connection initiated, verification pending.".to_string())).await;
                                    }
                                } else {
                                    let err_msg = mutation_error.unwrap_or_else(|| "Failed to connect with provided password.".to_string());
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                }

                                if let Ok(networks) = final_state {
                                    if let Ok(enabled) = net.wifi_enabled().await {
                                        let _ = tx_cmd_resp.send(AppEvent::UpdateNetworks(enabled, networks)).await;
                                    }
                                }
                            }
                        }
                        BackendCommand::ForgetNetwork(id) => {
                            if let Some(net) = &network_backend_for_cmd {
                                let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                    net.forget_network(&id),
                                    net.networks(),
                                    |networks: &Vec<crate::backends::Network>| {
                                        !networks.iter().any(|n| n.id == id && n.saved)
                                    },
                                    10, 100
                                );

                                if mutation_ok {
                                    if verified {
                                        let _ = tx_cmd_resp.send(AppEvent::Notification(format!("Wi-Fi profile '{}' forgotten.", id.0))).await;
                                    } else {
                                        let _ = tx_cmd_resp.send(AppEvent::Notification("Wi-Fi profile deleted, verification pending.".to_string())).await;
                                    }
                                } else {
                                    let err_msg = mutation_error.unwrap_or_else(|| "Failed to delete Wi-Fi profile.".to_string());
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                }

                                if let Ok(networks) = final_state {
                                    if let Ok(enabled) = net.wifi_enabled().await {
                                        let _ = tx_cmd_resp.send(AppEvent::UpdateNetworks(enabled, networks)).await;
                                    }
                                }
                            }
                        }
                        BackendCommand::RescanWifi => {
                            if let Some(net) = &network_backend_for_cmd {
                                let _ = net.rescan().await;
                                tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;
                                if let Ok(networks) = net.networks().await {
                                    if let Ok(enabled) = net.wifi_enabled().await {
                                        let _ = tx_cmd_resp.send(AppEvent::UpdateNetworks(enabled, networks)).await;
                                        let _ = tx_cmd_resp.send(AppEvent::Notification("Wi-Fi network list refreshed.".to_string())).await;
                                    }
                                }
                            }
                        }
                        BackendCommand::SetAudioDefault(id, is_sink) => {
                            if is_sink {
                                let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                    audio_backend_for_cmd.set_default_sink(id),
                                    audio_backend_for_cmd.get_sinks(),
                                    |sinks: &Vec<crate::backends::AudioDevice>| {
                                        sinks.iter().any(|s| s.id == id && s.is_default)
                                    },
                                    10, 50
                                );
                                if mutation_ok {
                                    if verified {
                                        let _ = tx_cmd_resp
                                            .send(AppEvent::Notification(
                                                "Default output audio device updated.".to_string(),
                                            ))
                                            .await;
                                    } else {
                                        let _ = tx_cmd_resp
                                            .send(AppEvent::Notification(
                                                "Audio default change could not be verified."
                                                    .to_string(),
                                            ))
                                            .await;
                                    }
                                } else {
                                    let err_msg = mutation_error.unwrap_or_else(|| format!("Failed to set default output device to {}", id));
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                }
                                if let Ok(sinks) = final_state {
                                    if let Ok(sources) = audio_backend_for_cmd.get_sources().await {
                                        if let Ok(streams) = audio_backend_for_cmd.get_streams().await {
                                            let _ = tx_cmd_resp
                                                .send(AppEvent::UpdateAudio(sinks, sources, streams))
                                                .await;
                                        }
                                    }
                                }
                            } else {
                                let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                    audio_backend_for_cmd.set_default_source(id),
                                    audio_backend_for_cmd.get_sources(),
                                    |sources: &Vec<crate::backends::AudioDevice>| {
                                        sources.iter().any(|s| s.id == id && s.is_default)
                                    },
                                    10, 50
                                );
                                if mutation_ok {
                                    if verified {
                                        let _ = tx_cmd_resp
                                            .send(AppEvent::Notification(
                                                "Default input audio device updated.".to_string(),
                                            ))
                                            .await;
                                    } else {
                                        let _ = tx_cmd_resp
                                            .send(AppEvent::Notification(
                                                "Audio input change could not be verified."
                                                    .to_string(),
                                            ))
                                            .await;
                                    }
                                } else {
                                    let err_msg = mutation_error.unwrap_or_else(|| format!("Failed to set default input device to {}", id));
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                }
                                if let Ok(sources) = final_state {
                                    if let Ok(sinks) = audio_backend_for_cmd.get_sinks().await {
                                        if let Ok(streams) = audio_backend_for_cmd.get_streams().await {
                                            let _ = tx_cmd_resp
                                                .send(AppEvent::UpdateAudio(sinks, sources, streams))
                                                .await;
                                        }
                                    }
                                }
                            }
                        }
                        BackendCommand::RemoveBluetoothDevice(id) => {
                            if let Some(bt) = &bt_backend_for_cmd {
                                let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                    bt.remove_device(&id),
                                    bt.devices(),
                                    |devices: &Vec<crate::backends::BluetoothDevice>| {
                                        !devices.iter().any(|d| d.id == id)
                                    },
                                    10, 100
                                );
                                if mutation_ok {
                                    if verified {
                                        let _ = tx_cmd_resp
                                            .send(AppEvent::Notification(
                                                "Bluetooth device forgotten.".to_string(),
                                            ))
                                            .await;
                                    } else {
                                        let _ = tx_cmd_resp
                                            .send(AppEvent::Notification(
                                                "Device removal could not be verified.".to_string(),
                                            ))
                                            .await;
                                    }
                                } else {
                                    let err_msg = mutation_error.unwrap_or_else(|| "Failed to remove Bluetooth device.".to_string());
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                }
                                if let Ok(devices) = final_state {
                                    let _ = tx_cmd_resp
                                        .send(AppEvent::UpdateBluetooth(devices))
                                        .await;
                                }
                            }
                        }
                        BackendCommand::LaunchApplication(exec) => {
                            if apps_backend.launch_application(&exec).await.is_err() {
                                let _ = tx_cmd_resp.send(AppEvent::Notification(format!("Failed to launch: {}", exec))).await;
                            } else {
                                let _ = tx_cmd_resp.send(AppEvent::Notification("Launched application.".to_string())).await;
                            }
                        }
                        BackendCommand::SystemPowerAction(action) => {
                            use crate::backends::SystemBackend;
                            if let Err(e) = sys_backend_for_cmd.power_action(&action).await {
                                let _ = tx_cmd_resp
                                    .send(AppEvent::Notification(format!(
                                        "Power action failed: {}",
                                        e
                                    )))
                                    .await;
                            } else {
                                let _ = tx_cmd_resp
                                    .send(AppEvent::Notification(format!(
                                        "Executing {}...",
                                        action
                                    )))
                                    .await;
                            }
                        }
                        BackendCommand::SetDisplayBrightness(target) => {
                            if let Some(disp) = &display_backend_for_cmd {
                                let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                    disp.set_brightness(target),
                                    disp.get_brightness(),
                                    |b: &Option<u32>| {
                                        if let Some(val) = b {
                                            (*val as i32 - target as i32).abs() <= 5
                                        } else {
                                            false
                                        }
                                    },
                                    10,
                                    50
                                );

                                if mutation_ok {
                                    if verified {
                                        let _ = tx_cmd_resp
                                            .send(AppEvent::Notification(format!(
                                                "Brightness set to {}%",
                                                target
                                            )))
                                            .await;
                                    } else {
                                        let _ = tx_cmd_resp
                                            .send(AppEvent::Notification(
                                                "Brightness change could not be verified."
                                                    .to_string(),
                                            ))
                                            .await;
                                    }
                                } else {
                                    let err_msg = mutation_error.unwrap_or_else(|| {
                                        "Failed to set display brightness.".to_string()
                                    });
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                }

                                if let Ok(b) = final_state {
                                    let _ = tx_cmd_resp.send(AppEvent::UpdateBrightness(b)).await;
                                }
                            }
                        }
                        BackendCommand::ToggleNightLight(target) => {
                            if let Some(disp) = &display_backend_for_cmd {
                                let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                    disp.set_night_light_enabled(target, 4500),
                                    disp.is_night_light_enabled(),
                                    |nl: &bool| *nl == target,
                                    10,
                                    100
                                );

                                if mutation_ok {
                                    if verified {
                                        let state_str = if target { "On" } else { "Off" };
                                        let _ = tx_cmd_resp
                                            .send(AppEvent::Notification(format!(
                                                "Night Light {}",
                                                state_str
                                            )))
                                            .await;
                                    } else {
                                        let _ = tx_cmd_resp
                                            .send(AppEvent::Notification(
                                                "Failed to verify Night Light change".to_string(),
                                            ))
                                            .await;
                                    }
                                } else if let Some(err) = mutation_error {
                                    let err_msg =
                                        format!("Failed to set Night Light: {}", err);
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                }

                                if let Ok(nl) = final_state {
                                    let _ = tx_cmd_resp.send(AppEvent::UpdateNightLight(Some(nl))).await;
                                }
                            }
                        }
                        BackendCommand::ToggleNTP(target) => {
                            let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                sys_backend_for_cmd.set_ntp(target),
                                sys_backend_for_cmd.get_info(),
                                |info: &SystemInfo| info.ntp_active == target,
                                15,
                                100
                            );

                            if mutation_ok {
                                if verified {
                                    let _ = tx_cmd_resp
                                        .send(AppEvent::Notification(format!(
                                            "NTP synchronization {}",
                                            if target { "enabled" } else { "disabled" }
                                        )))
                                        .await;
                                } else {
                                    let _ = tx_cmd_resp
                                        .send(AppEvent::Notification(
                                            "NTP synchronization change could not be verified."
                                                .to_string(),
                                        ))
                                        .await;
                                }
                            } else {
                                let err_msg = mutation_error.unwrap_or_else(|| {
                                    "Failed to configure NTP synchronization (polkit authorization may be required)."
                                        .to_string()
                                });
                                let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                            }

                            if let Ok(info) = final_state {
                                let _ =
                                    tx_cmd_resp.send(AppEvent::UpdateSystemInfo(info)).await;
                            }
                        }
                        BackendCommand::ToggleNaturalScroll(target) => {
                            if let Some(inp) = &input_backend_for_cmd {
                                let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                    inp.set_natural_scroll(target),
                                    inp.get_settings(),
                                    |settings: &InputSettings| settings.natural_scroll == target,
                                    10,
                                    50
                                );
                                if mutation_ok {
                                    if verified {
                                        let _ = tx_cmd_resp
                                            .send(AppEvent::Notification(format!(
                                                "Natural scrolling {}",
                                                if target { "enabled" } else { "disabled" }
                                            )))
                                            .await;
                                    } else {
                                        let _ = tx_cmd_resp
                                            .send(AppEvent::Notification(
                                                "Natural scrolling change could not be verified."
                                                    .to_string(),
                                            ))
                                            .await;
                                    }
                                } else {
                                    let err_msg = mutation_error.unwrap_or_else(|| {
                                        "Failed to toggle natural scrolling.".to_string()
                                    });
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                }

                                if let Ok(settings) = final_state {
                                    let _ = tx_cmd_resp
                                        .send(AppEvent::UpdateInputSettings(settings))
                                        .await;
                                }
                            }
                        }
                        BackendCommand::ToggleTapToClick(target) => {
                            if let Some(inp) = &input_backend_for_cmd {
                                let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                    inp.set_tap_to_click(target),
                                    inp.get_settings(),
                                    |settings: &InputSettings| settings.tap_to_click == target,
                                    10,
                                    50
                                );
                                if mutation_ok {
                                    if verified {
                                        let _ = tx_cmd_resp
                                            .send(AppEvent::Notification(format!(
                                                "Tap to click {}",
                                                if target { "enabled" } else { "disabled" }
                                            )))
                                            .await;
                                    } else {
                                        let _ = tx_cmd_resp
                                            .send(AppEvent::Notification(
                                                "Tap to click change could not be verified."
                                                    .to_string(),
                                            ))
                                            .await;
                                    }
                                } else {
                                    let err_msg = mutation_error.unwrap_or_else(|| {
                                        "Failed to toggle tap to click.".to_string()
                                    });
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                }

                                if let Ok(settings) = final_state {
                                    let _ = tx_cmd_resp
                                        .send(AppEvent::UpdateInputSettings(settings))
                                        .await;
                                }
                            }
                        }
                        BackendCommand::ToggleLeftHanded(target) => {
                            if let Some(inp) = &input_backend_for_cmd {
                                let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                    inp.set_left_handed(target),
                                    inp.get_settings(),
                                    |settings: &InputSettings| settings.left_handed == target,
                                    10,
                                    50
                                );
                                if mutation_ok {
                                    if verified {
                                        let _ = tx_cmd_resp
                                            .send(AppEvent::Notification(format!(
                                                "Left-handed mode {}",
                                                if target { "enabled" } else { "disabled" }
                                            )))
                                            .await;
                                    } else {
                                        let _ = tx_cmd_resp
                                            .send(AppEvent::Notification(
                                                "Left-handed mode change could not be verified."
                                                    .to_string(),
                                            ))
                                            .await;
                                    }
                                } else {
                                    let err_msg = mutation_error.unwrap_or_else(|| {
                                        "Failed to toggle left-handed mode.".to_string()
                                    });
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                }

                                if let Ok(settings) = final_state {
                                    let _ = tx_cmd_resp
                                        .send(AppEvent::UpdateInputSettings(settings))
                                        .await;
                                }
                            }
                        }
                        BackendCommand::SetPointerSensitivity(val) => {
                            if let Some(inp) = &input_backend_for_cmd {
                                let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                    inp.set_sensitivity(val),
                                    inp.get_settings(),
                                    |settings: &InputSettings| (settings.sensitivity - val).abs() < 0.06,
                                    10,
                                    50
                                );
                                if !mutation_ok {
                                    let err_msg = mutation_error.unwrap_or_else(|| {
                                        "Failed to set pointer speed.".to_string()
                                    });
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                } else if !verified {
                                    let _ = tx_cmd_resp
                                        .send(AppEvent::Notification(
                                            "Pointer speed change could not be verified."
                                                .to_string(),
                                        ))
                                        .await;
                                }

                                if let Ok(settings) = final_state {
                                    let _ = tx_cmd_resp
                                        .send(AppEvent::UpdateInputSettings(settings))
                                        .await;
                                }
                            }
                        }
                        BackendCommand::ToggleVpn(uuid, activate) => {
                            if let Some(net) = &network_backend_for_cmd {
                                let (mutation_ok, verified, _final_state, mutation_error) = execute_transaction!(
                                    net.toggle_vpn(&uuid, activate),
                                    net.get_vpns(),
                                    |vpns: &Vec<VpnConnection>| {
                                        if let Some(v) = vpns.iter().find(|v| v.uuid == uuid) {
                                            v.active == activate
                                        } else {
                                            // VPN removed from list means it was brought down
                                            !activate
                                        }
                                    },
                                    20, 200 // VPN connections can take a few seconds
                                );

                                if mutation_ok {
                                    if verified {
                                        let _ = tx_cmd_resp
                                            .send(AppEvent::Notification(format!(
                                                "VPN {}",
                                                if activate { "connected" } else { "disconnected" }
                                            )))
                                            .await;
                                    } else {
                                        let _ = tx_cmd_resp
                                            .send(AppEvent::Notification(
                                                "VPN change could not be verified.".to_string(),
                                            ))
                                            .await;
                                    }
                                } else {
                                    let err_msg = mutation_error.unwrap_or_else(|| {
                                        format!("Failed to {} VPN.", if activate { "connect" } else { "disconnect" })
                                    });
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                                }

                                // Always refresh VPN list after toggle attempt
                                if let Ok(vpns) = net.get_vpns().await {
                                    let _ = tx_cmd_resp.send(AppEvent::UpdateVpns(vpns)).await;
                                }
                            }
                        }
                        BackendCommand::SetHostname(name) => {
                            let target_name = name.clone();
                            let (mutation_ok, verified, final_state, mutation_error) = execute_transaction!(
                                sys_backend_for_cmd.set_hostname(&name),
                                sys_backend_for_cmd.get_info(),
                                |info: &SystemInfo| info.hostname == target_name,
                                15,
                                100
                            );

                            if mutation_ok {
                                if verified {
                                    let _ = tx_cmd_resp
                                        .send(AppEvent::Notification(format!(
                                            "Hostname updated to '{}'",
                                            target_name
                                        )))
                                        .await;
                                } else {
                                    let _ = tx_cmd_resp
                                        .send(AppEvent::Notification(
                                            "Hostname change could not be verified (elevation may be needed)."
                                                .to_string(),
                                        ))
                                        .await;
                                }
                            } else {
                                let err_msg = mutation_error.unwrap_or_else(|| {
                                    "Failed to set hostname (polkit elevation may be required).".to_string()
                                });
                                let _ = tx_cmd_resp.send(AppEvent::Notification(err_msg)).await;
                            }

                            if let Ok(info) = final_state {
                                let _ = tx_cmd_resp.send(AppEvent::UpdateSystemInfo(info)).await;
                            }
                        }
                        BackendCommand::CycleGtkTheme(next) => {
                            if let Some(appr) = &appearance_backend_for_cmd {
                                if let Ok(info) = appr.get_info().await {
                                    if !info.available_gtk_themes.is_empty() {
                                        let cur_idx = info
                                            .available_gtk_themes
                                            .iter()
                                            .position(|t| t == &info.gtk_theme)
                                            .unwrap_or(0);
                                        let target_idx = if next {
                                            (cur_idx + 1) % info.available_gtk_themes.len()
                                        } else if cur_idx == 0 {
                                            info.available_gtk_themes.len() - 1
                                        } else {
                                            cur_idx - 1
                                        };
                                        let target_theme =
                                            info.available_gtk_themes[target_idx].clone();
                                        let verify_target = target_theme.clone();

                                        let (mutation_ok, verified, final_state, mutation_error) =
                                            execute_transaction!(
                                                appr.set_gtk_theme(&target_theme),
                                                appr.get_info(),
                                                |info: &AppearanceInfo| info.gtk_theme
                                                    == verify_target,
                                                10,
                                                50
                                            );

                                        if mutation_ok {
                                            if verified {
                                                let _ = tx_cmd_resp
                                                    .send(AppEvent::Notification(format!(
                                                        "GTK theme set to '{}'",
                                                        verify_target
                                                    )))
                                                    .await;
                                            } else {
                                                let _ = tx_cmd_resp
                                                    .send(AppEvent::Notification(
                                                        "GTK theme change could not be verified."
                                                            .to_string(),
                                                    ))
                                                    .await;
                                            }
                                        } else {
                                            let err_msg = mutation_error.unwrap_or_else(|| {
                                                "Failed to set GTK theme.".to_string()
                                            });
                                            let _ = tx_cmd_resp
                                                .send(AppEvent::Notification(err_msg))
                                                .await;
                                        }

                                        if let Ok(actual_info) = final_state {
                                            let _ = tx_cmd_resp
                                                .send(AppEvent::UpdateAppearance(actual_info))
                                                .await;
                                        }
                                    }
                                }
                            }
                        }
                        BackendCommand::CycleCursorTheme(forward) => {
                            if let Some(appr) = &appearance_backend_for_cmd {
                                if let Ok(info) = appr.get_info().await {
                                    if !info.available_cursor_themes.is_empty() {
                                        let current_idx = info.available_cursor_themes.iter().position(|t| t == &info.cursor_theme).unwrap_or(0);
                                        let next_idx = if forward {
                                            (current_idx + 1) % info.available_cursor_themes.len()
                                        } else {
                                            if current_idx == 0 { info.available_cursor_themes.len() - 1 } else { current_idx - 1 }
                                        };
                                        let next_theme = info.available_cursor_themes[next_idx].clone();
                                        let _ = execute_transaction!(
                                            appr.set_cursor_theme(&next_theme),
                                            appr.get_info(),
                                            |new_info: &crate::backends::AppearanceInfo| new_info.cursor_theme == next_theme,
                                            5, 50
                                        );
                                        if let Ok(new_info) = appr.get_info().await {
                                            let _ = tx_cmd_resp.send(AppEvent::UpdateAppearance(new_info)).await;
                                        }
                                    }
                                }
                            }
                        }
                        
                                                BackendCommand::TestAudio => {
                            std::thread::spawn(|| {
                                let _ = std::process::Command::new("speaker-test")
                                    .args(["-t", "sine", "-f", "440", "-l", "1"])
                                    .output();
                            });
                        }
                        BackendCommand::SetWallpaper(wp) => {
                            if let Some(app) = &appearance_backend_for_cmd {
                                let _ = app.set_wallpaper(&wp).await;
                                if let Ok(info) = app.get_info().await {
                                    let _ = tx_cmd_resp.send(AppEvent::UpdateAppearance(info)).await;
                                }
                            }
                        }
                        BackendCommand::SetFontName(font) => {
                            if let Some(appr) = &appearance_backend_for_cmd {
                                let _ = execute_transaction!(
                                    appr.set_font_name(&font),
                                    appr.get_info(),
                                    |new_info: &crate::backends::AppearanceInfo| new_info.font_name == font,
                                    5, 50
                                );
                                if let Ok(new_info) = appr.get_info().await {
                                    let _ = tx_cmd_resp.send(AppEvent::UpdateAppearance(new_info)).await;
                                }
                            }
                        }
                        BackendCommand::CycleIconTheme(next) => {
                            if let Some(appr) = &appearance_backend_for_cmd {
                                if let Ok(info) = appr.get_info().await {
                                    if !info.available_icon_themes.is_empty() {
                                        let cur_idx = info
                                            .available_icon_themes
                                            .iter()
                                            .position(|t| t == &info.icon_theme)
                                            .unwrap_or(0);
                                        let target_idx = if next {
                                            (cur_idx + 1) % info.available_icon_themes.len()
                                        } else if cur_idx == 0 {
                                            info.available_icon_themes.len() - 1
                                        } else {
                                            cur_idx - 1
                                        };
                                        let target_theme =
                                            info.available_icon_themes[target_idx].clone();
                                        let verify_target = target_theme.clone();

                                        let (mutation_ok, verified, final_state, mutation_error) =
                                            execute_transaction!(
                                                appr.set_icon_theme(&target_theme),
                                                appr.get_info(),
                                                |info: &AppearanceInfo| info.icon_theme
                                                    == verify_target,
                                                10,
                                                50
                                            );

                                        if mutation_ok {
                                            if verified {
                                                let _ = tx_cmd_resp
                                                    .send(AppEvent::Notification(format!(
                                                        "Icon theme set to '{}'",
                                                        verify_target
                                                    )))
                                                    .await;
                                            } else {
                                                let _ = tx_cmd_resp
                                                    .send(AppEvent::Notification(
                                                        "Icon theme change could not be verified."
                                                            .to_string(),
                                                    ))
                                                    .await;
                                            }
                                        } else {
                                            let err_msg = mutation_error.unwrap_or_else(|| {
                                                "Failed to set icon theme.".to_string()
                                            });
                                            let _ = tx_cmd_resp
                                                .send(AppEvent::Notification(err_msg))
                                                .await;
                                        }

                                        if let Ok(actual_info) = final_state {
                                            let _ = tx_cmd_resp
                                                .send(AppEvent::UpdateAppearance(actual_info))
                                                .await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                _ = tokio::time::sleep_until(last_poll + Duration::from_secs(5)) => {
                    last_poll = tokio::time::Instant::now();
                    if let Ok(info) = sys_backend.get_info().await {
                        let _ = tx_backend.send(AppEvent::UpdateSystemInfo(info)).await;
                    }
                    if let Some(net) = &net_backend {
                        if let (Ok(enabled), Ok(nets)) = (net.wifi_enabled().await, net.networks().await) {
                        if let Ok(fm) = net.flight_mode_enabled().await {
                            let _ = tx_backend.send(AppEvent::UpdateFlightMode(Some(fm))).await;
                        }
                        if let Ok(hs) = net.hotspot_enabled().await {
                            let _ = tx_backend.send(AppEvent::UpdateHotspot(Some(hs))).await;
                        }
                            let _ = tx_backend.send(AppEvent::UpdateNetworks(enabled, nets)).await;
                        }
                        if let Ok(conn) = net.get_active_connection().await {
                            let _ = tx_backend.send(AppEvent::UpdateActiveConnection(conn)).await;
                        }
                        if let Ok(vpns) = net.get_vpns().await {
                            let _ = tx_backend.send(AppEvent::UpdateVpns(vpns)).await;
                        }
                    }
                    if let Some(bt) = &bt_backend {
                        if let Ok(adapter) = bt.get_adapter().await {
                            let _ = tx_backend.send(AppEvent::UpdateBluetoothAdapter(adapter)).await;
                        }
                        if let Ok(devices) = bt.devices().await {
                            let _ = tx_backend.send(AppEvent::UpdateBluetooth(devices)).await;
                        }
                    }
                    if let Some(power) = &power_backend {
                        if let Ok(info) = power.get_info().await {
                            let _ = tx_backend.send(AppEvent::UpdatePower(info)).await;
                        }
                    }
                    if let (Ok(sinks), Ok(sources), Ok(streams)) = (audio_backend.get_sinks().await, audio_backend.get_sources().await, audio_backend.get_streams().await) {
                        let _ = tx_backend.send(AppEvent::UpdateAudio(sinks, sources, streams)).await;
                    }
                    if let Some(svc) = &services_backend {
                        if let Ok(services) = svc.get_services().await {
                            let _ = tx_backend.send(AppEvent::UpdateServices(services)).await;
                        }
                    }
                    if let Some(disp) = &display_backend {
                        if let Ok(monitors) = disp.get_monitors().await {
                            let _ = tx_backend.send(AppEvent::UpdateMonitors(monitors)).await;
                        }
                        if let Ok(b) = disp.get_brightness().await {
                            let _ = tx_backend.send(AppEvent::UpdateBrightness(b)).await;
                        }
                        if let Ok(nl) = disp.is_night_light_enabled().await {
                            let _ = tx_backend.send(AppEvent::UpdateNightLight(Some(nl))).await;
                        }
                    }
                    if let Some(appr) = &appearance_backend {
                        if let Ok(info) = appr.get_info().await {
                            let _ = tx_backend.send(AppEvent::UpdateAppearance(info)).await;
                        }
                    }
                    if let Ok(apps) = apps_backend.get_applications().await {
                        let defs = apps_backend.get_default_apps().await.ok();
                        let _ = tx_backend.send(AppEvent::UpdateApplications(apps, defs)).await;
                    }
                    if let Some(inp) = &input_backend {
                        if let Ok(settings) = inp.get_settings().await {
                            let _ = tx_backend.send(AppEvent::UpdateInputSettings(settings)).await;
                        }
                    }
                }
            }
        }
    });

    // Main loop
    loop {
        let mut app_lock = app.lock().await;
        terminal.draw(|f| ui::draw(f, &mut app_lock))?;

        if app_lock.should_quit {
            break;
        }
        drop(app_lock); // release lock before awaiting

        if let Some(event) = rx.recv().await {
            let mut app_lock = app.lock().await;
            match event {
                AppEvent::Input(key) => app_lock.handle_key(key),
                AppEvent::Tick => app_lock.on_tick(),
                AppEvent::UpdateSystemInfo(info) => app_lock.system_info = Some(info),
                AppEvent::UpdateNetworks(enabled, nets) => {
                    app_lock.wifi_enabled = enabled;
                    app_lock.networks = nets;
                }
                AppEvent::UpdateBluetooth(devices) => app_lock.bluetooth_devices = devices,
                AppEvent::UpdateBluetoothAdapter(adapter) => {
                    if let Some(a) = adapter {
                        app_lock.bluetooth_powered = a.powered;
                    } else {
                        app_lock.bluetooth_powered = false;
                    }
                }
                AppEvent::UpdatePower(info) => app_lock.power_info = Some(info),
                AppEvent::UpdateAudio(sinks, sources, streams) => {
                    app_lock.audio_sinks = sinks;
                    app_lock.audio_sources = sources;
                    app_lock.audio_streams = streams;
                }
                AppEvent::UpdateServices(services) => app_lock.services = services,
                AppEvent::UpdateMonitors(monitors) => app_lock.monitors = monitors,
                AppEvent::UpdateBrightness(b) => app_lock.display_brightness = b,
                AppEvent::UpdateNightLight(nl) => app_lock.night_light_enabled = nl,
                AppEvent::UpdateFlightMode(fm) => app_lock.flight_mode_enabled = fm,
                AppEvent::UpdateHotspot(hs) => app_lock.hotspot_enabled = hs,
                AppEvent::UpdateActiveConnection(conn) => app_lock.active_connection = conn,
                AppEvent::UpdateAppearance(info) => app_lock.appearance_info = Some(info),
                AppEvent::UpdateApplications(apps, defs) => {
                    app_lock.applications = apps;
                    if defs.is_some() {
                        app_lock.default_apps = defs;
                    }
                }
                AppEvent::UpdateInputSettings(settings) => {
                    app_lock.input_settings = Some(settings);
                }
                AppEvent::UpdateVpns(vpns) => {
                    app_lock.vpns = vpns;
                }
                AppEvent::UpdateCapabilities(caps) => app_lock.capabilities = *caps,
                AppEvent::Notification(msg) => {
                    app_lock.notifications.push(msg);
                    app_lock.notification_timer = 12; // 3 seconds (250ms per tick)
                }
            }
            app_lock.clamp_selection();
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    #[test]
    fn test_app_navigation() {
        let mut app = App::new();
        assert_eq!(app.selected_category, 0);

        // Move down
        app.handle_key(press(KeyCode::Down));
        assert_eq!(app.selected_category, 1);

        // Move up
        app.handle_key(press(KeyCode::Up));
        assert_eq!(app.selected_category, 0);

        // Don't go below 0
        app.handle_key(press(KeyCode::Up));
        assert_eq!(app.selected_category, 0);
    }

    #[test]
    fn test_confirm_dialog_blocks_keys() {
        let mut app = App::new();
        // Inject a fake confirmation
        app.confirm_action = Some((
            "Test?".to_string(),
            BackendCommand::SystemPowerAction("suspend".to_string()),
        ));
        // 'q' should NOT quit when confirm dialog is active
        app.handle_key(press(KeyCode::Char('q')));
        assert!(
            !app.should_quit,
            "q should not quit while confirm dialog is shown"
        );
        assert!(
            app.confirm_action.is_some(),
            "confirm_action should still be set"
        );
    }

    #[test]
    fn test_confirm_dialog_cancel_with_n() {
        let mut app = App::new();
        app.confirm_action = Some((
            "Test?".to_string(),
            BackendCommand::SystemPowerAction("poweroff".to_string()),
        ));
        app.handle_key(press(KeyCode::Char('n')));
        assert!(
            app.confirm_action.is_none(),
            "confirm_action should be cleared on 'n'"
        );
    }

    #[test]
    fn test_confirm_dialog_cancel_with_esc() {
        let mut app = App::new();
        app.confirm_action = Some((
            "Test?".to_string(),
            BackendCommand::SystemPowerAction("reboot".to_string()),
        ));
        app.handle_key(press(KeyCode::Esc));
        assert!(
            app.confirm_action.is_none(),
            "confirm_action should be cleared on Esc"
        );
    }

    #[test]
    fn test_confirm_dialog_does_not_navigate() {
        let mut app = App::new();
        app.confirm_action = Some((
            "Test?".to_string(),
            BackendCommand::SystemPowerAction("hibernate".to_string()),
        ));
        let initial_cat = app.selected_category;
        app.handle_key(press(KeyCode::Down));
        assert_eq!(
            app.selected_category, initial_cat,
            "Navigation should be blocked by confirm dialog"
        );
    }

    #[test]
    fn test_no_panic_empty_lists_navigation() {
        let mut app = App::new();
        // Switch to content focus in "Sound" category (empty audio lists)
        let sound_idx = app.categories.iter().position(|c| c == "Sound").unwrap();
        app.selected_category = sound_idx;
        app.focus = Focus::Content;
        app.selected_item = 0;

        // Navigate down with empty lists — must not panic
        for _ in 0..10 {
            app.handle_key(press(KeyCode::Down));
        }
        // Navigate up with selected_item = 0 — must not underflow
        app.handle_key(press(KeyCode::Up));
        assert_eq!(app.selected_item, 0);
    }

    #[test]
    fn test_no_panic_empty_bluetooth_devices() {
        let mut app = App::new();
        let bt_idx = app
            .categories
            .iter()
            .position(|c| c == "Bluetooth")
            .unwrap();
        app.selected_category = bt_idx;
        app.focus = Focus::Content;
        app.selected_item = 0;
        app.bluetooth_devices = vec![]; // Empty

        // Delete key on empty list must not panic
        app.handle_key(press(KeyCode::Delete));
        assert!(app.confirm_action.is_none());
    }

    #[test]
    fn test_search_mode_toggle() {
        let mut app = App::new();
        assert!(!app.is_searching);

        app.handle_key(press(KeyCode::Char('/')));
        assert!(app.is_searching);

        app.handle_key(press(KeyCode::Esc));
        assert!(!app.is_searching);
    }

    #[test]
    fn test_focus_switch_right() {
        let mut app = App::new();
        assert_eq!(app.focus, Focus::Sidebar);
        app.handle_key(press(KeyCode::Right));
        assert_eq!(app.focus, Focus::Content);
    }

    #[test]
    fn test_focus_switch_left() {
        let mut app = App::new();
        app.focus = Focus::Content;
        app.handle_key(press(KeyCode::Left));
        assert_eq!(app.focus, Focus::Sidebar);
    }

    #[test]
    fn test_enter_moves_to_content_from_sidebar() {
        let mut app = App::new();
        assert_eq!(app.focus, Focus::Sidebar);
        app.handle_key(press(KeyCode::Enter));
        assert_eq!(app.focus, Focus::Content);
    }

    #[test]
    fn test_selected_item_resets_on_focus_change() {
        let mut app = App::new();
        app.focus = Focus::Content;
        app.selected_item = 5;
        // Move to sidebar then back
        app.handle_key(press(KeyCode::Left));
        app.handle_key(press(KeyCode::Enter));
        assert_eq!(
            app.selected_item, 0,
            "selected_item should reset when entering content"
        );
    }

    #[test]
    fn test_system_category_max_items() {
        let mut app = App::new();
        let sys_idx = app.categories.iter().position(|c| c == "System").unwrap();
        app.selected_category = sys_idx;
        app.focus = Focus::Content;
        app.selected_item = 0;

        // Navigate through all items (NTP + 4 power actions = 5 items)
        for _ in 0..10 {
            app.handle_key(press(KeyCode::Down));
        }
        // Must stay at 4 (max index for 5 items)
        assert_eq!(
            app.selected_item, 4,
            "System category should have max 5 items (0-4)"
        );
    }

    #[test]
    fn test_power_action_triggers_confirm_not_immediate() {
        let mut app = App::new();
        let sys_idx = app.categories.iter().position(|c| c == "System").unwrap();
        app.selected_category = sys_idx;
        app.focus = Focus::Content;
        app.selected_item = 4; // "Power Off" (index 4)

        // Space key must NOT trigger power action confirmation or execution
        app.handle_key(press(KeyCode::Char(' ')));
        assert!(
            app.confirm_action.is_none(),
            "Accidental Space key must never trigger power action"
        );

        // Enter should set confirm dialog with explicit shutdown warning
        app.handle_key(press(KeyCode::Enter));
        assert!(
            app.confirm_action.is_some(),
            "Power off must require confirmation"
        );
        let prompt = app.confirm_action.as_ref().unwrap().0.clone();
        assert!(
            prompt.contains("Shut down and power off"),
            "Expected distinct shutdown warning, got: {}",
            prompt
        );
        assert!(!app.should_quit, "App must not quit from power action");

        // Cancel and test Reboot distinct prompt
        app.confirm_action = None;
        app.selected_item = 3; // Reboot (index 3)
        app.handle_key(press(KeyCode::Enter));
        let reboot_prompt = app.confirm_action.as_ref().unwrap().0.clone();
        assert!(
            reboot_prompt.contains("Restart the computer"),
            "Expected distinct reboot warning, got: {}",
            reboot_prompt
        );
    }

    #[test]
    fn test_visible_categories_returns_all() {
        let app = App::new();
        let vis = app.visible_categories();
        assert!(vis.contains(&"Network".to_string()));
        assert!(vis.contains(&"System".to_string()));
        assert!(vis.contains(&"Mouse & Touchpad".to_string()));
        assert_eq!(vis.len(), 10, "Should have 10 categories");
    }

    #[test]
    fn test_notification_timer_decay() {
        let mut app = App::new();
        app.notifications.push("Test notification".to_string());
        app.notification_timer = 3;
        // Simulate ticks
        for _ in 0..3 {
            app.on_tick();
        }
        assert!(
            app.notifications.is_empty(),
            "Notifications should expire after timer"
        );
    }

    #[test]
    fn test_search_wifi() {
        let mut app = App::new();
        app.search_query = "wifi".to_string();
        app.update_search_results();
        assert!(!app.search_results.is_empty());
        assert_eq!(app.search_results[0].title, "Wi-Fi Switch");
        assert_eq!(app.search_results[0].category, "Network");
    }

    #[test]
    fn test_search_bluetooth() {
        let mut app = App::new();
        app.search_query = "bluetooth".to_string();
        app.update_search_results();
        assert!(!app.search_results.is_empty());
        assert!(app.search_results.iter().any(|r| r.category == "Bluetooth"));
    }

    #[test]
    fn test_search_power_off_and_reboot() {
        let mut app = App::new();
        app.search_query = "power off".to_string();
        app.update_search_results();
        assert!(!app.search_results.is_empty());
        assert_eq!(app.search_results[0].title, "Power Off");
        assert_eq!(app.search_results[0].category, "System");
        assert_eq!(app.search_results[0].target_item_idx, 4);

        app.search_query = "reboot".to_string();
        app.update_search_results();
        assert_eq!(app.search_results[0].title, "Reboot");
        assert_eq!(app.search_results[0].category, "System");
        assert_eq!(app.search_results[0].target_item_idx, 3);
    }

    #[test]
    fn test_search_dark_mode() {
        let mut app = App::new();
        app.search_query = "dark".to_string();
        app.update_search_results();
        assert!(!app.search_results.is_empty());
        assert_eq!(app.search_results[0].category, "Appearance");
    }

    #[test]
    fn test_search_selection_navigates_to_target_setting() {
        let mut app = App::new();
        app.is_searching = true;
        app.search_query = "reboot".to_string();
        app.update_search_results();
        assert!(!app.search_results.is_empty());

        // Press Enter on the search result
        app.handle_key(press(KeyCode::Enter));

        assert!(!app.is_searching, "Search mode should exit on selection");
        assert_eq!(app.focus, Focus::Content, "Focus should switch to content");
        let sys_idx = app.categories.iter().position(|c| c == "System").unwrap();
        assert_eq!(
            app.selected_category, sys_idx,
            "Should navigate to System category"
        );
        assert_eq!(app.selected_item, 3, "Should target Reboot item index");
    }

    #[test]
    fn test_clamp_selection_when_list_shrinks() {
        let mut app = App::new();
        let svc_idx = app.categories.iter().position(|c| c == "Services").unwrap();
        app.selected_category = svc_idx;
        app.focus = Focus::Content;

        // Populate with 10 services
        app.services = (0..10)
            .map(|i| crate::backends::ServiceInfo {
                name: format!("service_{}.service", i),
                description: "Test service".to_string(),
                active_state: "active".to_string(),
                sub_state: "running".to_string(),
            })
            .collect();

        app.selected_item = 8;
        app.clamp_selection();
        assert_eq!(app.selected_item, 8);

        // List shrinks to 3 items
        app.services.truncate(3);
        app.clamp_selection();
        assert_eq!(
            app.selected_item, 2,
            "selected_item must clamp to max valid index (2)"
        );
    }

    #[test]
    fn test_clamp_selection_empty_list() {
        let mut app = App::new();
        let svc_idx = app.categories.iter().position(|c| c == "Services").unwrap();
        app.selected_category = svc_idx;
        app.selected_item = 5;
        app.services.clear();

        app.clamp_selection();
        assert_eq!(
            app.selected_item, 0,
            "selected_item must reset to 0 for empty list"
        );
    }

    #[test]
    fn test_display_brightness_keys() {
        let mut app = App::new();
        let disp_idx = app.categories.iter().position(|c| c == "Display").unwrap();
        app.selected_category = disp_idx;
        app.focus = Focus::Content;
        app.selected_item = 0;
        app.display_brightness = Some(50);

        let (tx, mut rx) = mpsc::channel(10);
        app.cmd_tx = Some(tx);

        // Increase via '+'
        app.handle_key(press(KeyCode::Char('+')));
        if let Ok(BackendCommand::SetDisplayBrightness(val)) = rx.try_recv() {
            assert_eq!(val, 55, "Brightness should increase by 5");
        } else {
            panic!("Expected SetDisplayBrightness command on '+'");
        }

        // Decrease via '-'
        app.handle_key(press(KeyCode::Char('-')));
        if let Ok(BackendCommand::SetDisplayBrightness(val)) = rx.try_recv() {
            assert_eq!(val, 45, "Brightness should decrease by 5");
        } else {
            panic!("Expected SetDisplayBrightness command on '-'");
        }

        // Right key also increases
        app.handle_key(press(KeyCode::Right));
        if let Ok(BackendCommand::SetDisplayBrightness(val)) = rx.try_recv() {
            assert_eq!(val, 55, "Brightness should increase on Right arrow");
        } else {
            panic!("Expected SetDisplayBrightness command on Right");
        }
    }

    #[test]
    fn test_system_ntp_toggle_key() {
        let mut app = App::new();
        let sys_idx = app.categories.iter().position(|c| c == "System").unwrap();
        app.selected_category = sys_idx;
        app.focus = Focus::Content;
        app.selected_item = 0; // NTP item

        app.system_info = Some(SystemInfo {
            hostname: "test-host".to_string(),
            chassis: "laptop".to_string(),
            distro: "Linux".to_string(),
            kernel: "6.8".to_string(),
            uptime: 1000,
            memory_total: 16000000000,
            memory_used: 8000000000,
            timezone: "UTC".to_string(),
            ntp_active: true,
            disks: vec![],
            cpu_model: "Intel i5".to_string(),
            cpu_cores: 8,
        });

        let (tx, mut rx) = mpsc::channel(10);
        app.cmd_tx = Some(tx);

        // Press Enter on item 0
        app.handle_key(press(KeyCode::Enter));
        if let Ok(BackendCommand::ToggleNTP(active)) = rx.try_recv() {
            assert!(!active, "Toggling active NTP should set to false");
        } else {
            panic!("Expected ToggleNTP command on Enter");
        }
    }

    #[test]
    fn test_search_brightness_and_ntp() {
        let mut app = App::new();

        app.search_query = "brightness".to_string();
        app.update_search_results();
        assert!(!app.search_results.is_empty());
        assert_eq!(app.search_results[0].title, "Screen Brightness");
        assert_eq!(app.search_results[0].category, "Display");

        app.search_query = "ntp".to_string();
        app.update_search_results();
        assert!(!app.search_results.is_empty());
        assert_eq!(app.search_results[0].title, "Network Time (NTP)");
        assert_eq!(app.search_results[0].category, "System");
    }

    #[test]
    fn test_network_forget_key_confirmation() {
        let mut app = App::new();
        let net_idx = app.categories.iter().position(|c| c == "Network").unwrap();
        app.selected_category = net_idx;
        app.focus = Focus::Content;

        app.networks.push(crate::backends::Network {
            id: crate::backends::NetworkId("OfficeWiFi".to_string()),
            name: "OfficeWiFi".to_string(),
            connected: true,
            strength: 85,
            saved: true,
            security: "WPA2/WPA3".to_string(),
            frequency_mhz: 5240,
        });

        // selected_item = 1 corresponds to first network (item 0 is Wi-Fi Radio)
        app.selected_item = 3;

        // Press Backspace or Delete
        app.handle_key(press(KeyCode::Delete));

        assert!(
            app.confirm_action.is_some(),
            "Delete key on saved network should trigger confirmation modal"
        );

        let (prompt, cmd) = app.confirm_action.unwrap();
        assert!(prompt.contains("OfficeWiFi"));
        if let BackendCommand::ForgetNetwork(id) = cmd {
            assert_eq!(id.0, "OfficeWiFi");
        } else {
            panic!("Expected BackendCommand::ForgetNetwork");
        }
    }

    #[test]
    fn test_power_battery_telemetry_rendering() {
        let mut app = App::new();
        app.power_info = Some(crate::backends::PowerInfo {
            on_battery: true,
            battery_percentage: 75.0,
            battery_state: crate::backends::BatteryState::Discharging,
            power_profile: Some("balanced".to_string()),
            energy_wh: Some(41.2),
            energy_full_wh: Some(55.0),
            energy_full_design_wh: Some(70.0),
            energy_rate_w: Some(6.25),
            power_button_action: "suspend".to_string(),
            lid_action: "suspend".to_string(),
            idle_delay: Some(300),
            health_percentage: Some(78.5),
            charge_cycles: Some(420),
            voltage_v: Some(15.4),
            time_to_empty_secs: Some(7200),
            time_to_full_secs: None,
            battery_model: Some("L18M4PF5".to_string()),
            battery_vendor: Some("SMP".to_string()),
            charge_limit: None,
        });

        let lines = crate::ui::pages::power::render(&app, true);
        let rendered: String = lines.into_iter().map(|l| format!("{:?}", l)).collect();

        assert!(rendered.contains("Battery Level"));
        assert!(rendered.contains("75.0%"));
        assert!(rendered.contains("Discharging"));
        assert!(rendered.contains("2h 0m remaining"));
        assert!(rendered.contains("6.25 W"));
        assert!(rendered.contains("78.5%"));
        assert!(rendered.contains("420 cycles"));
        assert!(rendered.contains("15.40 V"));
        assert!(rendered.contains("SMP L18M4PF5"));
    }

    #[test]
    fn test_network_dns_and_security_rendering() {
        let mut app = App::new();
        app.active_connection = Some(crate::backends::ActiveConnectionInfo {
            interface: "wlan0".to_string(),
            ip_address: "192.168.1.50".to_string(),
            gateway: "192.168.1.1".to_string(),
            mac_address: "aa:bb:cc:dd:ee:ff".to_string(),
            dns_servers: vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()],
        });

        app.networks.push(crate::backends::Network {
            id: crate::backends::NetworkId("Home-5G".to_string()),
            name: "Home-5G".to_string(),
            connected: true,
            strength: 92,
            saved: true,
            security: "WPA2/WPA3".to_string(),
            frequency_mhz: 5180,
        });

        let lines = crate::ui::pages::network::render(&app, true);
        let rendered: String = lines.into_iter().map(|l| format!("{:?}", l)).collect();

        assert!(rendered.contains("wlan0"));
        assert!(rendered.contains("192.168.1.50"));
        assert!(rendered.contains("1.1.1.1, 8.8.8.8"));
        assert!(rendered.contains("Home-5G"));
        assert!(rendered.contains("5 GHz"));
        assert!(rendered.contains("WPA2/WPA3"));
        assert!(rendered.contains("Disconnect [Del: Forget]"));
    }

    #[test]
    fn test_network_password_modal_flow() {
        let mut app = App::new();
        let net_idx = app.categories.iter().position(|c| c == "Network").unwrap();
        app.selected_category = net_idx;
        app.focus = Focus::Content;

        // Unsecured network requiring password
        app.networks.push(crate::backends::Network {
            id: crate::backends::NetworkId("SecuredGuest".to_string()),
            name: "SecuredGuest".to_string(),
            connected: false,
            strength: 80,
            saved: false,
            security: "WPA2".to_string(),
            frequency_mhz: 2437,
        });

        app.selected_item = 3;

        let (tx, mut rx) = mpsc::channel(10);
        app.cmd_tx = Some(tx);

        // Press Enter -> should open PasswordModal instead of immediate connect
        app.handle_key(press(KeyCode::Enter));
        assert!(
            app.password_modal.is_some(),
            "Secured unsaved network should open password modal"
        );

        // Type "secretpass"
        for ch in "secretpass".chars() {
            app.handle_key(press(KeyCode::Char(ch)));
        }

        let modal = app.password_modal.as_ref().unwrap();
        assert_eq!(modal.password, "secretpass");
        assert!(!modal.show_password);

        // Toggle visibility via Tab
        app.handle_key(press(KeyCode::Tab));
        assert!(app.password_modal.as_ref().unwrap().show_password);

        // Backspace removes one char
        app.handle_key(press(KeyCode::Backspace));
        assert_eq!(app.password_modal.as_ref().unwrap().password, "secretpas");

        // Press Enter to submit
        app.handle_key(press(KeyCode::Enter));
        assert!(app.password_modal.is_none(), "Enter should dismiss modal");

        if let Ok(BackendCommand::ConnectNetworkWithPassword(id, pass)) = rx.try_recv() {
            assert_eq!(id.0, "SecuredGuest");
            assert_eq!(pass, "secretpas");
        } else {
            panic!("Expected ConnectNetworkWithPassword command");
        }
    }

    #[test]
    fn test_network_direct_connect_open() {
        let mut app = App::new();
        let net_idx = app.categories.iter().position(|c| c == "Network").unwrap();
        app.selected_category = net_idx;
        app.focus = Focus::Content;

        // Open Wi-Fi (no password required)
        app.networks.push(crate::backends::Network {
            id: crate::backends::NetworkId("AirportFreeWiFi".to_string()),
            name: "AirportFreeWiFi".to_string(),
            connected: false,
            strength: 95,
            saved: false,
            security: "Open".to_string(),
            frequency_mhz: 5200,
        });

        app.selected_item = 3;

        let (tx, mut rx) = mpsc::channel(10);
        app.cmd_tx = Some(tx);

        // Press Enter -> should connect directly
        app.handle_key(press(KeyCode::Enter));
        assert!(
            app.password_modal.is_none(),
            "Open network should not prompt for password"
        );

        if let Ok(BackendCommand::ConnectNetwork(id, connect)) = rx.try_recv() {
            assert_eq!(id.0, "AirportFreeWiFi");
            assert!(connect);
        } else {
            panic!("Expected ConnectNetwork command");
        }
    }

    #[test]
    fn test_network_rescan_key() {
        let mut app = App::new();
        let net_idx = app.categories.iter().position(|c| c == "Network").unwrap();
        app.selected_category = net_idx;
        app.focus = Focus::Content;

        let (tx, mut rx) = mpsc::channel(10);
        app.cmd_tx = Some(tx);

        app.handle_key(press(KeyCode::Char('r')));
        if let Ok(BackendCommand::RescanWifi) = rx.try_recv() {
            // Success
        } else {
            panic!("Expected RescanWifi command on 'r'");
        }
    }

    #[test]
    fn test_sound_default_source_selection() {
        let mut app = App::new();
        let sound_idx = app.categories.iter().position(|c| c == "Sound").unwrap();
        app.selected_category = sound_idx;
        app.focus = Focus::Content;

        // Add 1 sink (item 0) and 1 source (item 1)
        app.audio_sinks.push(crate::backends::AudioDevice {
            id: 42,
            name: "speakers".to_string(),
            description: "Builtin Speakers".to_string(),
            is_default: true,
            volume: 0.5,
            muted: false,
            is_sink: true,
            form_factor: "internal".to_string(),
        });
        app.audio_sources.push(crate::backends::AudioDevice {
            id: 88,
            name: "mic".to_string(),
            description: "USB Microphone".to_string(),
            is_default: false,
            volume: 0.8,
            muted: false,
            is_sink: false,
            form_factor: "usb".to_string(),
        });

        // Focus the microphone (item 1)
        app.selected_item = 1;

        let (tx, mut rx) = mpsc::channel(10);
        app.cmd_tx = Some(tx);

        app.handle_key(press(KeyCode::Enter));
        if let Ok(BackendCommand::SetAudioDefault(id, is_sink)) = rx.try_recv() {
            assert_eq!(id, 88);
            assert!(!is_sink, "Microphone must have is_sink = false");
        } else {
            panic!("Expected SetAudioDefault with is_sink = false");
        }
    }

    #[test]
    fn test_input_touchpad_toggles() {
        let mut app = App::new();
        let input_idx = app
            .categories
            .iter()
            .position(|c| c == "Mouse & Touchpad")
            .unwrap();
        app.selected_category = input_idx;
        app.focus = Focus::Content;

        app.input_settings = Some(InputSettings {
            natural_scroll: false,
            tap_to_click: true,
            left_handed: false,
            sensitivity: 0.0,
        });

        let (tx, mut rx) = mpsc::channel(10);
        app.cmd_tx = Some(tx);

        // Item 0: Natural scroll toggle
        app.selected_item = 0;
        app.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            rx.try_recv(),
            Ok(BackendCommand::ToggleNaturalScroll(true))
        ));

        // Item 1: Tap to click toggle
        app.selected_item = 1;
        app.handle_key(press(KeyCode::Char(' ')));
        assert!(matches!(
            rx.try_recv(),
            Ok(BackendCommand::ToggleTapToClick(false))
        ));

        // Item 2: Left handed toggle
        app.selected_item = 2;
        app.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            rx.try_recv(),
            Ok(BackendCommand::ToggleLeftHanded(true))
        ));
    }

    #[test]
    fn test_input_sensitivity_adjustment() {
        let mut app = App::new();
        let input_idx = app
            .categories
            .iter()
            .position(|c| c == "Mouse & Touchpad")
            .unwrap();
        app.selected_category = input_idx;
        app.focus = Focus::Content;

        app.input_settings = Some(InputSettings {
            natural_scroll: false,
            tap_to_click: false,
            left_handed: false,
            sensitivity: 0.2,
        });

        let (tx, mut rx) = mpsc::channel(10);
        app.cmd_tx = Some(tx);

        // Item 3: Pointer speed
        app.selected_item = 3;

        // Press Right arrow -> increase by 0.05
        app.handle_key(press(KeyCode::Right));
        if let Ok(BackendCommand::SetPointerSensitivity(val)) = rx.try_recv() {
            assert!((val - 0.25).abs() < 1e-4);
        } else {
            panic!("Expected SetPointerSensitivity(0.25)");
        }

        // Press Left arrow -> decrease by 0.05
        app.handle_key(press(KeyCode::Left));
        if let Ok(BackendCommand::SetPointerSensitivity(val)) = rx.try_recv() {
            assert!((val - 0.15).abs() < 1e-4);
        } else {
            panic!("Expected SetPointerSensitivity(0.15)");
        }
    }

    #[test]
    fn test_search_input_settings() {
        let mut app = App::new();
        app.input_settings = Some(InputSettings::default());

        app.search_query = "touchpad".to_string();
        app.update_search_results();

        assert!(!app.search_results.is_empty());
        let found = app
            .search_results
            .iter()
            .any(|r| r.category == "Mouse & Touchpad");
        assert!(
            found,
            "Search for 'touchpad' should yield Mouse & Touchpad settings"
        );
    }

    #[test]
    fn test_appearance_theme_cycling() {
        let mut app = App::new();
        let app_idx = app
            .categories
            .iter()
            .position(|c| c == "Appearance")
            .unwrap();
        app.selected_category = app_idx;
        app.focus = Focus::Content;

        let (tx, mut rx) = mpsc::channel(10);
        app.cmd_tx = Some(tx);

        // Item 1: GTK Theme
        app.selected_item = 1;
        app.handle_key(press(KeyCode::Right));
        if let Ok(BackendCommand::CycleGtkTheme(next)) = rx.try_recv() {
            assert!(next, "Right arrow should cycle next GTK theme");
        } else {
            panic!("Expected CycleGtkTheme(true)");
        }

        app.handle_key(press(KeyCode::Left));
        if let Ok(BackendCommand::CycleGtkTheme(next)) = rx.try_recv() {
            assert!(!next, "Left arrow should cycle previous GTK theme");
        } else {
            panic!("Expected CycleGtkTheme(false)");
        }

        // Item 2: Icon Theme
        app.selected_item = 2;
        app.handle_key(press(KeyCode::Enter));
        if let Ok(BackendCommand::CycleIconTheme(next)) = rx.try_recv() {
            assert!(next, "Enter should cycle next Icon theme");
        } else {
            panic!("Expected CycleIconTheme(true)");
        }
    }

    #[test]
    fn test_hostname_modal_flow() {
        let mut app = App::new();
        let sys_idx = app.categories.iter().position(|c| c == "System").unwrap();
        app.selected_category = sys_idx;
        app.focus = Focus::Content;

        // Press 'e' in System page -> opens hostname modal
        app.handle_key(press(KeyCode::Char('e')));
        assert!(app.hostname_modal.is_some());

        // Type 'new-host'
        app.handle_key(press(KeyCode::Backspace));
        app.handle_key(press(KeyCode::Char('a')));

        let (tx, mut rx) = mpsc::channel(10);
        app.cmd_tx = Some(tx);

        // Press Enter to submit
        app.handle_key(press(KeyCode::Enter));
        assert!(app.hostname_modal.is_none());

        if let Ok(BackendCommand::SetHostname(name)) = rx.try_recv() {
            assert!(!name.is_empty());
        } else {
            panic!("Expected SetHostname command on Enter in hostname modal");
        }
    }
}
