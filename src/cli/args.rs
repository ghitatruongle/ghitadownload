use clap::Parser;
use colored::Colorize;
use std::path::PathBuf;

use super::multi_input::MultiLinkInput;
use super::CliApp;
use crate::core::batch::{BatchProcessor, DEFAULT_CONCURRENCY};
use crate::core::config::{AppConfig, AudioFormat, AudioQuality, DownloadSettings};

#[derive(Parser, Debug)]
#[command(
    name = "ghitadownload",
    version = env!("CARGO_PKG_VERSION"),
    about = "🎵 Ghita Downloader - Tải nhạc YouTube, Spotify, SoundCloud, Bandcamp, Suno AI, TikTok, Facebook, Instagram, Threads",
    long_about = "🎵 Ghita Downloader (CLI)\n\nSử dụng:\n  ghitadownload                        Khởi động giao diện tương tác\n  ghitadownload --version              Xem phiên bản hiện tại\n  ghitadownload --help                 Xem trợ giúp đầy đủ\n\nChế độ tải nhanh (headless):\n  ghitadownload --link <URL> [<URL>...] --output <THƯ_MỤC> --format mp3|wav|original|video --quality 320k\n  ghitadownload --file links.txt --format original\n  ghitadownload --retry-failed --output <THƯ_MỤC_CHỨA_failed_tasks.json>"
)]
pub struct CliArgs {
    #[arg(long = "link", visible_alias = "links", num_args = 1.., value_name = "URL")]
    pub links: Vec<String>,

    #[arg(long, value_name = "FILE")]
    pub file: Option<PathBuf>,

    #[arg(long, value_name = "DIR")]
    pub output: Option<PathBuf>,

    #[arg(long, value_name = "FMT")]
    pub format: Option<String>,

    #[arg(long, value_name = "QUALITY")]
    pub quality: Option<String>,

    #[arg(long, value_name = "RES")]
    pub resolution: Option<String>,

    #[arg(long, value_name = "N")]
    pub concurrency: Option<usize>,

    #[arg(long)]
    pub retry_failed: bool,

    #[arg(long = "update-ytdlp", visible_alias = "update-tools")]
    pub update_ytdlp: bool,

    #[arg(long = "keep-accents")]
    pub keep_accents: bool,
}

impl CliArgs {
    pub fn is_headless(&self) -> bool {
        self.update_ytdlp
            || !self.links.is_empty()
            || self.file.is_some()
            || self.retry_failed
            || self.output.is_some()
            || self.format.is_some()
            || self.quality.is_some()
            || self.resolution.is_some()
            || self.concurrency.is_some()
            || self.keep_accents
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.update_ytdlp {
            let has_conflict = !self.links.is_empty()
                || self.file.is_some()
                || self.output.is_some()
                || self.format.is_some()
                || self.quality.is_some()
                || self.resolution.is_some()
                || self.concurrency.is_some()
                || self.retry_failed
                || self.keep_accents;
            return if has_conflict {
                Err("--update-ytdlp phải được chạy riêng, không kết hợp thao tác tải hoặc tùy chọn khác".to_string())
            } else {
                Ok(())
            };
        }

        if self.concurrency == Some(0) {
            return Err("--concurrency phải lớn hơn 0".to_string());
        }

        if self.retry_failed {
            let has_conflict = !self.links.is_empty()
                || self.file.is_some()
                || self.format.is_some()
                || self.quality.is_some()
                || self.resolution.is_some()
                || self.concurrency.is_some()
                || self.keep_accents;
            return if has_conflict {
                Err("--retry-failed chỉ chấp nhận --output và không được kết hợp nguồn hoặc tùy chọn định dạng".to_string())
            } else {
                Ok(())
            };
        }

        if self.links.is_empty() && self.file.is_none() {
            return Err("Cần ít nhất một nguồn: --link <URL> hoặc --file <FILE>".to_string());
        }

        if let Some(value) = self.format.as_deref() {
            let format = value.parse::<AudioFormat>()?;
            if matches!(format, AudioFormat::Original | AudioFormat::Video)
                && self.quality.is_some()
            {
                return Err(format!(
                    "--quality không được dùng với định dạng {}",
                    value.to_ascii_lowercase()
                ));
            }
            if !matches!(format, AudioFormat::Video) && self.resolution.is_some() {
                return Err("--resolution chỉ được dùng với --format video".to_string());
            }
        }

        if self.resolution.is_some() && self.format.is_none() {
            return Err("--resolution yêu cầu --format video".to_string());
        }

        if let Some(value) = self.quality.as_deref() {
            let quality = value.parse::<AudioQuality>()?;
            if let Some(format_value) = self.format.as_deref() {
                let format = format_value.parse::<AudioFormat>()?;
                if !format.is_compatible_quality(quality) {
                    return Err(format!(
                        "Chất lượng {} không tương thích với định dạng {}",
                        value.to_ascii_lowercase(),
                        format_value.to_ascii_lowercase()
                    ));
                }
            }
        }

        if let Some(value) = self.resolution.as_deref() {
            match value.to_ascii_lowercase().as_str() {
                "best" | "max" | "1080" | "720" | "480" => {}
                _ => {
                    return Err(format!(
                        "Độ phân giải không hợp lệ: {} (chấp nhận: 1080, 720, 480, best)",
                        value
                    ));
                }
            }
        }

        Ok(())
    }

