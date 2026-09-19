use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct StreamDownloader<'a> {
    ytdlp_path: &'a Path,
    has_node: bool,
}

impl<'a> StreamDownloader<'a> {
    pub fn new(ytdlp_path: &'a Path, has_node: bool) -> Self {
        Self {
            ytdlp_path,
            has_node,
        }
    }

    pub fn download_stream(&self, target_url_or_search: &str, temp_dir: &Path) -> Result<PathBuf> {
        let mut last_err = String::new();

        for attempt in 1..=2 {
            let unique_id = format!(
                "dl_{}_{}_{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis(),
                attempt
            );
            let output_template = temp_dir.join(format!("{}.%(ext)s", unique_id));
            let output_template_str = output_template.to_string_lossy();

            let mut cmd = Command::new(self.ytdlp_path);
            if self.has_node {
                cmd.arg("--js-runtimes").arg("node");
            }

            cmd.arg("-f").arg("ba/ba*/b/best")
                .arg("--no-playlist")
                .arg("--no-warnings")
                .arg("--no-check-certificates")
                .arg("--socket-timeout").arg("30")
                .arg("--retries").arg("5")
                .arg("--fragment-retries").arg("5")
                .arg("--concurrent-fragments").arg("5")
                .arg("--buffer-size").arg("64k")
                .arg("--no-mtime")
                .arg("--file-access-retries").arg("3")
                .arg("--extractor-args").arg("youtube:player_client=android,web")
                .arg("--user-agent").arg("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
                .arg("-o").arg(output_template_str.as_ref())
                .arg(target_url_or_search);

            let output = match cmd.output() {
                Ok(out) => out,
                Err(e) => {
                    last_err = format!("Không thể thực thi yt-dlp: {}", e);
                    continue;
                }
            };

            if output.status.success() {
                if let Ok(entries) = std::fs::read_dir(temp_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                            if file_name.starts_with(&unique_id)
                                && !file_name.ends_with(".part")
                                && !file_name.ends_with(".ytdl")
                                && !file_name.ends_with(".temp")
                            {
                                return Ok(path);
                            }
                        }
                    }
                }
                last_err = "Không tìm thấy tệp âm thanh hoàn chỉnh sau khi yt-dlp hoàn thành".to_string();
            } else {
                last_err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            }

            if attempt < 2 {
                std::thread::sleep(std::time::Duration::from_millis(800));
            }
        }

        Err(anyhow!("Lỗi tải luồng âm thanh qua yt-dlp: {}", last_err))
    }
}
