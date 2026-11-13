use anyhow::{anyhow, Result};
use indicatif::ProgressBar;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::LazyLock;
use tokio::io::AsyncBufReadExt;

use crate::core::ytdlp::YTDLP_DEFAULT_ARGS;

static DOWNLOAD_SEQ: AtomicU64 = AtomicU64::new(0);

const MAX_ATTEMPTS: u32 = 3;
const WATCHDOG_TICK_SECS: u64 = 30;
const WATCHDOG_IDLE_SECS: u64 = 90;
const WATCHDOG_ATTEMPT_SECS: u64 = 600;

pub(crate) fn backoff_ms(attempt: u32) -> u64 {
    let base = 800u64.saturating_mul(2u64.saturating_pow(attempt));
    let jitter = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        % 200) as u64;
    base.saturating_add(jitter)
}

static PCT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[download\]\s+([0-9.]+)%").unwrap());
static SIZE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"of\s+~?\s*([\d.]+)\s*(Bytes|KiB|MiB|GiB|TiB|kB|KB|MB|GB)").unwrap()
});
static INTERMEDIATE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.[fF]\d+\.").unwrap());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadMode {
    Audio,
    Video(Option<u32>),
}

pub struct StreamDownloader {
    ytdlp_path: PathBuf,
    has_node: bool,
    cookies_file: Option<PathBuf>,
    cookies_from_browser: Option<String>,
}

impl StreamDownloader {
    pub fn new(
        ytdlp_path: &Path,
        has_node: bool,
        cookies_file: Option<PathBuf>,
        cookies_from_browser: Option<String>,
    ) -> Self {
        Self {
            ytdlp_path: ytdlp_path.to_path_buf(),
            has_node,
            cookies_file,
            cookies_from_browser,
        }
    }

