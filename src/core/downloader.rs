use anyhow::{anyhow, Result};
use indicatif::ProgressBar;
use regex::Regex;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static DOWNLOAD_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadMode {
    Audio,
    Video(Option<u32>),
}

pub struct StreamDownloader {
    ytdlp_path: PathBuf,
    has_node: bool,
}

impl StreamDownloader {
    pub fn new(ytdlp_path: &Path, has_node: bool) -> Self {
        Self {
            ytdlp_path: ytdlp_path.to_path_buf(),
            has_node,
        }
    }

    pub fn download_stream(
        &self,
        target_url_or_search: &str,
        temp_dir: &Path,
        mode: DownloadMode,
        progress: Option<&ProgressBar>,
    ) -> Result<PathBuf> {
        let mut last_err = String::new();
        let pct_re = Regex::new(r"\[download\]\s+([0-9.]+)%")?;
        let size_re = Regex::new(r"of\s+~?\s*([\d.]+)\s*(Bytes|KiB|MiB|GiB|TiB|kB|KB|MB|GB)")?;
        let intermediate_re = Regex::new(r"\.[fF]\d+\.")?;

        for attempt in 1..=3 {
            let seq = DOWNLOAD_SEQ.fetch_add(1, Ordering::Relaxed);
            let unique_id = format!(
                "dl_{}_{}_{}_{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis(),
                seq,
                attempt
            );
            let output_template = temp_dir.join(format!("{}.%(ext)s", unique_id));
            let output_template_str = output_template.to_string_lossy();

            let mut cmd = Command::new(&self.ytdlp_path);
            if self.has_node {
                cmd.arg("--js-runtimes").arg("node");
            }

            match mode {
                DownloadMode::Audio => {
                    cmd.arg("-f").arg("ba/ba*/b/best");
                }
                DownloadMode::Video(resolution) => {
                    match resolution {
                        Some(h) => {
                            cmd.arg("-f")
                                .arg(format!("bv*[height<={}]+ba/b[height<={}]/b", h, h));
                        }
                        None => {
                            cmd.arg("-f").arg("bv+ba/b");
                        }
                    }
                    cmd.arg("-S").arg("res,ext:mp4:m4a");
                    cmd.arg("--merge-output-format").arg("mp4");
                }
            }

            cmd.arg("--no-playlist")
                .arg("--no-warnings")
                .arg("--socket-timeout").arg("30")
                .arg("--retries").arg("10")
                .arg("--fragment-retries").arg("10")
                .arg("--concurrent-fragments").arg("5")
                .arg("--buffer-size").arg("64k")
                .arg("--no-mtime")
                .arg("--file-access-retries").arg("3")
                .arg("--extractor-args").arg("youtube:player_client=android,web")
                .arg("--user-agent").arg("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
                .arg("-o").arg(output_template_str.as_ref())
                .arg("--")
                .arg(target_url_or_search);

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

            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if total_bytes == 0 {
                    if let Some(c) = size_re.captures(&line) {
                        total_bytes = parse_size_bytes(&c[1], &c[2]);
                        if total_bytes > 0 {
                            if let Some(bar) = progress {
                                bar.set_length(total_bytes);
                            }
                        }
                    }
                }
                if let Some(c) = pct_re.captures(&line) {
                    let pct: f64 = c[1].parse().unwrap_or(0.0);
                    if let Some(bar) = progress {
                        if total_bytes > 0 {
                            bar.set_position(((pct / 100.0) * total_bytes as f64) as u64);
                        } else {
                            bar.inc(1);
                        }
                    }
                }
                let upper = line.to_ascii_uppercase();
                if upper.contains("ERROR") || upper.contains("WARNING") {
                    if err_lines.len() >= 12 {
                        err_lines.remove(0);
                    }
                    err_lines.push(line);
                }
            }

            let status = child.wait()?;

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
                            || intermediate_re.is_match(file_name)
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
            } else {
                last_err = err_lines.join(" | ");
                if last_err.trim().is_empty() {
                    last_err = format!("yt-dlp thoát với mã lỗi {:?}", status.code());
                }
            }

            if attempt < 2 {
                std::thread::sleep(std::time::Duration::from_millis(800));
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
