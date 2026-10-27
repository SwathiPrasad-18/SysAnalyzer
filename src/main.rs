// Native desktop entry point for AI SysAnalyzer.
mod ai;
mod app;
mod collector;
mod gui;

use anyhow::Result;
use app::App as MonitorApp;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "ai-sysanalyzer",
    version = "1.0.0",
    about = "AI-Based Live System Analyzer — native desktop dashboard"
)]
struct Cli {
    /// Refresh interval in milliseconds (minimum 200)
    #[arg(short, long, default_value_t = 1000)]
    interval: u64,

    /// Log level (error, warn, info, debug)
    #[arg(short, long, default_value = "warn")]
    log_level: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let interval = cli.interval.max(200);

    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or(&cli.log_level),
    )
    .init();

    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("AI SysAnalyzer")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([980.0, 640.0]),
        ..Default::default()
    };

    eframe::run_native(
        "AI SysAnalyzer",
        native_options,
        Box::new(move |_cc| {
            let mut monitor = MonitorApp::new(interval);
            if let Err(err) = monitor.tick() {
                log::error!("Initial system collection failed: {err:#}");
            }
            Ok(Box::new(gui::DesktopApp::new(monitor)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Could not start desktop window: {e}"))?;

    Ok(())
}
