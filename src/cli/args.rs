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
    long_about = "🎵 Ghita Downloader (CLI)\n\nSử dụng:\n  ghitadownload                        Khởi chạy giao diện tương tác\n  ghitadownload --version              Xem phiên bản hiện tại\n  ghitadownload --help                 Xem trợ giúp đầy đủ\n\nChế độ tải nhanh (headless):\n  ghitadownload --link <URL> [<URL>...] --output <THƯ_MỤC> --format mp3|wav|original|video --quality 320k\n  ghitadownload --file links.txt --format original\n  ghitadownload --retry-failed --output <THƯ_MỤC_CHỨA_failed_tasks.json>"
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

    #[arg(long, default_value_t = DEFAULT_CONCURRENCY, value_name = "N")]
    pub concurrency: usize,

    #[arg(long)]
    pub retry_failed: bool,

    #[arg(long = "update-ytdlp", visible_alias = "update-tools")]
    pub update_ytdlp: bool,

    #[arg(long = "keep-accents")]
    pub keep_accents: bool,
}

impl CliArgs {
    pub fn is_headless(&self) -> bool {
        self.update_ytdlp || !self.links.is_empty() || self.file.is_some() || self.retry_failed
    }

    pub async fn run_headless(&self) -> i32 {
        let env = match CliApp::prepare_env().await {
            Some(e) => e,
            None => return 1,
        };

        if self.update_ytdlp {
            println!("{}", "⏳ Đang kiểm tra và cập nhật yt-dlp...".cyan());
            match crate::utils::env::update_ytdlp(&env.ytdlp_path) {
                Ok(msg) => {
                    println!("{} {}", "✔".green().bold(), msg);
                    return 0;
                }
                Err(e) => {
                    eprintln!("{} {}", "❌".red(), e);
                    return 1;
                }
            }
        }

        let config = AppConfig::load();
        let output_dir = self
            .output
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let format = match self.format.as_deref() {
            Some(s) => match s.parse::<AudioFormat>() {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("{}", format!("❌ {}", e).red());
                    return 1;
                }
            },
            None => config.last_format.unwrap_or(AudioFormat::Mp3),
        };

        let quality = match self.quality.as_deref() {
            Some(s) => match s.parse::<AudioQuality>() {
                Ok(q) => Some(q),
                Err(e) => {
                    eprintln!("{}", format!("❌ {}", e).red());
                    return 1;
                }
            },
            None => match format {
                AudioFormat::Original | AudioFormat::Video => None,
                _ => {
                    let last_q = config
                        .last_quality
                        .filter(|q| format.is_compatible_quality(*q));
                    Some(last_q.unwrap_or_else(|| format.default_quality()))
                }
            },
        };

        let video_resolution = match self.resolution.as_deref() {
            Some(s) => match s.to_ascii_lowercase().as_str() {
                "best" | "max" => None,
                other => match other.parse::<u32>() {
                    Ok(r) => Some(r),
                    Err(_) => {
                        eprintln!("{}", format!("❌ Độ phân giải không hợp lệ: {} (chấp nhận: 1080, 720, 480, best)", other).red());
                        return 1;
                    }
                },
            },
            None => config.video_resolution,
        };

        let keep_accents = self.keep_accents || config.keep_accents;
        let settings = DownloadSettings {
            output_dir: output_dir.clone(),
            format,
            quality,
            video_resolution,
            concurrency: self.concurrency,
            keep_accents,
        };

        let (settings, tasks) = if self.retry_failed {
            match BatchProcessor::retry_from_file(&output_dir) {
                Ok(pair) => pair,
                Err(e) => {
                    eprintln!("{}", format!("❌ {}", e).red());
                    return 1;
                }
            }
        } else {
            let mut raw = self.links.clone();
            if let Some(f) = &self.file {
                match std::fs::read_to_string(f) {
                    Ok(content) => raw.extend(content.lines().map(|l| l.to_string())),
                    Err(e) => {
                        eprintln!(
                            "{}",
                            format!("❌ Không đọc được tệp liên kết {}: {}", f.display(), e).red()
                        );
                        return 1;
                    }
                }
            }
            let links = MultiLinkInput::process_links(raw);
            if links.is_empty() {
                eprintln!(
                    "{}",
                    "❌ Không có liên kết nào hợp lệ. Dùng --link <URL> hoặc --file links.txt"
                        .yellow()
                );
                return 1;
            }
            let processor =
                BatchProcessor::new(settings, &env.ytdlp_path, &env.ffmpeg_path, env.has_node);
            match processor.resolve_inputs(&links).await {
                Ok(t) => {
                    drop(processor);
                    (
                        DownloadSettings {
                            output_dir,
                            format,
                            quality,
                            video_resolution,
                            concurrency: self.concurrency,
                            keep_accents,
                        },
                        t,
                    )
                }
                Err(e) => {
                    eprintln!("{}", format!("❌ {}", e).red());
                    return 1;
                }
            }
        };

        if tasks.is_empty() {
            eprintln!(
                "{}",
                "⚠ Không trích xuất được bài hát nào từ liên kết đã nhập.".yellow()
            );
            return 1;
        }

        let processor =
            BatchProcessor::new(settings, &env.ytdlp_path, &env.ffmpeg_path, env.has_node);

        match processor.execute_batch(&tasks).await {
            Ok(outcome) => {
                if outcome.failed_tasks.is_empty() {
                    0
                } else {
                    1
                }
            }
            Err(e) => {
                eprintln!("{}", format!("❌ {}", e).red());
                1
            }
        }
    }
}
