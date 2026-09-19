use anyhow::Result;
use regex::Regex;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SunoTrackMeta {
    pub uuid: String,
    pub title: String,
    pub artist: String,
    pub audio_url: String,
    pub fallback_audio_url: String,
    pub cover_url: Option<String>,
}

pub struct SunoClient;

impl SunoClient {
    pub fn new() -> Self {
        Self
    }

    pub async fn fetch_track(&self, uuid: &str) -> Result<SunoTrackMeta> {
        let short_id = if uuid.len() >= 8 { &uuid[0..8] } else { uuid };
        let default_title = format!("Suno AI Track {}", short_id);
        let default_artist = "Suno AI".to_string();
        let audio_url = format!("https://cdn1.suno.ai/{}.mp3", uuid);
        let fallback_audio_url = format!("https://audiopipe.suno.ai/?item_id={}", uuid);
        let default_cover = Some(format!("https://cdn1.suno.ai/image_{}.png", uuid));

        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(4))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
            .build()
        {
            Ok(c) => c,
            Err(_) => {
                return Ok(SunoTrackMeta {
                    uuid: uuid.to_string(),
                    title: default_title,
                    artist: default_artist,
                    audio_url,
                    fallback_audio_url,
                    cover_url: default_cover,
                });
            }
        };

        let page_url = format!("https://suno.com/song/{}", uuid);
        let resp = match client.get(&page_url).send().await {
            Ok(r) => r,
            Err(_) => {
                return Ok(SunoTrackMeta {
                    uuid: uuid.to_string(),
                    title: default_title,
                    artist: default_artist,
                    audio_url,
                    fallback_audio_url,
                    cover_url: default_cover,
                });
            }
        };

        if !resp.status().is_success() {
            return Ok(SunoTrackMeta {
                uuid: uuid.to_string(),
                title: default_title,
                artist: default_artist,
                audio_url,
                fallback_audio_url,
                cover_url: default_cover,
            });
        }

        let html = resp.text().await.unwrap_or_default();

        let mut title = default_title.clone();
        if let Ok(title_re) = Regex::new(r#"<meta\s+property=["']og:title["']\s+content=["']([^"']+)["']"#) {
            if let Some(caps) = title_re.captures(&html) {
                let raw_title = caps[1].trim().to_string();
                if !raw_title.is_empty() {
                    let mut clean_t = raw_title;
                    if let Some(pos) = clean_t.find(" | Suno") {
                        clean_t.truncate(pos);
                    }
                    if let Some(pos) = clean_t.find(" by @") {
                        clean_t.truncate(pos);
                    }
                    title = clean_t.trim().to_string();
                }
            }
        }

        let mut artist = default_artist;
        if let Ok(desc_re) = Regex::new(r#"<meta\s+property=["']og:description["']\s+content=["']Listen to [^b]+by\s+([^,.\n"']+)["']"#) {
            if let Some(caps) = desc_re.captures(&html) {
                let a = caps[1].trim();
                if !a.is_empty() {
                    artist = a.to_string();
                }
            }
        }

        let mut cover_url = default_cover;
        if let Ok(img_re) = Regex::new(r#"<meta\s+property=["']og:image["']\s+content=["']([^"']+)["']"#) {
            if let Some(caps) = img_re.captures(&html) {
                let img = caps[1].trim();
                if !img.is_empty() {
                    cover_url = Some(img.to_string());
                }
            }
        }

        Ok(SunoTrackMeta {
            uuid: uuid.to_string(),
            title,
            artist,
            audio_url,
            fallback_audio_url,
            cover_url,
        })
    }
}
