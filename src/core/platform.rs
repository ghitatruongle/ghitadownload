use regex::Regex;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlType {
    SpotifyTrack(String),
    SpotifyAlbum(String),
    SpotifyPlaylist(String),
    YouTubeMusicTrack(String),
    YouTubeMusicPlaylist(String),
    YouTubeVideo(String),
    YouTubePlaylist(String),
    SunoTrack(String),
    SunoPlaylist(String),
    SocialVideo { url: String, platform_name: String },
    DirectMedia(String),
    DirectSearch(String),
}

pub struct PlatformParser;

impl PlatformParser {
    pub fn parse(input: &str) -> UrlType {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return UrlType::DirectSearch(String::new());
        }

        if trimmed.starts_with("spotify:") {
            let parts: Vec<&str> = trimmed.split(':').collect();
            if parts.len() >= 3 && is_spotify_id(parts[2]) {
                return match parts[1] {
                    "track" => UrlType::SpotifyTrack(parts[2].to_string()),
                    "album" => UrlType::SpotifyAlbum(parts[2].to_string()),
                    "playlist" => UrlType::SpotifyPlaylist(parts[2].to_string()),
                    _ => UrlType::DirectSearch(trimmed.to_string()),
                };
            }
        }

        if let Some(url) = Url::parse(trimmed).ok().filter(|u| {
            matches!(u.scheme(), "http" | "https")
                && u.host_str()
                    .is_some_and(|h| h.eq_ignore_ascii_case("open.spotify.com"))
        }) {
            let segments: Vec<&str> = url.path_segments().map(|s| s.collect()).unwrap_or_default();
            for i in 0..segments.len().saturating_sub(1) {
                let kind = segments[i];
                let id = segments[i + 1];
                if is_spotify_id(id) {
                    if kind == "track" {
                        return UrlType::SpotifyTrack(id.to_string());
                    } else if kind == "album" {
                        return UrlType::SpotifyAlbum(id.to_string());
                    } else if kind == "playlist" {
                        return UrlType::SpotifyPlaylist(id.to_string());
                    }
                }
            }
        }

        if let Some(url) = Url::parse(trimmed).ok().filter(|u| {
            matches!(u.scheme(), "http" | "https")
                && u.host_str()
                    .is_some_and(|h| h.eq_ignore_ascii_case("music.youtube.com"))
        }) {
            return parse_youtube(&url, true);
        }

        if let Some(url) = Url::parse(trimmed).ok().filter(|u| {
            matches!(u.scheme(), "http" | "https")
                && u.host_str().is_some_and(|h| {
                    matches!(
                        h.to_ascii_lowercase().as_str(),
                        "youtube.com"
                            | "www.youtube.com"
                            | "m.youtube.com"
                            | "youtu.be"
                            | "www.youtu.be"
                    )
                })
        }) {
            return parse_youtube(&url, false);
        }

        if let Some(url) = Url::parse(trimmed).ok().filter(|u| {
            matches!(u.scheme(), "http" | "https")
                && u.host_str().is_some_and(|h| {
                    matches!(
                        h.to_ascii_lowercase().as_str(),
                        "suno.com"
                            | "www.suno.com"
                            | "suno.ai"
                            | "www.suno.ai"
                            | "studio-api.prod.suno.com"
                            | "studio-api-prod.suno.com"
                    )
                })
        }) {
            let suno_uuid_re = Regex::new(
                r"(?i)(?:^|[^0-9a-f])([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})(?:[^0-9a-f]|$)",
            )
            .unwrap();
            let haystack = format!(
                "{} {} {}",
                url.path(),
                url.query().unwrap_or_default(),
                url.fragment().unwrap_or_default()
            );
            if let Some(caps) = suno_uuid_re.captures(&haystack) {
                let uuid = caps[1].to_ascii_lowercase();
                if url.path().contains("/playlist/") || haystack.contains("playlist") {
                    return UrlType::SunoPlaylist(uuid);
                }
                return UrlType::SunoTrack(uuid);
            }
            return UrlType::DirectSearch(trimmed.to_string());
        }

        if is_direct_media_url(trimmed) {
            return UrlType::DirectMedia(trimmed.to_string());
        }

