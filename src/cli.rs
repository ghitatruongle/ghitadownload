pub mod multi_input;

use anyhow::Result;
use colored::Colorize;
use inquire::{Confirm, Select, Text};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::core::batch::BatchProcessor;
use crate::core::config::{AppConfig, AudioFormat, AudioQuality, DownloadSettings};
use crate::utils::env::{find_ffmpeg, find_ytdlp, has_node};
use crate::utils::file_helper::{clean_path, ensure_dir};
use multi_input::MultiLinkInput;

pub struct CliApp;

impl CliApp {
    pub async fn run() -> Result<()> {
        Self::print_banner();

        let ytdlp_path = match find_ytdlp() {
            Some(p) => p,
            None => {
                println!(
                    "{}",
                    "❌ Không tìm thấy công cụ yt-dlp! Vui lòng cài đặt hoặc đặt tệp yt-dlp.exe vào thư mục bin/".red().bold()
                );
                return Ok(());
            }
        };

        let ffmpeg_path = match find_ffmpeg() {
            Some(p) => p,
            None => {
                println!(
                    "{}",
                    "❌ Không tìm thấy FFmpeg! Vui lòng đảm bảo FFmpeg đã có trong biến môi trường PATH.".red().bold()
                );
                return Ok(());
            }
        };

        let has_node_runtime = has_node();
        let mut config = AppConfig::load();

        let terminal_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut active_output_dir = terminal_dir;
        let mut active_format = config.last_format.unwrap_or(AudioFormat::Mp3);
        let mut active_quality = config.last_quality.unwrap_or(AudioQuality::Mp3_320k);

        loop {
            Self::print_dashboard(&active_output_dir, active_format, active_quality);

            let menu_choices = vec![
                "🚀 Bắt đầu nhập liên kết & Tải ngay (Dùng cài đặt hiện tại)",
                "⚙ Thay đổi cài đặt (Thư mục, Định dạng MP3/WAV, Chất lượng)",
                "📂 Mở thư mục lưu nhạc trong File Explorer",
                "❌ Thoát ứng dụng",
            ];

            let choice = Select::new("📌 Chọn tác vụ:", menu_choices)
                .with_help_message("Dùng phím mũi tên Lên/Xuống và nhấn Enter để chọn")
                .prompt()?;

            if choice.starts_with("🚀") {
                let links_opt = MultiLinkInput::prompt()?;
                let links = match links_opt {
                    Some(l) => l,
                    None => {
                        println!("{}", "↩ Đã hủy nhập liên kết, quay lại menu chính.".yellow());
                        continue;
                    }
                };

                if links.is_empty() {
                    println!("{}", "⚠ Không có liên kết nào hợp lệ để tải!".yellow());
                    continue;
                }

                println!(
                    "\n{} Đã ghi nhận {} liên kết đầu vào.",
                    "✔".green().bold(),
                    links.len().to_string().cyan().bold()
                );

                let settings = DownloadSettings {
                    output_dir: active_output_dir.clone(),
                    format: active_format,
                    quality: active_quality,
                };

                let batch_processor = BatchProcessor::new(
                    settings,
                    &ytdlp_path,
                    &ffmpeg_path,
                    has_node_runtime,
                );

                let tasks = batch_processor.resolve_inputs(&links).await?;

                if tasks.is_empty() {
                    println!("{}", "⚠ Không trích xuất được bài hát nào từ liên kết đã nhập.".yellow());
                } else {
                    println!(
                        "{} Đã sẵn sàng {} bài hát để tải.",
                        "✔".green().bold(),
                        tasks.len().to_string().cyan().bold()
                    );

                    let confirm_download = Confirm::new("Bắt đầu tải ngay bây giờ?")
                        .with_default(true)
                        .prompt()?;

                    if confirm_download {
                        batch_processor.execute_batch(&tasks).await?;
                    }
                }

                let continue_more = Confirm::new("Bạn có muốn tải tiếp các bài hát khác không?")
                    .with_default(true)
                    .prompt()?;

                if !continue_more {
                    println!("\n{} Cảm ơn bạn đã sử dụng Ghita Downloader!", "👋".cyan().bold());
                    break;
                }
                println!("\n--------------------------------------------------------------\n");
            } else if choice.starts_with("⚙") {
                println!("\n{}", "⚙ THAY ĐỔI CÀI ĐẶT ỨNG DỤNG:".cyan().bold());

                let new_dir_str = Text::new("📁 Chọn hoặc nhập/dán thư mục lưu nhạc:")
                    .with_default(&active_output_dir.display().to_string())
                    .with_help_message("Nhấn Enter để giữ nguyên, hoặc dán đường dẫn mới (hỗ trợ cả dấu ngoặc kép)")
                    .prompt()?;
                let new_dir = clean_path(Path::new(new_dir_str.trim()));
                active_output_dir = new_dir.clone();

                let format_options = vec![AudioFormat::Mp3, AudioFormat::Wav];
                let new_format = Select::new("🎵 Chọn định dạng âm thanh:", format_options)
                    .with_help_message("Dùng phím mũi tên Lên/Xuống để chọn")
                    .prompt()?;
                active_format = new_format;

                let quality_options = match new_format {
                    AudioFormat::Mp3 => AudioQuality::all_mp3(),
                    AudioFormat::Wav => AudioQuality::all_wav(),
                };

                let new_quality = Select::new("🎚 Chọn mức chất lượng âm thanh:", quality_options)
                    .with_help_message("Chọn mức chất lượng theo nhu cầu")
                    .prompt()?;
                active_quality = new_quality;

                config.update_last_used(&active_output_dir, active_format, active_quality);
                println!(
                    "\n{} Đã cập nhật và áp dụng cài đặt thành công!\n",
                    "✔".green().bold()
                );
            } else if choice.starts_with("📂") {
                let _ = ensure_dir(&active_output_dir);
                let _ = Command::new("explorer").arg(&active_output_dir).spawn();
                println!(
                    "{} Đã mở thư mục trong Windows File Explorer: {}\n",
                    "✔".green(),
                    active_output_dir.display().to_string().cyan()
                );
            } else {
                println!("\n{} Cảm ơn bạn đã sử dụng Ghita Downloader!", "👋".cyan().bold());
                break;
            }
        }

        Ok(())
    }

