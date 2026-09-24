pub mod args;
pub mod multi_input;

use anyhow::Result;
use colored::Colorize;
use inquire::{Confirm, Select, Text};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::core::batch::{BatchProcessor, DEFAULT_CONCURRENCY};
use crate::core::config::{AppConfig, AudioFormat, AudioQuality, DownloadSettings};
use crate::utils::env::{find_ffmpeg, find_ytdlp, has_node, install_ffmpeg_auto, update_ytdlp};
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
    pub async fn prepare_env() -> Option<EnvTools> {
        Self::prepare_env_with_install(true).await
    }

    pub async fn prepare_env_headless() -> Option<EnvTools> {
        Self::prepare_env_with_install(false).await
    }

    async fn prepare_env_with_install(allow_prompt: bool) -> Option<EnvTools> {
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
                    "⚠ Không tìm thấy FFmpeg (công cụ chuyển mã & gắn thẻ âm thanh)!"
                        .yellow()
                        .bold()
                );
                if !allow_prompt {
                    eprintln!(
                        "{}",
                        "❌ Không tìm thấy FFmpeg. Chế độ headless không tự cài đặt; hãy cài thủ công hoặc chạy ứng dụng không có tham số.".red().bold()
                    );
                    return None;
                }

                let auto_install = Confirm::new(
                    "Bạn có muốn Ghita Downloader tự động cài đặt FFmpeg về máy không?",
                )
                .with_default(true)
                .with_help_message(
                    "Chỉ cần thực hiện 1 lần, app sẽ tự động tải FFmpeg và cấu hình cho bạn",
                )
                .prompt()
                .unwrap_or(false);

                if auto_install {
                    println!(
                        "{}",
                        "⏳ Đang tự động tải và thiết lập FFmpeg... (vui lòng đợi trong giây lát)"
                            .cyan()
                    );
                    match install_ffmpeg_auto() {
                        Ok(p) => {
                            println!(
                                "{} {}\n",
                                "✔ Cài đặt FFmpeg thành công:".green().bold(),
                                p.display().to_string().cyan()
                            );
                            p
                        }
                        Err(e) => {
                            println!(
                                "{} {}",
                                "❌ Không thể tự động cài đặt FFmpeg:".red().bold(),
                                e
                            );
                            return None;
                        }
                    }
                } else {
                    println!(
                        "{}",
                        "❌ Vui lòng cài đặt FFmpeg thủ công (chạy lệnh: winget install Gyan.FFmpeg) để tiếp tục.".red()
                    );
                    return None;
                }
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

        let env = match Self::prepare_env().await {
            Some(env) => env,
            None => {
                return Err(anyhow::anyhow!(
                    "Không thể chuẩn bị yt-dlp hoặc FFmpeg. Quá trình cài đặt phụ thuộc đã thất bại."
                ));
            }
        };
        let ytdlp_path = env.ytdlp_path;
        let ffmpeg_path = env.ffmpeg_path;
        let has_node_runtime = env.has_node;
        let mut config = AppConfig::try_load().map_err(anyhow::Error::msg)?;

        let terminal_dir =
            std::env::current_dir().unwrap_or_else(|_| AppConfig::system_music_dir());
        let mut active_output_dir = config.initial_output_dir(terminal_dir);
        let mut active_format = config.last_format.unwrap_or(AudioFormat::Mp3);
        let mut active_quality = config.last_quality.unwrap_or(AudioQuality::Mp3_320k);
        let mut active_resolution = config.video_resolution;

        loop {
            Self::print_dashboard(
                &active_output_dir,
                active_format,
                active_quality,
                active_resolution,
                config.keep_accents,
            );

            let menu_choices = vec![
                "🚀 Bắt đầu nhập liên kết & Tải ngay (Dùng cài đặt hiện tại)",
                "⚙ Thay đổi cài đặt (Thư mục, Định dạng MP3/WAV/FLAC/AAC/Gốc/Video, Tiếng Việt)",
                "🔁 Thử lại các bài lỗi gần nhất (failed_tasks.json)",
                "🔄 Cập nhật công cụ yt-dlp (Lấy bản mới nhất từ GitHub)",
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
                    keep_accents: config.keep_accents,
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

                let default_music = AppConfig::system_music_dir();
                let prompt_label = format!(
                    "📁 Chọn hoặc nhập/dán thư mục lưu (Mặc định: {}):",
                    default_music.display()
                );
                let new_dir_str = Text::new(&prompt_label)
                    .with_default(&active_output_dir.display().to_string())
                    .with_help_message("Nhấn Enter để giữ nguyên, dán đường dẫn mới, hoặc nhập 'default' để dùng thư mục Music của máy")
                    .prompt()?;
                let trimmed = new_dir_str.trim();
                let new_dir = if trimmed.eq_ignore_ascii_case("default")
                    || trimmed.eq_ignore_ascii_case("music")
                {
                    default_music
                } else if trimmed.is_empty() {
                    active_output_dir.clone()
                } else {
                    clean_path(Path::new(trimmed))
                };
                active_output_dir = new_dir.clone();

                let format_options = vec![
                    AudioFormat::Mp3,
                    AudioFormat::Wav,
                    AudioFormat::Flac,
                    AudioFormat::Aac,
                    AudioFormat::Original,
                    AudioFormat::Video,
                ];
                let new_format = Select::new("🎵 Chọn định dạng đầu ra:", format_options)
                    .with_help_message("Dùng phím mũi tên Lên/Xuống để chọn")
                    .prompt()?;
                active_format = new_format;

                match new_format {
                    AudioFormat::Mp3 | AudioFormat::Wav | AudioFormat::Flac | AudioFormat::Aac => {
                        let quality_options = match new_format {
                            AudioFormat::Mp3 => AudioQuality::all_mp3(),
                            AudioFormat::Wav => AudioQuality::all_wav(),
                            AudioFormat::Flac => AudioQuality::all_flac(),
                            AudioFormat::Aac => AudioQuality::all_aac(),
                            _ => AudioQuality::all_mp3(),
                        };
                        let new_quality =
                            Select::new("🎚 Chọn mức chất lượng âm thanh:", quality_options)
                                .with_help_message("Chọn mức chất lượng theo nhu cầu")
                                .prompt()?;
                        active_quality = new_quality;
                        config
                            .update_last_used(&active_output_dir, active_format, active_quality)
                            .map_err(anyhow::Error::msg)?;
                    }
                    AudioFormat::Original => {
                        config
                            .update_last_used(&active_output_dir, active_format, active_quality)
                            .map_err(anyhow::Error::msg)?;
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
                        config
                            .set_video_resolution(active_resolution)
                            .map_err(anyhow::Error::msg)?;
                        config
                            .update_last_used(&active_output_dir, active_format, active_quality)
                            .map_err(anyhow::Error::msg)?;
                    }
                }

                let accent_options = vec![
                    "Khử dấu an toàn (Khuyên dùng cho USB, ô tô, loa cũ, máy nghe nhạc)",
                    "Giữ nguyên dấu tiếng Việt Unicode (Đẹp trên Windows, điện thoại)",
                ];
                let default_accent_cursor = if config.keep_accents { 1 } else { 0 };
                let accent_choice = Select::new("🔤 Kiểu đặt tên tệp tiếng Việt:", accent_options)
                    .with_starting_cursor(default_accent_cursor)
                    .prompt()?;
                let keep_accents = accent_choice.starts_with("Giữ");
                config
                    .set_keep_accents(keep_accents)
                    .map_err(anyhow::Error::msg)?;

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
            } else if choice.starts_with("🔄") {
                println!(
                    "\n{}",
                    "⏳ Đang kết nối GitHub và kiểm tra cập nhật yt-dlp...".cyan()
                );
                match update_ytdlp(&ytdlp_path) {
                    Ok(msg) => {
                        println!("\n{} {}\n", "✔".green().bold(), msg.green().bold());
                    }
                    Err(e) => {
                        println!("\n{} {}\n", "❌ Lỗi cập nhật:".red().bold(), e);
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
        keep_accents: bool,
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
        let accent_label = if keep_accents {
            "Có dấu (Unicode)".to_string()
        } else {
            "Khử dấu an toàn".to_string()
        };

        println!(
            "{}",
            "┌─ CẤU HÌNH ĐANG SỬ DỤNG ─────────────────────────────────────".cyan()
        );
        println!("│ 📁 Thư mục lưu : {}", raw_dir.yellow().bold());
        println!(
            "│ 🎵 Định dạng   : {} (.{})",
            format!("{:?}", format).green().bold(),
            ext.green()
        );
        println!("│ 🎚 Chất lượng  : {}", quality_label.cyan());
        println!("│ 🔤 Tên tệp     : {}", accent_label.bright_white());
        println!(
            "{}",
            "└─────────────────────────────────────────────────────────────".cyan()
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
                "║                 GHITA DOWNLOADER ({:<10})              ║",
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
            "║  TikTok, FB, Reels, X | MP3/WAV/FLAC/AAC/Video | Song song   ║".bright_white()
        );
        println!(
            "{}",
            "╚══════════════════════════════════════════════════════════════╝".cyan()
        );
        println!();
    }
}
