pub mod args;
pub mod multi_input;

use anyhow::Result;
use colored::{ColoredString, Colorize};
use crossterm::terminal;
use inquire::{Confirm, MultiSelect, Select, Text};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::core::batch::{
    BatchProcessor, DownloadTask, DEFAULT_CONCURRENCY, FAILED_QUEUE_FILE, PENDING_QUEUE_FILE,
};
use crate::core::config::{AppConfig, AudioFormat, AudioQuality, DownloadSettings};
use crate::utils::env::{find_ffmpeg, find_ytdlp, has_node, install_ffmpeg_auto, update_ytdlp};
use crate::utils::file_helper::{clean_path, ensure_dir};
use multi_input::MultiLinkInput;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const RECENT_MANIFEST_LIMIT: usize = 8;

fn accent(text: &str) -> ColoredString {
    text.cyan()
}

fn ok(text: &str) -> ColoredString {
    text.green().bold()
}

fn warn(text: &str) -> ColoredString {
    text.yellow().bold()
}

fn err(text: &str) -> ColoredString {
    text.red().bold()
}

fn prompt_skippable<T>(prompt: Result<T, inquire::InquireError>) -> Result<Option<T>> {
    match prompt {
        Ok(value) => Ok(Some(value)),
        Err(inquire::InquireError::OperationCanceled) => Ok(None),
        Err(inquire::InquireError::OperationInterrupted) => Ok(None),
        Err(error) => Err(anyhow::Error::new(error)),
    }
}

fn term_width() -> usize {
    let (cols, _) = terminal::size().unwrap_or((80, 24));
    (cols as usize).clamp(40, 100)
}

fn frame_top(title: &str, width: usize) -> String {
    let prefix = format!("┌─ {} ─ ", title);
    let fill = width.saturating_sub(prefix.chars().count());
    format!("{}{}", prefix, "─".repeat(fill))
}

fn frame_bottom(width: usize) -> String {
    format!("└{}", "─".repeat(width.saturating_sub(1)))
}

