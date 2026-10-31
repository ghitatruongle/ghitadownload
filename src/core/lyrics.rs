use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::core::tagger::AudioMetadata;
use crate::utils::http;

const LRCLIB_GET_API: &str = "https://lrclib.net/api/get";
const LRCLIB_SEARCH_API: &str = "https://lrclib.net/api/search";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lyrics {
    pub plain: Option<String>,
    pub synced: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrcLibRecord {
    #[serde(default)]
    plain_lyrics: Option<String>,
    #[serde(default)]
    synced_lyrics: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
}

pub async fn fetch_lyrics(
    meta: &AudioMetadata,
    expected_duration_secs: Option<u64>,
) -> Result<Option<Lyrics>> {
    let client = http::shared_client()?;
    let title = meta.title.trim();
    if title.is_empty() {
        return Ok(None);
    }
    let artist = meta
        .artists
        .first()
        .map(String::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();

    if !artist.is_empty() {
        let mut request = client
            .get(LRCLIB_GET_API)
            .query(&[("artist_name", artist.as_str()), ("track_name", title)]);
        if let Some(duration) = expected_duration_secs {
            request = request.query(&[("duration", duration.to_string())]);
        }
        if let Ok(record) = send_record(request).await {
            if let Some((_, _, lyrics)) = record_to_lyrics(&record, expected_duration_secs) {
                if !is_trivial_lyrics(&lyrics) {
                    return Ok(Some(lyrics));
                }
            }
        }
    }

    let mut query = vec![("track_name", title)];
    if !artist.is_empty() {
        query.push(("artist_name", artist.as_str()));
    }
    let request = client.get(LRCLIB_SEARCH_API).query(&query);
    let records = send_records(request).await.unwrap_or_default();
    let best = records
        .iter()
        .filter_map(|record| record_to_lyrics(record, expected_duration_secs))
        .filter(|(_, _, lyrics)| !is_trivial_lyrics(lyrics))
        .min_by_key(|(distance, synced_preference, _)| (*distance, *synced_preference));
    Ok(best.map(|(_, _, lyrics)| lyrics))
}

fn is_trivial_lyrics(lyrics: &Lyrics) -> bool {
    fn trivial(text: &str) -> bool {
        let text = text.trim();
        text.chars().count() < 30 && text.split_whitespace().count() < 4
    }
    let provided = [&lyrics.plain, &lyrics.synced]
        .into_iter()
        .flatten()
        .count();
    provided > 0
        && [&lyrics.plain, &lyrics.synced]
            .into_iter()
            .flatten()
            .all(|text| trivial(text))
}

pub fn write_synced_lrc(final_path: &Path, synced: &str) -> Result<PathBuf> {
    let lrc_path = final_path.with_extension("lrc");
    std::fs::write(&lrc_path, synced.trim())?;
    Ok(lrc_path)
}

async fn send_record(request: reqwest::RequestBuilder) -> Result<LrcLibRecord> {
    let payload = fetch_text(request).await?;
    Ok(serde_json::from_str(&payload)?)
}

async fn send_records(request: reqwest::RequestBuilder) -> Result<Vec<LrcLibRecord>> {
    let payload = fetch_text(request).await?;
    Ok(serde_json::from_str(&payload)?)
}

async fn fetch_text(request: reqwest::RequestBuilder) -> Result<String> {
    http::with_retry(|| async {
        let response = request
            .try_clone()
            .ok_or_else(|| anyhow::anyhow!("Không thể nhân bản yêu cầu LRCLIB"))?
            .send()
            .await?
            .error_for_status()?;
        Ok(response.text().await?)
    })
    .await
}

fn record_to_lyrics(
    record: &LrcLibRecord,
    expected_duration_secs: Option<u64>,
) -> Option<(u64, u8, Lyrics)> {
    let lyrics = Lyrics {
        plain: record
            .plain_lyrics
            .clone()
            .filter(|text| !text.trim().is_empty()),
        synced: record
            .synced_lyrics
            .clone()
            .filter(|text| !text.trim().is_empty()),
    };
    if lyrics.plain.is_none() && lyrics.synced.is_none() {
        return None;
    }
    let distance = match (record.duration, expected_duration_secs) {
        (Some(actual), Some(expected)) => (actual - expected as f64).abs().round() as u64,
        _ => 0,
    };
    let synced_preference = if lyrics.synced.is_some() { 0 } else { 1 };
    Some((distance, synced_preference, lyrics))
}

#[cfg(test)]
mod tests {
    use super::{record_to_lyrics, write_synced_lrc, LrcLibRecord};
    use crate::core::tagger::AudioMetadata;

    fn record(json: &str) -> LrcLibRecord {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn record_to_lyrics_prefers_synced_and_closest_duration() {
        let synced =
            record(r#"{"plainLyrics":null,"syncedLyrics":"[00:01.00] Xin chao","duration":200.0}"#);
        let plain_far =
            record(r#"{"plainLyrics":"Loi khong dong","syncedLyrics":null,"duration":260.0}"#);
        let expected = Some(200u64);

        let picked_synced = record_to_lyrics(&synced, expected).unwrap();
        assert_eq!(picked_synced.1, 0);
        assert!(picked_synced.2.synced.is_some());

        let picked_plain = record_to_lyrics(&plain_far, expected).unwrap();
        assert_eq!(picked_plain.0, 60);
        assert_eq!(picked_plain.1, 1);
    }

    #[test]
    fn record_to_lyrics_rejects_empty_or_missing_lyrics() {
        let empty = record(r#"{"plainLyrics":"","syncedLyrics":null,"duration":100.0}"#);
        let missing = record(r#"{"plainLyrics":null,"syncedLyrics":null,"duration":100.0}"#);
        assert!(record_to_lyrics(&empty, None).is_none());
        assert!(record_to_lyrics(&missing, None).is_none());
    }

    #[test]
    fn write_synced_lrc_uses_lrc_extension_next_to_audio_file() {
        let dir = std::env::temp_dir().join(format!(
            "ghita_lyrics_lrc_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let audio = dir.join("Bai Hat.mp3");
        std::fs::write(&audio, b"x").unwrap();

        let path = write_synced_lrc(&audio, "[00:01.00] Xin chao\n").unwrap();
        assert_eq!(path, dir.join("Bai Hat.lrc"));
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "[00:01.00] Xin chao");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn is_trivial_lyrics_filters_probe_junk_and_keeps_real_lyrics() {
        let junk = super::Lyrics {
            plain: Some("probe".to_string()),
            synced: Some("[00:00.00]probe".to_string()),
        };
        assert!(super::is_trivial_lyrics(&junk));

        let junk_synced_only = super::Lyrics {
            plain: None,
            synced: Some("[00:00.00]probe".to_string()),
        };
        assert!(super::is_trivial_lyrics(&junk_synced_only));

        let real = super::Lyrics {
            plain: Some("We're no strangers to love\nYou know the rules and so do I".to_string()),
            synced: None,
        };
        assert!(!super::is_trivial_lyrics(&real));
    }

    #[test]
    fn fetch_lyrics_skips_empty_title_without_network() {
        let meta = AudioMetadata {
            title: String::new(),
            artists: vec!["Nghệ Sĩ".to_string()],
            album: String::new(),
            release_year: None,
            cover_url: None,
            track_number: None,
            total_tracks: None,
            lyrics: None,
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .unwrap();
        let result = runtime.block_on(super::fetch_lyrics(&meta, None)).unwrap();
        assert!(result.is_none());
    }
}
