use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;

use crate::utils::file_helper::{ensure_dir, ensure_unique_path, sanitize_name};

use super::config::{AudioFormat, DownloadSettings};
use super::downloader::StreamDownloader;
use super::platform::{PlatformParser, UrlType};
use super::spotify::{SpotifyClient, SpotifyTrackMeta};
use super::suno::SunoClient;
use super::tagger::{AudioMetadata, Tagger};
use super::transcoder::Transcoder;
use super::youtube::YouTubeClient;

#[derive(Debug, Clone)]
pub struct DownloadTask {
    pub display_title: String,
    pub source_or_search: String,
    pub fallback_searches: Vec<String>,
    pub metadata: AudioMetadata,
}

pub struct BatchProcessor<'a> {
    settings: DownloadSettings,
    ytdlp_path: &'a Path,
    ffmpeg_path: &'a Path,
    has_node: bool,
}

impl<'a> BatchProcessor<'a> {
    pub fn new(
        settings: DownloadSettings,
        ytdlp_path: &'a Path,
        ffmpeg_path: &'a Path,
        has_node: bool,
    ) -> Self {
        Self {
            settings,
            ytdlp_path,
            ffmpeg_path,
            has_node,
        }
    }

    pub async fn resolve_inputs(&self, raw_inputs: &[String]) -> Result<Vec<DownloadTask>> {
        let mut tasks = Vec::new();
        let spotify_client = SpotifyClient::new();
        let suno_client = SunoClient::new();
        let yt_client = YouTubeClient::new(self.ytdlp_path, self.has_node);

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
                    if let Ok((album_name, track_metas)) = spotify_client.fetch_album_tracks(&id).await {
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
                    if let Ok((pl_name, track_metas)) = spotify_client.fetch_playlist_tracks(&id).await {
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
                    spinner.set_message(format!("{} Đang quét danh sách phát YouTube...", step_str));
                    if let Ok(items) = yt_client.extract_playlist_items(&url) {
                        println!(
                            "  {} Tìm thấy Playlist YouTube với {} video/bài hát",
                            "✔".green().bold(),
                            items.len()
                        );
                        for item in items {
                            tasks.push(DownloadTask {
                                display_title: format!("{} - {}", item.uploader, item.title),
                                source_or_search: item.direct_url,
                                fallback_searches: vec![
                                    format!("ytsearch1:{} {}", item.uploader, item.title),
                                ],
                                metadata: AudioMetadata {
                                    title: item.title,
                                    artists: vec![item.uploader],
                                    album: "YouTube Music".to_string(),
                                    release_year: None,
                                    cover_url: item.thumbnail_url,
                                },
                            });
                        }
                    }
                }
                UrlType::YouTubeVideo(url) | UrlType::YouTubeMusicTrack(url) => {
                    spinner.set_message(format!("{} Đang lấy thông tin bài hát YouTube...", step_str));
                    if let Ok(item) = yt_client.fetch_video_info(&url) {
                        tasks.push(DownloadTask {
                            display_title: format!("{} - {}", item.uploader, item.title),
                            source_or_search: item.direct_url,
                            fallback_searches: vec![
                                format!("ytsearch1:{} {}", item.uploader, item.title),
                            ],
                            metadata: AudioMetadata {
                                title: item.title,
                                artists: vec![item.uploader],
                                album: "YouTube".to_string(),
                                release_year: None,
                                cover_url: item.thumbnail_url,
                            },
                        });
                    } else {
                        tasks.push(DownloadTask {
                            display_title: trimmed.to_string(),
                            source_or_search: trimmed.to_string(),
                            fallback_searches: Vec::new(),
                            metadata: AudioMetadata {
                                title: sanitize_name(trimmed),
                                artists: vec!["YouTube".to_string()],
                                album: "YouTube".to_string(),
                                release_year: None,
                                cover_url: None,
                            },
                        });
                    }
                }
                UrlType::SunoTrack(uuid) => {
                    spinner.set_message(format!("{} Đang lấy thông tin bài hát Suno AI...", step_str));
                    let meta = suno_client.fetch_track(&uuid).await.unwrap_or_else(|_| {
                        crate::core::suno::SunoTrackMeta {
                            uuid: uuid.clone(),
                            title: format!("Suno AI - {}", &uuid[0..8.min(uuid.len())]),
                            artist: "Suno AI".to_string(),
                            audio_url: format!("https://cdn1.suno.ai/{}.mp3", uuid),
                            fallback_audio_url: format!("https://audiopipe.suno.ai/?item_id={}", uuid),
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
                        metadata: AudioMetadata {
                            title: meta.title,
                            artists: vec![meta.artist],
                            album: "Suno AI Music".to_string(),
                            release_year: None,
                            cover_url: meta.cover_url,
                        },
                    });
                }
                UrlType::SocialVideo { url, platform_name } => {
                    spinner.set_message(format!("{} Đang lấy thông tin {}...", step_str, platform_name));
                    if let Ok(item) = yt_client.fetch_media_info(&url, &platform_name) {
                        tasks.push(DownloadTask {
                            display_title: format!("{} - {}", item.uploader, item.title),
                            source_or_search: item.direct_url,
                            fallback_searches: Vec::new(),
                            metadata: AudioMetadata {
                                title: item.title,
                                artists: vec![item.uploader],
                                album: platform_name.clone(),
                                release_year: None,
                                cover_url: item.thumbnail_url,
                            },
                        });
                    } else {
                        tasks.push(DownloadTask {
                            display_title: format!("{} - {}", platform_name, sanitize_name(trimmed)),
                            source_or_search: trimmed.to_string(),
                            fallback_searches: Vec::new(),
                            metadata: AudioMetadata {
                                title: format!("{} Audio", platform_name),
                                artists: vec![platform_name.clone()],
                                album: platform_name.clone(),
                                release_year: None,
                                cover_url: None,
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
                        metadata: AudioMetadata {
                            title: query.clone(),
                            artists: vec!["Search".to_string()],
                            album: "Search".to_string(),
                            release_year: None,
                            cover_url: None,
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
            format!("ytsearch1:{} {} audio", primary_artist, meta.title),
            format!("ytsearch1:{} - {}", primary_artist, meta.title),
            format!("ytsearch1:{} {}", primary_artist, meta.title),
            format!("ytsearch3:{} {}", primary_artist, meta.title),
        ];

        DownloadTask {
            display_title,
            source_or_search: search_target,
            fallback_searches,
            metadata: AudioMetadata {
                title: meta.title,
                artists: meta.artists,
                album: meta.album,
                release_year: meta.release_year,
                cover_url: meta.cover_url,
            },
        }
    }

    pub async fn execute_batch(&self, tasks: &[DownloadTask]) -> Result<()> {
        if tasks.is_empty() {
            println!("{}", "Không có bài hát nào trong hàng đợi tải!".yellow());
            return Ok(());
        }

        let clean_output_dir = ensure_dir(&self.settings.output_dir)?;
        let temp_dir = clean_output_dir.join(".ghita_temp");
        ensure_dir(&temp_dir)?;

        let downloader = StreamDownloader::new(self.ytdlp_path, self.has_node);
        let transcoder = Transcoder::new(self.ffmpeg_path);
        let tagger = Tagger::new();

        println!(
            "\n{} Chuẩn bị tải {} bài hát vào thư mục: {}\n",
            "🚀".cyan(),
            tasks.len().to_string().green().bold(),
            clean_output_dir.display().to_string().yellow()
        );

        let mut success_count = 0;
        let mut failed_count = 0;

        for (index, task) in tasks.iter().enumerate() {
            let step_num = index + 1;
            println!(
                "[{}/{}] {} {}",
                step_num,
                tasks.len(),
                "Đang xử lý:".bold(),
                task.display_title.cyan()
            );

            let mut targets_to_try = vec![task.source_or_search.clone()];
            targets_to_try.extend(task.fallback_searches.clone());

            let mut downloaded_path = None;

            for (attempt_idx, target) in targets_to_try.iter().enumerate() {
                if attempt_idx > 0 {
                    print!("    {} Thử tìm kiếm dự phòng ({}/{})... ", "🔄".yellow(), attempt_idx, targets_to_try.len() - 1);
                } else {
                    print!("    {} Tải luồng âm thanh gốc... ", "⬇".blue());
                }

                match downloader.download_stream(target, &temp_dir) {
                    Ok(path) => {
                        println!("{}", "Xong".green());
                        downloaded_path = Some(path);
                        break;
                    }
                    Err(err) => {
                        if attempt_idx + 1 < targets_to_try.len() {
                            println!("{}", "Thử phương án khác...".yellow());
                        } else {
                            println!("{} ({})", "Thất bại".red(), err);
                        }
                    }
                }
            }

            let temp_audio_file = match downloaded_path {
                Some(p) => p,
                None => {
                    failed_count += 1;
                    continue;
                }
            };

            let ext = self.settings.quality.file_extension(self.settings.format);
            let final_file_path = ensure_unique_path(
                &clean_output_dir,
                &task.display_title,
                ext,
            );

            print!(
                "    {} Chuyển mã sang .{} ({:?})... ",
                "⚡".yellow(),
                ext.to_uppercase(),
                self.settings.quality
            );

            let transcode_res = transcoder.transcode(
                &temp_audio_file,
                &final_file_path,
                self.settings.format,
                self.settings.quality,
                &task.metadata,
            );

            let _ = std::fs::remove_file(&temp_audio_file);

            if let Err(err) = transcode_res {
                println!("{} ({})", "Thất bại".red(), err);
                failed_count += 1;
                continue;
            }
            println!("{}", "Xong".green());

            if self.settings.format == AudioFormat::Mp3 {
                print!("    {} Nhúng thẻ thông tin ID3 & ảnh bìa... ", "🏷".magenta());
                if tagger.tag_mp3(&final_file_path, &task.metadata).await.is_ok() {
                    println!("{}", "Xong".green());
                } else {
                    println!("{}", "Bỏ qua ảnh bìa".yellow());
                }
            }

            println!(
                "    {} Lưu tệp thành công: {}\n",
                "✔".green().bold(),
                final_file_path.file_name().unwrap_or_default().to_string_lossy().bright_white()
            );
            success_count += 1;
        }

        let _ = std::fs::remove_dir_all(&temp_dir);

        println!("══════════════════════════════════════════════");
        println!(
            "{} Hoàn thành tải hàng loạt! Thành công: {} | Lỗi: {}",
            "🎉".green(),
            success_count.to_string().green().bold(),
            failed_count.to_string().red()
        );
        println!(
            "Thư mục lưu bài hát: {}",
            clean_output_dir.display().to_string().cyan()
        );
        println!("══════════════════════════════════════════════\n");

        Ok(())
    }
}
