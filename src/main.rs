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
        println!("Checking backend dependencies...");
        let has_systemctl = std::process::Command::new("systemctl")
            .arg("--version")
            .output()
            .is_ok();
        let has_nmcli = std::process::Command::new("nmcli")
            .arg("--version")
            .output()
            .is_ok();
        let has_wpctl = std::process::Command::new("wpctl")
            .arg("--version")
            .output()
            .is_ok();
        println!(
            "systemctl: {}",
            if has_systemctl { "Found" } else { "Missing" }
        );
        println!("nmcli: {}", if has_nmcli { "Found" } else { "Missing" });
        println!("wpctl: {}", if has_wpctl { "Found" } else { "Missing" });
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
