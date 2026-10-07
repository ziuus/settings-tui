use super::{AppEntry, ApplicationsBackend, AutostartEntry, DefaultAppsInfo};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub struct DesktopEntryBackend;

fn get_user_autostart_dir() -> Option<std::path::PathBuf> {
    if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
        Some(std::path::PathBuf::from(config_home).join("autostart"))
    } else if let Ok(home) = std::env::var("HOME") {
        Some(std::path::PathBuf::from(home).join(".config/autostart"))
    } else {
        None
    }
}

pub fn parse_autostart_content(
    content: &str,
    file_name: &str,
    user_owned: bool,
) -> Option<AutostartEntry> {
    let mut name = String::new();
    let mut description = String::new();
    let mut exec = String::new();
    let mut is_hidden = false;
    let mut autostart_enabled = true;
    let mut in_desktop_entry = false;

    for line in content.lines() {
        let line = line.trim();
        if line == "[Desktop Entry]" {
            in_desktop_entry = true;
            continue;
        } else if line.starts_with('[') {
            in_desktop_entry = false;
        }

        if in_desktop_entry {
            if line.starts_with("Name=") && name.is_empty() {
                name = line.strip_prefix("Name=").unwrap_or("").to_string();
            } else if (line.starts_with("Comment=") || line.starts_with("GenericName="))
                && description.is_empty()
            {
                description = line
                    .split_once('=')
                    .map(|(_, v)| v)
                    .unwrap_or("")
                    .to_string();
            } else if line.starts_with("Exec=") && exec.is_empty() {
                exec = line.strip_prefix("Exec=").unwrap_or("").to_string();
            } else if let Some((_, val)) = line.split_once("Hidden=") {
                is_hidden = val.trim().eq_ignore_ascii_case("true");
            } else if let Some((_, val)) = line.split_once("X-GNOME-Autostart-enabled=") {
                autostart_enabled = val.trim().eq_ignore_ascii_case("true");
            }
        }
    }

    if name.is_empty() && !exec.is_empty() {
        name = file_name.trim_end_matches(".desktop").to_string();
    }

    if name.is_empty() && exec.is_empty() {
        return None;
    }

    let enabled = !is_hidden && autostart_enabled;

    Some(AutostartEntry {
        id: file_name.to_string(),
        name,
        description,
        exec,
        enabled,
        user_owned,
    })
}

