use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityStatus {
    Supported,
    Partial(String),
    Unavailable(String),
}

#[allow(dead_code)]
impl CapabilityStatus {
    pub fn is_supported(&self) -> bool {
        matches!(self, CapabilityStatus::Supported)
    }

    pub fn is_available(&self) -> bool {
        !matches!(self, CapabilityStatus::Unavailable(_))
    }

    pub fn status_text(&self) -> &str {
        match self {
            CapabilityStatus::Supported => "Supported",
            CapabilityStatus::Partial(s) => s.as_str(),
            CapabilityStatus::Unavailable(s) => s.as_str(),
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct EnvironmentInfo {
    pub desktop: String,
    pub session_type: String,
    pub is_hyprland: bool,
    pub is_systemd: bool,
    pub has_pipewire_socket: bool,
    pub has_network_manager_socket: bool,
}

impl EnvironmentInfo {
    pub fn detect() -> Self {
        let desktop =
            std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "Unknown".to_string());
        let session_type =
            std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "Unknown".to_string());
        let is_hyprland = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok();
        let is_systemd = Path::new("/run/systemd/system").exists();

        let runtime_dir =
            std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/run/user/1000".to_string());
        let pipewire_socket = format!("{}/pipewire-0", runtime_dir);
        let has_pipewire_socket = Path::new(&pipewire_socket).exists();

        let has_network_manager_socket = Path::new("/run/NetworkManager").exists()
            || Path::new("/var/run/NetworkManager").exists();

        Self {
            desktop,
            session_type,
            is_hyprland,
            is_systemd,
            has_pipewire_socket,
            has_network_manager_socket,
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PlatformCapabilities {
    pub env: EnvironmentInfo,
    pub network: CapabilityStatus,
    pub bluetooth: CapabilityStatus,
    pub power: CapabilityStatus,
    pub sound: CapabilityStatus,
    pub display: CapabilityStatus,
    pub appearance: CapabilityStatus,
    pub applications: CapabilityStatus,
    pub services: CapabilityStatus,
    pub system: CapabilityStatus,
}

impl PlatformCapabilities {
    pub fn detect(
        has_network_backend: bool,
        has_bluetooth_backend: bool,
        has_power_backend: bool,
        has_audio_backend: bool,
        has_display_backend: bool,
        has_appearance_backend: bool,
        has_services_backend: bool,
    ) -> Self {
        let env = EnvironmentInfo::detect();

        let network = if has_network_backend {
            CapabilityStatus::Supported
        } else if env.has_network_manager_socket {
            CapabilityStatus::Partial(
                "NetworkManager socket found but D-Bus connection failed".into(),
            )
        } else {
            CapabilityStatus::Unavailable(
                "NetworkManager daemon not found (systemd-networkd / iwd unmanaged)".into(),
            )
        };

        let bluetooth = if has_bluetooth_backend {
            CapabilityStatus::Supported
        } else {
            CapabilityStatus::Unavailable(
                "BlueZ Bluetooth daemon not available on system bus".into(),
            )
        };

        let power = if has_power_backend {
            CapabilityStatus::Supported
        } else {
            CapabilityStatus::Unavailable("UPower daemon not found on system bus".into())
        };

        let sound = if has_audio_backend {
            CapabilityStatus::Supported
        } else if env.has_pipewire_socket {
            CapabilityStatus::Partial(
                "PipeWire socket active but wpctl/pw-dump query failed".into(),
            )
        } else {
            CapabilityStatus::Unavailable("PipeWire audio server not running".into())
        };

        let display = if has_display_backend {
            CapabilityStatus::Supported
        } else if env.session_type == "wayland" {
            CapabilityStatus::Unavailable(format!(
                "Display management unavailable under '{}' (Hyprland hyprctl required)",
                env.desktop
            ))
        } else if env.session_type == "x11" {
            CapabilityStatus::Unavailable(
                "Display management under X11 requires xrandr integration".into(),
            )
        } else {
            CapabilityStatus::Unavailable(
                "No active Wayland or X11 display session detected".into(),
            )
        };

        let appearance = if has_appearance_backend {
            CapabilityStatus::Supported
        } else {
            CapabilityStatus::Unavailable("gsettings binary not found in PATH".into())
        };

        let applications = CapabilityStatus::Supported;

        let services = if has_services_backend {
            CapabilityStatus::Supported
        } else if env.is_systemd {
            CapabilityStatus::Partial(
                "systemd active but Systemd1 D-Bus Manager interface unreachable".into(),
            )
        } else {
            CapabilityStatus::Unavailable(
                "Non-systemd init system (systemd required for service management)".into(),
            )
        };

        let system = if env.is_systemd {
            CapabilityStatus::Supported
        } else {
            CapabilityStatus::Partial(
                "Read-only: systemctl power actions require systemd-logind".into(),
            )
        };

        Self {
            env,
            network,
            bluetooth,
            power,
            sound,
            display,
            appearance,
            applications,
            services,
            system,
        }
    }

    pub fn for_category(&self, category: &str) -> &CapabilityStatus {
        match category {
            "Network" => &self.network,
            "Bluetooth" => &self.bluetooth,
            "Power" => &self.power,
            "Sound" => &self.sound,
            "Display" => &self.display,
            "Appearance" => &self.appearance,
            "Applications" => &self.applications,
            "Services" => &self.services,
            "System" => &self.system,
            _ => &CapabilityStatus::Supported,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_capabilities_all_supported() {
        let caps = PlatformCapabilities::detect(true, true, true, true, true, true, true);
        assert!(caps.applications.is_supported());
        assert_eq!(
            caps.for_category("Applications"),
            &CapabilityStatus::Supported
        );
        assert!(caps.applications.is_available());
        assert_eq!(caps.applications.status_text(), "Supported");
    }

    #[test]
    fn test_platform_capabilities_missing_network_manager() {
        let caps = PlatformCapabilities::detect(false, true, true, true, true, true, true);
        assert!(!caps.network.is_supported());
        assert!(matches!(
            caps.for_category("Network"),
            CapabilityStatus::Partial(_) | CapabilityStatus::Unavailable(_)
        ));
    }

    #[test]
    fn test_platform_capabilities_missing_bluez() {
        let caps = PlatformCapabilities::detect(true, false, true, true, true, true, true);
        assert_eq!(
            caps.for_category("Bluetooth"),
            &CapabilityStatus::Unavailable(
                "BlueZ Bluetooth daemon not available on system bus".into()
            )
        );
        assert!(!caps.bluetooth.is_available());
    }

    #[test]
    fn test_platform_capabilities_missing_display() {
        let caps = PlatformCapabilities::detect(true, true, true, true, false, true, true);
        assert!(!caps.display.is_supported());
    }

    #[test]
    fn test_platform_capabilities_missing_services() {
        let caps = PlatformCapabilities::detect(true, true, true, true, true, true, false);
        assert!(!caps.services.is_supported());
    }
}
