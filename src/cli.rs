pub mod args;
pub mod multi_input;

use anyhow::Result;
use colored::Colorize;
use inquire::{Confirm, Select, Text};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::core::batch::{BatchProcessor, DEFAULT_CONCURRENCY};
use crate::core::config::{AppConfig, AudioFormat, AudioQuality, DownloadSettings};
use crate::utils::env::{find_ffmpeg, find_ytdlp, has_node};
use crate::utils::file_helper::{clean_path, ensure_dir};
use multi_input::MultiLinkInput;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct EnvTools {
    pub ytdlp_path: PathBuf,
    pub ffmpeg_path: PathBuf,
    pub has_node: bool,
}

pub struct CliApp;

impl CliApp {
    pub fn prepare_env() -> Option<EnvTools> {
        let ytdlp_path = match find_ytdlp() {
            Some(p) => p,
            None => {
                println!(
                    "{}",
                    "❌ Không tìm thấy công cụ yt-dlp! Vui lòng cài đặt hoặc đặt tệp yt-dlp.exe vào thư mục bin/".red().bold()
                );
                return None;
            }
        };

        let ffmpeg_path = match find_ffmpeg() {
            Some(p) => p,
            None => {
                println!(
                    "{}",
                    "❌ Không tìm thấy FFmpeg! Vui lòng đảm bảo FFmpeg đã có trong biến môi trường PATH.".red().bold()
                );
                return None;
            }
        };

        Some(EnvTools {
            ytdlp_path,
            ffmpeg_path,
            has_node: has_node(),
        })
    }

    pub async fn run() -> Result<()> {
        Self::print_banner();

        let env = match Self::prepare_env() {
            Some(e) => e,
            None => return Ok(()),
        };
        let ytdlp_path = env.ytdlp_path;
        let ffmpeg_path = env.ffmpeg_path;
        let has_node_runtime = env.has_node;
        let mut config = AppConfig::load();

        let terminal_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut active_output_dir = terminal_dir;
        let mut active_format = config.last_format.unwrap_or(AudioFormat::Mp3);
        let mut active_quality = config.last_quality.unwrap_or(AudioQuality::Mp3_320k);
        let mut active_resolution = config.video_resolution;

        loop {
            Self::print_dashboard(
                &active_output_dir,
                active_format,
                active_quality,
                active_resolution,
            );

            let menu_choices = vec![
                "🚀 Bắt đầu nhập liên kết & Tải ngay (Dùng cài đặt hiện tại)",
                "⚙ Thay đổi cài đặt (Thư mục, Định dạng MP3/WAV/Gốc/Video, Chất lượng)",
                "🔁 Thử lại các bài lỗi gần nhất (failed_tasks.json)",
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
                        println!(
                            "{}",
                            "↩ Đã hủy nhập liên kết, quay lại menu chính.".yellow()
                        );
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

                let quality_opt = match active_format {
                    AudioFormat::Original | AudioFormat::Video => None,
                    _ => Some(active_quality),
                };
                let settings = DownloadSettings {
                    output_dir: active_output_dir.clone(),
                    format: active_format,
                    quality: quality_opt,
                    video_resolution: active_resolution,
                    concurrency: DEFAULT_CONCURRENCY,
                };

                let batch_processor =
                    BatchProcessor::new(settings, &ytdlp_path, &ffmpeg_path, has_node_runtime);

                let tasks = batch_processor.resolve_inputs(&links).await?;

                if tasks.is_empty() {
                    println!(
                        "{}",
                        "⚠ Không trích xuất được bài hát nào từ liên kết đã nhập.".yellow()
                    );
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
                        let outcome = batch_processor.execute_batch(&tasks).await?;
                        Self::print_batch_summary(&outcome);
                    }
                }

                let continue_more = Confirm::new("Bạn có muốn tải tiếp các bài hát khác không?")
                    .with_default(true)
                    .prompt()?;

                if !continue_more {
                    println!(
                        "\n{} Cảm ơn bạn đã sử dụng Ghita Downloader!",
                        "👋".cyan().bold()
                    );
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

                let format_options = vec![
                    AudioFormat::Mp3,
                    AudioFormat::Wav,
                    AudioFormat::Original,
                    AudioFormat::Video,
                ];
                let new_format = Select::new("🎵 Chọn định dạng đầu ra:", format_options)
                    .with_help_message("Dùng phím mũi tên Lên/Xuống để chọn")
                    .prompt()?;
                active_format = new_format;

                match new_format {
                    AudioFormat::Mp3 | AudioFormat::Wav => {
                        let quality_options = match new_format {
                            AudioFormat::Mp3 => AudioQuality::all_mp3(),
                            _ => AudioQuality::all_wav(),
                        };
                        let new_quality =
                            Select::new("🎚 Chọn mức chất lượng âm thanh:", quality_options)
                                .with_help_message("Chọn mức chất lượng theo nhu cầu")
                                .prompt()?;
                        active_quality = new_quality;
                        config.update_last_used(&active_output_dir, active_format, active_quality);
                    }
                    AudioFormat::Original => {
                        config.update_last_used(&active_output_dir, active_format, active_quality);
                    }
                    AudioFormat::Video => {
                        let res_options = vec!["1080p", "720p", "480p", "Best (cao nhất)"];
                        let res_choice = Select::new("🎬 Chọn độ phân giải video:", res_options)
                            .with_help_message(
                                "Video được giữ nguyên chất lượng nguồn không vượt quá mức chọn",
                            )
                            .prompt()?;
                        active_resolution = match res_choice {
                            "1080p" => Some(1080),
                            "720p" => Some(720),
                            "480p" => Some(480),
                            _ => None,
                        };
                        config.set_video_resolution(active_resolution);
                        config.update_last_used(&active_output_dir, active_format, active_quality);
                    }
                }

                println!(
                    "\n{} Đã cập nhật và áp dụng cài đặt thành công!\n",
                    "✔".green().bold()
                );
            } else if choice.starts_with("🔁") {
                match BatchProcessor::retry_from_file(&active_output_dir) {
                    Ok((settings, tasks)) => {
                        println!(
                            "{} Phát hiện {} bài lỗi đang chờ thử lại.",
                            "✔".green().bold(),
                            tasks.len().to_string().cyan().bold()
                        );
                        let processor = BatchProcessor::new(
                            settings,
                            &ytdlp_path,
                            &ffmpeg_path,
                            has_node_runtime,
                        );
                        let outcome = processor.execute_batch(&tasks).await?;
                        Self::print_batch_summary(&outcome);
                    }
                    Err(e) => {
                        println!("{}", format!("⚠ {}", e).yellow());
                    }
                }
            } else if choice.starts_with("📂") {
                let _ = ensure_dir(&active_output_dir);
                let _ = Command::new("explorer").arg(&active_output_dir).spawn();
                println!(
                    "{} Đã mở thư mục trong Windows File Explorer: {}\n",
                    "✔".green(),
                    active_output_dir.display().to_string().cyan()
                );
            } else {
                println!(
                    "\n{} Cảm ơn bạn đã sử dụng Ghita Downloader!",
                    "👋".cyan().bold()
                );
                break;
            }
        }

        Ok(())
    }

