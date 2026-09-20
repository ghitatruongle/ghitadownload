use regex::Regex;

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
    SocialVideo { url: String, platform_name: String },
    DirectSearch(String),
}

pub struct PlatformParser;

impl PlatformParser {
    pub fn parse(input: &str) -> UrlType {
        let trimmed = input.trim();

        if trimmed.contains("open.spotify.com") || trimmed.starts_with("spotify:") {
            if let Some(caps) = Regex::new(r"track[/:]([a-zA-Z0-9]+)")
                .unwrap()
                .captures(trimmed)
            {
                return UrlType::SpotifyTrack(caps[1].to_string());
            }
            if let Some(caps) = Regex::new(r"album[/:]([a-zA-Z0-9]+)")
                .unwrap()
                .captures(trimmed)
            {
                return UrlType::SpotifyAlbum(caps[1].to_string());
            }
            if let Some(caps) = Regex::new(r"playlist[/:]([a-zA-Z0-9]+)")
                .unwrap()
                .captures(trimmed)
            {
                return UrlType::SpotifyPlaylist(caps[1].to_string());
            }
        }

        if trimmed.contains("music.youtube.com") {
            if let Some(caps) = Regex::new(r"[?&]v=([a-zA-Z0-9_-]{11})")
                .unwrap()
                .captures(trimmed)
            {
                return UrlType::YouTubeMusicTrack(format!(
                    "https://music.youtube.com/watch?v={}",
                    &caps[1]
                ));
            }
            if trimmed.contains("list=") {
                return UrlType::YouTubeMusicPlaylist(trimmed.to_string());
            }
            return UrlType::YouTubeMusicTrack(trimmed.to_string());
        }

        if trimmed.contains("youtube.com") || trimmed.contains("youtu.be") {
            let vid_regex =
                Regex::new(r"(?:[?&]v=|youtu\.be/|shorts/)([a-zA-Z0-9_-]{11})").unwrap();
            if let Some(caps) = vid_regex.captures(trimmed) {
                let vid_id = &caps[1];
                return UrlType::YouTubeVideo(format!(
                    "https://www.youtube.com/watch?v={}",
                    vid_id
                ));
            }

            if trimmed.contains("list=") {
                return UrlType::YouTubePlaylist(trimmed.to_string());
            }
            return UrlType::YouTubeVideo(trimmed.to_string());
        }

        if trimmed.contains("suno.com") || trimmed.contains("suno.ai") {
            let suno_uuid_re = Regex::new(
                r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}",
            )
            .unwrap();
            if let Some(caps) = suno_uuid_re.captures(trimmed) {
                return UrlType::SunoTrack(caps[0].to_string());
            }
        }

        if trimmed.contains("tiktok.com") {
            return UrlType::SocialVideo {
                url: trimmed.to_string(),
                platform_name: "TikTok".to_string(),
            };
        }

        if trimmed.contains("facebook.com")
            || trimmed.contains("fb.watch")
            || trimmed.contains("fb.com")
        {
            return UrlType::SocialVideo {
                url: trimmed.to_string(),
                platform_name: "Facebook".to_string(),
            };
        }

        if trimmed.contains("instagram.com") {
            return UrlType::SocialVideo {
                url: trimmed.to_string(),
                platform_name: "Instagram".to_string(),
            };
        }

        if trimmed.contains("threads.net") {
            return UrlType::SocialVideo {
                url: trimmed.to_string(),
                platform_name: "Threads".to_string(),
            };
        }

        if trimmed.contains("soundcloud.com") || trimmed.contains("sndcdn.com") {
            return UrlType::SocialVideo {
                url: trimmed.to_string(),
                platform_name: "SoundCloud".to_string(),
            };
        }

        if trimmed.contains("bandcamp.com") {
            return UrlType::SocialVideo {
                url: trimmed.to_string(),
                platform_name: "Bandcamp".to_string(),
            };
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
        )
    }
}
