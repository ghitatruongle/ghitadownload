use anyhow::{anyhow, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

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
                        self.settings.cookies_file.as_ref(),
                        self.settings.cookies_from_browser.as_deref(),
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
                                tasks.push(build_task(
                                    format!("{} - {}", item.uploader, item.title),
                                    item.direct_url,
                                    vec![format!("ytsearch1:{} {}", item.uploader, item.title)],
                                    None,
                                    item.title,
                                    vec![item.uploader],
                                    "YouTube Music".to_string(),
                                    None,
                                    item.thumbnail_url,
                                    Some(track_num),
                                    Some(total_items),
                                ));
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
                        self.settings.cookies_file.as_ref(),
                        self.settings.cookies_from_browser.as_deref(),
                    )
                    .await
                    {
                        Ok(item) => {
                            let expected = item.duration_secs.map(|d| d.round() as u64);
                            tasks.push(build_task(
                                format!("{} - {}", item.uploader, item.title),
                                item.direct_url,
                                vec![format!("ytsearch1:{} {}", item.uploader, item.title)],
                                expected,
                                item.title,
                                vec![item.uploader],
                                "YouTube".to_string(),
                                None,
                                item.thumbnail_url,
                                None,
                                None,
                            ));
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
                            tasks.push(build_task(
                                format!("{} - {}", meta.artist, meta.title),
                                format!("suno:{}", uuid),
                                fallback_searches,
                                meta.duration_secs.map(|d| d.round() as u64),
                                meta.title,
                                vec![meta.artist],
                                "Suno AI Music".to_string(),
                                None,
                                meta.cover_url,
                                None,
                                None,
                            ));
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
                                tasks.push(build_task(
                                    format!("{} - {}", meta.artist, meta.title),
                                    format!("suno:{}", meta.uuid),
                                    fallback_searches,
                                    meta.duration_secs.map(|d| d.round() as u64),
                                    meta.title,
                                    vec![meta.artist],
                                    playlist_name.clone(),
                                    None,
                                    meta.cover_url,
                                    Some((idx + 1) as u32),
                                    Some(total),
                                ));
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
                    tasks.push(build_task(
                        title.clone(),
                        url.clone(),
                        Vec::new(),
                        None,
                        title,
                        vec!["Direct".to_string()],
                        "Direct Media".to_string(),
                        None,
                        None,
                        None,
                        None,
                    ));
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
                        self.settings.cookies_file.as_ref(),
                        self.settings.cookies_from_browser.as_deref(),
                    )
                    .await
                    {
                        tasks.push(build_task(
                            format!("{} - {}", item.uploader, item.title),
                            item.direct_url,
                            Vec::new(),
                            None,
                            item.title,
                            vec![item.uploader],
                            platform_name.clone(),
                            None,
                            item.thumbnail_url,
                            None,
                            None,
                        ));
                    } else {
                        tasks.push(build_task(
                            format!("{} - {}", platform_name, sanitize_name(trimmed)),
                            trimmed.to_string(),
                            Vec::new(),
                            None,
                            format!("{} Audio", platform_name),
                            vec![platform_name.clone()],
                            platform_name.clone(),
                            None,
                            None,
                            None,
                            None,
                        ));
                    }
                }
                UrlType::DirectSearch(query) => {
                    spinner.set_message(format!("{} Tìm kiếm: {}...", step_str, query));
                    tasks.push(build_task(
                        query.clone(),
                        format!("ytsearch1:{}", query),
                        vec![
                            format!("ytsearch3:{}", query),
                            format!("ytsearch1:{} official audio", query),
                        ],
                        None,
                        query.clone(),
                        vec!["Search".to_string()],
                        "Search".to_string(),
                        None,
                        None,
                        None,
                        None,
                    ));
                }
            }
        }

        spinner.finish_and_clear();
        Ok(tasks)
    }

    fn fallback_task(&self, source: &str, platform_name: &str) -> DownloadTask {
        let title = sanitize_name(source);
        build_task(
            format!("{} - {}", platform_name, title),
            source.to_string(),
            Vec::new(),
            None,
            title,
            vec![platform_name.to_string()],
            platform_name.to_string(),
            None,
            None,
            None,
            None,
        )
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

        build_task(
            display_title,
            search_target,
            fallback_searches,
            expected_duration_secs,
            meta.title,
            meta.artists,
            meta.album,
            meta.release_year,
            meta.cover_url,
            meta.track_number,
            meta.total_tracks,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn build_task(
    display_title: String,
    source_or_search: String,
    fallback_searches: Vec<String>,
    expected_duration_secs: Option<u64>,
    title: String,
    artists: Vec<String>,
    album: String,
    release_year: Option<u32>,
    cover_url: Option<String>,
    track_number: Option<u32>,
    total_tracks: Option<u32>,
) -> DownloadTask {
    DownloadTask {
        display_title,
        source_or_search,
        fallback_searches,
        expected_duration_secs,
        metadata: AudioMetadata {
            title,
            artists,
            album,
            release_year,
            cover_url,
            track_number,
            total_tracks,
        },
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
    cookies_file: Option<&PathBuf>,
    cookies_from_browser: Option<&str>,
) -> Result<Vec<YouTubeTrackMeta>> {
    let path = ytdlp_path.to_path_buf();
    let cookies_file = cookies_file.cloned();
    let cookies_from_browser = cookies_from_browser.map(str::to_string);
    tokio::task::spawn_blocking(move || {
        YouTubeClient::new(
            &path,
            has_node,
            cookies_file.as_ref(),
            cookies_from_browser.as_deref(),
        )
        .extract_playlist_items(&url)
    })
    .await
    .map_err(|error| anyhow!("Không thể đọc playlist YouTube: {error}"))?
}

async fn resolve_youtube_video(
    ytdlp_path: &Path,
    has_node: bool,
    url: String,
    cookies_file: Option<&PathBuf>,
    cookies_from_browser: Option<&str>,
) -> Result<YouTubeTrackMeta> {
    let path = ytdlp_path.to_path_buf();
    let cookies_file = cookies_file.cloned();
    let cookies_from_browser = cookies_from_browser.map(str::to_string);
    tokio::task::spawn_blocking(move || {
        YouTubeClient::new(
            &path,
            has_node,
            cookies_file.as_ref(),
            cookies_from_browser.as_deref(),
        )
        .fetch_video_info(&url)
    })
    .await
    .map_err(|error| anyhow!("Không thể đọc video YouTube: {error}"))?
}

async fn resolve_youtube_media(
    ytdlp_path: &Path,
    has_node: bool,
    url: String,
    platform_name: String,
    cookies_file: Option<&PathBuf>,
    cookies_from_browser: Option<&str>,
) -> Result<YouTubeTrackMeta> {
    let path = ytdlp_path.to_path_buf();
    let cookies_file = cookies_file.cloned();
    let cookies_from_browser = cookies_from_browser.map(str::to_string);
    tokio::task::spawn_blocking(move || {
        YouTubeClient::new(
            &path,
            has_node,
            cookies_file.as_ref(),
            cookies_from_browser.as_deref(),
        )
        .fetch_media_info(&url, &platform_name)
    })
    .await
    .map_err(|error| anyhow!("Không thể đọc media YouTube: {error}"))?
}
