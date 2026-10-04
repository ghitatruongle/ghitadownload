use anyhow::{anyhow, Result};
use id3::frame::{Picture, PictureType};
use id3::{Tag, TagLike, Version};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioMetadata {
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub release_year: Option<u32>,
    pub cover_url: Option<String>,
    #[serde(default)]
    pub track_number: Option<u32>,
    #[serde(default)]
    pub total_tracks: Option<u32>,
}

pub struct Tagger {
    http_client: Client,
}

impl Default for Tagger {
    fn default() -> Self {
        Self::new()
    }
}

impl Tagger {
    pub fn new() -> Self {
        let http_client = crate::utils::http::shared_client()
            .cloned()
            .unwrap_or_default();
        Self { http_client }
    }

    pub async fn tag_mp3(&self, file_path: &Path, meta: &AudioMetadata) -> Result<()> {
        let mut tag = Tag::read_from_path(file_path).map_err(|e| {
            anyhow!(
                "Không thể đọc metadata MP3 tại {}: {}",
                file_path.display(),
                e
            )
        })?;

        tag.set_title(&meta.title);
        tag.set_artist(meta.artists.join(", "));
        tag.set_album(&meta.album);

        if let Some(year) = meta.release_year {
            tag.set_year(year as i32);
        }

        if let Some(track) = meta.track_number {
            tag.set_track(track);
        }

        if let Some(total) = meta.total_tracks {
            tag.set_total_tracks(total);
        }

        if let Some(ref raw_cover_url) = meta.cover_url {
            let (bytes, mime_type) = self.fetch_cover_bytes(raw_cover_url).await?;
            tag.add_frame(Picture {
                mime_type,
                picture_type: PictureType::CoverFront,
                description: "Cover Art".to_string(),
                data: bytes,
            });
        }

        tag.write_to_path(file_path, Version::Id3v24)?;
        Ok(())
    }

    pub async fn fetch_cover_bytes(&self, raw_cover_url: &str) -> Result<(Vec<u8>, String)> {
        const MAX_COVER_BYTES: usize = 10 * 1024 * 1024;
        let normalized_url = if raw_cover_url.contains("vi_webp") {
            raw_cover_url
                .replace("vi_webp", "vi")
                .replace(".webp", ".jpg")
        } else if raw_cover_url.ends_with(".webp") {
            raw_cover_url.replace(".webp", ".jpg")
        } else {
            raw_cover_url.to_string()
        };

        let response = self
            .http_client
            .get(&normalized_url)
            .send()
            .await
            .map_err(|e| anyhow!("Không thể tải ảnh bìa: {}", e))?;
        if !response.status().is_success() {
            return Err(anyhow!(
                "Tải ảnh bìa thất bại với HTTP {}",
                response.status()
            ));
        }
        if let Some(length) = response.content_length() {
            if length > MAX_COVER_BYTES as u64 {
                return Err(anyhow!(
                    "Ảnh bìa vượt quá giới hạn {} byte",
                    MAX_COVER_BYTES
                ));
            }
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|e| anyhow!("Không thể đọc ảnh bìa: {}", e))?;
        if bytes.is_empty() || bytes.len() > MAX_COVER_BYTES {
            return Err(anyhow!("Ảnh bìa rỗng hoặc quá lớn"));
        }
        let mime_type = detect_image_mime(&bytes)
            .ok_or_else(|| anyhow!("Dữ liệu ảnh bìa không phải PNG hoặc JPEG hợp lệ"))?;
        Ok((bytes.to_vec(), mime_type))
    }

    pub async fn fetch_cover_file(
        &self,
        meta: &AudioMetadata,
        dir: &Path,
    ) -> Result<Option<PathBuf>> {
        let Some(ref raw_cover_url) = meta.cover_url else {
            return Ok(None);
        };
        let (bytes, mime_type) = self.fetch_cover_bytes(raw_cover_url).await?;
        let extension = if mime_type == "image/png" {
            "png"
        } else {
            "jpg"
        };
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = dir.join(format!(
            "ghita_cover_{}_{}.{}",
            std::process::id(),
            stamp,
            extension
        ));
        std::fs::write(&path, &bytes)?;
        Ok(Some(path))
    }
}

fn detect_image_mime(bytes: &[u8]) -> Option<String> {
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        Some("image/png".to_string())
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg".to_string())
    } else {
        None
    }
}