    fn print_dashboard(dir: &Path, format: AudioFormat, quality: AudioQuality) {
        let ext = quality.file_extension(format);
        let raw_dir = dir.display().to_string();
        let display_dir = if raw_dir.chars().count() > 41 {
            let end_chars: String = raw_dir.chars().rev().take(38).collect::<Vec<_>>().into_iter().rev().collect();
            format!("...{}", end_chars)
        } else {
            raw_dir
        };

        println!("{}", "┌─────────────────────────────────────────────────────────────┐".cyan());
        println!("{}", "│             CẤU HÌNH HIỆN TẠI ĐANG DÙNG (0.0.0)             │".cyan().bold());
        println!("{}", "├─────────────────────────────────────────────────────────────┤".cyan());
        println!("│ 📁 Thư mục lưu:  {:<43} │", display_dir.yellow());
        println!("│ 🎵 Định dạng:    {:<43} │", format!("{:?} (.{})", format, ext).green().bold());
        println!("│ 🎚 Chất lượng:   {:<43} │", quality.to_string().cyan());
        println!("{}", "└─────────────────────────────────────────────────────────────┘".cyan());
        println!();
    }

    fn print_banner() {
        println!("{}", "╔══════════════════════════════════════════════════════════════╗".cyan());
        println!("{}", "║                 GHITA DOWNLOADER (0.0.0)                     ║".cyan().bold());
        println!("{}", "║   Tải nhạc YouTube, Spotify, Suno AI, TikTok, FB, Threads    ║".bright_white());
        println!("{}", "║   Hỗ trợ tải hàng loạt | Đầy đủ chất lượng từ cao tới thấp   ║".bright_white());
        println!("{}", "╚══════════════════════════════════════════════════════════════╝".cyan());
        println!();
    }
}
