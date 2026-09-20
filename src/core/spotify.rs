use anyhow::{anyhow, Result};
use regex::Regex;
use reqwest::Client;
use serde_json::Value;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SpotifyTrackMeta {
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub release_year: Option<u32>,
    pub duration_ms: Option<u64>,
    pub cover_url: Option<String>,
    pub search_query: String,
}

pub struct SpotifyClient {
    client: Client,
}

impl Default for SpotifyClient {
    fn default() -> Self {
        Self::new()
    }
}

impl SpotifyClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .connect_timeout(std::time::Duration::from_secs(10))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .unwrap_or_default();
        Self { client }
    }

    pub async fn fetch_track(&self, track_id: &str) -> Result<SpotifyTrackMeta> {
        let embed_url = format!("https://open.spotify.com/embed/track/{}", track_id);
        let resp = self.client.get(&embed_url).send().await?.text().await?;

        let re =
            Regex::new(r#"<script id="__NEXT_DATA__" type="application/json">([^<]+)</script>"#)?;
        if let Some(caps) = re.captures(&resp) {
            let json_str = &caps[1];
            let v: Value = serde_json::from_str(json_str)?;

            let entity = &v["props"]["pageProps"]["state"]["data"]["entity"];
            let title = entity["name"]
                .as_str()
                .or_else(|| entity["title"].as_str())
                .unwrap_or("Unknown Title")
                .to_string();

            let mut artists = Vec::new();
            if let Some(arr) = entity["artists"].as_array() {
                for a in arr {
                    if let Some(name) = a["name"].as_str() {
                        artists.push(name.to_string());
                    }
                }
            }
            if artists.is_empty() {
                if let Some(subtitle) = entity["subtitle"].as_str() {
                    artists.push(subtitle.to_string());
                }
            }
            if artists.is_empty() {
                artists.push("Unknown Artist".to_string());
            }

            let album = entity["album"]["name"]
                .as_str()
                .unwrap_or(&title)
                .to_string();

            let release_year = entity["releaseDate"]["isoString"]
                .as_str()
                .and_then(|s| s.get(0..4))
                .and_then(|y| y.parse::<u32>().ok());

            let duration_ms = entity["duration"].as_u64();

            let mut cover_url = None;
            if let Some(images) = entity["visualIdentity"]["image"].as_array() {
                if let Some(last_img) = images.last() {
                    cover_url = last_img["url"].as_str().map(|s| s.to_string());
                }
            }

            let primary_artist = artists.first().cloned().unwrap_or_default();
            let search_query = format!("{} {} official audio", primary_artist, title);

            return Ok(SpotifyTrackMeta {
                title,
                artists,
                album,
                release_year,
                duration_ms,
                cover_url,
                search_query,
            });
        }

        if let Ok(og_title_re) = Regex::new(r#"<meta property="og:title" content="([^"]+)""#) {
            if let Some(caps) = og_title_re.captures(&resp) {
                let full_title = caps[1].to_string();
                let og_desc_re =
                    Regex::new(r#"<meta property="og:description" content="([^"]+)""#).ok();
                let og_img_re = Regex::new(r#"<meta property="og:image" content="([^"]+)""#).ok();

                let mut artist = "Spotify Artist".to_string();
                if let Some(desc_re) = og_desc_re {
                    if let Some(desc_caps) = desc_re.captures(&resp) {
                        let desc = &desc_caps[1];
                        if let Some(first_part) = desc.split('·').next() {
                            let a = first_part.trim();
                            if !a.is_empty() {
                                artist = a.to_string();
                            }
                        }
                    }
                }

                let cover_url = og_img_re
                    .and_then(|r| r.captures(&resp))
                    .map(|c| c[1].to_string());

                let search_query = format!("{} {} official audio", artist, full_title);

                return Ok(SpotifyTrackMeta {
                    title: full_title.clone(),
                    artists: vec![artist],
                    album: full_title,
                    release_year: None,
                    duration_ms: None,
                    cover_url,
                    search_query,
                });
            }
        }

        self.fetch_via_oembed(&format!("https://open.spotify.com/track/{}", track_id))
            .await
    }

    pub async fn fetch_album_tracks(
        &self,
        album_id: &str,
    ) -> Result<(String, Vec<SpotifyTrackMeta>)> {
        let embed_url = format!("https://open.spotify.com/embed/album/{}", album_id);
        let resp = self.client.get(&embed_url).send().await?.text().await?;

        let re =
            Regex::new(r#"<script id="__NEXT_DATA__" type="application/json">([^<]+)</script>"#)?;
        let caps = re
            .captures(&resp)
            .ok_or_else(|| anyhow!("Không thể phân tích dữ liệu Album Spotify"))?;
        let v: Value = serde_json::from_str(&caps[1])?;

        let entity = &v["props"]["pageProps"]["state"]["data"]["entity"];
        let album_name = entity["name"]
            .as_str()
            .unwrap_or("Spotify Album")
            .to_string();

        let mut cover_url = None;
        if let Some(images) = entity["visualIdentity"]["image"].as_array() {
            if let Some(last_img) = images.last() {
                cover_url = last_img["url"].as_str().map(|s| s.to_string());
            }
        }

        let mut tracks = Vec::new();
        if let Some(track_list) = entity["trackList"].as_array() {
            for item in track_list {
                let title = item["title"]
                    .as_str()
                    .unwrap_or("Unknown Title")
                    .to_string();
                let subtitle = item["subtitle"]
                    .as_str()
                    .unwrap_or("Unknown Artist")
                    .to_string();
                let duration_ms = item["duration"].as_u64();
                let search_query = format!("{} {} official audio", subtitle, title);

                tracks.push(SpotifyTrackMeta {
                    title,
                    artists: vec![subtitle],
                    album: album_name.clone(),
                    release_year: None,
                    duration_ms,
                    cover_url: cover_url.clone(),
                    search_query,
                });
            }
        }

        Ok((album_name, tracks))
    }

    pub async fn fetch_playlist_tracks(
        &self,
        playlist_id: &str,
    ) -> Result<(String, Vec<SpotifyTrackMeta>)> {
        let embed_url = format!("https://open.spotify.com/embed/playlist/{}", playlist_id);
        let resp = self.client.get(&embed_url).send().await?.text().await?;

        let re =
            Regex::new(r#"<script id="__NEXT_DATA__" type="application/json">([^<]+)</script>"#)?;
        let caps = re
            .captures(&resp)
            .ok_or_else(|| anyhow!("Không thể phân tích dữ liệu Playlist Spotify"))?;
        let v: Value = serde_json::from_str(&caps[1])?;

        let entity = &v["props"]["pageProps"]["state"]["data"]["entity"];
        let playlist_name = entity["name"]
            .as_str()
            .unwrap_or("Spotify Playlist")
            .to_string();

        let mut cover_url = None;
        if let Some(images) = entity["visualIdentity"]["image"].as_array() {
            if let Some(last_img) = images.last() {
                cover_url = last_img["url"].as_str().map(|s| s.to_string());
            }
        }

        let mut tracks = Vec::new();
        if let Some(track_list) = entity["trackList"].as_array() {
            for item in track_list {
                let title = item["title"].as_str().unwrap_or("Unknown").to_string();
                let subtitle = item["subtitle"].as_str().unwrap_or("Unknown").to_string();
                let duration_ms = item["duration"].as_u64();
                let search_query = format!("{} {} official audio", subtitle, title);

                tracks.push(SpotifyTrackMeta {
                    title,
                    artists: vec![subtitle],
                    album: playlist_name.clone(),
                    release_year: None,
                    duration_ms,
                    cover_url: cover_url.clone(),
                    search_query,
                });
            }
        }

        Ok((playlist_name, tracks))
    }

    async fn fetch_via_oembed(&self, url: &str) -> Result<SpotifyTrackMeta> {
        let endpoint = format!("https://open.spotify.com/oembed?url={}", url);
        let v: Value = self.client.get(&endpoint).send().await?.json().await?;

        let title = v["title"].as_str().unwrap_or("Unknown Title").to_string();
        let artist = v["author_name"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("Spotify Artist")
            .to_string();

        let cover_url = v["thumbnail_url"].as_str().map(|s| s.to_string());
        let search_query = format!("{} {} official audio", artist, title);

        Ok(SpotifyTrackMeta {
            title: title.clone(),
            artists: vec![artist],
            album: title.clone(),
            release_year: None,
            duration_ms: None,
            cover_url,
            search_query,
        })
    }
}
