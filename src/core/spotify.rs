use anyhow::{anyhow, Result};
use regex::Regex;
use reqwest::Client;
use serde_json::Value;
use url::Url;

fn artists_from_subtitle(subtitle: &str) -> Vec<String> {
    let artist = subtitle
        .split(" • ")
        .next()
        .unwrap_or(subtitle)
        .split(" · ")
        .next()
        .unwrap_or(subtitle)
        .trim();

    if artist.is_empty() {
        vec!["Unknown Artist".to_string()]
    } else {
        vec![artist.to_string()]
    }
}

fn required_text(value: &Value, field: &str, context: &str) -> Result<String> {
    value[field]
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("Thiếu trường {} trong {}", field, context))
}

fn track_meta_from_item(
    item: &Value,
    album: &str,
    cover_url: Option<String>,
    track_number: Option<u32>,
    total_tracks: Option<u32>,
) -> Result<SpotifyTrackMeta> {
    let title = required_text(item, "title", "track Spotify")?;
    let subtitle = required_text(item, "subtitle", "track Spotify")?;
    let artists = artists_from_subtitle(&subtitle);
    let search_query = format!("{} {} official audio", artists[0], title);

    Ok(SpotifyTrackMeta {
        title,
        artists,
        album: album.to_string(),
        release_year: None,
        duration_ms: item["duration"].as_u64(),
        cover_url,
        search_query,
        track_number,
        total_tracks,
    })
}

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
    pub track_number: Option<u32>,
    pub total_tracks: Option<u32>,
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
        let resp = self
            .client
            .get(&embed_url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        let re =
            Regex::new(r#"<script id="__NEXT_DATA__" type="application/json">([^<]+)</script>"#)?;
        if let Some(caps) = re.captures(&resp) {
            let json_str = &caps[1];
            let v: Value = serde_json::from_str(json_str)?;

            let entity = &v["props"]["pageProps"]["state"]["data"]["entity"];
            let title = entity["name"]
                .as_str()
                .or_else(|| entity["title"].as_str())
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| anyhow!("Thiếu tiêu đề track Spotify"))?
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
                    artists = artists_from_subtitle(subtitle);
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
                track_number: None,
                total_tracks: None,
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
                    track_number: None,
                    total_tracks: None,
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
        let resp = self
            .client
            .get(&embed_url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

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

        let track_list = entity["trackList"]
            .as_array()
            .filter(|tracks| !tracks.is_empty())
            .ok_or_else(|| anyhow!("Album Spotify không có danh sách track"))?;
        let total = track_list.len() as u32;
        let mut tracks = Vec::with_capacity(track_list.len());
        for (idx, item) in track_list.iter().enumerate() {
            tracks.push(track_meta_from_item(
                item,
                &album_name,
                cover_url.clone(),
                Some((idx + 1) as u32),
                Some(total),
            )?);
        }

        Ok((album_name, tracks))
    }

    pub async fn fetch_playlist_tracks(
        &self,
        playlist_id: &str,
    ) -> Result<(String, Vec<SpotifyTrackMeta>)> {
        let embed_url = format!("https://open.spotify.com/embed/playlist/{}", playlist_id);
        let resp = self
            .client
            .get(&embed_url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

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

        let track_list = entity["trackList"]
            .as_array()
            .filter(|tracks| !tracks.is_empty())
            .ok_or_else(|| anyhow!("Playlist Spotify không có danh sách track"))?;
        let total = track_list.len() as u32;
        let mut tracks = Vec::with_capacity(track_list.len());
        for (idx, item) in track_list.iter().enumerate() {
            tracks.push(track_meta_from_item(
                item,
                &playlist_name,
                cover_url.clone(),
                Some((idx + 1) as u32),
                Some(total),
            )?);
        }

        Ok((playlist_name, tracks))
    }

    async fn fetch_via_oembed(&self, url: &str) -> Result<SpotifyTrackMeta> {
        let mut endpoint = Url::parse("https://open.spotify.com/oembed")?;
        endpoint.query_pairs_mut().append_pair("url", url);
        let v: Value = self
            .client
            .get(endpoint)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let title = required_text(&v, "title", "oEmbed Spotify")?;
        let artist = v["author_name"]
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow!("Thiếu tác giả trong oEmbed Spotify"))?
            .to_string();
        let cover_url = v["thumbnail_url"]
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string);
        let search_query = format!("{} {} official audio", artist, title);

        Ok(SpotifyTrackMeta {
            title: title.clone(),
            artists: vec![artist],
            album: title,
            release_year: None,
            duration_ms: None,
            cover_url,
            search_query,
            track_number: None,
            total_tracks: None,
        })
    }
}