/// Tokenizes a desktop file Exec= line, preserving quoted arguments and stripping % field codes.
pub fn parse_exec_args(exec: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = ' ';

    for ch in exec.chars() {
        match ch {
            '"' | '\'' if !in_quotes => {
                in_quotes = true;
                quote_char = ch;
            }
            c if in_quotes && c == quote_char => {
                in_quotes = false;
            }
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    if !current.starts_with('%') {
                        args.push(current);
                    }
                    current = String::new();
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }
    if !current.is_empty() && !current.starts_with('%') {
        args.push(current);
    }
    args
}

impl DesktopEntryBackend {
    pub fn new() -> Self {
        Self
    }

    pub fn parse_desktop_content(content: &str, file_name: &str) -> Option<AppEntry> {
        let mut name = String::new();
        let mut description = String::new();
        let mut exec = String::new();
        let mut is_no_display = false;

        let mut in_desktop_entry = false;

        for line in content.lines() {
            let line = line.trim();
            if line == "[Desktop Entry]" {
                in_desktop_entry = true;
                continue;
            } else if line.starts_with('[') {
                in_desktop_entry = false;
            }

            if in_desktop_entry {
                if line.starts_with("Name=") && name.is_empty() {
                    name = line[5..].to_string();
                } else if line.starts_with("Comment=") && description.is_empty() {
                    description = line[8..].to_string();
                } else if line.starts_with("Exec=") && exec.is_empty() {
                    exec = line[5..].to_string();
                } else if line.starts_with("NoDisplay=true") {
                    is_no_display = true;
                }
            }
        }

        if name.is_empty() || exec.is_empty() || is_no_display {
            return None;
        }

        let id = file_name.to_string();
        let is_flatpak = exec.contains("flatpak run") || id.contains("org.");

        Some(AppEntry {
            id,
            name,
            description,
            exec,
            is_flatpak,
        })
    }

    fn parse_desktop_file(path: &Path) -> Option<AppEntry> {
        let content = fs::read_to_string(path).ok()?;
        let file_name = path.file_name()?.to_string_lossy().to_string();
        Self::parse_desktop_content(&content, &file_name)
    }
}

#[async_trait]
impl ApplicationsBackend for DesktopEntryBackend {
    async fn get_applications(&self) -> Result<Vec<AppEntry>> {
        let mut apps = Vec::new();
        let mut seen_ids = HashSet::new();
        let mut check_dirs = Vec::new();

        // 1. User applications directory ($XDG_DATA_HOME/applications or ~/.local/share/applications)
        if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
            check_dirs.push(format!("{}/applications", data_home.trim_end_matches('/')));
        } else if let Ok(home) = std::env::var("HOME") {
            check_dirs.push(format!("{}/.local/share/applications", home));
        }

        // 2. System data dirs ($XDG_DATA_DIRS/applications or /usr/local/share and /usr/share)
        if let Ok(data_dirs) = std::env::var("XDG_DATA_DIRS") {
            for dir in data_dirs.split(':') {
                let trimmed = dir.trim_end_matches('/');
                if !trimmed.is_empty() {
                    check_dirs.push(format!("{}/applications", trimmed));
                }
            }
        } else {
            check_dirs.push("/usr/local/share/applications".to_string());
            check_dirs.push("/usr/share/applications".to_string());
        }

        for dir_path in check_dirs {
            if let Ok(entries) = fs::read_dir(&dir_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().is_some_and(|e| e == "desktop") {
                        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                            // User entries processed first take precedence over system entries
                            if !seen_ids.contains(file_name) {
                                if let Some(app) = Self::parse_desktop_file(&path) {
                                    seen_ids.insert(file_name.to_string());
                                    apps.push(app);
                                }
                            }
                        }
                    }
                }
            }
        }

        apps.sort_by_key(|a| a.name.to_lowercase());
        Ok(apps)
    }

    async fn launch_application(&self, exec: &str) -> Result<()> {
        let args = parse_exec_args(exec);
        if args.is_empty() {
            return Err(anyhow!("Empty exec command"));
        }

        let mut command = std::process::Command::new(&args[0]);
        if args.len() > 1 {
            command.args(&args[1..]);
        }

        command
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;

        Ok(())
    }

    async fn get_default_apps(&self) -> Result<DefaultAppsInfo> {
        let query_mime = |mime: &str| -> Option<String> {
            let output = std::process::Command::new("xdg-mime")
                .args(["query", "default", mime])
                .output()
                .ok()?;
            if output.status.success() {
                let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
            None
        };

        Ok(DefaultAppsInfo {
            web_browser: query_mime("x-scheme-handler/https")
                .or_else(|| query_mime("x-scheme-handler/http")),
            file_manager: query_mime("inode/directory"),
            mail_client: query_mime("x-scheme-handler/mailto"),
            text_editor: query_mime("text/plain"),
        })
    }

    async fn get_autostart_entries(&self) -> Result<Vec<AutostartEntry>> {
        let mut entries = Vec::new();
        let mut seen_ids = HashSet::new();

        // 1. User autostart directory takes precedence
        if let Some(user_dir) = get_user_autostart_dir() {
            if let Ok(read_dir) = fs::read_dir(&user_dir) {
                for entry in read_dir.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().is_some_and(|e| e == "desktop") {
                        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                            if let Ok(content) = fs::read_to_string(&path) {
                                if let Some(item) =
                                    parse_autostart_content(&content, file_name, true)
                                {
                                    seen_ids.insert(file_name.to_string());
                                    entries.push(item);
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. System autostart directory (/etc/xdg/autostart)
        let sys_dir = std::path::Path::new("/etc/xdg/autostart");
        if sys_dir.exists() {
            if let Ok(read_dir) = fs::read_dir(sys_dir) {
                for entry in read_dir.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().is_some_and(|e| e == "desktop") {
                        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                            if !seen_ids.contains(file_name) {
                                if let Ok(content) = fs::read_to_string(&path) {
                                    if let Some(item) =
                                        parse_autostart_content(&content, file_name, false)
                                    {
                                        seen_ids.insert(file_name.to_string());
                                        entries.push(item);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        entries.sort_by_key(|e| e.name.to_lowercase());
        Ok(entries)
    }

    async fn toggle_autostart_entry(&self, id: &str, enable: bool) -> Result<()> {
        let user_dir = get_user_autostart_dir()
            .ok_or_else(|| anyhow!("Could not determine user autostart dir"))?;
        fs::create_dir_all(&user_dir)?;
        let target_path = user_dir.join(id);

        let content = if target_path.exists() {
            fs::read_to_string(&target_path)?
        } else {
            let sys_path = std::path::Path::new("/etc/xdg/autostart").join(id);
            if sys_path.exists() {
                fs::read_to_string(&sys_path)?
            } else {
                return Err(anyhow!("Autostart file {} not found", id));
            }
        };

        let mut new_lines = Vec::new();
        let mut in_desktop_entry = false;
        let mut had_hidden = false;
        let mut had_gnome = false;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed == "[Desktop Entry]" {
                in_desktop_entry = true;
                new_lines.push(line.to_string());
                continue;
            } else if trimmed.starts_with('[') {
                if in_desktop_entry {
                    if !had_hidden {
                        new_lines.push(format!("Hidden={}", !enable));
                    }
                    if !had_gnome {
                        new_lines.push(format!("X-GNOME-Autostart-enabled={}", enable));
                    }
                }
                in_desktop_entry = false;
                new_lines.push(line.to_string());
                continue;
            }

            if in_desktop_entry {
                if trimmed.starts_with("Hidden=") {
                    had_hidden = true;
                    new_lines.push(format!("Hidden={}", !enable));
                    continue;
                } else if trimmed.starts_with("X-GNOME-Autostart-enabled=") {
                    had_gnome = true;
                    new_lines.push(format!("X-GNOME-Autostart-enabled={}", enable));
                    continue;
                }
            }
            new_lines.push(line.to_string());
        }

        if in_desktop_entry {
            if !had_hidden {
                new_lines.push(format!("Hidden={}", !enable));
            }
            if !had_gnome {
                new_lines.push(format!("X-GNOME-Autostart-enabled={}", enable));
            }
        }

        fs::write(&target_path, new_lines.join("\n"))?;
        Ok(())
    }

    async fn remove_autostart_entry(&self, id: &str) -> Result<()> {
        let user_dir = get_user_autostart_dir()
            .ok_or_else(|| anyhow!("Could not determine user autostart dir"))?;
        let target_path = user_dir.join(id);
        if target_path.exists() {
            let _ = fs::remove_file(target_path);
        }
        Ok(())
    }

    async fn add_autostart_app(&self, app: &AppEntry) -> Result<()> {
        let user_dir = get_user_autostart_dir()
            .ok_or_else(|| anyhow!("Could not determine user autostart dir"))?;
        fs::create_dir_all(&user_dir)?;
        let file_name = if app.id.ends_with(".desktop") {
            app.id.clone()
        } else {
            format!("{}.desktop", app.id)
        };
        let target_path = user_dir.join(&file_name);

        let content = format!(
            "[Desktop Entry]\nType=Application\nName={}\nComment={}\nExec={}\nHidden=false\nX-GNOME-Autostart-enabled=true\nTerminal=false\n",
            app.name, app.description, app.exec
        );
        fs::write(target_path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_exec_args_standard() {
        let args = parse_exec_args("firefox %u");
        assert_eq!(args, vec!["firefox"]);
    }

    #[test]
    fn test_parse_exec_args_quoted_and_spaces() {
        let args = parse_exec_args(r#""/opt/My App/bin" --flag "two words" %F"#);
        assert_eq!(args, vec!["/opt/My App/bin", "--flag", "two words"]);
    }

    #[test]
    fn test_parse_exec_args_flatpak() {
        let args = parse_exec_args("flatpak run --branch=stable org.gnome.Calculator @@u %U @@");
        assert_eq!(
            args,
            vec![
                "flatpak",
                "run",
                "--branch=stable",
                "org.gnome.Calculator",
                "@@u",
                "@@"
            ]
        );
    }

    #[test]
    fn test_parse_desktop_content_valid() {
        let content = r#"
[Desktop Entry]
Name=Text Editor
Comment=Edit text files
Exec=gedit %U
Icon=org.gnome.gedit
Type=Application
Categories=GNOME;GTK;Utility;TextEditor;
"#;
        let app = DesktopEntryBackend::parse_desktop_content(content, "gedit.desktop")
            .expect("Should parse valid desktop file");
        assert_eq!(app.id, "gedit.desktop");
        assert_eq!(app.name, "Text Editor");
        assert_eq!(app.description, "Edit text files");
        assert_eq!(app.exec, "gedit %U");
        assert!(!app.is_flatpak);
    }

    #[test]
    fn test_parse_desktop_content_nodisplay() {
        let content = r#"
[Desktop Entry]
Name=Hidden Utility
Exec=hidden-tool
NoDisplay=true
"#;
        assert!(DesktopEntryBackend::parse_desktop_content(content, "hidden.desktop").is_none());
    }

    #[test]
    fn test_parse_desktop_content_flatpak() {
        let content = r#"
[Desktop Entry]
Name=GIMP
Exec=flatpak run org.gimp.GIMP %U
"#;
        let app = DesktopEntryBackend::parse_desktop_content(content, "org.gimp.GIMP.desktop")
            .expect("Should parse flatpak desktop");
        assert!(app.is_flatpak);
    }

    #[test]
    fn test_parse_autostart_content_enabled() {
        let content = r#"
[Desktop Entry]
Type=Application
Name=Handy Notes
Comment=Quick floating notes
Exec=handy-notes
Hidden=false
X-GNOME-Autostart-enabled=true
"#;
        let entry = parse_autostart_content(content, "handy.desktop", true)
            .expect("Should parse valid autostart entry");
        assert_eq!(entry.id, "handy.desktop");
        assert_eq!(entry.name, "Handy Notes");
        assert_eq!(entry.description, "Quick floating notes");
        assert_eq!(entry.exec, "handy-notes");
        assert!(entry.enabled);
        assert!(entry.user_owned);
    }

    #[test]
    fn test_parse_autostart_content_disabled() {
        let content = r#"
[Desktop Entry]
Type=Application
Name=Disabled Tool
Exec=disabled-tool
Hidden=true
"#;
        let entry = parse_autostart_content(content, "disabled.desktop", false)
            .expect("Should parse disabled autostart entry");
        assert!(!entry.enabled);
        assert!(!entry.user_owned);
    }
}