fn middle_truncate(text: &str, max_len: usize) -> String {
    if max_len == 0 {
        return String::new();
    }
    if text.chars().count() <= max_len {
        return text.to_string();
    }
    let ellipsis = "…";
    let parts: Vec<&str> = text
        .split(['\\', '/'])
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() < 2 {
        let head: String = text.chars().take(max_len.saturating_sub(1)).collect();
        return format!("{}{}", head, ellipsis);
    }
    let first = parts[0];
    let last = parts[parts.len() - 1];
    let mut tail = last.to_string();
    loop {
        let candidate = format!("{}{}{}", first, ellipsis, tail);
        if candidate.chars().count() <= max_len {
            return candidate;
        }
        if tail.is_empty() {
            break;
        }
        let overflow = candidate.chars().count() - max_len;
        let keep = tail.chars().count().saturating_sub(overflow);
        tail = tail.chars().skip(tail.chars().count() - keep).collect();
    }
    let head: String = text.chars().take(max_len.saturating_sub(1)).collect();
    format!("{}{}", head, ellipsis)
}

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
                    println!(
                        "{}",
                        "⏳ Chế độ headless: đang tự động tải và thiết lập FFmpeg... (vui lòng đợi)".cyan()
                    );
                    match install_ffmpeg_auto().await {
                        Ok(path) => {
                            println!(
                                "{} {}\n",
                                "✔ Cài đặt FFmpeg thành công:".green().bold(),
                                path.display().to_string().cyan()
                            );
                            return Some(EnvTools {
                                ytdlp_path,
                                ffmpeg_path: path,
                                has_node: has_node(),
                            });
                        }
                        Err(e) => {
                            eprintln!(
                                "{} {}",
                                "❌ Không thể tự động cài đặt FFmpeg ở chế độ headless:"
                                    .red()
                                    .bold(),
                                e
                            );
                            return None;
                        }
                    }
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
                    match install_ffmpeg_auto().await {
                        Ok(p) => {
                            println!(
                                "{} {}\n",
                                "✔ Cài đặt FFmpeg thành công:".green().bold(),
                                p.display().to_string().cyan()
                            );
                            p
                        }
                        Err(e) => {
                            println!("{} {}", err("❌ Không thể tự động cài đặt FFmpeg:"), e);
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

        crate::core::cancel::init_global();
        tokio::spawn(async {
            let mut press_count: usize = 0;
            while tokio::signal::ctrl_c().await.is_ok() {
                crate::core::cancel::global().cancel();
                press_count += 1;
                if press_count >= 2 {
                    std::process::exit(130);
                }
            }
        });

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
                config.fetch_lyrics,
            );

            let pending_badge = Self::pending_task_count(&active_output_dir)
                .map(|count| format!(" (có {} bài dở)", count))
                .unwrap_or_default();
            let menu_choices = vec![
                "🚀 Bắt đầu nhập liên kết & Tải ngay (Dùng cài đặt hiện tại)".to_string(),
                "⚙ Thay đổi cài đặt (Thư mục, Định dạng MP3/WAV/FLAC/AAC/Opus/Gốc/Video, Tiếng Việt)"
                    .to_string(),
                "🔁 Thử lại các bài lỗi gần nhất (failed_tasks.json)".to_string(),
                format!("⏯ Tiếp tục batch dở dang{}", pending_badge),
                "📜 Lịch sử tải gần đây".to_string(),
                "🔄 Cập nhật công cụ yt-dlp (Lấy bản mới nhất từ GitHub)".to_string(),
                "📂 Mở thư mục lưu nhạc trong File Explorer".to_string(),
                "❌ Thoát ứng dụng".to_string(),
            ];

            let choice = match prompt_skippable(
                Select::new("📌 Chọn tác vụ:", menu_choices)
                    .with_help_message("Dùng phím mũi tên Lên/Xuống và nhấn Enter để chọn")
                    .prompt(),
            )? {
                Some(choice) => choice,
                None => {
                    println!(
                        "{}",
                        warn("↩ Đã hủy — chọn tác vụ khác hoặc chọn ❌ để thoát.")
                    );
                    continue;
                }
            };

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
                    cookies_file: None,
                    cookies_from_browser: None,
                    raw: false,
                    fetch_lyrics: config.fetch_lyrics,
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

                    let confirm_download = match prompt_skippable(
                        Confirm::new("Bắt đầu tải ngay bây giờ?")
                            .with_default(true)
                            .prompt(),
                    )? {
                        Some(value) => value,
                        None => {
                            println!("{}", warn("↩ Đã hủy — quay lại menu chính."));
                            continue;
                        }
                    };

                    if confirm_download {
                        let outcome = batch_processor.execute_batch(&tasks).await?;
                        Self::print_batch_summary(&outcome, &active_output_dir);
                    }
                }

                let continue_more = match prompt_skippable(
                    Confirm::new("Bạn có muốn tải tiếp các bài hát khác không?")
                        .with_default(true)
                        .prompt(),
                )? {
                    Some(value) => value,
                    None => {
                        println!("{}", warn("↩ Đã hủy — quay lại menu chính."));
                        continue;
                    }
                };

                if !continue_more {
                    println!(
                        "\n{} Cảm ơn bạn đã sử dụng Ghita Downloader!",
                        "👋".cyan().bold()
                    );
                    break;
                }
                println!("\n--------------------------------------------------------------\n");
            } else if choice.starts_with("⚙") {
                Self::run_settings_flow(
                    &mut config,
                    &mut active_output_dir,
                    &mut active_format,
                    &mut active_quality,
                    &mut active_resolution,
                )
                .await?;
            } else if choice.starts_with("🔁") {
                match BatchProcessor::retry_from_file(&active_output_dir) {
                    Ok((settings, tasks)) => {
                        let options: Vec<String> = tasks
                            .iter()
                            .enumerate()
                            .map(|(index, task)| {
                                let duration = match task.expected_duration_secs {
                                    Some(secs) => format!(" (~{} min {} s)", secs / 60, secs % 60),
                                    None => String::new(),
                                };
                                format!("{}. {}{}", index + 1, task.display_title, duration)
                            })
                            .collect();
                        if options.is_empty() {
                            println!(
                                "{}",
                                warn("⚠ Hàng đợi bài lỗi rỗng — không có gì để thử lại.")
                            );
                            continue;
                        }
                        println!(
                            "\n{} tổng {} bài",
                            accent("🔁 CHỌN BÀI LỖI CẦN THỬ LẠI:"),
                            options.len()
                        );
                        let defaults: Vec<usize> = (0..options.len()).collect();
                        let selected = match prompt_skippable(
                            MultiSelect::new("Chọn bài cần thử lại:", options)
                                .with_default(&defaults)
                                .with_scorer(
                                    &|input: &str,
                                      _option: &String,
                                      string_value: &str,
                                      _index: usize| {
                                        if input.is_empty()
                                            || string_value
                                                .to_lowercase()
                                                .contains(&input.to_lowercase())
                                        {
                                            Some(0)
                                        } else {
                                            None
                                        }
                                    },
                                )
                                .with_help_message(
                                    "Gõ để lọc bài hát, Space để chọn/bỏ chọn, Enter để xác nhận",
                                )
                                .raw_prompt(),
                        )? {
                            Some(value) => value,
                            None => {
                                println!("{}", warn("↩ Đã hủy thử lại — quay lại menu chính."));
                                continue;
                            }
                        };
                        if selected.is_empty() {
                            println!("{}", warn("⚠ Bạn chưa chọn bài nào để thử lại."));
                            continue;
                        }
                        let picked: Vec<DownloadTask> = selected
                            .iter()
                            .filter_map(|entry| tasks.get(entry.index).cloned())
                            .collect();
                        if picked.is_empty() {
                            println!("{}", warn("⚠ Không tìm thấy bài nào khớp lựa chọn."));
                            continue;
                        }
                        let summary_dir = settings.output_dir.clone();
                        let processor = BatchProcessor::new(
                            settings,
                            &ytdlp_path,
                            &ffmpeg_path,
                            has_node_runtime,
                        );
                        let outcome = processor.execute_batch(&picked).await?;
                        Self::print_batch_summary(&outcome, &summary_dir);
                    }
                    Err(e) => {
                        println!("{}", format!("⚠ {}", e).yellow());
                    }
                }
            } else if choice.starts_with("⏯") {
                match BatchProcessor::resume_from_file(&active_output_dir) {
                    Ok((settings, tasks)) => {
                        if tasks.is_empty() {
                            println!("{}", warn("⚠ Batch dở dang này không còn bài nào để tải."));
                            continue;
                        }
                        let confirm_resume = match prompt_skippable(
                            Confirm::new(&format!("Tiếp tục tải {} bài còn dở?", tasks.len()))
                                .with_default(true)
                                .prompt(),
                        )? {
                            Some(value) => value,
                            None => {
                                println!("{}", warn("↩ Đã hủy tiếp tục — quay lại menu chính."));
                                continue;
                            }
                        };
                        if !confirm_resume {
                            println!("{}", warn("↩ Đã bỏ qua tiếp tục batch dở dang."));
                            continue;
                        }
                        let summary_dir = settings.output_dir.clone();
                        let processor = BatchProcessor::new(
                            settings,
                            &ytdlp_path,
                            &ffmpeg_path,
                            has_node_runtime,
                        );
                        let outcome = processor.execute_batch(&tasks).await?;
                        Self::print_batch_summary(&outcome, &summary_dir);
                    }
                    Err(_) => {
                        println!(
                            "{}",
                            warn("ℹ Không có batch dở dang nào trong thư mục lưu hiện tại.")
                        );
                    }
                }
            } else if choice.starts_with("📜") {
                Self::print_recent_history(&active_output_dir);
            } else if choice.starts_with("🔄") {
                println!(
                    "\n{}",
                    accent("⏳ Đang kết nối GitHub và kiểm tra cập nhật yt-dlp...")
                );
                let ytdlp_for_update = ytdlp_path.clone();
                let update_result =
                    tokio::task::spawn_blocking(move || update_ytdlp(&ytdlp_for_update))
                        .await
                        .map_err(anyhow::Error::new)
                        .and_then(|inner| inner);
                match update_result {
                    Ok(msg) => {
                        println!("\n{} {}\n", ok("✔"), ok(&msg));
                    }
                    Err(e) => {
                        println!("\n{} {}\n", err("❌ Lỗi cập nhật:"), e);
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

    pub fn print_batch_summary(outcome: &crate::core::batch::BatchOutcome, output_dir: &Path) {
        let width = term_width();
        let success_count = outcome.success_count;
        let failed_count = outcome.failed_tasks.len();
        let total_secs = outcome.elapsed_secs as u64;
        let minutes = total_secs / 60;
        let seconds = total_secs % 60;

        println!("\n{}", accent(&frame_top("KẾT QUẢ TẢI HÀNG LOẠT", width)));
        println!("│ ✔ Thành công : {}", ok(&format!("{} bài", success_count)));
        println!(
            "│ ✖ Thất bại   : {}",
            if failed_count > 0 {
                warn(&format!("{} bài", failed_count))
            } else {
                ok(&format!("{} bài", failed_count))
            }
        );
        println!(
            "│ ⏱ Thời gian  : {}",
            accent(&format!("{} min {} s", minutes, seconds))
        );
        println!(
            "│ 💾 Dung lượng: {}",
            accent(&Self::human_size(outcome.total_bytes))
        );
        if failed_count > 0 {
            let queue_path = output_dir.join(FAILED_QUEUE_FILE);
            println!(
                "│ 🔁 Thử lại   : {}",
                warn(&format!("Danh sách lỗi: {}", queue_path.display()))
            );
            println!(
                "│   {}",
                warn("Chọn \"🔁 Thử lại các bài lỗi gần nhất\" ở menu để tải lại.")
            );
        }
        println!("{}", accent(&frame_bottom(width)));
        println!();
    }

    fn human_size(bytes: u64) -> String {
        const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
        let mut value = bytes as f64;
        let mut unit = 0usize;
        while value >= 1024.0 && unit < UNITS.len() - 1 {
            value /= 1024.0;
            unit += 1;
        }
        if unit == 0 {
            format!("{} {}", bytes, UNITS[0])
        } else {
            format!("{:.2} {}", value, UNITS[unit])
        }
    }

    fn pending_task_count(dir: &Path) -> Option<usize> {
        let path = dir.join(PENDING_QUEUE_FILE);
        let content = std::fs::read_to_string(&path).ok()?;
        let value: serde_json::Value = serde_json::from_str(&content).ok()?;
        value
            .get("tasks")
            .and_then(|tasks| tasks.as_array())
            .map(|tasks| tasks.len())
    }

    fn print_recent_history(dir: &Path) {
        let recent = crate::core::manifest::read_recent_manifests(dir, RECENT_MANIFEST_LIMIT);
        if recent.is_empty() {
            println!(
                "{}",
                warn("ℹ Chưa có lịch sử tải nào (không tìm thấy tệp manifest) trong thư mục này.")
            );
            return;
        }
        let width = term_width();
        println!("\n{}", accent(&frame_top("LỊCH SỬ TẢI GẦN ĐÂY", width)));
        println!(
            "│ {:<20} {:>4} {:>4}  {:<10}",
            "Thời điểm", "OK", "Lỗi", "Định dạng"
        );
        for manifest in &recent {
            let format_short = manifest
                .format
                .split(" - ")
                .next()
                .unwrap_or(&manifest.format)
                .to_string();
            let left = format!(
                "│ {:<20} {:>4} {:>4}  {:<10} ",
                manifest.timestamp_label,
                manifest.success_count,
                manifest.failed_count,
                format_short
            );
            let dir_display = middle_truncate(
                &manifest.output_dir,
                width.saturating_sub(left.chars().count()),
            );
            println!("{}{}", left, accent(&dir_display));
            println!("    {}", manifest.path.bright_black());
        }
        println!("{}", accent(&frame_bottom(width)));
        println!();
    }

    fn print_dashboard(
        dir: &Path,
        format: AudioFormat,
        quality: AudioQuality,
        resolution: Option<u32>,
        keep_accents: bool,
        fetch_lyrics: bool,
    ) {
        let width = term_width();
        let quality_label = match format {
            AudioFormat::Video => match resolution {
                Some(r) => format!("Độ phân giải {}p (giữ nguyên gốc)", r),
                None => "Độ phân giải cao nhất (giữ nguyên gốc)".to_string(),
            },
            AudioFormat::Original => "Giữ nguyên luồng âm thanh gốc".to_string(),
            _ => quality.to_string(),
        };
        let accent_label = if keep_accents {
            "Có dấu (Unicode)".to_string()
        } else {
            "Khử dấu an toàn".to_string()
        };
        let lyrics_label = if fetch_lyrics {
            "Bật (LRCLIB)"
        } else {
            "Tắt"
        };

        let dir_prefix = "│ 📁 Thư mục lưu : ";
        let format_prefix = "│ 🎵 Định dạng   : ";
        let quality_prefix = "│ 🎚 Chất lượng  : ";
        let ext = quality.file_extension(format);
        let format_label = format!("{} (.{})", format, ext);
        let dir_display = middle_truncate(
            &dir.display().to_string(),
            width.saturating_sub(dir_prefix.chars().count()),
        );
        let format_display = middle_truncate(
            &format_label,
            width.saturating_sub(format_prefix.chars().count()),
        );
        let quality_display = middle_truncate(
            &quality_label,
            width.saturating_sub(quality_prefix.chars().count()),
        );

        println!("\n{}", accent(&frame_top("CẤU HÌNH ĐANG SỬ DỤNG", width)));
        println!("{}{}", dir_prefix, warn(&dir_display));
        println!("{}{}", format_prefix, ok(&format_display));
        println!("{}{}", quality_prefix, accent(&quality_display));
        println!("│ 🔤 Tên tệp     : {}", accent_label.bright_white());
        println!("│ 🎵 Lời bài hát : {}", lyrics_label.bright_white());
        println!("{}", accent(&frame_bottom(width)));
        println!();
    }

    fn print_banner() {
        const WIDTH: usize = 62;
        let title = format!("GHITA DOWNLOADER ({})", APP_VERSION);
        let title_width = title.chars().count();
        let remaining = WIDTH.saturating_sub(title_width).saturating_sub(4);
        let left = remaining / 2;
        let right = remaining - left;
        println!("{}", format!("╔{}╗", "═".repeat(WIDTH)).cyan());
        println!(
            "{}",
            format!(
                "║{}{}{}║",
                " ".repeat(left + 2),
                title,
                " ".repeat(right + 2)
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
        println!("{}", format!("╚{}╝", "═".repeat(WIDTH)).cyan());
        println!();
    }

    async fn run_settings_flow(
        config: &mut AppConfig,
        active_output_dir: &mut PathBuf,
        active_format: &mut AudioFormat,
        active_quality: &mut AudioQuality,
        active_resolution: &mut Option<u32>,
    ) -> Result<()> {
        let cancel_message = "↩ Đã hủy thay đổi cài đặt — quay lại menu chính.";
        println!("\n{}", accent("⚙ THAY ĐỔI CÀI ĐẶT ỨNG DỤNG:"));

        let default_music = AppConfig::system_music_dir();
        let prompt_label = format!(
            "📁 Chọn hoặc nhập/dán thư mục lưu (Mặc định: {}):",
            default_music.display()
        );
        let new_dir_str = match prompt_skippable(
            Text::new(&prompt_label)
                .with_default(&active_output_dir.display().to_string())
                .with_help_message(
                    "Nhấn Enter để giữ nguyên, dán đường dẫn mới, hoặc nhập 'default' để dùng thư mục Music của máy",
                )
                .prompt(),
        )? {
            Some(value) => value,
            None => {
                println!("{}", warn(cancel_message));
                return Ok(());
            }
        };
        let trimmed = new_dir_str.trim();
        let new_dir =
            if trimmed.eq_ignore_ascii_case("default") || trimmed.eq_ignore_ascii_case("music") {
                default_music
            } else if trimmed.is_empty() {
                active_output_dir.clone()
            } else {
                clean_path(Path::new(trimmed))
            };
        *active_output_dir = new_dir;

        let format_options = vec![
            AudioFormat::Mp3,
            AudioFormat::Wav,
            AudioFormat::Flac,
            AudioFormat::Aac,
            AudioFormat::Opus,
            AudioFormat::Original,
            AudioFormat::Video,
        ];
        let new_format = match prompt_skippable(
            Select::new("🎵 Chọn định dạng đầu ra:", format_options)
                .with_help_message("Dùng phím mũi tên Lên/Xuống để chọn")
                .prompt(),
        )? {
            Some(value) => value,
            None => {
                println!("{}", warn(cancel_message));
                return Ok(());
            }
        };
        *active_format = new_format;

        match new_format {
            AudioFormat::Mp3
            | AudioFormat::Wav
            | AudioFormat::Flac
            | AudioFormat::Aac
            | AudioFormat::Opus => {
                let quality_options = match new_format {
                    AudioFormat::Mp3 => AudioQuality::all_mp3(),
                    AudioFormat::Wav => AudioQuality::all_wav(),
                    AudioFormat::Flac => AudioQuality::all_flac(),
                    AudioFormat::Aac => AudioQuality::all_aac(),
                    AudioFormat::Opus => AudioQuality::all_opus(),
                    _ => AudioQuality::all_mp3(),
                };
                let new_quality = match prompt_skippable(
                    Select::new("🎚 Chọn mức chất lượng âm thanh:", quality_options)
                        .with_help_message("Chọn mức chất lượng theo nhu cầu")
                        .prompt(),
                )? {
                    Some(value) => value,
                    None => {
                        println!("{}", warn(cancel_message));
                        return Ok(());
                    }
                };
                *active_quality = new_quality;
                config
                    .update_last_used(active_output_dir, *active_format, *active_quality)
                    .map_err(anyhow::Error::msg)?;
            }
            AudioFormat::Original => {
                config
                    .update_last_used(active_output_dir, *active_format, *active_quality)
                    .map_err(anyhow::Error::msg)?;
            }
            AudioFormat::Video => {
                let res_options = vec!["1080p", "720p", "480p", "Best (cao nhất)"];
                let res_choice = match prompt_skippable(
                    Select::new("🎬 Chọn độ phân giải video:", res_options)
                        .with_help_message(
                            "Video được giữ nguyên chất lượng nguồn không vượt quá mức chọn",
                        )
                        .prompt(),
                )? {
                    Some(value) => value,
                    None => {
                        println!("{}", warn(cancel_message));
                        return Ok(());
                    }
                };
                *active_resolution = match res_choice {
                    "1080p" => Some(1080),
                    "720p" => Some(720),
                    "480p" => Some(480),
                    _ => None,
                };
                config
                    .set_video_resolution(*active_resolution)
                    .map_err(anyhow::Error::msg)?;
                config
                    .update_last_used(active_output_dir, *active_format, *active_quality)
                    .map_err(anyhow::Error::msg)?;
            }
        }

        let accent_options = vec![
            "Khử dấu an toàn (Khuyên dùng cho USB, ô tô, loa cũ, máy nghe nhạc)",
            "Giữ nguyên dấu tiếng Việt Unicode (Đẹp trên Windows, điện thoại)",
        ];
        let default_accent_cursor = if config.keep_accents { 1 } else { 0 };
        let accent_choice = match prompt_skippable(
            Select::new("🔤 Kiểu đặt tên tệp tiếng Việt:", accent_options)
                .with_starting_cursor(default_accent_cursor)
                .prompt(),
        )? {
            Some(value) => value,
            None => {
                println!("{}", warn(cancel_message));
                return Ok(());
            }
        };
        config
            .set_keep_accents(accent_choice.starts_with("Giữ"))
            .map_err(anyhow::Error::msg)?;

        let lyrics_options = vec![
            "Tự động tải & nhúng lời bài hát (LRCLIB, kèm .lrc đồng bộ)",
            "Tắt tự động tải lời bài hát",
        ];
        let default_lyrics_cursor = if config.fetch_lyrics { 0 } else { 1 };
        let lyrics_choice = match prompt_skippable(
            Select::new("🎵 Tự động tải lời bài hát:", lyrics_options)
                .with_starting_cursor(default_lyrics_cursor)
                .prompt(),
        )? {
            Some(value) => value,
            None => {
                println!("{}", warn(cancel_message));
                return Ok(());
            }
        };
        config
            .set_fetch_lyrics(lyrics_choice.starts_with("Tự động"))
            .map_err(anyhow::Error::msg)?;

        println!("\n{} Đã cập nhật và áp dụng cài đặt thành công!\n", ok("✔"));
        Ok(())
    }
}