    pub async fn download_stream(
        &self,
        target_url_or_search: &str,
        temp_dir: &Path,
        mode: DownloadMode,
        progress: Option<&ProgressBar>,
        cancel: tokio::sync::watch::Receiver<bool>,
    ) -> Result<PathBuf> {
        let mut last_err = String::new();
        let seq = DOWNLOAD_SEQ.fetch_add(1, Ordering::Relaxed);
        let unique_id = format!(
            "dl_{}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            seq
        );

        for attempt in 1..=MAX_ATTEMPTS {
            if *cancel.borrow() {
                return Err(anyhow!("Đã hủy"));
            }
            let output_template = temp_dir.join(format!("{}.%(ext)s", unique_id));
            let output_template_str = output_template.to_string_lossy();

            let mut cmd = tokio::process::Command::new(&self.ytdlp_path);
            let mut args: Vec<String> = Vec::new();
            if self.has_node {
                args.push("--js-runtimes".to_string());
                args.push("node".to_string());
            }

            match mode {
                DownloadMode::Audio => {
                    args.push("-f".to_string());
                    args.push("ba/ba*/b/best".to_string());
                }
                DownloadMode::Video(resolution) => {
                    match resolution {
                        Some(h) => {
                            args.push("-f".to_string());
                            args.push(format!("bv*[height<={}]+ba/b[height<={}]/b", h, h));
                        }
                        None => {
                            args.push("-f".to_string());
                            args.push("bv+ba/b".to_string());
                        }
                    }
                    args.push("-S".to_string());
                    args.push("res,ext:mp4:m4a".to_string());
                    args.push("--merge-output-format".to_string());
                    args.push("mp4".to_string());
                }
            }

            args.push("--no-playlist".to_string());
            args.push("--continue".to_string());
            args.extend(YTDLP_DEFAULT_ARGS.iter().map(|s| s.to_string()));
            if let Some(path) = &self.cookies_file {
                args.push("--cookies".to_string());
                args.push(path.to_string_lossy().into_owned());
            }
            if let Some(browser) = &self.cookies_from_browser {
                args.push("--cookies-from-browser".to_string());
                args.push(browser.clone());
            }
            args.push("--socket-timeout".to_string());
            args.push("30".to_string());
            args.push("--retries".to_string());
            args.push("10".to_string());
            args.push("--fragment-retries".to_string());
            args.push("10".to_string());
            args.push("--concurrent-fragments".to_string());
            args.push("8".to_string());
            args.push("--buffer-size".to_string());
            args.push("64k".to_string());
            args.push("--no-mtime".to_string());
            args.push("--file-access-retries".to_string());
            args.push("3".to_string());
            args.push("-o".to_string());
            args.push(output_template_str.into_owned());
            args.push("--".to_string());
            args.push(target_url_or_search.to_string());
            cmd.args(&args);

            cmd.stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped());

            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    last_err = format!("Không thể thực thi yt-dlp: {}", e);
                    continue;
                }
            };

            if let Some(bar) = progress {
                bar.set_length(0);
            }

            let stderr = child
                .stderr
                .take()
                .ok_or_else(|| anyhow!("Mất luồng stderr của yt-dlp"))?;
            let mut total_bytes: u64 = 0;
            let mut err_lines: Vec<String> = Vec::new();
            let mut cancel = cancel.clone();
            let mut lines = tokio::io::BufReader::new(stderr).lines();
            let started = std::time::Instant::now();
            let mut last_activity = std::time::Instant::now();
            let mut watchdog_msg: Option<String> = None;
            let mut watchdog_tick;
            let mut post_process = false;

            loop {
                watchdog_tick = false;
                let line = tokio::select! {
                    line = lines.next_line() => match line {
                        Ok(Some(line)) => Some(line),
                        Ok(None) => None,
                        Err(_) => None,
                    },
                    _ = cancel.changed() => {
                        let _ = child.kill().await;
                        return Err(anyhow!("Đã hủy"));
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs(WATCHDOG_TICK_SECS)) => {
                        watchdog_tick = true;
                        None
                    }
                };
                let Some(line) = line else {
                    if watchdog_tick {
                        let idle = if post_process {
                            std::time::Duration::ZERO
                        } else {
                            last_activity.elapsed()
                        };
                        if let Some(message) = watchdog_timeout(idle, started.elapsed()) {
                            let _ = child.kill().await;
                            watchdog_msg = Some(message.to_string());
                            break;
                        }
                        continue;
                    }
                    break;
                };
                last_activity = std::time::Instant::now();
                if line.contains("[Merger]")
                    || line.contains("[ExtractAudio]")
                    || line.contains("100% of")
                {
                    post_process = true;
                }
                {
                    if total_bytes == 0 {
                        if let Some(c) = SIZE_RE.captures(&line) {
                            total_bytes = parse_size_bytes(&c[1], &c[2]);
                            if total_bytes > 0 {
                                if let Some(bar) = progress {
                                    bar.set_length(total_bytes);
                                }
                            }
                        }
                    }
                    if let Some(c) = PCT_RE.captures(&line) {
                        let pct: f64 = c[1].parse().unwrap_or(0.0);
                        if let Some(bar) = progress {
                            if total_bytes > 0 {
                                bar.set_position(((pct / 100.0) * total_bytes as f64) as u64);
                            } else {
                                bar.inc(1);
                            }
                        }
                    }
                    if is_error_context_line(&line) {
                        if err_lines.len() >= 12 {
                            err_lines.remove(0);
                        }
                        err_lines.push(line);
                    }
                }
                if let Some(message) =
                    watchdog_timeout(std::time::Duration::ZERO, started.elapsed())
                {
                    let _ = child.kill().await;
                    watchdog_msg = Some(message.to_string());
                    break;
                }
            }

            let status = tokio::select! {
                status = child.wait() => status?,
                _ = cancel.changed() => {
                    let _ = child.kill().await;
                    return Err(anyhow!("Đã hủy"));
                }
            };

            if status.success() {
                let mut candidates = Vec::new();
                if let Ok(entries) = std::fs::read_dir(temp_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
                            continue;
                        };
                        if !file_name.starts_with(&unique_id)
                            || file_name.ends_with(".part")
                            || file_name.ends_with(".ytdl")
                            || file_name.ends_with(".temp")
                            || INTERMEDIATE_RE.is_match(file_name)
                        {
                            continue;
                        }
                        let Ok(metadata) = std::fs::metadata(&path) else {
                            continue;
                        };
                        if metadata.is_file()
                            && metadata.len() > 0
                            && path.extension().and_then(|e| e.to_str()).is_some()
                        {
                            candidates.push((path, metadata.len()));
                        }
                    }
                }
                if let Some((path, _)) = candidates.into_iter().max_by_key(|(_, size)| *size) {
                    return Ok(path);
                }
                last_err =
                    "Không tìm thấy tệp media hoàn chỉnh sau khi yt-dlp kết thúc".to_string();
            } else if let Some(message) = watchdog_msg {
                last_err = message;
            } else {
                last_err = err_lines.join(" | ");
                if last_err.trim().is_empty() {
                    last_err = format!("yt-dlp thoát với mã lỗi {:?}", status.code());
                }
            }

            if attempt < MAX_ATTEMPTS {
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_millis(backoff_ms(attempt - 1))) => {}
                    _ = cancel.changed() => {
                        return Err(anyhow!("Đã hủy"));
                    }
                }
            }
        }

        Err(anyhow!("Lỗi tải qua yt-dlp: {}", last_err))
    }
}

