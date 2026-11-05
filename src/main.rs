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
    let args = CliArgs::parse();
    if args.is_headless() {
        std::process::exit(args.run_headless().await);
    }

    if let Err(error) = CliApp::run().await {
        if error.downcast_ref::<inquire::InquireError>().is_some() {
            println!("👋 Tạm biệt!");
            return Ok(());
        }
        return Err(error);
    }
    Ok(())
}
