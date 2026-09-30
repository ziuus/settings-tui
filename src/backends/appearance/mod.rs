use super::{AppearanceBackend, AppearanceInfo};
use anyhow::Result;
use async_trait::async_trait;
use std::process::Command;

pub struct GsettingsBackend;

impl GsettingsBackend {
    pub fn new() -> Option<Self> {
        match Command::new("gsettings").arg("help").output() {
            Ok(output) if output.status.success() => Some(Self),
            _ => None,
        }
    }

    fn get_key(schema: &str, key: &str) -> String {
        if let Ok(output) = Command::new("gsettings")
            .arg("get")
            .arg(schema)
            .arg(key)
            .output()
        {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            // gsettings returns values in quotes like 'Adwaita-dark'
            s.trim_matches('\'').to_string()
        } else {
            "Unknown".to_string()
        }
    }
}

#[async_trait]
impl AppearanceBackend for GsettingsBackend {
    async fn get_info(&self) -> Result<AppearanceInfo> {
        Ok(AppearanceInfo {
            color_scheme: Self::get_key("org.gnome.desktop.interface", "color-scheme"),
            gtk_theme: Self::get_key("org.gnome.desktop.interface", "gtk-theme"),
            icon_theme: Self::get_key("org.gnome.desktop.interface", "icon-theme"),
            cursor_theme: Self::get_key("org.gnome.desktop.interface", "cursor-theme"),
            font_name: Self::get_key("org.gnome.desktop.interface", "font-name"),
        })
    }

    async fn set_color_scheme(&self, scheme: &str) -> Result<()> {
        let status = Command::new("gsettings")
            .arg("set")
            .arg("org.gnome.desktop.interface")
            .arg("color-scheme")
            .arg(scheme)
            .status()?;

        if status.success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("gsettings set color-scheme failed"))
        }
    }
}
