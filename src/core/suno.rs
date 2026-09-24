use anyhow::{anyhow, Result};
use regex::Regex;
use serde_json::Value;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SunoTrackMeta {
    pub uuid: String,
    pub title: String,
    pub artist: String,
    pub audio_url: String,
    pub fallback_audio_urls: Vec<String>,
    pub cover_url: Option<String>,
    pub duration_secs: Option<f64>,
    pub is_protected: bool,
    pub is_encrypted: bool,
    pub page_url: String,
}

#[derive(Default)]
pub struct SunoClient;

fn decode_entities(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

fn is_forbidden_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains("/forbidden") || lower.contains("unauthorized") || lower.trim().is_empty()
}

fn is_encrypted_encoding(encoding: Option<&str>) -> bool {
    match encoding.map(str::trim).filter(|value| !value.is_empty()) {
        None | Some("") | Some("none") | Some("clear") | Some("identity") => false,
        Some(_) => true,
    }
}

fn media_entry_url(entry: &Value) -> Option<String> {
    entry["url"]
        .as_str()
        .map(str::trim)
        .filter(|url| url.starts_with("https://"))
        .map(str::to_string)
}

fn collect_media_urls(value: &Value) -> (Vec<String>, bool) {
    let mut urls = Vec::new();
    let mut encrypted = false;
    if let Some(items) = value["media_urls"].as_array() {
        for item in items {
            let encoding = item["encoding"].as_str();
            if is_encrypted_encoding(encoding) {
                encrypted = true;
            }
            if let Some(url) = media_entry_url(item) {
                if !urls.contains(&url) {
                    urls.push(url);
                }
            }
        }
    }
    (urls, encrypted)
}

fn decrypt_aes_gcm_wrapped(wrapped_b64: &str, aad: &[u8], user_key: &[u8]) -> Result<Vec<u8>> {
    use aes_gcm::aead::{Aead, KeyInit, Payload};
    use aes_gcm::{Aes256Gcm, Key, Nonce};
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;

    let raw = STANDARD
        .decode(wrapped_b64.trim())
        .map_err(|e| anyhow!("Lỗi giải mã base64: {e}"))?;
    if raw.len() < 28 {
        return Err(anyhow!("Dữ liệu wrapped key quá ngắn"));
    }
    let iv_bytes = &raw[..12];
    let ciphertext_and_tag = &raw[12..];
    let key = Key::<Aes256Gcm>::from_slice(user_key);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(iv_bytes);
    let payload = Payload {
        msg: ciphertext_and_tag,
        aad,
    };
    cipher
        .decrypt(nonce, payload)
        .map_err(|e| anyhow!("Lỗi giải mã Mango GCM: {:?}", e))
}

fn decrypt_aes_128_ctr(encrypted_data: &mut [u8], key_bytes: &[u8], iv_bytes: &[u8]) -> Result<()> {
    use ctr::cipher::{KeyIvInit, StreamCipher};
    use ctr::Ctr128BE;
    type Aes128Ctr = Ctr128BE<aes::Aes128>;

    let mut cipher = Aes128Ctr::new(key_bytes.into(), iv_bytes.into());
    cipher.apply_keystream(encrypted_data);
    Ok(())
}

