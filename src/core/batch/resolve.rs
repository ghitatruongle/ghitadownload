use anyhow::{anyhow, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::IsTerminal;
use std::path::Path;

use crate::core::platform::{PlatformParser, UrlType};
use crate::core::spotify::{SpotifyClient, SpotifyTrackMeta};
use crate::core::suno::SunoClient;
use crate::core::tagger::AudioMetadata;
use crate::core::youtube::{YouTubeClient, YouTubeTrackMeta};
use crate::utils::file_helper::sanitize_name;

use super::{BatchProcessor, DownloadTask};

impl BatchProcessor {
    pub async fn resolve_inputs(&self, raw_inputs: &[String]) -> Result<Vec<DownloadTask>> {
        let mut tasks = Vec::new();
        let spotify_client = SpotifyClient::new();
        let suno_client = SunoClient::new();
        let interactive = std::io::stdout().is_terminal();

        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                .template("{spinner:.green} {msg}")?,
        );
        spinner.set_message("Đang phân tích và giải nén các liên kết...".to_string());
        spinner.enable_steady_tick(std::time::Duration::from_millis(80));

        let total_inputs = raw_inputs.len();
        for (idx, input) in raw_inputs.iter().enumerate() {
            let trimmed = input.trim();
            if trimmed.is_empty() {
                continue;
            }

            let step_str = format!("[{}/{}]", idx + 1, total_inputs);

            match PlatformParser::parse(trimmed) {
                UrlType::SpotifyTrack(id) => {
                    spinner.set_message(format!("{} Đang trích xuất bài hát Spotify...", step_str));
                    match spotify_client.fetch_track(&id).await {
                        Ok(meta) => tasks.push(self.spotify_meta_to_task(meta)),
                        Err(e) => {
                            spinner_emit(
                                &spinner,
                                interactive,
                                format!(
                                    "  {} Không lấy được metadata Spotify: {}",
                                    "⚠".yellow(),
                                    e
                                ),
                            );
                            tasks.push(self.fallback_task(trimmed, "Spotify"));
                        }
                    }
                }
                UrlType::SpotifyAlbum(id) => {
                    spinner.set_message(format!("{} Đang quét Album Spotify...", step_str));
                    match spotify_client.fetch_album_tracks(&id).await {
                        Ok((album_name, track_metas)) => {
                            spinner_emit(
                                &spinner,
                                interactive,
                                format!(
                                    "  {} Tìm thấy Album: {} ({} bài hát)",
                                    "✔".green().bold(),
                                    album_name.cyan(),
                                    track_metas.len()
                                ),
                            );
                            for meta in track_metas {
                                tasks.push(self.spotify_meta_to_task(meta));
                            }
                        }
                        Err(e) => {
                            spinner_emit(
                                &spinner,
                                interactive,
                                format!("  {} Không lấy được album Spotify: {}", "⚠".yellow(), e),
                            );
                            tasks.push(self.fallback_task(trimmed, "Spotify"));
                        }
                    }
                }
                UrlType::SpotifyPlaylist(id) => {
                    spinner.set_message(format!("{} Đang quét Playlist Spotify...", step_str));
                    match spotify_client.fetch_playlist_tracks(&id).await {
                        Ok((pl_name, track_metas)) => {
                            spinner_emit(
                                &spinner,
                                interactive,
                                format!(
                                    "  {} Tìm thấy Playlist: {} ({} bài hát)",
                                    "✔".green().bold(),
                                    pl_name.cyan(),
                                    track_metas.len()
                                ),
                            );
                            for meta in track_metas {
                                tasks.push(self.spotify_meta_to_task(meta));
                            }
                        }
                        Err(e) => {
                            spinner_emit(
                                &spinner,
                                interactive,
                                format!(
                                    "  {} Không lấy được playlist Spotify: {}",
                                    "⚠".yellow(),
                                    e
                                ),
                            );
                            tasks.push(self.fallback_task(trimmed, "Spotify"));
                        }
                    }
                }
                UrlType::YouTubePlaylist(url) | UrlType::YouTubeMusicPlaylist(url) => {
                    spinner
                        .set_message(format!("{} Đang quét danh sách phát YouTube...", step_str));
                    match resolve_youtube_playlist(
                        self.ytdlp_path.as_path(),
                        self.has_node,
                        url.clone(),
                    )
                    .await
                    {
                        Ok(items) => {
                            spinner_emit(
                                &spinner,
                                interactive,
                                format!(
                                    "  {} Tìm thấy Playlist YouTube với {} video/bài hát",
                                    "✔".green().bold(),
                                    items.len()
                                ),
                            );
                            let total_items = items.len() as u32;
                            for (idx, item) in items.into_iter().enumerate() {
                                let track_num = (idx + 1) as u32;
                                tasks.push(DownloadTask {
                                    display_title: format!("{} - {}", item.uploader, item.title),
                                    source_or_search: item.direct_url,
                                    fallback_searches: vec![format!(
                                        "ytsearch1:{} {}",
                                        item.uploader, item.title
                                    )],
                                    expected_duration_secs: None,
                                    metadata: AudioMetadata {
                                        title: item.title,
                                        artists: vec![item.uploader],
                                        album: "YouTube Music".to_string(),
                                        release_year: None,
                                        cover_url: item.thumbnail_url,
                                        track_number: Some(track_num),
                                        total_tracks: Some(total_items),
                                    },
                                });
                            }
                        }
                        Err(e) => {
                            spinner_emit(
                                &spinner,
                                interactive,
                                format!(
                                    "  {} Không lấy được playlist YouTube: {}",
                                    "⚠".yellow(),
                                    e
                                ),
                            );
                            tasks.push(self.fallback_task(trimmed, "YouTube"));
                        }
                    }
                }
                UrlType::YouTubeVideo(url) | UrlType::YouTubeMusicTrack(url) => {
                    spinner.set_message(format!(
                        "{} Đang lấy thông tin bài hát YouTube...",
                        step_str
                    ));
                    match resolve_youtube_video(
                        self.ytdlp_path.as_path(),
                        self.has_node,
                        url.clone(),
                    )
                    .await
                    {
                        Ok(item) => {
                            let expected = item.duration_secs.map(|d| d.round() as u64);
                            tasks.push(DownloadTask {
                                display_title: format!("{} - {}", item.uploader, item.title),
                                source_or_search: item.direct_url,
                                fallback_searches: vec![format!(
                                    "ytsearch1:{} {}",
                                    item.uploader, item.title
                                )],
                                expected_duration_secs: expected,
                                metadata: AudioMetadata {
                                    title: item.title,
                                    artists: vec![item.uploader],
                                    album: "YouTube".to_string(),
                                    release_year: None,
                                    cover_url: item.thumbnail_url,
                                    track_number: None,
                                    total_tracks: None,
                                },
                            });
                        }
                        Err(_) => {
                            tasks.push(self.fallback_task(trimmed, "YouTube"));
                        }
                    }
                }
                UrlType::SunoTrack(uuid) => {
                    spinner.set_message(format!(
                        "{} Đang lấy thông tin bài hát Suno AI...",
                        step_str
                    ));
                    match suno_client.fetch_track(&uuid).await {
                        Ok(meta) => {
                            spinner_emit(
                                &spinner,
                                interactive,
                                format!(
                                    "  {} Tìm thấy bài hát Suno: {}",
                                    "✔".green().bold(),
                                    meta.title.cyan()
                                ),
                            );

                            let mut fallback_searches = meta.fallback_audio_urls.clone();
                            fallback_searches
                                .push(format!("ytsearch1:{} {}", meta.artist, meta.title));
                            fallback_searches
                                .push(format!("ytsearch3:{} {}", meta.artist, meta.title));
                            tasks.push(DownloadTask {
                                display_title: format!("{} - {}", meta.artist, meta.title),
                                source_or_search: format!("suno:{}", uuid),
                                fallback_searches,
                                expected_duration_secs: meta
                                    .duration_secs
                                    .map(|d| d.round() as u64),
                                metadata: AudioMetadata {
                                    title: meta.title,
                                    artists: vec![meta.artist],
                                    album: "Suno AI Music".to_string(),
                                    release_year: None,
                                    cover_url: meta.cover_url,
                                    track_number: None,
                                    total_tracks: None,
                                },
                            });
                        }
                        Err(error) => {
                            spinner_emit(
                                &spinner,
                                interactive,
                                format!("  {} Không lấy được media Suno: {}", "✖".red(), error),
                            );
                            tasks.push(self.fallback_task(trimmed, "Suno AI"));
                        }
                    }
                }
                UrlType::SunoPlaylist(uuid) => {
                    spinner.set_message(format!("{} Đang quét Playlist Suno AI...", step_str));
                    match suno_client.fetch_playlist(&uuid).await {
                        Ok((playlist_name, tracks)) => {
                            spinner_emit(
                                &spinner,
                                interactive,
                                format!(
                                    "  {} Tìm thấy Playlist Suno: {} ({} bài hát)",
                                    "✔".green().bold(),
                                    playlist_name.cyan(),
                                    tracks.len()
                                ),
                            );
                            let total = tracks.len() as u32;
                            for (idx, meta) in tracks.into_iter().enumerate() {
                                let mut fallback_searches = meta.fallback_audio_urls.clone();
                                fallback_searches
                                    .push(format!("ytsearch1:{} {}", meta.artist, meta.title));
                                fallback_searches
                                    .push(format!("ytsearch3:{} {}", meta.artist, meta.title));
                                tasks.push(DownloadTask {
                                    display_title: format!("{} - {}", meta.artist, meta.title),
                                    source_or_search: format!("suno:{}", meta.uuid),
                                    fallback_searches,
                                    expected_duration_secs: meta
                                        .duration_secs
                                        .map(|d| d.round() as u64),
                                    metadata: AudioMetadata {
                                        title: meta.title,
                                        artists: vec![meta.artist],
                                        album: playlist_name.clone(),
                                        release_year: None,
                                        cover_url: meta.cover_url,
                                        track_number: Some((idx + 1) as u32),
                                        total_tracks: Some(total),
                                    },
                                });
                            }
                        }
                        Err(error) => {
                            spinner_emit(
                                &spinner,
                                interactive,
                                format!("  {} Không lấy được playlist Suno: {}", "✖".red(), error),
                            );
                            tasks.push(self.fallback_task(trimmed, "Suno AI"));
                        }
                    }
                }
                UrlType::DirectMedia(url) => {
                    spinner.set_message(format!(
                        "{} Đang nhận diện tệp media trực tiếp...",
                        step_str
                    ));
                    let title = url
                        .rsplit('/')
                        .next()
                        .map(|name| name.split('?').next().unwrap_or(name).to_string())
                        .filter(|name| !name.is_empty())
                        .unwrap_or_else(|| "media_track".to_string());
                    let title = title
                        .rsplit_once('.')
                        .map(|(stem, _)| stem.to_string())
                        .unwrap_or(title);
                    tasks.push(DownloadTask {
                        display_title: title.clone(),
                        source_or_search: url.clone(),
                        fallback_searches: Vec::new(),
                        expected_duration_secs: None,
                        metadata: AudioMetadata {
                            title,
                            artists: vec!["Direct".to_string()],
                            album: "Direct Media".to_string(),
                            release_year: None,
                            cover_url: None,
                            track_number: None,
                            total_tracks: None,
                        },
                    });
                }
                UrlType::SocialVideo { url, platform_name } => {
                    spinner.set_message(format!(
                        "{} Đang lấy thông tin {}...",
                        step_str, platform_name
                    ));
                    if let Ok(item) = resolve_youtube_media(
                        self.ytdlp_path.as_path(),
                        self.has_node,
                        url.clone(),
                        platform_name.clone(),
                    )
                    .await
                    {
                        tasks.push(DownloadTask {
                            display_title: format!("{} - {}", item.uploader, item.title),
                            source_or_search: item.direct_url,
                            fallback_searches: Vec::new(),
                            expected_duration_secs: None,
                            metadata: AudioMetadata {
                                title: item.title,
                                artists: vec![item.uploader],
                                album: platform_name.clone(),
                                release_year: None,
                                cover_url: item.thumbnail_url,
                                track_number: None,
                                total_tracks: None,
                            },
                        });
                    } else {
                        tasks.push(DownloadTask {
                            display_title: format!(
                                "{} - {}",
                                platform_name,
                                sanitize_name(trimmed)
                            ),
                            source_or_search: trimmed.to_string(),
                            fallback_searches: Vec::new(),
                            expected_duration_secs: None,
                            metadata: AudioMetadata {
                                title: format!("{} Audio", platform_name),
                                artists: vec![platform_name.clone()],
                                album: platform_name.clone(),
                                release_year: None,
                                cover_url: None,
                                track_number: None,
                                total_tracks: None,
                            },
                        });
                    }
                }
                UrlType::DirectSearch(query) => {
                    spinner.set_message(format!("{} Tìm kiếm: {}...", step_str, query));
                    tasks.push(DownloadTask {
                        display_title: query.clone(),
                        source_or_search: format!("ytsearch1:{}", query),
                        fallback_searches: vec![
                            format!("ytsearch3:{}", query),
                            format!("ytsearch1:{} official audio", query),
                        ],
                        expected_duration_secs: None,
                        metadata: AudioMetadata {
                            title: query.clone(),
                            artists: vec!["Search".to_string()],
                            album: "Search".to_string(),
                            release_year: None,
                            cover_url: None,
                            track_number: None,
                            total_tracks: None,
                        },
                    });
                }
            }
        }

        spinner.finish_and_clear();
        Ok(tasks)
    }

    fn fallback_task(&self, source: &str, platform_name: &str) -> DownloadTask {
        let title = sanitize_name(source);
        DownloadTask {
            display_title: format!("{} - {}", platform_name, title),
            source_or_search: source.to_string(),
            fallback_searches: Vec::new(),
            expected_duration_secs: None,
            metadata: AudioMetadata {
                title,
                artists: vec![platform_name.to_string()],
                album: platform_name.to_string(),
                release_year: None,
                cover_url: None,
                track_number: None,
                total_tracks: None,
            },
        }
    }

    fn spotify_meta_to_task(&self, meta: SpotifyTrackMeta) -> DownloadTask {
        let artist_str = meta.artists.join(", ");
        let display_title = format!("{} - {}", artist_str, meta.title);
        let primary_artist = meta.artists.first().cloned().unwrap_or_default();
        let search_target = format!("ytsearch1:{}", meta.search_query);

        let fallback_searches = vec![
            format!("ytsearch1:{} {} Topic", primary_artist, meta.title),
            format!("ytsearch1:{} {} official audio", primary_artist, meta.title),
            format!("ytsearch1:{} - {}", primary_artist, meta.title),
            format!("ytsearch1:{} {}", primary_artist, meta.title),
            format!("ytsearch3:{} {}", primary_artist, meta.title),
        ];

        let expected_duration_secs = meta.duration_ms.map(|ms| ms / 1000);

        DownloadTask {
            display_title,
            source_or_search: search_target,
            fallback_searches,
            expected_duration_secs,
            metadata: AudioMetadata {
                title: meta.title,
                artists: meta.artists,
                album: meta.album,
                release_year: meta.release_year,
                cover_url: meta.cover_url,
                track_number: meta.track_number,
                total_tracks: meta.total_tracks,
            },
        }
    }
}

