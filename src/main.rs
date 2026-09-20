mod cli;
mod core;
mod utils;

use anyhow::Result;
use clap::Parser;
use cli::args::CliArgs;
use cli::CliApp;

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();
    if args.is_headless() {
        std::process::exit(args.run_headless().await);
    }

    CliApp::run().await
}
