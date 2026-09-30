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
    AudioBackend, AudioDevice, BluetoothBackend, BluetoothDevice, DisplayBackend, Monitor, Network,
    NetworkBackend, PowerBackend, PowerInfo, ServiceInfo, ServicesBackend, SystemBackend,
    SystemInfo,
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
    UpdateApplications(Vec<AppEntry>),
    UpdateCapabilities(Box<crate::platform::PlatformCapabilities>),
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
    ToggleWifi(bool),
    SetDisplayResolution(String, i32, i32, f64),
    ConnectNetwork(crate::backends::NetworkId, bool),
    SetAudioDefault(u32),
    RemoveBluetoothDevice(crate::backends::BluetoothDeviceId),
    LaunchApplication(String),
    SystemPowerAction(String),
}

#[derive(PartialEq, Eq, Debug)]
pub enum Focus {
    Sidebar,
    Content,
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
    pub networks: Vec<crate::backends::Network>,
    pub bluetooth_devices: Vec<crate::backends::BluetoothDevice>,
    pub bluetooth_powered: bool,
    pub power_info: Option<crate::backends::PowerInfo>,
    pub audio_sinks: Vec<crate::backends::AudioDevice>,
    pub audio_sources: Vec<crate::backends::AudioDevice>,
    pub audio_streams: Vec<crate::backends::AudioStream>,
    pub services: Vec<crate::backends::ServiceInfo>,
    pub monitors: Vec<crate::backends::Monitor>,
    pub appearance_info: Option<crate::backends::AppearanceInfo>,
    pub applications: Vec<crate::backends::AppEntry>,
    pub cmd_tx: Option<mpsc::Sender<BackendCommand>>,
    pub notifications: Vec<String>,
    pub notification_timer: usize,
    pub is_searching: bool,
    pub search_query: String,
    pub search_results: Vec<SearchResult>,
    pub search_selected_idx: usize,
    pub confirm_action: Option<(String, BackendCommand)>,
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
                "Services".to_string(),
                "System".to_string(),
            ],
            system_info: None,
            wifi_enabled: true,
            networks: vec![],
            confirm_action: None,
            capabilities: crate::platform::PlatformCapabilities::detect(
                true, true, true, true, true, true, true,
            ),
            bluetooth_devices: vec![],
            bluetooth_powered: false,
            power_info: None,
            audio_sinks: vec![],
            audio_sources: vec![],
            audio_streams: vec![],
            services: vec![],
            monitors: vec![],
            appearance_info: None,
            applications: vec![],
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
                    target_item_idx: i,
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
                title: "Dark Mode / Theming".to_string(),
                category: "Appearance".to_string(),
                description: "System visual style and color scheme".to_string(),
                target_item_idx: 0,
                score: app_score,
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

        // Search System Power Actions
        let sys_actions = [
            (
                "Suspend",
                "Suspend system to RAM (sleep mode)",
                "sleep standby",
                0,
            ),
            (
                "Hibernate",
                "Hibernate system state to disk",
                "hibernate disk",
                1,
            ),
            ("Reboot", "Restart the computer", "restart reboot", 2),
            (
                "Power Off",
                "Shut down the computer system",
                "shutdown poweroff halt power off",
                3,
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
                        self.networks.len() + 1
                    } else {
                        1
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
                "Appearance" => 1,
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
                }
            }
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Esc => {
                if self.focus == Focus::Content {
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
                                    self.networks.len() + 1
                                } else {
                                    1
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
                            "Display" => self.monitors.len(),
                            "Applications" => self.applications.len(),
                            "Appearance" => 1,
                            "System" => 4,
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
                            } else if self.selected_item <= self.networks.len() {
                                let net = &self.networks[self.selected_item - 1];
                                if let Some(tx) = &self.cmd_tx {
                                    let _ = tx.try_send(BackendCommand::ConnectNetwork(
                                        net.id.clone(),
                                        !net.connected,
                                    ));
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
                                            let _ =
                                                tx.try_send(BackendCommand::SetAudioDefault(id));
                                        }
                                    }
                                } else {
                                    if let Some(tx) = &self.cmd_tx {
                                        let _ = tx
                                            .try_send(BackendCommand::ToggleAudioMute(id, is_sink));
                                    }
                                }
                            }
                        } else if cat == "Appearance" && self.selected_item == 0 {
                            if let Some(tx) = &self.cmd_tx {
                                let _ = tx.try_send(BackendCommand::ToggleColorScheme);
                            }
                        } else if cat == "Power" && self.selected_item == 0 {
                            if let Some(tx) = &self.cmd_tx {
                                let _ = tx.try_send(BackendCommand::CyclePowerProfile);
                            }
                        } else if cat == "Applications"
                            && self.selected_item < self.applications.len()
                        {
                            let app = &self.applications[self.selected_item];
                            if let Some(tx) = &self.cmd_tx {
                                let _ = tx
                                    .try_send(BackendCommand::LaunchApplication(app.exec.clone()));
                            }
                        } else if cat == "System" && is_enter {
                            let action = match self.selected_item {
                                0 => "suspend",
                                1 => "hibernate",
                                2 => "reboot",
                                3 => "poweroff",
                                _ => "",
                            };
                            if !action.is_empty() {
                                self.confirm_action = Some((
                                    format!("Are you sure you want to {}?", action),
                                    BackendCommand::SystemPowerAction(action.to_string()),
                                ));
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
                            let dev = &self.bluetooth_devices[self.selected_item - 1];
                            self.confirm_action = Some((
                                format!("Forget Bluetooth device '{}'?", dev.name),
                                BackendCommand::RemoveBluetoothDevice(dev.id.clone()),
                            ));
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
                        } else if cat == "Display" && self.selected_item < self.monitors.len() {
                            let m = &self.monitors[self.selected_item];
                            if !m.supported_modes.is_empty() {
                                let _current_res =
                                    format!("{}x{}@{:.2}Hz", m.width, m.height, m.refresh_rate);
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
                                        if let (Ok(w), Ok(h)) =
                                            (parts[0].parse::<i32>(), parts[1].parse::<i32>())
                                        {
                                            let refresh =
                                                if let Some(hz) = next_mode.split('@').nth(1) {
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
    let sys_backend = RealSystemBackend::new();

    let net_backend = crate::backends::network::NetworkManagerBackend::new()
        .await
        .ok();
    let bt_backend = crate::backends::bluetooth::BlueZBackend::new().await.ok();
    let power_backend = crate::backends::power::UPowerBackend::new().await.ok();
    let audio_backend = crate::backends::audio::WpctlBackend::new();
    let services_backend = crate::backends::systemd::SystemdBackend::new().await.ok();

    let display_backend = {
        let is_hyprland = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok();
        let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
        if is_hyprland && session_type == "wayland" {
            crate::backends::display::HyprlandBackend::new().ok()
        } else {
            None
        }
    };

    let appearance_backend = crate::backends::appearance::GsettingsBackend::new();
    let apps_backend = crate::backends::applications::DesktopEntryBackend::new();

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
    let sys_backend_for_cmd = crate::backends::system::RealSystemBackend::new();
    let display_backend_for_cmd = {
        let is_hyprland = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok();
        let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
        if is_hyprland && session_type == "wayland" {
            crate::backends::display::HyprlandBackend::new().ok()
        } else {
            None
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
    );
    let _ = tx_backend
        .send(AppEvent::UpdateCapabilities(Box::new(capabilities)))
        .await;

    tokio::spawn(async move {
        let mut last_poll = tokio::time::Instant::now();
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

                                let (mutation_ok, verified, final_state, _mutation_error) = execute_transaction!(
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
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(format!("Failed to {} service {}.", if start { "start" } else { "stop" }, name))).await;
                                }

                                if let Ok(svcs) = final_state {
                                    let _ = tx_cmd_resp.send(AppEvent::UpdateServices(svcs)).await;
                                }
                            }
                        }
                        BackendCommand::ToggleBluetoothPower(target_state) => {
                            if let Some(bt) = &bt_backend_for_cmd {
                                let (mutation_ok, verified, final_state, _mutation_error) = execute_transaction!(
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
                                    let _ = tx_cmd_resp.send(AppEvent::Notification("Failed to toggle Bluetooth.".to_string())).await;
                                }

                                if let Ok(adapter) = final_state {
                                    let _ = tx_cmd_resp.send(AppEvent::UpdateBluetoothAdapter(adapter)).await;
                                }
                            }
                        }
                        BackendCommand::ConnectBluetooth(id, connect) => {
                            if let Some(bt) = &bt_backend_for_cmd {
                                let mutate = if connect { bt.connect_device(&id) } else { bt.disconnect_device(&id) };
                                let (mutation_ok, verified, final_state, _mutation_error) = execute_transaction!(
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
                                    let _ = tx_cmd_resp.send(AppEvent::Notification(format!("Failed to {} Bluetooth device.", if connect { "connect" } else { "disconnect" }))).await;
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

                            let (mutation_ok, verified, _final_state, _mutation_error) = execute_transaction!(
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
                                let _ = tx_cmd_resp
                                    .send(AppEvent::Notification(format!(
                                        "Failed to toggle mute for audio device {}",
                                        id
                                    )))
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
                        BackendCommand::SetAudioVolume(id, vol) => {
                            let (mutation_ok, verified, _final_state, _mutation_error) = execute_transaction!(
                                audio_backend_for_cmd.set_volume(id, vol),
                                audio_backend_for_cmd.get_volume(id),
                                |(cur_vol, _): &(f64, bool)| (*cur_vol - vol).abs() < 0.03,
                                10, 50
                            );
                            if !mutation_ok {
                                let _ = tx_cmd_resp
                                    .send(AppEvent::Notification(format!(
                                        "Failed to set volume for device {}",
                                        id
                                    )))
                                    .await;
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
                        BackendCommand::SetDisplayResolution(name, width, height, refresh) => {
                            if let Some(disp) = &display_backend_for_cmd {
                                let (mutation_ok, verified, final_state, _mutation_error) = execute_transaction!(
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
                                    let _ = tx_cmd_resp.send(AppEvent::Notification("Failed to set display resolution.".to_string())).await;
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
                                    let (mutation_ok, verified, final_state, _mutation_error) = execute_transaction!(
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
                                        let _ = tx_cmd_resp.send(AppEvent::Notification("Failed to set color scheme.".to_string())).await;
                                    }

                                    if let Ok(actual_info) = final_state {
                                        let _ = tx_cmd_resp.send(AppEvent::UpdateAppearance(actual_info)).await;
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
                                        let (mutation_ok, verified, final_state, _mutation_error) = execute_transaction!(
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
                                            let _ = tx_cmd_resp.send(AppEvent::Notification("Failed to set power profile.".to_string())).await;
                                        }

                                        if let Ok(actual_info) = final_state {
                                            let _ = tx_cmd_resp.send(AppEvent::UpdatePower(actual_info)).await;
                                        }
                                    }
                                }
                            }
                        }
                        BackendCommand::ToggleWifi(target_state) => {
                            if let Some(net) = &network_backend_for_cmd {
                                let (mutation_ok, verified, final_state, _mutation_error) = execute_transaction!(
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
                                    let _ = tx_cmd_resp.send(AppEvent::Notification("Failed to execute Wi-Fi command.".to_string())).await;
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
                        BackendCommand::SetAudioDefault(id) => {
                            let (mutation_ok, verified, final_state, _mutation_error) = execute_transaction!(
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
                                            "Default audio device updated.".to_string(),
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
                                let _ = tx_cmd_resp
                                    .send(AppEvent::Notification(format!(
                                        "Failed to set default device to {}",
                                        id
                                    )))
                                    .await;
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
                        }
                        BackendCommand::RemoveBluetoothDevice(id) => {
                            if let Some(bt) = &bt_backend_for_cmd {
                                let (mutation_ok, verified, final_state, _mutation_error) = execute_transaction!(
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
                                    let _ = tx_cmd_resp
                                        .send(AppEvent::Notification(
                                            "Failed to remove Bluetooth device.".to_string(),
                                        ))
                                        .await;
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
                                let _ = tx_cmd_resp.send(AppEvent::Notification(format!("Power action failed: {}", e))).await;
                            } else {
                                let _ = tx_cmd_resp.send(AppEvent::Notification(format!("Executing {}...", action))).await;
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
                            let _ = tx_backend.send(AppEvent::UpdateNetworks(enabled, nets)).await;
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
                    }
                    if let Some(appr) = &appearance_backend {
                        if let Ok(info) = appr.get_info().await {
                            let _ = tx_backend.send(AppEvent::UpdateAppearance(info)).await;
                        }
                    }
                    if let Ok(apps) = apps_backend.get_applications().await {
                        let _ = tx_backend.send(AppEvent::UpdateApplications(apps)).await;
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
                AppEvent::UpdateAppearance(info) => app_lock.appearance_info = Some(info),
                AppEvent::UpdateApplications(apps) => app_lock.applications = apps,
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

        // Navigate through all 4 power actions
        for _ in 0..10 {
            app.handle_key(press(KeyCode::Down));
        }
        // Must stay at 3 (max index for 4 items)
        assert_eq!(
            app.selected_item, 3,
            "System category should have max 4 items (0-3)"
        );
    }

    #[test]
    fn test_power_action_triggers_confirm_not_immediate() {
        let mut app = App::new();
        let sys_idx = app.categories.iter().position(|c| c == "System").unwrap();
        app.selected_category = sys_idx;
        app.focus = Focus::Content;
        app.selected_item = 3; // "Power Off"

        // Enter should set confirm dialog, NOT directly execute
        app.handle_key(press(KeyCode::Enter));
        assert!(
            app.confirm_action.is_some(),
            "Power off must require confirmation"
        );
        assert!(!app.should_quit, "App must not quit from power action");
    }

    #[test]
    fn test_visible_categories_returns_all() {
        let app = App::new();
        let vis = app.visible_categories();
        assert!(vis.contains(&"Network".to_string()));
        assert!(vis.contains(&"System".to_string()));
        assert_eq!(vis.len(), 9, "Should have 9 categories");
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
        assert_eq!(app.search_results[0].target_item_idx, 3);

        app.search_query = "reboot".to_string();
        app.update_search_results();
        assert_eq!(app.search_results[0].title, "Reboot");
        assert_eq!(app.search_results[0].category, "System");
        assert_eq!(app.search_results[0].target_item_idx, 2);
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
        assert_eq!(app.selected_item, 2, "Should target Reboot item index");
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
}
