with open("src/backends/system/mod.rs", "r") as f:
    c = f.read()

func = """async fn check_updates() -> Option<usize> {
    if std::path::Path::new("/usr/bin/checkupdates").exists() {
        if let Ok(output) = tokio::process::Command::new("checkupdates").output().await {
            let out = String::from_utf8_lossy(&output.stdout);
            let count = out.lines().filter(|l| !l.trim().is_empty()).count();
            return Some(count);
        }
    }
    if std::path::Path::new("/usr/bin/apt-get").exists() {
        if let Ok(output) = tokio::process::Command::new("apt-get").args(["-s", "upgrade"]).output().await {
            let out = String::from_utf8_lossy(&output.stdout);
            let count = out.lines().filter(|l| l.starts_with("Inst ")).count();
            return Some(count);
        }
    }
    if std::path::Path::new("/usr/bin/dnf").exists() {
        if let Ok(output) = tokio::process::Command::new("dnf").args(["check-update", "-q"]).output().await {
            let out = String::from_utf8_lossy(&output.stdout);
            let count = out.lines().filter(|l| !l.trim().is_empty()).count();
            return Some(count);
        }
    }
    None
}

impl RealSystemBackend {"""

c = c.replace("impl RealSystemBackend {", func)

with open("src/backends/system/mod.rs", "w") as f:
    f.write(c)
