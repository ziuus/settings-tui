use clap::Parser;
use std::error::Error;
use tracing_subscriber::EnvFilter;

mod app;
mod backends;
mod config;
mod dbus;
mod platform;
mod security;
mod settings;
mod ui;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    search: Option<String>,

    #[arg(long)]
    section: Option<String>,

    #[arg(long)]
    diagnose: bool,

    #[arg(short, long)]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    if args.diagnose {
        println!("Checking Linux platform and backend capabilities...\n");
        let caps = platform::PlatformCapabilities::detect(
            std::process::Command::new("nmcli")
                .arg("--version")
                .output()
                .is_ok(),
            std::process::Command::new("bluetoothctl")
                .arg("--version")
                .output()
                .is_ok(),
            std::path::Path::new("/run/systemd/system").exists(),
            std::process::Command::new("wpctl")
                .arg("--version")
                .output()
                .is_ok(),
            std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok(),
            std::process::Command::new("gsettings")
                .arg("help")
                .output()
                .is_ok(),
            std::path::Path::new("/run/systemd/system").exists(),
        );

        println!("  Desktop:       {}", caps.env.desktop);
        println!("  Session Type:  {}", caps.env.session_type);
        println!(
            "  Compositor:    {}",
            if caps.env.is_hyprland {
                "Hyprland"
            } else {
                "Other / Unknown"
            }
        );
        println!(
            "  Init System:   {}",
            if caps.env.is_systemd {
                "systemd"
            } else {
                "Non-systemd"
            }
        );
        println!("  Network:       {}", caps.network.status_text());
        println!("  Bluetooth:     {}", caps.bluetooth.status_text());
        println!("  Power:         {}", caps.power.status_text());
        println!("  Sound:         {}", caps.sound.status_text());
        println!("  Display:       {}", caps.display.status_text());
        println!("  Appearance:    {}", caps.appearance.status_text());
        println!("  Services:      {}", caps.services.status_text());
        println!("  System:        {}", caps.system.status_text());
        return Ok(());
    }

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("settings_tui=debug".parse()?))
        .init();

    // Start TUI
    app::run(args).await?;

    Ok(())
}
