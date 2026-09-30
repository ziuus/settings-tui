use super::{AppEntry, ApplicationsBackend};
use anyhow::Result;
use async_trait::async_trait;
use std::fs;
use std::path::Path;

pub struct DesktopEntryBackend;

impl DesktopEntryBackend {
    pub fn new() -> Self {
        Self
    }

    fn parse_desktop_file(path: &Path) -> Option<AppEntry> {
        let content = fs::read_to_string(path).ok()?;
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
                in_desktop_entry = false; // another section like [Desktop Action ...]
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

        let id = path.file_name()?.to_string_lossy().to_string();
        let is_flatpak = exec.contains("flatpak run") || id.contains("org.");

        Some(AppEntry {
            id,
            name,
            description,
            exec,
            is_flatpak,
        })
    }
}

#[async_trait]
impl ApplicationsBackend for DesktopEntryBackend {
    async fn get_applications(&self) -> Result<Vec<AppEntry>> {
        let mut apps = Vec::new();

        let mut check_dirs = vec!["/usr/share/applications".to_string()];
        if let Ok(home) = std::env::var("HOME") {
            check_dirs.push(format!("{}/.local/share/applications", home));
        }

        for dir_path in check_dirs {
            if let Ok(entries) = fs::read_dir(dir_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().is_some_and(|e| e == "desktop") {
                        if let Some(app) = Self::parse_desktop_file(&path) {
                            apps.push(app);
                        }
                    }
                }
            }
        }

        apps.sort_by(|a, b| a.name.cmp(&b.name));
        apps.dedup_by(|a, b| a.name == b.name);

        Ok(apps)
    }

    async fn launch_application(&self, exec: &str) -> Result<()> {
        let mut parts = exec.split_whitespace();
        let cmd = parts.next().unwrap_or("");
        if cmd.is_empty() {
            return Err(anyhow::anyhow!("Empty exec string"));
        }

        let mut command = std::process::Command::new(cmd);
        for arg in parts {
            // Strip %u, %U, %f, %F from desktop exec
            if !arg.starts_with('%') {
                command.arg(arg);
            }
        }

        command
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;

        Ok(())
    }
}