    pub async fn run_headless(&self) -> i32 {
        if let Err(message) = self.validate() {
            eprintln!("❌ {message}");
            return 1;
        }

        if self.update_ytdlp {
            let path = crate::utils::env::ytdlp_update_path();
            println!("{}", "⏳ Đang kiểm tra và cập nhật yt-dlp...".cyan());
            return match crate::utils::env::update_ytdlp(&path) {
                Ok(msg) => {
                    println!("{} {}", "✔".green().bold(), msg);
                    0
                }
                Err(e) => {
                    eprintln!("{} {}", "❌".red(), e);
                    1
                }
            };
        }

        let config = match AppConfig::try_load() {
            Ok(config) => config,
            Err(message) => {
                eprintln!("❌ {message}");
                return 1;
            }
        };
        let fallback_dir =
            std::env::current_dir().unwrap_or_else(|_| AppConfig::system_music_dir());
        let output_dir = self
            .output
            .clone()
            .unwrap_or_else(|| config.initial_output_dir(fallback_dir));
        let concurrency = self.concurrency.unwrap_or(DEFAULT_CONCURRENCY);

        let mut raw = self.links.clone();
        if let Some(file) = &self.file {
            match std::fs::read_to_string(file) {
                Ok(content) => raw.extend(
                    content
                        .lines()
                        .map(str::trim)
                        .filter(|line| !line.is_empty())
                        .map(str::to_string),
                ),
                Err(error) => {
                    eprintln!(
                        "❌ Không đọc được tệp liên kết {}: {}",
                        file.display(),
                        error
                    );
                    return 1;
                }
            }
        }
        let links = MultiLinkInput::process_links(raw);
        if !self.retry_failed && links.is_empty() {
            eprintln!("❌ Không có liên kết hợp lệ trong đầu vào");
            return 1;
        }

        let env = match CliApp::prepare_env_headless().await {
            Some(env) => env,
            None => return 1,
        };

        let (settings, tasks) = if self.retry_failed {
            match BatchProcessor::retry_from_file(&output_dir) {
                Ok((mut settings, tasks)) => {
                    settings.concurrency = concurrency;
                    settings.keep_accents = self.keep_accents || settings.keep_accents;
                    (settings, tasks)
                }
                Err(error) => {
                    eprintln!("❌ {error}");
                    return 1;
                }
            }
        } else {
            let format = match self.format.as_deref() {
                Some(value) => match value.parse::<AudioFormat>() {
                    Ok(format) => format,
                    Err(error) => {
                        eprintln!("❌ {error}");
                        return 1;
                    }
                },
                None => config.last_format.unwrap_or(AudioFormat::Mp3),
            };
            let quality = match self.quality.as_deref() {
                Some(value) => match value.parse::<AudioQuality>() {
                    Ok(quality) => Some(quality),
                    Err(error) => {
                        eprintln!("❌ {error}");
                        return 1;
                    }
                },
                None => match format {
                    AudioFormat::Original | AudioFormat::Video => None,
                    _ => Some(
                        config
                            .last_quality
                            .filter(|quality| format.is_compatible_quality(*quality))
                            .unwrap_or_else(|| format.default_quality()),
                    ),
                },
            };
            if quality.is_some_and(|quality| !format.is_compatible_quality(quality)) {
                eprintln!(
                    "❌ Chất lượng đã chọn không tương thích với định dạng {}",
                    format.to_string().to_ascii_lowercase()
                );
                return 1;
            }
            let video_resolution = if format == AudioFormat::Video {
                match self.resolution.as_deref() {
                    Some("best") | Some("max") | Some("BEST") | Some("MAX") => None,
                    Some(value) => match value.parse::<u32>() {
                        Ok(resolution) => Some(resolution),
                        Err(_) => {
                            eprintln!("❌ Độ phân giải không hợp lệ: {value}");
                            return 1;
                        }
                    },
                    None => config.video_resolution,
                }
            } else {
                None
            };
            let settings = DownloadSettings {
                output_dir,
                format,
                quality,
                video_resolution,
                concurrency,
                keep_accents: self.keep_accents || config.keep_accents,
            };
            let processor = BatchProcessor::new(
                settings.clone(),
                &env.ytdlp_path,
                &env.ffmpeg_path,
                env.has_node,
            );
            match processor.resolve_inputs(&links).await {
                Ok(tasks) => (settings, tasks),
                Err(error) => {
                    eprintln!("❌ {error}");
                    return 1;
                }
            }
        };

        if tasks.is_empty() {
            eprintln!("⚠ Không trích xuất được bài hát nào từ đầu vào");
            return 1;
        }

        let processor =
            BatchProcessor::new(settings, &env.ytdlp_path, &env.ffmpeg_path, env.has_node);
        match processor.execute_batch(&tasks).await {
            Ok(outcome) if outcome.failed_tasks.is_empty() => 0,
            Ok(_) => 1,
            Err(error) => {
                eprintln!("❌ {error}");
                1
            }
        }
    }
}