fn spinner_emit(spinner: &ProgressBar, interactive: bool, message: String) {
    if interactive {
        spinner.println(message);
    } else {
        println!("{message}");
    }
}

async fn resolve_youtube_playlist(
    ytdlp_path: &Path,
    has_node: bool,
    url: String,
) -> Result<Vec<YouTubeTrackMeta>> {
    let path = ytdlp_path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        YouTubeClient::new(&path, has_node).extract_playlist_items(&url)
    })
    .await
    .map_err(|error| anyhow!("Không thể đọc playlist YouTube: {error}"))?
}

async fn resolve_youtube_video(
    ytdlp_path: &Path,
    has_node: bool,
    url: String,
) -> Result<YouTubeTrackMeta> {
    let path = ytdlp_path.to_path_buf();
    tokio::task::spawn_blocking(move || YouTubeClient::new(&path, has_node).fetch_video_info(&url))
        .await
        .map_err(|error| anyhow!("Không thể đọc video YouTube: {error}"))?
}

async fn resolve_youtube_media(
    ytdlp_path: &Path,
    has_node: bool,
    url: String,
    platform_name: String,
) -> Result<YouTubeTrackMeta> {
    let path = ytdlp_path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        YouTubeClient::new(&path, has_node).fetch_media_info(&url, &platform_name)
    })
    .await
    .map_err(|error| anyhow!("Không thể đọc media YouTube: {error}"))?
}
