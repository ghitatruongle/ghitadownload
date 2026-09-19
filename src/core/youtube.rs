use anyhow::{anyhow, Result};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct YouTubeTrackMeta {
    pub id: String,
    pub title: String,
    pub uploader: String,
    pub thumbnail_url: Option<String>,
    pub direct_url: String,
}

pub struct YouTubeClient<'a> {
    ytdlp_path: &'a Path,
    has_node: bool,
}

impl<'a> YouTubeClient<'a> {
    pub fn new(ytdlp_path: &'a Path, has_node: bool) -> Self {
        Self {
            ytdlp_path,
            has_node,
        }
    }

    pub fn fetch_video_info(&self, url_or_query: &str) -> Result<YouTubeTrackMeta> {
        let mut cmd = Command::new(self.ytdlp_path);
        if self.has_node {
            cmd.arg("--js-runtimes").arg("node");
        }
        cmd.arg("--no-playlist")
            .arg("--no-warnings")
            .arg("--no-check-certificates")
            .arg("--socket-timeout").arg("10")
            .arg("--extractor-args").arg("youtube:player_client=android,web")
            .arg("--print")
            .arg("%(id)s###%(title)s###%(uploader)s###%(thumbnail)s")
            .arg(url_or_query);

        let output = cmd.output()?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Lỗi khi lấy thông tin YouTube: {}", err));
        }

        let out_str = String::from_utf8_lossy(&output.stdout);
        let line = out_str.lines().next().ok_or_else(|| anyhow!("Không nhận được phản hồi từ YouTube"))?;
        let parts: Vec<&str> = line.split("###").collect();

        if parts.len() < 3 {
            return Err(anyhow!("Định dạng metadata không hợp lệ"));
        }

        let id = parts[0].trim().to_string();
        let title = parts[1].trim().to_string();
        let uploader = parts[2].trim().to_string();
        let thumbnail_url = parts.get(3).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let direct_url = format!("https://www.youtube.com/watch?v={}", id);

        Ok(YouTubeTrackMeta {
            id,
            title,
            uploader,
            thumbnail_url,
            direct_url,
        })
    }

    pub fn fetch_media_info(&self, url: &str, platform_name: &str) -> Result<YouTubeTrackMeta> {
        let mut cmd = Command::new(self.ytdlp_path);
        if self.has_node {
            cmd.arg("--js-runtimes").arg("node");
        }
        cmd.arg("--no-playlist")
            .arg("--no-warnings")
            .arg("--no-check-certificates")
            .arg("--socket-timeout").arg("12")
            .arg("--user-agent").arg("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
            .arg("--print")
            .arg("%(id)s###%(title)s###%(uploader)s###%(thumbnail)s")
            .arg(url);

        let output = cmd.output()?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Lỗi khi lấy thông tin {}: {}", platform_name, err));
        }

        let out_str = String::from_utf8_lossy(&output.stdout);
        let line = out_str.lines().next().ok_or_else(|| anyhow!("Không nhận được phản hồi từ {}", platform_name))?;
        let parts: Vec<&str> = line.split("###").collect();

        if parts.is_empty() {
            return Err(anyhow!("Định dạng metadata không hợp lệ"));
        }

        let id = parts[0].trim().to_string();
        let raw_title = parts.get(1).map(|s| s.trim()).unwrap_or("");
        let title = if !raw_title.is_empty() {
            raw_title.to_string()
        } else {
            format!("{} Audio", platform_name)
        };
        let raw_uploader = parts.get(2).map(|s| s.trim()).unwrap_or("");
        let uploader = if !raw_uploader.is_empty() {
            raw_uploader.to_string()
        } else {
            platform_name.to_string()
        };
        let thumbnail_url = parts.get(3).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());

        Ok(YouTubeTrackMeta {
            id,
            title,
            uploader,
            thumbnail_url,
            direct_url: url.to_string(),
        })
    }

    pub fn extract_playlist_items(&self, playlist_url: &str) -> Result<Vec<YouTubeTrackMeta>> {
        let mut cmd = Command::new(self.ytdlp_path);
        if self.has_node {
            cmd.arg("--js-runtimes").arg("node");
        }
        cmd.arg("--flat-playlist")
            .arg("--no-warnings")
            .arg("--no-check-certificates")
            .arg("--socket-timeout").arg("15")
            .arg("--extractor-args").arg("youtube:player_client=android,web")
            .arg("--print")
            .arg("%(id)s###%(title)s###%(uploader)s")
            .arg(playlist_url);

        let output = cmd.output()?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Lỗi khi lấy danh sách Playlist YouTube: {}", err));
        }

        let out_str = String::from_utf8_lossy(&output.stdout);
        let mut items = Vec::new();

        for line in out_str.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let parts: Vec<&str> = trimmed.split("###").collect();
            if parts.len() >= 2 {
                let id = parts[0].trim().to_string();
                let title = parts[1].trim().to_string();
                let uploader = parts.get(2).unwrap_or(&"YouTube").trim().to_string();
                let direct_url = format!("https://www.youtube.com/watch?v={}", id);

                items.push(YouTubeTrackMeta {
                    id,
                    title,
                    uploader,
                    thumbnail_url: None,
                    direct_url,
                });
            }
        }

        Ok(items)
    }
}