fn parse_clip_payload(value: &Value, uuid: &str) -> Result<SunoTrackMeta> {
    let title = value["title"]
        .as_str()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(decode_entities)
        .unwrap_or_else(|| format!("Suno AI Track {}", &uuid[..uuid.len().min(8)]));

    let artist = value["display_name"]
        .as_str()
        .map(str::trim)
        .filter(|artist| !artist.is_empty())
        .map(decode_entities)
        .or_else(|| {
            value["handle"]
                .as_str()
                .map(str::trim)
                .filter(|artist| !artist.is_empty())
                .map(decode_entities)
        })
        .unwrap_or_else(|| "Suno AI".to_string());

    let cover_url = value["image_large_url"]
        .as_str()
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .map(decode_entities)
        .or_else(|| {
            value["image_url"]
                .as_str()
                .map(str::trim)
                .filter(|url| !url.is_empty())
                .map(decode_entities)
        });

    let duration_secs = value["metadata"]["duration"]
        .as_f64()
        .or_else(|| value["duration"].as_f64())
        .filter(|duration| *duration > 0.0);

    let (mut media_urls, media_encrypted) = collect_media_urls(value);
    let audio_url = value["audio_url"]
        .as_str()
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .map(decode_entities)
        .unwrap_or_default()
        .to_string();

    let mut fallback_audio_urls = Vec::new();
    if !is_forbidden_url(&audio_url) && !media_urls.iter().any(|url| url == &audio_url) {
        media_urls.insert(0, audio_url.clone());
    }
    for url in media_urls {
        if !is_forbidden_url(&url) && !fallback_audio_urls.contains(&url) && url != audio_url {
            fallback_audio_urls.push(url);
        }
    }

    if let Some(video_url) = value["video_url"]
        .as_str()
        .map(str::trim)
        .filter(|url| url.starts_with("https://"))
        .map(decode_entities)
    {
        if video_url != audio_url && !fallback_audio_urls.contains(&video_url) {
            fallback_audio_urls.push(video_url);
        }
    }

    let is_forbidden = is_forbidden_url(&audio_url);
    if is_forbidden && fallback_audio_urls.is_empty() {
        return Err(anyhow!(
            "Bài Suno không cấp URL âm thanh công khai (audio_url forbidden)"
        ));
    }

    let primary = if !is_forbidden {
        audio_url.clone()
    } else {
        fallback_audio_urls
            .first()
            .cloned()
            .ok_or_else(|| anyhow!("Suno không cung cấp media_urls hợp lệ"))?
    };

    let rest: Vec<String> = fallback_audio_urls
        .into_iter()
        .filter(|url| url != &primary)
        .collect();

    let is_encrypted = media_encrypted || is_forbidden;

    Ok(SunoTrackMeta {
        uuid: uuid.to_string(),
        title,
        artist,
        audio_url: primary,
        fallback_audio_urls: rest,
        cover_url,
        duration_secs,
        is_protected: is_encrypted,
        is_encrypted,
        page_url: format!("https://suno.com/song/{}", uuid),
    })
}