        for (domain, platform_name) in [
            ("tiktok.com", "TikTok"),
            ("facebook.com", "Facebook"),
            ("fb.watch", "Facebook"),
            ("fb.com", "Facebook"),
            ("instagram.com", "Instagram"),
            ("threads.net", "Threads"),
            ("soundcloud.com", "SoundCloud"),
            ("sndcdn.com", "SoundCloud"),
            ("bandcamp.com", "Bandcamp"),
            ("twitter.com", "X"),
            ("x.com", "X"),
            ("bilibili.com", "Bilibili"),
            ("douyin.com", "Douyin"),
            ("kuaishou.com", "Kuaishou"),
            ("vimeo.com", "Vimeo"),
            ("dailymotion.com", "Dailymotion"),
            ("mixcloud.com", "Mixcloud"),
            ("pinterest.com", "Pinterest"),
            ("reddit.com", "Reddit"),
        ] {
            if let Some(_url) = Url::parse(trimmed).ok().filter(|u| {
                matches!(u.scheme(), "http" | "https")
                    && u.host_str().is_some_and(|h| {
                        h.eq_ignore_ascii_case(domain)
                            || h.to_ascii_lowercase().ends_with(&format!(".{}", domain))
                    })
                    && !u.path().trim_matches('/').is_empty()
            }) {
                return UrlType::SocialVideo {
                    url: trimmed.to_string(),
                    platform_name: platform_name.to_string(),
                };
            }
        }

        UrlType::DirectSearch(trimmed.to_string())
    }

    #[allow(dead_code)]
    pub fn is_playlist_or_album(url_type: &UrlType) -> bool {
        matches!(
            url_type,
            UrlType::SpotifyAlbum(_)
                | UrlType::SpotifyPlaylist(_)
                | UrlType::YouTubePlaylist(_)
                | UrlType::YouTubeMusicPlaylist(_)
                | UrlType::SunoPlaylist(_)
                | UrlType::DirectMedia(_)
        )
    }
}

fn parse_youtube(url: &Url, music: bool) -> UrlType {
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let video_id = if host == "youtu.be" {
        url.path_segments()
            .and_then(|mut segments| {
                let id = segments.next()?;
                if segments.next().is_some() {
                    None
                } else {
                    Some(id)
                }
            })
            .filter(|id| is_youtube_id(id))
            .map(str::to_string)
    } else if url.path() == "/watch" {
        url.query_pairs()
            .find(|(key, _)| key == "v")
            .map(|(_, value)| value.into_owned())
            .filter(|id| is_youtube_id(id))
    } else if url.path() == "/shorts" || url.path().starts_with("/shorts/") {
        url.path_segments()
            .map(|segments| segments.collect::<Vec<_>>())
            .filter(|segments| segments.len() == 2)
            .map(|segments| segments[1])
            .filter(|id| is_youtube_id(id))
            .map(str::to_string)
    } else if url.path() == "/live" || url.path().starts_with("/live/") {
        url.path_segments()
            .map(|segments| segments.collect::<Vec<_>>())
            .filter(|segments| segments.len() == 2)
            .map(|segments| segments[1])
            .filter(|id| is_youtube_id(id))
            .map(str::to_string)
    } else if url.path() == "/embed" || url.path().starts_with("/embed/") {
        url.path_segments()
            .map(|segments| segments.collect::<Vec<_>>())
            .filter(|segments| segments.len() == 2)
            .map(|segments| segments[1])
            .filter(|id| is_youtube_id(id))
            .map(str::to_string)
    } else if url.path() == "/v" || url.path().starts_with("/v/") {
        url.path_segments()
            .map(|segments| segments.collect::<Vec<_>>())
            .filter(|segments| segments.len() == 2)
            .map(|segments| segments[1])
            .filter(|id| is_youtube_id(id))
            .map(str::to_string)
    } else {
        None
    };

    if let Some(id) = video_id {
        return if music {
            UrlType::YouTubeMusicTrack(format!("https://music.youtube.com/watch?v={}", id))
        } else {
            UrlType::YouTubeVideo(format!("https://www.youtube.com/watch?v={}", id))
        };
    }

    if url
        .query_pairs()
        .find(|(key, _)| key == "list")
        .map(|(_, value)| value.into_owned())
        .filter(|list| !list.is_empty())
        .is_some()
    {
        return if music {
            UrlType::YouTubeMusicPlaylist(trimmed_url(url))
        } else {
            UrlType::YouTubePlaylist(trimmed_url(url))
        };
    }

    UrlType::DirectSearch(trimmed_url(url))
}

fn trimmed_url(url: &Url) -> String {
    url.to_string()
}

fn is_youtube_id(id: &str) -> bool {
    id.len() == 11
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn is_spotify_id(id: &str) -> bool {
    id.len() == 22 && id.chars().all(|c| c.is_ascii_alphanumeric())
}

fn is_direct_media_url(input: &str) -> bool {
    let Ok(url) = Url::parse(input) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return false;
    }
    let path = url.path().to_ascii_lowercase();
    [
        ".mp3", ".m4a", ".aac", ".wav", ".flac", ".ogg", ".opus", ".webm", ".mp4", ".m4b", ".oga",
        ".aiff", ".aif",
    ]
    .iter()
    .any(|ext| path.ends_with(ext))
}
