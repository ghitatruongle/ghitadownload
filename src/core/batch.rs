use anyhow::{anyhow, Result};
use colored::Colorize;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::utils::file_helper::{ensure_dir, sanitize_name};

use super::config::{AudioFormat, DownloadSettings};
use super::downloader::{DownloadMode, StreamDownloader};
use super::platform::{PlatformParser, UrlType};
use super::spotify::{SpotifyClient, SpotifyTrackMeta};
use super::suno::SunoClient;
use super::tagger::{AudioMetadata, Tagger};
use super::transcoder::Transcoder;
use super::verify;
use super::youtube::YouTubeClient;

pub const DEFAULT_CONCURRENCY: usize = 3;
pub const DURATION_TOLERANCE_SECS: f64 = 10.0;
pub const FAILED_QUEUE_FILE: &str = "failed_tasks.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTask {
    pub display_title: String,
    pub source_or_search: String,
    pub fallback_searches: Vec<String>,
    pub expected_duration_secs: Option<u64>,
    pub metadata: AudioMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedQueue {
    pub settings: DownloadSettings,
    pub tasks: Vec<DownloadTask>,
}

pub struct BatchOutcome {
    pub success_count: usize,
    pub failed_tasks: Vec<DownloadTask>,
}

struct TaskOutcome {
    ok: bool,
    logs: Vec<String>,
    task: DownloadTask,
}

pub struct BatchProcessor {
    settings: DownloadSettings,
    ytdlp_path: Arc<PathBuf>,
    ffmpeg_path: Arc<PathBuf>,
    has_node: bool,
}

impl BatchProcessor {
    pub fn new(
        settings: DownloadSettings,
        ytdlp_path: &Path,
        ffmpeg_path: &Path,
        has_node: bool,
    ) -> Self {
        Self {
            settings,
            ytdlp_path: Arc::new(ytdlp_path.to_path_buf()),
            ffmpeg_path: Arc::new(ffmpeg_path.to_path_buf()),
            has_node,
        }
    }

