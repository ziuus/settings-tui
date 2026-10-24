use super::{AppearanceBackend, AppearanceInfo};
use anyhow::Result;
use async_trait::async_trait;
use std::path::{Path, PathBuf};
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

    fn scan_theme_dirs(
        dirs: &[std::path::PathBuf],
        require_index: bool,
        require_cursors: bool,
    ) -> Vec<String> {
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

    fn get_home_dir() -> PathBuf {
        if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home)
        } else {
            PathBuf::from(".")
        }
    }

    fn sync_terminals(is_dark: bool) {
        let home = Self::get_home_dir();

        // 1. Foot Terminal
        let foot_dir = home.join(".config/foot");
        let foot_ini = foot_dir.join("foot.ini");
        if foot_dir.exists() || foot_ini.exists() {
            let _ = std::fs::create_dir_all(&foot_dir);
            let theme_ini = foot_dir.join("theme.ini");

            let theme_path = if is_dark {
                if Path::new("/usr/share/foot/themes/catppuccin-mocha").exists() {
                    Some("/usr/share/foot/themes/catppuccin-mocha")
                } else if Path::new("/usr/share/foot/themes/gruvbox-dark").exists() {
                    Some("/usr/share/foot/themes/gruvbox-dark")
                } else {
                    None
                }
            } else if Path::new("/usr/share/foot/themes/catppuccin-latte").exists() {
                Some("/usr/share/foot/themes/catppuccin-latte")
            } else if Path::new("/usr/share/foot/themes/gruvbox-light").exists() {
                Some("/usr/share/foot/themes/gruvbox-light")
            } else {
                None
            };

            let theme_content = if let Some(path) = theme_path {
                format!("include={}\n", path)
            } else if is_dark {
                "[colors-dark]\nbackground=1e1e2e\nforeground=cdd6f4\n".to_string()
            } else {
                "[colors-light]\nbackground=eff1f5\nforeground=4c4f69\n".to_string()
            };

            let _ = std::fs::write(&theme_ini, theme_content);

            // Ensure foot.ini includes theme.ini
            if foot_ini.exists() {
                if let Ok(content) = std::fs::read_to_string(&foot_ini) {
                    if !content.contains("theme.ini") {
                        let new_content = format!("include=~/.config/foot/theme.ini\n{}", content);
                        let _ = std::fs::write(&foot_ini, new_content);
                    }
                }
            } else {
                let _ = std::fs::write(&foot_ini, "include=~/.config/foot/theme.ini\n");
            }

            // Signal foot to switch theme live
            // SIGUSR1 = switch to dark, SIGUSR2 = switch to light
            let sig = if is_dark { "-SIGUSR1" } else { "-SIGUSR2" };
            let _ = Command::new("pkill").arg(sig).arg("foot").output();
        }

        // 2. Kitty Terminal
        let kitty_dir = home.join(".config/kitty");
        if kitty_dir.exists() {
            let theme_conf = kitty_dir.join("theme.conf");
            let kitty_theme = if is_dark {
                // Catppuccin Mocha
                r#"# Catppuccin Mocha (Dark)
foreground              #CDD6F4
background              #1E1E2E
selection_foreground    #1E1E2E
selection_background    #F5E0DC
cursor                  #F5E0DC
cursor_text_color       #1E1E2E
url_color               #B4BEFE
color0 #45475A
color8 #585B70
color1 #F38BA8
color9 #F38BA8
color2 #A6E3A1
color10 #A6E3A1
color3 #F9E2AF
color11 #F9E2AF
color4 #89B4FA
color12 #89B4FA
color5 #F5C2E7
color13 #F5C2E7
color6 #94E2D5
color14 #94E2D5
color7 #BAC2DE
color15 #A6ADC8
"#
            } else {
                // Catppuccin Latte
                r#"# Catppuccin Latte (Light)
foreground              #4C4F69
background              #EFF1F5
selection_foreground    #4C4F69
selection_background    #CC0000
cursor                  #DC8A78
cursor_text_color       #EFF1F5
url_color               #1E66F5
color0 #5C5F77
color8 #6C6F85
color1 #D20F39
color9 #D20F39
color2 #40A02B
color10 #40A02B
color3 #DF8E1D
color11 #DF8E1D
color4 #1E66F5
color12 #1E66F5
color5 #EA76CB
color13 #EA76CB
color6 #179299
color14 #179299
color7 #ACB0BE
color15 #BCC0CC
"#
            };

            let _ = std::fs::write(&theme_conf, kitty_theme);

            // Signal kitty processes to live-reload configuration
            let _ = Command::new("pkill").arg("-SIGUSR1").arg("kitty").output();
        }

        // 3. Alacritty
        let alacritty_dir = home.join(".config/alacritty");
        let alacritty_toml = alacritty_dir.join("alacritty.toml");
        if alacritty_toml.exists() {
            if let Ok(content) = std::fs::read_to_string(&alacritty_toml) {
                let target_mode = if is_dark { "dark" } else { "light" };
                let old_mode = if is_dark { "light" } else { "dark" };
                if content.contains(&format!("{}.toml", old_mode)) {
                    let updated = content.replace(
                        &format!("{}.toml", old_mode),
                        &format!("{}.toml", target_mode),
                    );
                    let _ = std::fs::write(&alacritty_toml, updated);
                }
            }
        }

        // 4. Ghostty
        let ghostty_config = home.join(".config/ghostty/config");
        if ghostty_config.exists() {
            if let Ok(content) = std::fs::read_to_string(&ghostty_config) {
                let mut lines = Vec::new();
                let mut replaced = false;
                for line in content.lines() {
                    if line.trim_start().starts_with("theme =") {
                        let new_theme = if is_dark {
                            "catppuccin-mocha"
                        } else {
                            "catppuccin-latte"
                        };
                        lines.push(format!("theme = {}", new_theme));
                        replaced = true;
                    } else {
                        lines.push(line.to_string());
                    }
                }
                if !replaced {
                    let new_theme = if is_dark {
                        "catppuccin-mocha"
                    } else {
                        "catppuccin-latte"
                    };
                    lines.push(format!("theme = {}", new_theme));
                }
                let _ = std::fs::write(&ghostty_config, lines.join("\n") + "\n");
            }
        }
    }

    fn sync_gtk_files(is_dark: bool) {
        let home = Self::get_home_dir();
        let prefer_dark_val = if is_dark { "1" } else { "0" };

        for dir_name in &["gtk-3.0", "gtk-4.0"] {
            let gtk_dir = home.join(".config").join(dir_name);
            let _ = std::fs::create_dir_all(&gtk_dir);
            let ini_path = gtk_dir.join("settings.ini");

            let mut lines: Vec<String> = if ini_path.exists() {
                std::fs::read_to_string(&ini_path)
                    .unwrap_or_default()
                    .lines()
                    .map(|s| s.to_string())
                    .collect()
            } else {
                vec!["[Settings]".to_string()]
            };

            let mut has_prefer_dark = false;
            for line in lines.iter_mut() {
                if line
                    .trim_start()
                    .starts_with("gtk-application-prefer-dark-theme")
                {
                    *line = format!("gtk-application-prefer-dark-theme={}", prefer_dark_val);
                    has_prefer_dark = true;
                }
            }

            if !has_prefer_dark {
                if let Some(pos) = lines.iter().position(|l| l.trim() == "[Settings]") {
                    lines.insert(
                        pos + 1,
                        format!("gtk-application-prefer-dark-theme={}", prefer_dark_val),
                    );
                } else {
                    lines.push("[Settings]".to_string());
                    lines.push(format!(
                        "gtk-application-prefer-dark-theme={}",
                        prefer_dark_val
                    ));
                }
            }

            let _ = std::fs::write(&ini_path, lines.join("\n") + "\n");
        }
    }

    fn sync_qt_and_kde(is_dark: bool) {
        let home = Self::get_home_dir();
        let kdeglobals = home.join(".config/kdeglobals");
        let color_scheme = if is_dark { "BreezeDark" } else { "BreezeLight" };

        if kdeglobals.exists() {
            if let Ok(content) = std::fs::read_to_string(&kdeglobals) {
                let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                let mut in_general = false;
                let mut has_color_scheme = false;

                for line in lines.iter_mut() {
                    let trimmed = line.trim();
                    if trimmed.starts_with('[') && trimmed.ends_with(']') {
                        in_general = trimmed == "[General]";
                    } else if in_general && trimmed.starts_with("ColorScheme=") {
                        *line = format!("ColorScheme={}", color_scheme);
                        has_color_scheme = true;
                    }
                }

                if !has_color_scheme {
                    if let Some(pos) = lines.iter().position(|l| l.trim() == "[General]") {
                        lines.insert(pos + 1, format!("ColorScheme={}", color_scheme));
                    } else {
                        lines.push("[General]".to_string());
                        lines.push(format!("ColorScheme={}", color_scheme));
                    }
                }

                let _ = std::fs::write(&kdeglobals, lines.join("\n") + "\n");
            }
        }

        // Run kwriteconfig6 / kwriteconfig5 if available
        for bin in &["kwriteconfig6", "kwriteconfig5"] {
            let _ = Command::new(bin)
                .args([
                    "--file",
                    "kdeglobals",
                    "--group",
                    "General",
                    "--key",
                    "ColorScheme",
                    color_scheme,
                ])
                .output();
        }
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
        if !cursor_theme.is_empty()
            && cursor_theme != "Unknown"
            && !available_cursor_themes.contains(&cursor_theme)
        {
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
        let is_dark = scheme.contains("dark");

        // 1. Broadcast to GNOME / XDG Desktop Portal / Web Browsers / Electron
        let output = Command::new("gsettings")
            .arg("set")
            .arg("org.gnome.desktop.interface")
            .arg("color-scheme")
            .arg(scheme)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let msg = stderr
                .lines()
                .next()
                .unwrap_or("Failed to set color scheme")
                .trim();
            return Err(anyhow::anyhow!("{}", msg));
        }

        // 2. Automatically sync matching GTK theme variant if currently using Adwaita
        let current_theme = Self::get_key("org.gnome.desktop.interface", "gtk-theme");
        if is_dark && current_theme == "Adwaita" {
            let _ = Command::new("gsettings")
                .args([
                    "set",
                    "org.gnome.desktop.interface",
                    "gtk-theme",
                    "Adwaita-dark",
                ])
                .output();
        } else if !is_dark && current_theme == "Adwaita-dark" {
            let _ = Command::new("gsettings")
                .args(["set", "org.gnome.desktop.interface", "gtk-theme", "Adwaita"])
                .output();
        }

        // 3. Sync GTK 3 & GTK 4 settings.ini (ensures legacy/file-based GTK apps switch immediately)
        Self::sync_gtk_files(is_dark);

        // 4. Sync Qt & KDE Plasma configuration
        Self::sync_qt_and_kde(is_dark);

        // 5. Sync Terminal Emulators (Foot, Kitty, Alacritty, Ghostty) with live signal reload
        Self::sync_terminals(is_dark);

        Ok(())
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
            std::fs::canonicalize(path_obj)
                .unwrap_or(path_obj.to_path_buf())
                .display()
                .to_string()
        };
        let uri = if path_str.starts_with("file://") {
            path_str.to_string()
        } else {
            format!("file://{}", path_str)
        };
        let _ = std::process::Command::new("gsettings")
            .args(["set", "org.gnome.desktop.background", "picture-uri", &uri])
            .output();
        let _ = std::process::Command::new("gsettings")
            .args([
                "set",
                "org.gnome.desktop.background",
                "picture-uri-dark",
                &uri,
            ])
            .output();

        // Also support hyprpaper
        if let Ok(mut cmd) = std::process::Command::new("hyprctl")
            .args(["hyprpaper", "wallpaper", &format!(",{}", path_str)])
            .spawn()
        {
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
        let fake_dirs = vec![std::path::PathBuf::from(
            "/nonexistent/directory/path/themes",
        )];
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
