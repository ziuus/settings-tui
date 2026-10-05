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

    fn scan_theme_dirs(dirs: &[std::path::PathBuf], require_index: bool, require_cursors: bool) -> Vec<String> {
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
                                && (!require_cursors || path.join("cursors").exists())
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

        let mut available_gtk_themes = Self::scan_theme_dirs(&gtk_dirs, false, false);
        if !gtk_theme.is_empty()
            && gtk_theme != "Unknown"
            && !available_gtk_themes.contains(&gtk_theme)
        {
            available_gtk_themes.insert(0, gtk_theme.clone());
        }

        let mut available_cursor_themes = Self::scan_theme_dirs(&icon_dirs, false, true);
        let cursor_theme = Self::get_key("org.gnome.desktop.interface", "cursor-theme");
        if !cursor_theme.is_empty() && cursor_theme != "Unknown" && !available_cursor_themes.contains(&cursor_theme) {
            available_cursor_themes.insert(0, cursor_theme.clone());
        }
        let mut available_icon_themes = Self::scan_theme_dirs(&icon_dirs, true, false);
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
            cursor_theme,
            font_name: Self::get_key("org.gnome.desktop.interface", "font-name"),
            wallpaper: Some(Self::get_key("org.gnome.desktop.background", "picture-uri")),
            available_gtk_themes,
            available_icon_themes,
            available_cursor_themes,
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

    async fn set_cursor_theme(&self, theme: &str) -> Result<()> {
        let output = Command::new("gsettings")
            .arg("set")
            .arg("org.gnome.desktop.interface")
            .arg("cursor-theme")
            .arg(theme)
            .output()?;
        if output.status.success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to set cursor theme"))
        }
    }

    async fn set_font_name(&self, font: &str) -> Result<()> {
        let output = Command::new("gsettings")
            .arg("set")
            .arg("org.gnome.desktop.interface")
            .arg("font-name")
            .arg(font)
            .output()?;
        if output.status.success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to set font name"))
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

    async fn set_wallpaper(&self, path: &str) -> Result<()> {
        let path_obj = std::path::Path::new(path);
        let path_str = if path_obj.is_absolute() {
            path.to_string()
        } else {
            std::fs::canonicalize(path_obj).unwrap_or(path_obj.to_path_buf()).display().to_string()
        };
        let uri = if path_str.starts_with("file://") { path_str.to_string() } else { format!("file://{}", path_str) };
        let _ = std::process::Command::new("gsettings")
            .args(["set", "org.gnome.desktop.background", "picture-uri", &uri])
            .output();
        let _ = std::process::Command::new("gsettings")
            .args(["set", "org.gnome.desktop.background", "picture-uri-dark", &uri])
            .output();
        
        // Also support hyprpaper
        if let Ok(mut cmd) = std::process::Command::new("hyprctl")
            .args(["hyprpaper", "wallpaper", &format!(",{}", path_str)])
            .spawn() {
                let _ = cmd.wait();
        }
        Ok(())
    }

}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_theme_dirs_nonexistent() {
        let fake_dirs = vec![std::path::PathBuf::from("/nonexistent/directory/path/themes")];
        let themes = GsettingsBackend::scan_theme_dirs(&fake_dirs, false, false);
        assert!(themes.is_empty());
    }

    #[test]
    fn test_dummy_123() {
        let fake_dirs = vec![std::path::PathBuf::from(
            "/nonexistent/directory/path/themes",
        )];
        let themes = GsettingsBackend::scan_theme_dirs(&fake_dirs, false, false);
        assert!(themes.is_empty());
    }

    #[test]
    fn test_scan_theme_dirs_system() {
        let sys_dirs = vec![std::path::PathBuf::from("/usr/share/themes")];
        let themes = GsettingsBackend::scan_theme_dirs(&sys_dirs, false, false);
        // On Linux /usr/share/themes exists or is empty, shouldn't crash
        for t in &themes {
            assert!(!t.starts_with('.'));
        }
    }
}
