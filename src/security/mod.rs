use std::fs::{self, File};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

/// Performs an atomic, crash-safe file write with strict 0o600 permissions.
///
/// Steps:
/// 1. Ensures parent directory exists.
/// 2. Creates a unique temporary file in the same directory (guaranteeing same filesystem mount).
/// 3. Enforces user-only read/write permissions (0o600).
/// 4. Writes content and invokes `sync_all()` (fsync) to flush OS buffers to storage.
/// 5. Performs atomic rename (`std::fs::rename`).
/// 6. Flushes parent directory metadata.
#[allow(dead_code)]
pub fn atomic_write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, content: C) -> io::Result<()> {
    let path = path.as_ref();
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Target path must have a parent directory",
        )
    })?;

    if !parent.exists() {
        fs::create_dir_all(parent)?;
    }

    let pid = std::process::id();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("config");
    let tmp_path = parent.join(format!(".tmp.{}.{}.{}", filename, pid, now));

    // Open with strict 0o600 permissions (user only)
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp_path)?;

    file.write_all(content.as_ref())?;
    file.sync_all()?;
    drop(file);

    fs::rename(&tmp_path, path)?;

    // Sync parent directory to ensure directory entry is durable on crash
    if let Ok(dir) = File::open(parent) {
        let _ = dir.sync_all();
    }

    Ok(())
}

/// Whitelists permitted systemctl power action verbs.
/// Rejects any arbitrary or shell-injected verbs.
pub fn sanitize_systemctl_action(action: &str) -> Option<&'static str> {
    match action {
        "poweroff" => Some("poweroff"),
        "reboot" => Some("reboot"),
        "suspend" => Some("suspend"),
        "hibernate" => Some("hibernate"),
        _ => None,
    }
}

/// Validates whether a service name is a valid, safe systemd unit identifier.
/// Prevents command injection, path traversal, and null-byte injection.
pub fn validate_service_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 256
        && !name.contains('/')
        && !name.contains("..")
        && !name.contains('\0')
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '-' || c == '@')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_write_creates_file_and_permissions() {
        let temp_dir =
            std::env::temp_dir().join(format!("settings_tui_test_{}", std::process::id()));
        let test_file = temp_dir.join("test_config.json");

        let content = b"{\"theme\": \"dark\", \"sound\": true}";
        assert!(atomic_write(&test_file, content).is_ok());

        assert!(test_file.exists());
        let read_back = fs::read(&test_file).expect("Failed to read back");
        assert_eq!(read_back, content);

        // Check permissions (0o600)
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(&test_file).expect("Failed to get metadata");
        let permissions = metadata.permissions().mode() & 0o777;
        assert_eq!(permissions, 0o600, "File permissions must be 0600");

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_sanitize_systemctl_action() {
        assert_eq!(sanitize_systemctl_action("poweroff"), Some("poweroff"));
        assert_eq!(sanitize_systemctl_action("reboot"), Some("reboot"));
        assert_eq!(sanitize_systemctl_action("suspend"), Some("suspend"));
        assert_eq!(sanitize_systemctl_action("hibernate"), Some("hibernate"));

        // Malicious or unauthorized actions rejected
        assert_eq!(sanitize_systemctl_action("rm -rf /"), None);
        assert_eq!(sanitize_systemctl_action("poweroff; reboot"), None);
        assert_eq!(sanitize_systemctl_action("default"), None);
        assert_eq!(sanitize_systemctl_action(""), None);
    }

    #[test]
    fn test_validate_service_name() {
        assert!(validate_service_name("bluetooth.service"));
        assert!(validate_service_name("NetworkManager.service"));
        assert!(validate_service_name("user@1000.service"));
        assert!(validate_service_name("systemd-resolved.service"));

        // Dangerous / invalid names rejected
        assert!(!validate_service_name(""));
        assert!(!validate_service_name("../../etc/shadow"));
        assert!(!validate_service_name("svc; reboot"));
        assert!(!validate_service_name("svc\0malicious"));
        assert!(!validate_service_name("svc|curl"));
    }
}
