use anyhow::Result;
use id3::frame::{Picture, PictureType};
use id3::{Tag, TagLike, Version};
use reqwest::Client;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct AudioMetadata {
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub release_year: Option<u32>,
    pub cover_url: Option<String>,
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
        Self {
            http_client: Client::new(),
        }
    }

    pub async fn tag_mp3(&self, file_path: &Path, meta: &AudioMetadata) -> Result<()> {
        let mut tag = Tag::read_from_path(file_path).unwrap_or_else(|_| Tag::new());

        tag.set_title(&meta.title);
        tag.set_artist(meta.artists.join(", "));
        tag.set_album(&meta.album);

        if let Some(year) = meta.release_year {
            tag.set_year(year as i32);
        }

        if let Some(ref raw_cover_url) = meta.cover_url {
            let normalized_url = if raw_cover_url.contains("vi_webp") {
                raw_cover_url.replace("vi_webp", "vi").replace(".webp", ".jpg")
            } else if raw_cover_url.ends_with(".webp") {
                raw_cover_url.replace(".webp", ".jpg")
            } else {
                raw_cover_url.clone()
            };

            if let Ok(resp) = self.http_client.get(&normalized_url).send().await {
                if let Ok(bytes) = resp.bytes().await {
                    if !bytes.is_empty() {
                        let mime_type = if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
                            "image/png".to_string()
                        } else {
                            "image/jpeg".to_string()
                        };

                        tag.add_frame(Picture {
                            mime_type,
                            picture_type: PictureType::CoverFront,
                            description: "Cover Art".to_string(),
                            data: bytes.to_vec(),
                        });
                    }
                }
            }
        }

        tag.write_to_path(file_path, Version::Id3v24)?;
        Ok(())
    }
}