    fn print_batch_summary(outcome: &crate::core::batch::BatchOutcome) {
        let ok = outcome.success_count;
        let failed = outcome.failed_tasks.len();
        println!(
            "\n{} Hoàn tất: {} bài thành công, {} bài lỗi.",
            "📊".cyan().bold(),
            ok.to_string().green().bold(),
            failed.to_string().yellow().bold()
        );
        if failed > 0 {
            println!(
                "{} Đã lưu hàng đợi bài lỗi vào failed_tasks.json — chọn \"Thử lại các bài lỗi\" ở menu để tải lại.",
                "ℹ".yellow()
            );
        }
        println!();
    }

    fn print_dashboard(
        dir: &Path,
        format: AudioFormat,
        quality: AudioQuality,
        resolution: Option<u32>,
    ) {
        let ext = quality.file_extension(format);
        let quality_label = match format {
            AudioFormat::Video => match resolution {
                Some(r) => format!("Độ phân giải {}p (giữ nguyên gốc)", r),
                None => "Độ phân giải cao nhất (giữ nguyên gốc)".to_string(),
            },
            AudioFormat::Original => "Giữ nguyên luồng âm thanh gốc".to_string(),
            _ => quality.to_string(),
        };
        let raw_dir = dir.display().to_string();
        let display_dir = if raw_dir.chars().count() > 41 {
            let end_chars: String = raw_dir
                .chars()
                .rev()
                .take(38)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            format!("...{}", end_chars)
        } else {
            raw_dir
        };

        println!(
            "{}",
            "┌─────────────────────────────────────────────────────────────┐".cyan()
        );
        println!(
            "{}",
            format!(
                "│             CẤU HÌNH HIỆN TẠI ĐANG DÙNG ({})             │",
                APP_VERSION
            )
            .cyan()
            .bold()
        );
        println!(
            "{}",
            "├─────────────────────────────────────────────────────────────┤".cyan()
        );
        println!("│ 📁 Thư mục lưu:  {:<43} │", display_dir.yellow());
        println!(
            "│ 🎵 Định dạng:    {:<43} │",
            format!("{:?} (.{})", format, ext).green().bold()
        );
        println!("│ 🎚 Chất lượng:   {:<43} │", quality_label.cyan());
        println!(
            "{}",
            "└─────────────────────────────────────────────────────────────┘".cyan()
        );
        println!();
    }

    fn print_banner() {
        println!(
            "{}",
            "╔══════════════════════════════════════════════════════════════╗".cyan()
        );
        println!(
            "{}",
            format!(
                "║                 GHITA DOWNLOADER ({})                     ║",
                APP_VERSION
            )
            .cyan()
            .bold()
        );
        println!(
            "{}",
            "║  Tải nhạc YouTube, Spotify, Suno AI, SoundCloud, Bandcamp    ║".bright_white()
        );
        println!(
            "{}",
            "║   TikTok, FB, Threads | MP3/WAV/Gốc/Video | Tải song song    ║".bright_white()
        );
        println!(
            "{}",
            "╚══════════════════════════════════════════════════════════════╝".cyan()
        );
        println!();
    }
}
