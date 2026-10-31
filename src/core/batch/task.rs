use anyhow::anyhow;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use crate::core::config::{AudioFormat, DownloadSettings};
use crate::core::downloader::{DownloadMode, StreamDownloader};
use crate::core::matching::{self, SearchCandidate};
use crate::core::suno::SunoClient;
use crate::core::tagger::Tagger;
use crate::core::transcoder::Transcoder;
use crate::core::verify;
use crate::core::youtube::YouTubeClient;
use crate::utils::file_helper::ensure_unique_path_with_options;

use super::{DownloadTask, TaskOutcome, DURATION_TOLERANCE_SECS};

static SUNO_UUID_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}").unwrap()
});

pub(super) struct TaskContext {
    pub(super) settings: Arc<DownloadSettings>,
    pub(super) ytdlp: Arc<PathBuf>,
    pub(super) ffmpeg: Arc<PathBuf>,
    pub(super) tagger: Arc<Tagger>,
    pub(super) has_node: bool,
    pub(super) temp_dir: PathBuf,
    pub(super) out_dir: PathBuf,
    pub(super) cancel: crate::core::cancel::Cancellation,
}

pub(super) async fn process_task(
    task: DownloadTask,
    index: usize,
    total: usize,
    handle: ProgressBar,
    ctx: TaskContext,
) -> TaskOutcome {
    let mut task = task;
    let TaskContext {
        settings,
        ytdlp,
        ffmpeg,
        tagger,
        has_node,
        temp_dir,
        out_dir,
        cancel,
    } = ctx;

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
    let mut source_note: Option<String> = None;
    let requires_audio_stream = matches!(
        settings.format,
        AudioFormat::Mp3
            | AudioFormat::Wav
            | AudioFormat::Flac
            | AudioFormat::Aac
            | AudioFormat::Opus
    );
    let lossless_target = match settings.format {
        AudioFormat::Flac => Some("FLAC"),
        AudioFormat::Wav => Some("WAV"),
        _ => None,
    };

    for (attempt_idx, target) in targets.iter().enumerate() {
        if cancel.is_cancelled() {
            handle.finish_and_clear();
            logs.push(format!("    {} {}", "✖".red(), "Đã hủy tải"));
            return TaskOutcome {
                ok: false,
                logs,
                task,
                task_index: index,
                source_note: None,
            };
        }
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

        let path = if let Some(uuid) = target.strip_prefix("suno:") {
            let suno_temp_path = temp_dir.join(format!(
                "suno_{}_{}_{}.m4a",
                std::process::id(),
                index,
                attempt_idx
            ));
            let uuid_str = uuid.to_string();
            let bar_clone = handle.clone();
            let s_client = SunoClient::new();
            let p_clone = suno_temp_path.clone();
            match s_client
                .download_track(&uuid_str, &p_clone, Some(&bar_clone))
                .await
            {
                Ok(()) => p_clone,
                Err(e) => {
                    logs.push(format!(
                        "    {} {}",
                        "✖".red(),
                        e.to_string().replace(['\r', '\n'], " ").trim()
                    ));
                    continue;
                }
            }
        } else {
            let ytdlp_p = ytdlp.clone();
            let temp = temp_dir.clone();
            let mut tgt = target.clone();
            if tgt.starts_with("ytsearch") {
                if let Some((picked_url, picked_title)) = rank_search_target(
                    ytdlp_p.as_path(),
                    has_node,
                    settings.cookies_file.as_ref(),
                    settings.cookies_from_browser.as_deref(),
                    &tgt,
                    &task,
                )
                .await
                {
                    logs.push(format!(
                        "    {} Khớp bài: chọn kết quả tốt nhất: {}",
                        "🎯".cyan(),
                        picked_title
                    ));
                    tgt = picked_url;
                }
            }
            let bar = handle.clone();
            let dl = StreamDownloader::new(
                ytdlp_p.as_path(),
                has_node,
                settings.cookies_file.clone(),
                settings.cookies_from_browser.clone(),
            );
            let dl_res = dl
                .download_stream(&tgt, &temp, mode, Some(&bar), cancel.subscribe())
                .await;

            match dl_res {
                Ok(p) => p,
                Err(e) => {
                    logs.push(format!(
                        "    {} {}",
                        "✖".red(),
                        e.to_string().replace(['\r', '\n'], " ").trim()
                    ));
                    if cancel.is_cancelled() {
                        handle.finish_and_clear();
                        logs.push(format!("    {} {}", "✖".red(), "Đã hủy tải"));
                        return TaskOutcome {
                            ok: false,
                            logs,
                            task,
                            task_index: index,
                            source_note: None,
                        };
                    }
                    continue;
                }
            }
        };

        if cancel.is_cancelled() {
            handle.finish_and_clear();
            logs.push(format!("    {} {}", "✖".red(), "Đã hủy tải"));
            let _ = std::fs::remove_file(&path);
            return TaskOutcome {
                ok: false,
                logs,
                task,
                task_index: index,
                source_note: None,
            };
        }

        let ff = ffmpeg.clone();
        let probe_path = path.clone();
        let expected = task.expected_duration_secs;
        let sig_path = probe_path.clone();
        let missing_signature =
            tokio::task::spawn_blocking(move || verify::has_media_signature(&sig_path).is_err())
                .await
                .unwrap_or(false);

        if missing_signature {
            let filename = probe_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            let suno_re = &*SUNO_UUID_RE;
            if let Some(caps) = suno_re.captures(filename) {
                let uuid = caps[0].to_string();
                if let Ok(mut bytes) = tokio::fs::read(&probe_path).await {
                    let s_client = SunoClient::new();
                    if s_client.decrypt_clip_media(&uuid, &mut bytes).await.is_ok() {
                        let _ = tokio::fs::write(&probe_path, &bytes).await;
                    }
                }
            }
        }

        let mut probed_codec: Option<String> = None;
        let mut probed_bitrate: Option<u64> = None;
        let check = match verify::probe_media(&path, ff.as_path(), cancel.subscribe()).await {
            Ok(info) => {
                probed_codec = info.audio_codec.clone();
                probed_bitrate = info.audio_bitrate_kbps;
                verify::validate_downloaded(&info, expected, DURATION_TOLERANCE_SECS).and_then(
                    |_| {
                        if !info.has_audio && requires_audio_stream {
                            return Err(anyhow!(
                                "Nguồn không chứa luồng âm thanh (video im lặng hoặc audio bị khóa), không thể xuất tệp âm thanh"
                            ));
                        }
                        Ok(())
                    },
                )
            }
            Err(e) => Err(e),
        };

        match check {
            Ok(()) => {
                if let Some(target_label) = lossless_target {
                    let note = verify::lossy_transcode_note(
                        target_label,
                        probed_codec.as_deref(),
                        probed_bitrate,
                    );
                    if let Some(text) = note {
                        source_note = Some(text);
                        logs.push(format!(
                            "    {} {}",
                            "ℹ".yellow(),
                            source_note.clone().unwrap_or_default().yellow()
                        ));
                    }
                }
                downloaded = Some(path);
                break;
            }
            Err(e) => {
                let _ = std::fs::remove_file(&path);
                logs.push(format!(
                    "    {} Tệp không đạt kiểm chứng: {}",
                    "✖".red(),
                    e.to_string().replace(['\r', '\n'], " ").trim()
                ));
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
                task_index: index,
                source_note: None,
            };
        }
    };

    let fmt = settings.format;
    let ext = match fmt {
        AudioFormat::Mp3 => "mp3".to_string(),
        AudioFormat::Wav => "wav".to_string(),
        AudioFormat::Flac => "flac".to_string(),
        AudioFormat::Aac => "m4a".to_string(),
        AudioFormat::Opus => "opus".to_string(),
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
    let final_path = match ensure_unique_path_with_options(
        &out_dir,
        &formatted_title,
        &ext,
        settings.keep_accents,
    ) {
        Ok(path) => path,
        Err(e) => {
            handle.finish_and_clear();
            logs.push(format!(
                "    {} {}",
                "✖".red(),
                e.to_string().replace(['\r', '\n'], " ").trim()
            ));
            return TaskOutcome {
                ok: false,
                logs,
                task,
                task_index: index,
                source_note: None,
            };
        }
    };

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
    let tr_res = match fmt {
        AudioFormat::Mp3
        | AudioFormat::Wav
        | AudioFormat::Flac
        | AudioFormat::Aac
        | AudioFormat::Opus => {
            tr.transcode(
                &src,
                &dst,
                fmt,
                q.unwrap_or_else(|| fmt.default_quality()),
                &meta,
                cancel.subscribe(),
            )
            .await
        }
        _ => tr.remux_copy(&src, &dst, &meta, cancel.subscribe()).await,
    };
    let _ = std::fs::remove_file(&temp_file);

    let mut ok = true;
    match tr_res {
        Ok(()) => {}
        Err(e) => {
            logs.push(format!("    {} Xử lý tệp thất bại: {}", "✖".red(), e));
            ok = false;
        }
    }

    let mut lyrics_synced: Option<String> = None;
    if ok
        && settings.fetch_lyrics
        && matches!(
            fmt,
            AudioFormat::Mp3 | AudioFormat::Flac | AudioFormat::Aac | AudioFormat::Opus
        )
    {
        match crate::core::lyrics::fetch_lyrics(&task.metadata, task.expected_duration_secs).await {
            Ok(Some(lyrics)) => {
                if lyrics.plain.is_some() || lyrics.synced.is_some() {
                    logs.push(format!(
                        "    {} {}",
                        "🎵".magenta(),
                        "Đã tải lời bài hát".green()
                    ));
                }
                if lyrics.plain.is_some() {
                    task.metadata.lyrics = lyrics.plain;
                }
                lyrics_synced = lyrics.synced;
            }
            Ok(None) => logs.push(format!(
                "    {} {}",
                "ℹ".yellow(),
                "Không tìm thấy lời bài hát trên LRCLIB"
            )),
            Err(e) => logs.push(format!(
                "    {} Lấy lời bài hát bỏ qua: {}",
                "ℹ".yellow(),
                e.to_string().replace(['\r', '\n'], " ").trim()
            )),
        }
    }

    if ok && fmt == AudioFormat::Mp3 {
        match tagger.tag_mp3(&final_path, &task.metadata).await {
            Ok(_) => logs.push(format!(
                "    {} {}",
                "🏷".magenta(),
                "Nhúng ID3 & ảnh bìa: Xong".green()
            )),
            Err(e) => {
                logs.push(format!("    {} Ghi metadata thất bại: {}", "✖".red(), e));
                ok = false;
            }
        }
    }

    if ok && matches!(fmt, AudioFormat::Flac | AudioFormat::Aac) {
        match tagger.fetch_cover_file(&task.metadata, &temp_dir).await {
            Ok(Some(cover)) => {
                let embed_input = final_path.clone();
                let embed_cover = cover.clone();
                let embed_ffmpeg = ffmpeg.clone();
                let embed_res = tokio::task::spawn_blocking(move || {
                    crate::core::transcoder::embed_cover_ffmpeg(
                        &embed_input,
                        &embed_cover,
                        &embed_ffmpeg,
                    )
                })
                .await
                .unwrap_or_else(|error| Err(anyhow!("Tác vụ nhúng ảnh bìa bị ngắt: {error}")));
                match embed_res {
                    Ok(muxed) => match crate::utils::fs::replace_file(&muxed, &final_path) {
                        Ok(()) => logs.push(format!(
                            "    {} {}",
                            "🖼".magenta(),
                            "Nhúng ảnh bìa: Xong".green()
                        )),
                        Err(e) => logs.push(format!(
                            "    {} Không thay thế tệp sau nhúng ảnh bìa: {}",
                            "✖".red(),
                            e
                        )),
                    },
                    Err(e) => logs.push(format!("    {} Nhúng ảnh bìa thất bại: {}", "✖".red(), e)),
                }
                let _ = std::fs::remove_file(&cover);
            }
            Ok(None) => {}
            Err(e) => logs.push(format!("    {} Tải ảnh bìa thất bại: {}", "✖".red(), e)),
        }
    }

    if ok
        && task.metadata.lyrics.is_some()
        && matches!(
            fmt,
            AudioFormat::Flac | AudioFormat::Aac | AudioFormat::Opus
        )
    {
        let embed_input = final_path.clone();
        let embed_lyrics = task.metadata.lyrics.clone().unwrap_or_default();
        let embed_ffmpeg = ffmpeg.clone();
        let embed_res = tokio::task::spawn_blocking(move || {
            crate::core::transcoder::set_lyrics_ffmpeg(&embed_input, &embed_lyrics, &embed_ffmpeg)
        })
        .await
        .unwrap_or_else(|error| Err(anyhow!("Tác vụ nhúng lời bài hát bị ngắt: {error}")));
        match embed_res {
            Ok(muxed) => match crate::utils::fs::replace_file(&muxed, &final_path) {
                Ok(()) => logs.push(format!(
                    "    {} {}",
                    "🎵".magenta(),
                    "Nhúng lời bài hát: Xong".green()
                )),
                Err(e) => logs.push(format!(
                    "    {} Không thay thế tệp sau nhúng lời bài hát: {}",
                    "✖".red(),
                    e
                )),
            },
            Err(e) => logs.push(format!(
                "    {} Nhúng lời bài hát thất bại: {}",
                "✖".red(),
                e
            )),
        }
    }

    if ok {
        if let Some(ref synced) = lyrics_synced {
            match crate::core::lyrics::write_synced_lrc(&final_path, synced) {
                Ok(path) => logs.push(format!(
                    "    {} Lưu lời đồng bộ: {}",
                    "🎵".magenta(),
                    path.display().to_string().bright_white()
                )),
                Err(e) => logs.push(format!("    {} Ghi tệp .lrc thất bại: {}", "✖".red(), e)),
            }
        }
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
    TaskOutcome {
        ok,
        logs,
        task,
        task_index: index,
        source_note,
    }
}

async fn rank_search_target(
    ytdlp: &Path,
    has_node: bool,
    cookies_file: Option<&PathBuf>,
    cookies_from_browser: Option<&str>,
    target: &str,
    task: &DownloadTask,
) -> Option<(String, String)> {
    let query = target.split_once(':')?.1.trim().to_string();
    if query.is_empty() {
        return None;
    }
    let search = format!("ytsearch8:{query}");
    let ytdlp_owned = ytdlp.to_path_buf();
    let cookies_owned = cookies_file.cloned();
    let browser_owned = cookies_from_browser.map(str::to_string);
    let items = tokio::task::spawn_blocking(move || {
        let client = YouTubeClient::new(
            &ytdlp_owned,
            has_node,
            cookies_owned.as_ref(),
            browser_owned.as_deref(),
        );
        client.extract_playlist_items(&search)
    })
    .await
    .ok()?
    .ok()?;
    let candidates: Vec<SearchCandidate> = items
        .into_iter()
        .map(|item| SearchCandidate {
            url: item.direct_url,
            title: item.title,
            uploader: item.uploader,
            duration_secs: item.duration_secs,
        })
        .collect();
    let best = matching::pick_best(
        &candidates,
        &task.metadata.title,
        &task.metadata.artists,
        task.expected_duration_secs,
    )?;
    Some((best.url.clone(), best.title.clone()))
}
