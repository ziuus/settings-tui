use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    settings_tui::run_main().await
}
