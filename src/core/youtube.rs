use anyhow::{anyhow, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::core::ytdlp::YTDLP_DEFAULT_ARGS;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct YouTubeTrackMeta {
    pub id: String,
    pub title: String,
    pub uploader: String,
    pub thumbnail_url: Option<String>,
    pub duration_secs: Option<f64>,
    pub direct_url: String,
}

pub struct YouTubeClient<'a> {
    ytdlp_path: &'a Path,
    has_node: bool,
    cookies_file: Option<&'a PathBuf>,
    cookies_from_browser: Option<&'a str>,
}

fn metadata_from_json(
    value: &Value,
    platform_name: &str,
    direct_url: Option<String>,
) -> Result<YouTubeTrackMeta> {
    let id = value["id"]
        .as_str()
        .map(str::to_string)
        .or_else(|| value["id"].as_i64().map(|n| n.to_string()))
        .or_else(|| value["id"].as_u64().map(|n| n.to_string()))
        .filter(|id| !id.trim().is_empty())
        .or_else(|| direct_url.as_ref().cloned())
        .unwrap_or_else(|| "media".to_string());
    let title = value["title"]
        .as_str()
        .or_else(|| value["track"].as_str())
        .or_else(|| value["description"].as_str())
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("{} Audio", platform_name));
    let uploader = value["uploader"]
        .as_str()
        .or_else(|| value["channel"].as_str())
        .or_else(|| value["creator"].as_str())
        .or_else(|| value["artist"].as_str())
        .map(str::trim)
        .filter(|uploader| !uploader.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| platform_name.to_string());
    let thumbnail_url = value["thumbnail"]
        .as_str()
        .filter(|thumbnail| !thumbnail.trim().is_empty())
        .map(str::to_string);
    let duration_secs = value["duration"].as_f64();
    let direct_url =
        direct_url.unwrap_or_else(|| format!("https://www.youtube.com/watch?v={}", id));

    Ok(YouTubeTrackMeta {
        id,
        title,
        uploader,
        thumbnail_url,
        duration_secs,
        direct_url,
    })
}

impl<'a> YouTubeClient<'a> {
    pub fn new(
        ytdlp_path: &'a Path,
        has_node: bool,
        cookies_file: Option<&'a PathBuf>,
        cookies_from_browser: Option<&'a str>,
    ) -> Self {
        Self {
            ytdlp_path,
            has_node,
            cookies_file,
            cookies_from_browser,
        }
    }

    fn append_cookie_args(&self, args: &mut Vec<String>) {
        if let Some(path) = self.cookies_file {
            args.push("--cookies".to_string());
            args.push(path.to_string_lossy().into_owned());
        }
        if let Some(browser) = self.cookies_from_browser {
            args.push("--cookies-from-browser".to_string());
            args.push(browser.to_string());
        }
    }

    fn configure_command(&self, url_or_query: &str, socket_timeout: &str) -> Command {
        let mut cmd = Command::new(self.ytdlp_path);
        let mut args: Vec<String> = Vec::new();
        if self.has_node {
            args.push("--js-runtimes".to_string());
            args.push("node".to_string());
        }
        args.push("--no-playlist".to_string());
        args.push("--socket-timeout".to_string());
        args.push(socket_timeout.to_string());
        args.extend(YTDLP_DEFAULT_ARGS.iter().map(|s| s.to_string()));
        self.append_cookie_args(&mut args);
        args.push("--dump-single-json".to_string());
        args.push(url_or_query.to_string());
        cmd.args(&args);
        cmd
    }

    pub fn fetch_video_info(&self, url_or_query: &str) -> Result<YouTubeTrackMeta> {
        let output = self
            .configure_command(url_or_query, "10")
            .output()
            .map_err(|error| anyhow!("Không thể chạy yt-dlp: {error}"))?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Lỗi khi lấy thông tin YouTube: {}", err));
        }
        let value: Value = serde_json::from_slice(&output.stdout)
            .map_err(|error| anyhow!("JSON metadata YouTube không hợp lệ: {error}"))?;
        metadata_from_json(&value, "YouTube", None)
    }

    pub fn fetch_media_info(&self, url: &str, platform_name: &str) -> Result<YouTubeTrackMeta> {
        let mut cmd = self.configure_command(url, "12");
        let output = cmd
            .output()
            .map_err(|error| anyhow!("Không thể chạy yt-dlp: {error}"))?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Lỗi khi lấy thông tin {}: {}", platform_name, err));
        }
        let value: Value = serde_json::from_slice(&output.stdout)
            .map_err(|error| anyhow!("JSON metadata {} không hợp lệ: {error}", platform_name))?;
        metadata_from_json(&value, platform_name, Some(url.to_string()))
    }

    pub fn extract_playlist_items(&self, playlist_url: &str) -> Result<Vec<YouTubeTrackMeta>> {
        let mut cmd = Command::new(self.ytdlp_path);
        let mut args: Vec<String> = Vec::new();
        if self.has_node {
            args.push("--js-runtimes".to_string());
            args.push("node".to_string());
        }
        args.push("--flat-playlist".to_string());
        args.push("--socket-timeout".to_string());
        args.push("15".to_string());
        args.extend(YTDLP_DEFAULT_ARGS.iter().map(|s| s.to_string()));
        self.append_cookie_args(&mut args);
        args.push("--dump-single-json".to_string());
        args.push(playlist_url.to_string());
        cmd.args(&args);
        let output = cmd
            .output()
            .map_err(|error| anyhow!("Không thể chạy yt-dlp: {error}"))?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Lỗi khi lấy danh sách Playlist YouTube: {}", err));
        }
        let value: Value = serde_json::from_slice(&output.stdout)
            .map_err(|error| anyhow!("JSON playlist YouTube không hợp lệ: {error}"))?;
        let entries = value["entries"]
            .as_array()
            .ok_or_else(|| anyhow!("Dữ liệu playlist YouTube không có danh sách track"))?;

        let mut items = Vec::with_capacity(entries.len());
        for entry in entries {
            items.push(metadata_from_json(entry, "YouTube", None)?);
        }
        if items.is_empty() {
            return Err(anyhow!("Playlist YouTube không có track"));
        }
        Ok(items)
    }
}