    pub async fn resolve_inputs(&self, raw_inputs: &[String]) -> Result<Vec<DownloadTask>> {
        let mut tasks = Vec::new();
        let spotify_client = SpotifyClient::new();
        let suno_client = SunoClient::new();
        let yt_client = YouTubeClient::new(self.ytdlp_path.as_path(), self.has_node);

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
                    if let Ok(meta) = spotify_client.fetch_track(&id).await {
                        tasks.push(self.spotify_meta_to_task(meta));
                    }
                }
                UrlType::SpotifyAlbum(id) => {
                    spinner.set_message(format!("{} Đang quét Album Spotify...", step_str));
                    if let Ok((album_name, track_metas)) =
                        spotify_client.fetch_album_tracks(&id).await
                    {
                        println!(
                            "  {} Tìm thấy Album: {} ({} bài hát)",
                            "✔".green().bold(),
                            album_name.cyan(),
                            track_metas.len()
                        );
                        for meta in track_metas {
                            tasks.push(self.spotify_meta_to_task(meta));
                        }
                    }
                }
                UrlType::SpotifyPlaylist(id) => {
                    spinner.set_message(format!("{} Đang quét Playlist Spotify...", step_str));
                    if let Ok((pl_name, track_metas)) =
                        spotify_client.fetch_playlist_tracks(&id).await
                    {
                        println!(
                            "  {} Tìm thấy Playlist: {} ({} bài hát)",
                            "✔".green().bold(),
                            pl_name.cyan(),
                            track_metas.len()
                        );
                        for meta in track_metas {
                            tasks.push(self.spotify_meta_to_task(meta));
                        }
                    }
                }
                UrlType::YouTubePlaylist(url) | UrlType::YouTubeMusicPlaylist(url) => {
                    spinner
                        .set_message(format!("{} Đang quét danh sách phát YouTube...", step_str));
                    if let Ok(items) = yt_client.extract_playlist_items(&url) {
                        println!(
                            "  {} Tìm thấy Playlist YouTube với {} video/bài hát",
                            "✔".green().bold(),
                            items.len()
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
                }
                UrlType::YouTubeVideo(url) | UrlType::YouTubeMusicTrack(url) => {
                    spinner.set_message(format!(
                        "{} Đang lấy thông tin bài hát YouTube...",
                        step_str
                    ));
                    if let Ok(item) = yt_client.fetch_video_info(&url) {
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
                    } else {
                        tasks.push(DownloadTask {
                            display_title: trimmed.to_string(),
                            source_or_search: trimmed.to_string(),
                            fallback_searches: Vec::new(),
                            expected_duration_secs: None,
                            metadata: AudioMetadata {
                                title: sanitize_name(trimmed),
                                artists: vec!["YouTube".to_string()],
                                album: "YouTube".to_string(),
                                release_year: None,
                                cover_url: None,
                                track_number: None,
                                total_tracks: None,
                            },
                        });
                    }
                }
                UrlType::SunoTrack(uuid) => {
                    spinner.set_message(format!(
                        "{} Đang lấy thông tin bài hát Suno AI...",
                        step_str
                    ));
                    let meta = suno_client.fetch_track(&uuid).await.unwrap_or_else(|_| {
                        crate::core::suno::SunoTrackMeta {
                            uuid: uuid.clone(),
                            title: format!("Suno AI - {}", &uuid[0..8.min(uuid.len())]),
                            artist: "Suno AI".to_string(),
                            audio_url: format!("https://cdn1.suno.ai/{}.mp3", uuid),
                            fallback_audio_url: format!(
                                "https://audiopipe.suno.ai/?item_id={}",
                                uuid
                            ),
                            cover_url: None,
                        }
                    });

                    println!(
                        "  {} Tìm thấy bài hát Suno: {}",
                        "✔".green().bold(),
                        meta.title.cyan()
                    );

                    tasks.push(DownloadTask {
                        display_title: format!("{} - {}", meta.artist, meta.title),
                        source_or_search: meta.audio_url,
                        fallback_searches: vec![meta.fallback_audio_url],
                        expected_duration_secs: None,
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
                UrlType::SocialVideo { url, platform_name } => {
                    spinner.set_message(format!(
                        "{} Đang lấy thông tin {}...",
                        step_str, platform_name
                    ));
                    if let Ok(item) = yt_client.fetch_media_info(&url, &platform_name) {
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

    pub fn retry_from_file(output_dir: &Path) -> Result<(DownloadSettings, Vec<DownloadTask>)> {
        let path = output_dir.join(FAILED_QUEUE_FILE);
        if !path.exists() {
            return Err(anyhow!(
                "Không tìm thấy hàng đợi bài lỗi tại: {}",
                path.display()
            ));
        }
        let content = std::fs::read_to_string(&path)?;
        let queue: FailedQueue = serde_json::from_str(&content)?;
        Ok((queue.settings, queue.tasks))
    }

    pub async fn execute_batch(&self, tasks: &[DownloadTask]) -> Result<BatchOutcome> {
        if tasks.is_empty() {
            println!("{}", "Không có bài hát nào trong hàng đợi tải!".yellow());
            return Ok(BatchOutcome {
                success_count: 0,
                failed_tasks: Vec::new(),
            });
        }

        let clean_output_dir = ensure_dir(&self.settings.output_dir)?;
        let temp_dir = clean_output_dir.join(".ghita_temp");
        ensure_dir(&temp_dir)?;

        println!(
            "\n{} Chuẩn bị tải {} bài hát (song song tối đa {}) vào thư mục: {}\n",
            "🚀".cyan(),
            tasks.len().to_string().green().bold(),
            self.settings.concurrency.to_string().yellow().bold(),
            clean_output_dir.display().to_string().yellow()
        );

        let concurrency = if self.settings.concurrency == 0 {
            DEFAULT_CONCURRENCY
        } else {
            self.settings.concurrency
        };

        let settings = Arc::new(self.settings.clone());
        let semaphore = Arc::new(Semaphore::new(concurrency));
        let multi = Arc::new(MultiProgress::new());
        let tagger = Arc::new(Tagger::new());
        let ytdlp = self.ytdlp_path.clone();
        let ffmpeg = self.ffmpeg_path.clone();
        let has_node = self.has_node;
        let total = tasks.len();

        let mut set = JoinSet::new();

        for (index, task) in tasks.iter().enumerate() {
            let task = task.clone();
            let settings = settings.clone();
            let semaphore = semaphore.clone();
            let multi = multi.clone();
            let tagger = tagger.clone();
            let ytdlp = ytdlp.clone();
            let ffmpeg = ffmpeg.clone();
            let temp_dir = temp_dir.clone();
            let out_dir = clean_output_dir.clone();

            set.spawn(async move {
                let _permit = match semaphore.acquire_owned().await {
                    Ok(p) => p,
                    Err(_) => {
                        return TaskOutcome {
                            ok: false,
                            logs: vec![format!("    {} Mất khóa điều phối tác vụ", "✖".red())],
                            task,
                        };
                    }
                };

                let handle = multi.add(ProgressBar::new(0));
                handle.set_style(
                    ProgressStyle::default_bar()
                        .template("{wide_bar:.cyan/blue} {bytes:>12}/{total_bytes:>12} {msg}")
                        .unwrap()
                        .progress_chars("=>-"),
                );
                handle.set_message(format!("[{}/{}] chờ tới lượt", index + 1, total));

                let mut logs: Vec<String> = Vec::new();
                let mut targets = vec![task.source_or_search.clone()];
                targets.extend(task.fallback_searches.iter().cloned());

                let mode = match settings.format {
                    AudioFormat::Video => DownloadMode::Video(settings.video_resolution),
                    _ => DownloadMode::Audio,
                };

                let mut downloaded: Option<PathBuf> = None;

                for (attempt_idx, target) in targets.iter().enumerate() {
                    let label = if attempt_idx == 0 {
                        "tải luồng gốc".to_string()
                    } else {
                        format!("dự phòng {}/{}", attempt_idx + 1, targets.len())
                    };
                    handle.set_message(format!(
                        "[{}/{}] {} • {}",
                        index + 1,
                        total,
                        label,
                        task.display_title
                    ));

                    let ytdlp_p = ytdlp.clone();
                    let temp = temp_dir.clone();
                    let tgt = target.clone();
                    let bar = handle.clone();
                    let dl_res = tokio::task::spawn_blocking(move || {
                        let dl = StreamDownloader::new(ytdlp_p.as_path(), has_node);
                        dl.download_stream(&tgt, &temp, mode, Some(&bar))
                    })
                    .await;

                    let path = match dl_res {
                        Ok(Ok(p)) => p,
                        Ok(Err(e)) => {
                            logs.push(format!("    {} {}", "⚠".yellow(), e));
                            continue;
                        }
                        Err(e) => {
                            logs.push(format!("    {} Lỗi luồng tải: {}", "⚠".yellow(), e));
                            continue;
                        }
                    };

                    let ff = ffmpeg.clone();
                    let probe_path = path.clone();
                    let expected = task.expected_duration_secs;
                    let check = tokio::task::spawn_blocking(move || {
                        verify::probe_media(&probe_path, ff.as_path()).and_then(|info| {
                            verify::validate_downloaded(&info, expected, DURATION_TOLERANCE_SECS)
                        })
                    })
                    .await;

                    match check {
                        Ok(Ok(())) => {
                            downloaded = Some(path);
                            break;
                        }
                        Ok(Err(e)) => {
                            let _ = std::fs::remove_file(&path);
                            logs.push(format!(
                                "    {} Tệp không đạt kiểm chứng: {}",
                                "⚠".yellow(),
                                e
                            ));
                        }
                        Err(e) => {
                            let _ = std::fs::remove_file(&path);
                            logs.push(format!("    {} Lỗi kiểm chứng tệp: {}", "⚠".yellow(), e));
                        }
                    }
                }

                let temp_file = match downloaded {
                    Some(f) => f,
                    None => {
                        handle.finish_and_clear();
                        logs.push(format!(
                            "    {} {} thất bại sau {} phương án tải",
                            "✖".red(),
                            task.display_title,
                            targets.len()
                        ));
                        return TaskOutcome {
                            ok: false,
                            logs,
                            task,
                        };
                    }
                };

                let fmt = settings.format;
                let ext = match fmt {
                    AudioFormat::Mp3 => "mp3".to_string(),
                    AudioFormat::Wav => "wav".to_string(),
                    AudioFormat::Flac => "flac".to_string(),
                    AudioFormat::Aac => "m4a".to_string(),
                    AudioFormat::Video => "mp4".to_string(),
                    AudioFormat::Original => temp_file
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|s| s.to_ascii_lowercase())
                        .unwrap_or_else(|| "m4a".to_string()),
                };
                let formatted_title = if let Some(num) = task.metadata.track_number {
                    if task.metadata.total_tracks.unwrap_or(0) > 1 {
                        format!("{:02}. {}", num, task.display_title)
                    } else {
                        task.display_title.clone()
                    }
                } else {
                    task.display_title.clone()
                };
                let final_path = crate::utils::file_helper::ensure_unique_path_with_options(
                    &out_dir,
                    &formatted_title,
                    &ext,
                    settings.keep_accents,
                );

                handle.set_message(format!(
                    "[{}/{}] hoàn thiện • {}",
                    index + 1,
                    total,
                    task.display_title
                ));

                let tr = Transcoder::new(ffmpeg.as_path());
                let src = temp_file.clone();
                let dst = final_path.clone();
                let meta = task.metadata.clone();
                let q = settings.quality;
                let tr_res = tokio::task::spawn_blocking(move || match fmt {
                    AudioFormat::Mp3 | AudioFormat::Wav | AudioFormat::Flac | AudioFormat::Aac => {
                        tr.transcode(
                            &src,
                            &dst,
                            fmt,
                            q.unwrap_or_else(|| fmt.default_quality()),
                            &meta,
                        )
                    }
                    _ => tr.remux_copy(&src, &dst, &meta),
                })
                .await;

                let _ = std::fs::remove_file(&temp_file);

                let mut ok = true;
                match tr_res {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        logs.push(format!("    {} Xử lý tệp thất bại: {}", "✖".red(), e));
                        ok = false;
                    }
                    Err(e) => {
                        logs.push(format!("    {} Lỗi luồng chuyển mã: {}", "✖".red(), e));
                        ok = false;
                    }
                }

                if ok && fmt == AudioFormat::Mp3 {
                    match tagger.tag_mp3(&final_path, &task.metadata).await {
                        Ok(_) => logs.push(format!(
                            "    {} {}",
                            "🏷".magenta(),
                            "Nhúng ID3 & ảnh bìa: Xong".green()
                        )),
                        Err(_) => logs.push(format!("    {}", "Bỏ qua ảnh bìa".yellow())),
                    }
                }

                if ok {
                    logs.push(format!(
                        "    {} Lưu tệp thành công: {}",
                        "✔".green().bold(),
                        final_path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .bright_white()
                    ));
                } else {
                    let _ = std::fs::remove_file(&final_path);
                }

                handle.finish_and_clear();
                TaskOutcome { ok, logs, task }
            });
        }

        let mut success_count = 0usize;
        let mut failed_tasks: Vec<DownloadTask> = Vec::new();

        while let Some(joined) = set.join_next().await {
            match joined {
                Ok(outcome) => {
                    for line in &outcome.logs {
                        let _ = multi.println(line);
                    }
                    if outcome.ok {
                        success_count += 1;
                    } else {
                        failed_tasks.push(outcome.task);
                    }
                }
                Err(e) => {
                    let _ = multi.println(format!("{}", format!("    Lỗi tác vụ: {}", e).red()));
                }
            }
        }

        let _ = std::fs::remove_dir_all(&temp_dir);

        if !failed_tasks.is_empty() {
            let queue = FailedQueue {
                settings: (*settings).clone(),
                tasks: failed_tasks.clone(),
            };
            if let Ok(json) = serde_json::to_string_pretty(&queue) {
                let queue_path = clean_output_dir.join(FAILED_QUEUE_FILE);
                if std::fs::write(&queue_path, json).is_ok() {
                    println!(
                        "{}",
                        format!(
                            "  Đã lưu hàng đợi {} bài lỗi vào: {}",
                            failed_tasks.len(),
                            queue_path.display()
                        )
                        .yellow()
                    );
                }
            }
        } else {
            let _ = std::fs::remove_file(clean_output_dir.join(FAILED_QUEUE_FILE));
        }

        println!("══════════════════════════════════════════════");
        println!(
            "{} Hoàn thành tải hàng loạt! Thành công: {} | Lỗi: {}",
            "🎉".green(),
            success_count.to_string().green().bold(),
            failed_tasks.len().to_string().red()
        );
        println!(
            "Thư mục lưu bài hát: {}",
            clean_output_dir.display().to_string().cyan()
        );
        println!("══════════════════════════════════════════════\n");

        Ok(BatchOutcome {
            success_count,
            failed_tasks,
        })
    }
}
