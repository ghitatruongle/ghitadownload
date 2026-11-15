mod cli;
mod core;
mod utils;

use anyhow::Result;
use clap::Parser;
use cli::args::CliArgs;
use cli::CliApp;

#[tokio::main]
async fn main() -> Result<()> {
    utils::enable_utf8_console();
    utils::crash::install_panic_hook();
    utils::crash::install_shutdown_handlers();
    utils::job::init_global_job();
    utils::logger::log_line(&format!(
        "app start v{} (pid {})",
        env!("CARGO_PKG_VERSION"),
        std::process::id()
    ));
    let args = CliArgs::parse();
    if args.is_headless() {
        std::process::exit(args.run_headless().await);
    }

    if let Err(error) = CliApp::run().await {
        if error.downcast_ref::<inquire::InquireError>().is_some() {
            println!("👋 Tạm biệt!");
            return Ok(());
        }
        let detail = format!("{error:#}");
        utils::logger::log_line(&format!("fatal: {detail}"));
        eprintln!("❌ {detail}");
        std::process::exit(crate::core::errors::classify_exit_code(&detail));
    }
    Ok(())
}