fn extract_title(html: &str, uuid: &str) -> String {
    if let Ok(regex) = Regex::new(r#"<meta\s+property=["']og:title["']\s+content=["']([^"']+)["']"#)
    {
        if let Some(captures) = regex.captures(html) {
            let title = decode_entities(captures[1].trim());
            if !title.is_empty() {
                return title;
            }
        }
    }
    format!("Suno AI Track {}", &uuid[..uuid.len().min(8)])
}

fn extract_artist(html: &str) -> String {
    if let Ok(regex) = Regex::new(r#"(?i)<title>.*? by (.*?) \| Suno</title>"#) {
        if let Some(captures) = regex.captures(html) {
            let artist = decode_entities(captures[1].trim());
            if !artist.is_empty() {
                return artist;
            }
        }
    }
    if let Ok(regex) = Regex::new(r#"\\"display_name\\":\\"([^\\"]+)\\""#) {
        if let Some(captures) = regex.captures(html) {
            let artist = decode_entities(captures[1].trim());
            if !artist.is_empty() {
                return artist;
            }
        }
    }
    "Suno AI".to_string()
}

fn extract_cover(html: &str) -> Option<String> {
    let regex =
        Regex::new(r#"<meta\s+property=["']og:image["']\s+content=["']([^"']+)["']"#).ok()?;
    regex
        .captures(html)
        .map(|captures| decode_entities(captures[1].trim()))
        .filter(|value| !value.is_empty())
}

fn extract_media_urls_from_html(html: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let regex = Regex::new(r#"https://[^"\\]+\.(?:m4a|mp3|wav|flac|mp4|ogg|opus|webm)"#).unwrap();
    for captures in regex.captures_iter(html) {
        let url = captures[0].to_string();
        if !urls.contains(&url) {
            urls.push(url);
        }
    }
    urls
}

fn html_looks_encrypted_meta(html: &str) -> bool {
    html.contains("\\\"encoding\\\":\\\"1.0.0\\\"")
        || html.contains("\"encoding\":\"1.0.0\"")
        || html.contains("api/forbidden")
}

impl SunoClient {
    pub fn new() -> Self {
        Self
    }

    fn http_client() -> Result<reqwest::Client> {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
            .build()
            .map_err(|error| anyhow!("Không thể tạo HTTP client Suno: {error}"))
    }

    pub async fn fetch_track(&self, uuid: &str) -> Result<SunoTrackMeta> {
        let client = Self::http_client()?;
        let api_url = format!("https://studio-api.prod.suno.com/api/clip/{}", uuid);
        match client
            .get(&api_url)
            .header("Accept", "application/json")
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                let value: Value = response
                    .json()
                    .await
                    .map_err(|error| anyhow!("JSON metadata Suno không hợp lệ: {error}"))?;
                return parse_clip_payload(&value, uuid);
            }
            _ => {}
        }

        let page_url = format!("https://suno.com/song/{}", uuid);
        let html = client
            .get(&page_url)
            .send()
            .await
            .map_err(|error| anyhow!("Không thể truy cập Suno: {error}"))?
            .error_for_status()
            .map_err(|error| anyhow!("Suno trả về lỗi: {error}"))?
            .text()
            .await
            .map_err(|error| anyhow!("Không đọc được dữ liệu Suno: {error}"))?;

        let media_urls = extract_media_urls_from_html(&html);
        if media_urls.is_empty() {
            return Err(anyhow!("Suno page không chứa media_urls hợp lệ"));
        }

        let is_enc = html_looks_encrypted_meta(&html);

        Ok(SunoTrackMeta {
            uuid: uuid.to_string(),
            title: extract_title(&html, uuid),
            artist: extract_artist(&html),
            audio_url: media_urls[0].clone(),
            fallback_audio_urls: media_urls[1..].to_vec(),
            cover_url: extract_cover(&html),
            duration_secs: None,
            is_protected: is_enc,
            is_encrypted: is_enc,
            page_url,
        })
    }

    pub async fn decrypt_clip_media(&self, uuid: &str, bytes: &mut [u8]) -> Result<()> {
        use sha2::{Digest, Sha256};

        let client = Self::http_client()?;
        let rights_url = "https://studio-api.prod.suno.com/api/mango/rights";
        let body = serde_json::json!({
            "content_params": {
                "content_id": uuid,
                "content_type": "clip"
            }
        });

        let res = client
            .post(rights_url)
            .json(&body)
            .header("Accept", "application/json")
            .send()
            .await?
            .error_for_status()?;

        let rights: Value = res.json().await?;
        let key_str = rights["key"]
            .as_str()
            .ok_or_else(|| anyhow!("Thiếu key trong rights Suno"))?;
        let iv_str = rights["iv"]
            .as_str()
            .ok_or_else(|| anyhow!("Thiếu iv trong rights Suno"))?;
        let glt_str = rights["glt"]
            .as_str()
            .ok_or_else(|| anyhow!("Thiếu glt trong rights Suno"))?;

        let mut hasher = Sha256::new();
        hasher.update(glt_str.as_bytes());
        let user_key = hasher.finalize();

        let content_key = decrypt_aes_gcm_wrapped(key_str, uuid.as_bytes(), &user_key)?;
        let content_iv = decrypt_aes_gcm_wrapped(iv_str, uuid.as_bytes(), &user_key)?;

        if content_key.len() != 16 || content_iv.len() != 16 {
            return Err(anyhow!("Độ dài content key hoặc iv không hợp lệ"));
        }

        decrypt_aes_128_ctr(bytes, &content_key, &content_iv)?;
        Ok(())
    }

    pub async fn download_track(
        &self,
        uuid: &str,
        output_path: &std::path::Path,
        progress: Option<&indicatif::ProgressBar>,
    ) -> Result<()> {
        let meta = self.fetch_track(uuid).await?;
        let client = Self::http_client()?;

        let mut download_urls = vec![meta.audio_url.clone()];
        for u in &meta.fallback_audio_urls {
            if !download_urls.contains(u) {
                download_urls.push(u.clone());
            }
        }

        for url in download_urls {
            if is_forbidden_url(&url) {
                continue;
            }

            let resp = match client.get(&url).send().await {
                Ok(r) if r.status().is_success() => r,
                _ => continue,
            };

            let total_size = resp.content_length().unwrap_or(0);
            if let Some(bar) = progress {
                if total_size > 0 {
                    bar.set_length(total_size);
                }
            }

            let mut resp = resp;
            let mut bytes = Vec::new();
            while let Ok(Some(chunk)) = resp.chunk().await {
                bytes.extend_from_slice(&chunk);
                if let Some(bar) = progress {
                    bar.inc(chunk.len() as u64);
                }
            }

            if bytes.len() < 1024 {
                continue;
            }

            let is_encrypted = meta.is_encrypted
                || (bytes.len() > 8
                    && &bytes[4..8] != b"ftyp"
                    && !bytes.starts_with(b"\x1a\x45\xdf\xa3"));

            if is_encrypted {
                let mut decrypted = bytes.clone();
                if self.decrypt_clip_media(uuid, &mut decrypted).await.is_ok() {
                    bytes = decrypted;
                }
            }

            std::fs::write(output_path, &bytes)?;
            return Ok(());
        }

        Err(anyhow!(
            "Không thể tải tệp âm thanh Suno sau các URL khả dụng"
        ))
    }

    pub async fn fetch_playlist(&self, playlist_id: &str) -> Result<(String, Vec<SunoTrackMeta>)> {
        let client = Self::http_client()?;
        let api_url = format!(
            "https://studio-api.prod.suno.com/api/playlist/{}/?page=1",
            playlist_id
        );
        let resp = client
            .get(&api_url)
            .header("Accept", "application/json")
            .send()
            .await?
            .error_for_status()?;
        let value: Value = resp.json().await?;
        let name = value["name"]
            .as_str()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or("Suno Playlist")
            .to_string();

        let mut tracks = Vec::new();
        if let Some(clips) = value["playlist_clips"].as_array() {
            for item in clips {
                let clip = &item["clip"];
                let clip_id = clip["id"].as_str().unwrap_or_default();
                if !clip_id.is_empty() {
                    if let Ok(meta) = parse_clip_payload(clip, clip_id) {
                        tracks.push(meta);
                    }
                }
            }
        }
        if tracks.is_empty() {
            return Err(anyhow!("Playlist Suno không có bài hát nào"));
        }
        Ok((name, tracks))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        collect_media_urls, decrypt_aes_128_ctr, is_encrypted_encoding, is_forbidden_url,
        parse_clip_payload,
    };
    use serde_json::json;

    #[test]
    fn detects_encrypted_encoding() {
        assert!(is_encrypted_encoding(Some("1.0.0")));
        assert!(!is_encrypted_encoding(None));
        assert!(!is_encrypted_encoding(Some("none")));
        assert!(!is_encrypted_encoding(Some("clear")));
    }

    #[test]
    fn detects_forbidden_audio_url() {
        assert!(is_forbidden_url(
            "https://studio-api.prod.suno.com/api/forbidden"
        ));
        assert!(!is_forbidden_url("https://cdn.example.com/a.m4a"));
    }

    #[test]
    fn accepts_encrypted_media_urls() {
        let value = json!({
            "title": "Demo",
            "display_name": "Artist",
            "audio_url": "https://studio-api.prod.suno.com/api/forbidden",
            "media_urls": [{
                "url": "https://d2lwuy8qc234o3.cloudfront.net/1/clip/id.m4a",
                "content_type": "m4a-opus",
                "encoding": "1.0.0"
            }],
            "is_public": false,
            "metadata": {"duration": 12.5}
        });
        let meta = parse_clip_payload(&value, "12345678").unwrap();
        assert_eq!(meta.title, "Demo");
        assert!(meta.is_encrypted);
    }

    #[test]
    fn accepts_clear_media_urls() {
        let value = json!({
            "title": "Demo Song",
            "display_name": "Test Artist",
            "audio_url": "https://cdn.example.com/clip/id.m4a",
            "media_urls": [{
                "url": "https://cdn.example.com/clip/id.m4a",
                "content_type": "audio/mp4",
                "encoding": "none"
            }],
            "image_large_url": "https://cdn.example.com/cover.jpeg",
            "is_public": true,
            "metadata": {"duration": 201.7}
        });
        let meta = parse_clip_payload(&value, "abcd1234").unwrap();
        assert_eq!(meta.title, "Demo Song");
        assert_eq!(meta.artist, "Test Artist");
        assert_eq!(meta.audio_url, "https://cdn.example.com/clip/id.m4a");
        assert_eq!(meta.duration_secs, Some(201.7));
        assert!(!meta.is_protected);
    }

    #[test]
    fn rejects_forbidden_without_media() {
        let value = json!({
            "title": "Private",
            "display_name": "Owner",
            "audio_url": "https://studio-api.prod.suno.com/api/forbidden",
            "media_urls": [],
            "is_public": false
        });
        assert!(parse_clip_payload(&value, "abcd1234").is_err());
    }

    #[test]
    fn collects_media_urls_and_flags_encryption() {
        let value = json!({
            "media_urls": [
                {"url": "https://a.example/x.m4a", "encoding": "1.0.0"},
                {"url": "https://b.example/y.m4a", "encoding": "none"}
            ]
        });
        let (urls, encrypted) = collect_media_urls(&value);
        assert!(encrypted);
        assert_eq!(urls.len(), 2);
    }

    #[test]
    fn test_decrypt_aes_128_ctr_roundtrip() {
        let key = [0x42_u8; 16];
        let iv = [0x24_u8; 16];
        let mut data = b"hello world 1234567890".to_vec();
        let original = data.clone();
        decrypt_aes_128_ctr(&mut data, &key, &iv).unwrap();
        assert_ne!(data, original);
        decrypt_aes_128_ctr(&mut data, &key, &iv).unwrap();
        assert_eq!(data, original);
    }
}
