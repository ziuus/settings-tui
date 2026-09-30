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

    fn scan_theme_dirs(dirs: &[std::path::PathBuf], require_index: bool) -> Vec<String> {
        let mut themes = Vec::new();
        for dir in dirs {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if let Ok(ft) = entry.file_type() {
                        if ft.is_dir() {
                            let path = entry.path();
                            let name = entry.file_name().to_string_lossy().to_string();
                            if !name.starts_with('.')
                                && (!require_index || path.join("index.theme").exists())
                                && !themes.contains(&name)
                            {
                                themes.push(name);
                            }
                        }
                    }
                }
            }
        }
        themes.sort();
        themes
    }
}

#[async_trait]
impl AppearanceBackend for GsettingsBackend {
    async fn get_info(&self) -> Result<AppearanceInfo> {
        let gtk_theme = Self::get_key("org.gnome.desktop.interface", "gtk-theme");
        let icon_theme = Self::get_key("org.gnome.desktop.interface", "icon-theme");

        let home = std::env::var("HOME").unwrap_or_default();
        let home_path = std::path::Path::new(&home);

        let gtk_dirs = vec![
            std::path::PathBuf::from("/usr/share/themes"),
            std::path::PathBuf::from("/usr/local/share/themes"),
            home_path.join(".themes"),
            home_path.join(".local/share/themes"),
        ];

        let icon_dirs = vec![
            std::path::PathBuf::from("/usr/share/icons"),
            std::path::PathBuf::from("/usr/local/share/icons"),
            home_path.join(".icons"),
            home_path.join(".local/share/icons"),
        ];

        let mut available_gtk_themes = Self::scan_theme_dirs(&gtk_dirs, false);
        if !gtk_theme.is_empty()
            && gtk_theme != "Unknown"
            && !available_gtk_themes.contains(&gtk_theme)
        {
            available_gtk_themes.insert(0, gtk_theme.clone());
        }

        let mut available_icon_themes = Self::scan_theme_dirs(&icon_dirs, true);
        if !icon_theme.is_empty()
            && icon_theme != "Unknown"
            && !available_icon_themes.contains(&icon_theme)
        {
            available_icon_themes.insert(0, icon_theme.clone());
        }

        Ok(AppearanceInfo {
            color_scheme: Self::get_key("org.gnome.desktop.interface", "color-scheme"),
            gtk_theme,
            icon_theme,
            cursor_theme: Self::get_key("org.gnome.desktop.interface", "cursor-theme"),
            font_name: Self::get_key("org.gnome.desktop.interface", "font-name"),
            available_gtk_themes,
            available_icon_themes,
        })
    }

    async fn set_color_scheme(&self, scheme: &str) -> Result<()> {
        let output = Command::new("gsettings")
            .arg("set")
            .arg("org.gnome.desktop.interface")
            .arg("color-scheme")
            .arg(scheme)
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let msg = stderr
                .lines()
                .next()
                .unwrap_or("Failed to set color scheme")
                .trim();
            Err(anyhow::anyhow!("{}", msg))
        }
    }

    async fn set_gtk_theme(&self, theme: &str) -> Result<()> {
        let output = Command::new("gsettings")
            .arg("set")
            .arg("org.gnome.desktop.interface")
            .arg("gtk-theme")
            .arg(theme)
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let msg = stderr
                .lines()
                .next()
                .unwrap_or("Failed to set GTK theme")
                .trim();
            Err(anyhow::anyhow!("{}", msg))
        }
    }

    async fn set_icon_theme(&self, theme: &str) -> Result<()> {
        let output = Command::new("gsettings")
            .arg("set")
            .arg("org.gnome.desktop.interface")
            .arg("icon-theme")
            .arg(theme)
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let msg = stderr
                .lines()
                .next()
                .unwrap_or("Failed to set icon theme")
                .trim();
            Err(anyhow::anyhow!("{}", msg))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_theme_dirs_nonexistent() {
        let fake_dirs = vec![std::path::PathBuf::from(
            "/nonexistent/directory/path/themes",
        )];
        let themes = GsettingsBackend::scan_theme_dirs(&fake_dirs, false);
        assert!(themes.is_empty());
    }

    #[test]
    fn test_scan_theme_dirs_system() {
        let sys_dirs = vec![std::path::PathBuf::from("/usr/share/themes")];
        let themes = GsettingsBackend::scan_theme_dirs(&sys_dirs, false);
        // On Linux /usr/share/themes exists or is empty, shouldn't crash
        for t in &themes {
            assert!(!t.starts_with('.'));
        }
    }
}
