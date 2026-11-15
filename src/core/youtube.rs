use crate::utils::process::hide_console_window;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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
        let mut cmd = hide_console_window(Command::new(self.ytdlp_path));
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
        args.push("--".to_string());
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
        let output = self
            .build_flat_playlist_command(playlist_url)
            .output()
            .map_err(|error| anyhow!("Không thể chạy yt-dlp: {error}"))?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "Lỗi khi lấy danh sách Playlist YouTube: {}",
                err.trim()
            ));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let items: Vec<YouTubeTrackMeta> = stdout
            .lines()
            .filter(|line| !line.trim().is_empty())
            .filter_map(Self::parse_flat_playlist_line)
            .collect();
        if items.is_empty() {
            return Err(anyhow!("Playlist YouTube không có track"));
        }
        Ok(items)
    }

    fn build_flat_playlist_command(&self, playlist_url: &str) -> Command {
        let mut cmd = hide_console_window(Command::new(self.ytdlp_path));
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
        args.push("--print".to_string());
        args.push(
            "%(id)s\t%(title)s\t%(uploader)s\t%(duration)s\t%(thumbnail)s\t%(webpage_url)s"
                .to_string(),
        );
        args.push("--".to_string());
        args.push(playlist_url.to_string());
        cmd.args(&args);
        cmd.stdin(Stdio::null());
        cmd
    }

    fn parse_flat_playlist_line(line: &str) -> Option<YouTubeTrackMeta> {
        let mut parts = line.split('\t');
        let id = parts.next().unwrap_or("").trim().to_string();
        let title = parts.next().unwrap_or("").trim().to_string();
        let uploader = parts.next().unwrap_or("").trim().to_string();
        let duration_raw = parts.next().unwrap_or("").trim().to_string();
        let thumbnail_raw = parts.next().unwrap_or("").trim().to_string();
        let webpage_raw = parts.next().unwrap_or("").trim().to_string();
        if id.is_empty() || id == "NA" || title.is_empty() || title == "NA" {
            return None;
        }
        let duration_secs = duration_raw
            .parse::<f64>()
            .ok()
            .filter(|duration| *duration > 0.0);
        let thumbnail_url = if thumbnail_raw.is_empty() || thumbnail_raw == "NA" {
            None
        } else {
            Some(thumbnail_raw)
        };
        let direct_url = if webpage_raw.is_empty() || webpage_raw == "NA" {
            format!("https://www.youtube.com/watch?v={}", id)
        } else {
            webpage_raw
        };
        let uploader = if uploader.is_empty() || uploader == "NA" {
            "YouTube".to_string()
        } else {
            uploader
        };
        Some(YouTubeTrackMeta {
            id,
            title,
            uploader,
            thumbnail_url,
            duration_secs,
            direct_url,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> YouTubeClient<'static> {
        YouTubeClient::new(Path::new("yt-dlp"), false, None, None)
    }

    fn command_args(command: &Command) -> Vec<String> {
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn flat_playlist_command_puts_url_after_double_dash() {
        let client = client();
        let url = "--no-playlist&inject=1";
        let command = client.build_flat_playlist_command(url);
        let args = command_args(&command);
        assert!(args.contains(&"--flat-playlist".to_string()));
        assert!(args.contains(&"--print".to_string()));
        assert_eq!(args.last().map(String::as_str), Some(url));
        assert_eq!(args.get(args.len() - 2).map(String::as_str), Some("--"));
    }

    #[test]
    fn flat_playlist_command_includes_print_template_and_default_args() {
        let client = client();
        let command =
            client.build_flat_playlist_command("https://www.youtube.com/playlist?list=PLghita");
        let args = command_args(&command);
        assert_eq!(command.get_program().to_string_lossy(), "yt-dlp");
        let print_pos = args
            .iter()
            .position(|arg| arg == "--print")
            .expect("--print flag must be present");
        assert_eq!(
            args[print_pos + 1],
            "%(id)s\t%(title)s\t%(uploader)s\t%(duration)s\t%(thumbnail)s\t%(webpage_url)s"
        );
        assert!(args.contains(&"--no-warnings".to_string()));
        assert!(args.contains(&"--newline".to_string()));
        assert!(args.contains(&"--socket-timeout".to_string()));
    }

    #[test]
    fn flat_playlist_command_with_cookies_appends_cookie_args() {
        let cookies = PathBuf::from("cookies.txt");
        let client = YouTubeClient::new(Path::new("yt-dlp"), true, Some(&cookies), Some("firefox"));
        let command =
            client.build_flat_playlist_command("https://www.youtube.com/playlist?list=PLghita");
        let args = command_args(&command);
        let cookies_pos = args
            .iter()
            .position(|arg| arg == "--cookies")
            .expect("--cookies flag must be present");
        assert_eq!(args[cookies_pos + 1], "cookies.txt");
        let browser_pos = args
            .iter()
            .position(|arg| arg == "--cookies-from-browser")
            .expect("--cookies-from-browser flag must be present");
        assert_eq!(args[browser_pos + 1], "firefox");
        assert!(args.contains(&"--js-runtimes".to_string()));
    }

    #[test]
    fn parse_flat_playlist_line_parses_realistic_tsv_line() {
        let line = "dQw4w9WgXcQ\tNever Gonna Give You Up\tRick Astley\t212.0\thttps://i.ytimg.com/vi/dQw4w9WgXcQ/default.jpg\thttps://www.youtube.com/watch?v=dQw4w9WgXcQ";
        let item = YouTubeClient::parse_flat_playlist_line(line).expect("valid line must parse");
        assert_eq!(item.id, "dQw4w9WgXcQ");
        assert_eq!(item.title, "Never Gonna Give You Up");
        assert_eq!(item.uploader, "Rick Astley");
        assert_eq!(item.duration_secs, Some(212.0));
        assert_eq!(
            item.thumbnail_url.as_deref(),
            Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/default.jpg")
        );
        assert_eq!(
            item.direct_url,
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
        );
    }

    #[test]
    fn parse_flat_playlist_line_fills_defaults_for_na_fields() {
        let line = "jNQXAC9IVRw\tMe at the zoo\tNA\tNA\tNA\tNA";
        let item =
            YouTubeClient::parse_flat_playlist_line(line).expect("line with NA fields must parse");
        assert_eq!(item.id, "jNQXAC9IVRw");
        assert_eq!(item.title, "Me at the zoo");
        assert_eq!(item.uploader, "YouTube");
        assert_eq!(item.duration_secs, None);
        assert_eq!(item.thumbnail_url, None);
        assert_eq!(
            item.direct_url,
            "https://www.youtube.com/watch?v=jNQXAC9IVRw"
        );
    }

    #[test]
    fn parse_flat_playlist_line_rejects_missing_id_or_title() {
        assert!(YouTubeClient::parse_flat_playlist_line("").is_none());
        assert!(YouTubeClient::parse_flat_playlist_line("NA\tTitle").is_none());
        assert!(YouTubeClient::parse_flat_playlist_line("id\tNA").is_none());
        assert!(YouTubeClient::parse_flat_playlist_line("id\t   ").is_none());
        assert!(YouTubeClient::parse_flat_playlist_line("\t\t\t\t\t").is_none());
    }
}
