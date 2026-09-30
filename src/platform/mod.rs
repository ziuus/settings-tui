use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityStatus {
    /// Fully supported and mutable by the user without elevation.
    Mutable,
    /// Telemetry and status inspection available, but mutation is read-only.
    ReadOnly(String),
    /// Mutation is available, but may require Polkit privilege elevation.
    RequiresPermission(String),
    /// Partially implemented (some features work, others unmanaged/restricted).
    Partial(String),
    /// Subsystem or compositor unsupported under current desktop/environment.
    Unsupported(String),
    /// Subsystem daemon or socket not found on the system.
    Unavailable(String),
}

#[allow(dead_code)]
impl CapabilityStatus {
    pub fn is_mutable(&self) -> bool {
        matches!(
            self,
            CapabilityStatus::Mutable | CapabilityStatus::RequiresPermission(_)
        )
    }

    pub fn is_available(&self) -> bool {
        !matches!(
            self,
            CapabilityStatus::Unavailable(_) | CapabilityStatus::Unsupported(_)
        )
    }

    pub fn status_text(&self) -> &str {
        match self {
            CapabilityStatus::Mutable => "Mutable (Full)",
            CapabilityStatus::ReadOnly(s) => s.as_str(),
            CapabilityStatus::RequiresPermission(s) => s.as_str(),
            CapabilityStatus::Partial(s) => s.as_str(),
            CapabilityStatus::Unsupported(s) => s.as_str(),
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
            CapabilityStatus::Partial(
                "Wi-Fi scanning and connection supported (VPN/DNS unmanaged)".into(),
            )
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
            CapabilityStatus::Partial(
                "Power toggle and device connect/forget supported (PIN pairing unmanaged)".into(),
            )
        } else {
            CapabilityStatus::Unavailable(
                "BlueZ Bluetooth daemon not available on system bus".into(),
            )
        };

        let power = if has_power_backend {
            CapabilityStatus::Mutable
        } else {
            CapabilityStatus::Unavailable("UPower daemon not found on system bus".into())
        };

        let sound = if has_audio_backend {
            CapabilityStatus::Mutable
        } else if env.has_pipewire_socket {
            CapabilityStatus::Partial(
                "PipeWire socket active but wpctl/pw-dump query failed".into(),
            )
        } else {
            CapabilityStatus::Unavailable("PipeWire audio server not running".into())
        };

        let display = if has_display_backend {
            CapabilityStatus::Partial(
                "Resolution and refresh rate mutable via hyprctl (scaling read-only)".into(),
            )
        } else if env.session_type == "wayland" {
            CapabilityStatus::Unsupported(format!(
                "Display management unsupported under '{}' (Hyprland hyprctl required)",
                env.desktop
            ))
        } else if env.session_type == "x11" {
            CapabilityStatus::Unsupported(
                "Display management under X11 unsupported (xrandr required)".into(),
            )
        } else {
            CapabilityStatus::Unavailable(
                "No active Wayland or X11 display session detected".into(),
            )
        };

        let appearance = if has_appearance_backend {
            CapabilityStatus::Partial(
                "Color scheme toggle supported (font and icon themes read-only)".into(),
            )
        } else {
            CapabilityStatus::Unavailable("gsettings binary not found in PATH".into())
        };

        let applications = CapabilityStatus::Mutable;

        let services = if has_services_backend {
            CapabilityStatus::RequiresPermission(
                "Service state mutations require Polkit elevation".into(),
            )
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
            CapabilityStatus::RequiresPermission(
                "Power actions require active logind session / Polkit".into(),
            )
        } else {
            CapabilityStatus::ReadOnly(
                "Read-only telemetry: systemctl power actions require systemd-logind".into(),
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
            _ => &CapabilityStatus::Mutable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_capabilities_all_supported() {
        let caps = PlatformCapabilities::detect(true, true, true, true, true, true, true);
        assert!(caps.applications.is_available());
        assert!(caps.applications.is_mutable());
        assert_eq!(
            caps.for_category("Applications"),
            &CapabilityStatus::Mutable
        );
        assert_eq!(caps.applications.status_text(), "Mutable (Full)");
    }

    #[test]
    fn test_platform_capabilities_missing_network_manager() {
        let caps = PlatformCapabilities::detect(false, true, true, true, true, true, true);
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
        assert!(matches!(
            caps.display,
            CapabilityStatus::Unsupported(_) | CapabilityStatus::Unavailable(_)
        ));
    }

    #[test]
    fn test_platform_capabilities_missing_services() {
        let caps = PlatformCapabilities::detect(true, true, true, true, true, true, false);
        assert!(matches!(
            caps.services,
            CapabilityStatus::Partial(_) | CapabilityStatus::Unavailable(_)
        ));
    }

    #[test]
    fn test_services_and_system_require_permission() {
        let caps = PlatformCapabilities::detect(true, true, true, true, true, true, true);
        assert!(matches!(
            caps.services,
            CapabilityStatus::RequiresPermission(_)
        ));
        assert!(caps.services.is_mutable());
    }
}