fn parse_size_bytes(num: &str, unit: &str) -> u64 {
    let value: f64 = num.parse().unwrap_or(0.0);
    let mult: f64 = match unit {
        "Bytes" => 1.0,
        "KiB" => 1024.0,
        "MiB" => 1024.0 * 1024.0,
        "GiB" => 1024.0 * 1024.0 * 1024.0,
        "TiB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        "kB" | "KB" => 1_000.0,
        "MB" => 1_000_000.0,
        "GB" => 1_000_000_000.0,
        _ => 1.0,
    };
    (value * mult) as u64
}

fn is_error_context_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    let upper = trimmed.to_ascii_uppercase();
    if upper.starts_with("WARNING")
        || upper.contains(" WARNING")
        || upper.contains("[WARNING")
        || upper.contains("(WARNING")
    {
        return false;
    }
    upper.contains("ERROR")
}

fn watchdog_timeout(
    idle: std::time::Duration,
    attempt: std::time::Duration,
) -> Option<&'static str> {
    if idle > std::time::Duration::from_secs(WATCHDOG_IDLE_SECS) {
        Some("yt-dlp không có tiến triển trong 90 giây (watchdog)")
    } else if attempt > std::time::Duration::from_secs(WATCHDOG_ATTEMPT_SECS) {
        Some("yt-dlp vượt quá 10 phút cho một lượt tải (watchdog)")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{backoff_ms, is_error_context_line, watchdog_timeout};

    #[test]
    fn backoff_ms_increases_monotonically() {
        let first = backoff_ms(0);
        let second = backoff_ms(1);
        let third = backoff_ms(2);
        assert!((800..1000).contains(&first));
        assert!(first < second);
        assert!(second < third);
        assert!(third < 3400);
    }

    #[test]
    fn error_context_keeps_error_lines() {
        assert!(is_error_context_line("ERROR: [youtube] video unavailable"));
        assert!(is_error_context_line(
            "  ERROR: sign in to confirm your age  "
        ));
        assert!(is_error_context_line(
            "fatal: unable to connect ERROR state"
        ));
        assert!(is_error_context_line(
            "[youtube] xxx: ERROR during extraction"
        ));
    }

    #[test]
    fn error_context_skips_warnings_blanks_and_noise() {
        assert!(!is_error_context_line(
            "WARNING: [youtube] throttling detected"
        ));
        assert!(!is_error_context_line("warning: precheck skipped"));
        assert!(!is_error_context_line(
            "extractor WARNING unexpectedly ignored ERROR reported"
        ));
        assert!(!is_error_context_line(""));
        assert!(!is_error_context_line("   "));
        assert!(!is_error_context_line("[download]  42.3% of 4.50MiB"));
        assert!(!is_error_context_line("[info] Writing video metadata"));
    }

    #[test]
    fn watchdog_timeout_flags_idle_and_attempt_caps() {
        assert_eq!(
            watchdog_timeout(
                std::time::Duration::from_secs(91),
                std::time::Duration::from_secs(5)
            ),
            Some("yt-dlp không có tiến triển trong 90 giây (watchdog)")
        );
        assert_eq!(
            watchdog_timeout(
                std::time::Duration::from_secs(10),
                std::time::Duration::from_secs(601)
            ),
            Some("yt-dlp vượt quá 10 phút cho một lượt tải (watchdog)")
        );
        assert_eq!(
            watchdog_timeout(
                std::time::Duration::from_secs(30),
                std::time::Duration::from_secs(599)
            ),
            None
        );
        assert_eq!(
            watchdog_timeout(
                std::time::Duration::ZERO,
                std::time::Duration::from_secs(601)
            ),
            Some("yt-dlp vượt quá 10 phút cho một lượt tải (watchdog)")
        );
    }
}
