mod cli;
mod core;
mod utils;

use anyhow::Result;
use cli::CliApp;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "--version" | "-v" => {
                println!("ghitadownload v{}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                println!("🎵 Ghita Downloader (CLI)");
                println!("Sử dụng:");
                println!("  ghitadownload          Khởi chạy giao diện tương tác");
                println!("  ghitadownload -v       Xem phiên bản hiện tại");
                println!("  ghitadownload -h       Xem trợ giúp");
                return Ok(());
            }
            _ => {}
        }
    }

    CliApp::run().await
}

