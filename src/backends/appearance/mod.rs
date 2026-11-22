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

    pub fn get_home_dir() -> PathBuf {
        if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home)
        } else {
            PathBuf::from(".")
        }
    }

    pub fn get_config_home() -> PathBuf {
        if let Some(cfg) = std::env::var_os("XDG_CONFIG_HOME") {
            if !cfg.is_empty() {
                return PathBuf::from(cfg);
            }
        }
        Self::get_home_dir().join(".config")
    }

    pub fn get_data_home() -> PathBuf {
        if let Some(data) = std::env::var_os("XDG_DATA_HOME") {
            if !data.is_empty() {
                return PathBuf::from(data);
            }
        }
        Self::get_home_dir().join(".local/share")
    }

    pub fn get_data_dirs() -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        if let Some(val) = std::env::var_os("XDG_DATA_DIRS") {
            for p in std::env::split_paths(&val) {
                if !dirs.contains(&p) {
                    dirs.push(p);
                }
            }
        }
        if dirs.is_empty() {
            dirs.push(PathBuf::from("/usr/local/share"));
            dirs.push(PathBuf::from("/usr/share"));
        }
        // NixOS system and user paths
        let nix_sw = PathBuf::from("/run/current-system/sw/share");
        if nix_sw.exists() && !dirs.contains(&nix_sw) {
            dirs.push(nix_sw);
        }
        let nix_user = Self::get_home_dir().join(".nix-profile/share");
        if nix_user.exists() && !dirs.contains(&nix_user) {
            dirs.push(nix_user);
        }
        dirs
    }

    fn is_writable(path: &Path) -> bool {
        let target = if path.is_symlink() {
            std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
        } else {
            path.to_path_buf()
        };
        if target.exists() {
            if let Ok(meta) = std::fs::metadata(&target) {
                if meta.permissions().readonly() {
                    return false;
                }
            }
        }
        if let Some(parent) = target.parent() {
            if parent.exists() {
                if let Ok(meta) = std::fs::metadata(parent) {
                    if meta.permissions().readonly() {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn safe_write(path: &Path, content: &str) {
        if !Self::is_writable(path) {
            return;
        }
        let _ = crate::security::atomic_write(path, content.as_bytes());
    }

    fn find_foot_theme(theme_name: &str) -> Option<PathBuf> {
        for data_dir in Self::get_data_dirs() {
            let p = data_dir.join("foot/themes").join(theme_name);
            if p.exists() {
                return Some(p);
            }
        }
        let default_path = PathBuf::from(format!("/usr/share/foot/themes/{}", theme_name));
        if default_path.exists() {
            return Some(default_path);
        }
        None
    }

    fn sync_terminals(is_dark: bool) {
        let config_home = Self::get_config_home();

        // 1. Foot Terminal
        let foot_dir = config_home.join("foot");
        let foot_ini = foot_dir.join("foot.ini");
        if foot_dir.exists() || foot_ini.exists() {
            let _ = std::fs::create_dir_all(&foot_dir);
            let theme_ini = foot_dir.join("theme.ini");

            let theme_path = if is_dark {
                Self::find_foot_theme("catppuccin-mocha")
                    .or_else(|| Self::find_foot_theme("gruvbox-dark"))
            } else {
                Self::find_foot_theme("catppuccin-latte")
                    .or_else(|| Self::find_foot_theme("gruvbox-light"))
            };

            let theme_content = if let Some(path) = theme_path {
                format!("include={}\n", path.display())
            } else if is_dark {
                "[colors-dark]\nbackground=1e1e2e\nforeground=cdd6f4\n".to_string()
            } else {
                "[colors-light]\nbackground=eff1f5\nforeground=4c4f69\n".to_string()
            };

            Self::safe_write(&theme_ini, &theme_content);

            // Ensure foot.ini includes theme.ini
            let include_line = format!("include={}/theme.ini\n", foot_dir.display());
            if foot_ini.exists() {
                if let Ok(content) = std::fs::read_to_string(&foot_ini) {
                    if !content.contains("theme.ini") {
                        let new_content = format!("{}{}", include_line, content);
                        Self::safe_write(&foot_ini, &new_content);
                    }
                }
            } else {
                Self::safe_write(&foot_ini, &include_line);
            }

            // Signal foot to switch theme live
            // SIGUSR1 = switch to dark, SIGUSR2 = switch to light
            let sig = if is_dark { "-SIGUSR1" } else { "-SIGUSR2" };
            let _ = Command::new("pkill").arg(sig).arg("foot").output();
        }

        // 2. Kitty Terminal
        let kitty_dir = config_home.join("kitty");
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

            Self::safe_write(&theme_conf, kitty_theme);

            // Signal kitty processes to live-reload configuration
            let _ = Command::new("pkill").arg("-SIGUSR1").arg("kitty").output();
        }

        // 3. Alacritty
        let alacritty_dir = config_home.join("alacritty");
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
                    Self::safe_write(&alacritty_toml, &updated);
                }
            }
        }

        // 4. Ghostty
        let ghostty_config = config_home.join("ghostty/config");
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
                Self::safe_write(&ghostty_config, &(lines.join("\n") + "\n"));
            }
        }
    }

    fn sync_gtk_files(is_dark: bool) {
        let config_home = Self::get_config_home();
        let prefer_dark_val = if is_dark { "1" } else { "0" };

        for dir_name in &["gtk-3.0", "gtk-4.0"] {
            let gtk_dir = config_home.join(dir_name);
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

            Self::safe_write(&ini_path, &(lines.join("\n") + "\n"));
        }
    }

    fn sync_qt_and_kde(is_dark: bool) {
        let config_home = Self::get_config_home();
        let kdeglobals = config_home.join("kdeglobals");
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

                Self::safe_write(&kdeglobals, &(lines.join("\n") + "\n"));
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

        let mut gtk_dirs = vec![
            Self::get_data_home().join("themes"),
            Self::get_home_dir().join(".themes"),
        ];
        for d in Self::get_data_dirs() {
            let p = d.join("themes");
            if !gtk_dirs.contains(&p) {
                gtk_dirs.push(p);
            }
        }

        let mut icon_dirs = vec![
            Self::get_data_home().join("icons"),
            Self::get_home_dir().join(".icons"),
        ];
        for d in Self::get_data_dirs() {
            let p = d.join("icons");
            if !icon_dirs.contains(&p) {
                icon_dirs.push(p);
            }
        }

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
                .unwrap_or_else(|_| path_obj.to_path_buf())
                .display()
                .to_string()
        };
        let uri = if path_str.starts_with("file://") {
            path_str.to_string()
        } else {
            format!("file://{}", path_str)
        };

        // 1. GNOME / generic GSettings
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

        // 2. KDE Plasma
        if crate::platform::binary_exists("plasma-apply-wallpaperimage") {
            let _ = std::process::Command::new("plasma-apply-wallpaperimage")
                .arg(&path_str)
                .output();
        }

        // 3. Hyprland (hyprpaper preload then wallpaper, or swww)
        if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()
            || crate::platform::binary_exists("hyprctl")
        {
            let _ = std::process::Command::new("hyprctl")
                .args(["hyprpaper", "preload", &path_str])
                .output();
            let _ = std::process::Command::new("hyprctl")
                .args(["hyprpaper", "wallpaper", &format!(",{}", path_str)])
                .output();
        }
        if crate::platform::binary_exists("swww") {
            let _ = std::process::Command::new("swww")
                .args(["img", &path_str])
                .output();
        }

        // 4. Sway / generic wlroots
        if std::env::var("SWAYSOCK").is_ok() || crate::platform::binary_exists("swaymsg") {
            let _ = std::process::Command::new("swaymsg")
                .args(["output", "*", "bg", &path_str, "fill"])
                .output();
        }
        if crate::platform::binary_exists("swaybg") {
            let _ = std::process::Command::new("swaybg")
                .args(["-i", &path_str, "-m", "fill"])
                .spawn();
        }

        // 5. X11 window managers
        if std::env::var("DISPLAY").is_ok() {
            if crate::platform::binary_exists("feh") {
                let _ = std::process::Command::new("feh")
                    .args(["--bg-fill", &path_str])
                    .output();
            } else if crate::platform::binary_exists("nitrogen") {
                let _ = std::process::Command::new("nitrogen")
                    .args(["--set-zoom-fill", &path_str, "--save"])
                    .output();
            }
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

    #[test]
    fn test_xdg_paths_and_data_dirs() {
        let config_home = GsettingsBackend::get_config_home();
        assert!(!config_home.as_os_str().is_empty());

        let data_home = GsettingsBackend::get_data_home();
        assert!(!data_home.as_os_str().is_empty());

        let data_dirs = GsettingsBackend::get_data_dirs();
        assert!(!data_dirs.is_empty());
        assert!(data_dirs.iter().any(|d| d.ends_with("share")));
    }

    #[test]
    fn test_safe_write_respects_readonly() {
        let tmp_dir = std::env::temp_dir().join("settings_tui_appearance_test");
        let _ = std::fs::create_dir_all(&tmp_dir);
        let ro_file = tmp_dir.join("readonly_test.conf");
        let _ = std::fs::write(&ro_file, "original content");

        // Set read-only
        let mut perms = std::fs::metadata(&ro_file).unwrap().permissions();
        perms.set_readonly(true);
        let _ = std::fs::set_permissions(&ro_file, perms);

        // Safe write should detect non-writability and skip without panic
        GsettingsBackend::safe_write(&ro_file, "new overwritten content");

        // Revert permissions to clean up
        let mut perms = std::fs::metadata(&ro_file).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        let _ = std::fs::set_permissions(&ro_file, perms);
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_find_foot_theme_graceful() {
        let result = GsettingsBackend::find_foot_theme("nonexistent_theme_99999");
        assert!(result.is_none());
    }
}
