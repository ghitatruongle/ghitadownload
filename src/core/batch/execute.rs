use anyhow::{anyhow, Result};
use colored::Colorize;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::future::Future;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use tokio::sync::Semaphore;
use tokio::task::{JoinError, JoinSet};

use crate::core::cancel::{global, Cancellation};
use crate::core::config::DownloadSettings;
use crate::core::tagger::Tagger;
use crate::utils::file_helper::ensure_dir;

use super::task::{process_task, TaskContext};
use super::{
    clear_pending_state, write_pending_state, BatchOutcome, BatchProcessor, DownloadTask,
    FailedQueue, TaskOutcome, DEFAULT_CONCURRENCY, FAILED_QUEUE_FILE,
};

const PENDING_WRITE_EVERY: usize = 15;

fn is_youtube_source_target(target: &str) -> bool {
    let normalized = target.trim().to_ascii_lowercase();
    normalized.starts_with("ytsearch")
        || normalized.starts_with("https://www.youtube.com/")
        || normalized.starts_with("https://music.youtube.com/")
        || normalized.starts_with("https://youtu.be/")
        || normalized.starts_with("https://m.youtube.com/")
        || normalized.starts_with("https://www.youtu.be/")
}

fn batch_should_warn_youtube_quality(settings: &DownloadSettings, tasks: &[DownloadTask]) -> bool {
    if settings.cookies_file.is_some() || settings.cookies_from_browser.is_some() {
        return false;
    }
    tasks
        .iter()
        .any(|task| is_youtube_source_target(&task.source_or_search))
}

static TOTAL_BAR_STYLE: LazyLock<ProgressStyle> = LazyLock::new(|| {
    ProgressStyle::default_bar()
        .template("{wide_bar:.cyan/blue} {pos}/{len} bài · {eta} · {msg}")
        .unwrap()
        .progress_chars("=>-")
});

impl BatchProcessor {
    pub async fn execute_batch(&self, tasks: &[DownloadTask]) -> Result<BatchOutcome> {
        if tasks.is_empty() {
            println!("{}", "Không có bài hát nào trong hàng đợi tải!".yellow());
            return Ok(BatchOutcome {
                success_count: 0,
                failed_tasks: Vec::new(),
                elapsed_secs: 0.0,
                total_bytes: 0,
            });
        }

        let clean_output_dir = ensure_dir(&self.settings.output_dir)?;
        let _ = crate::utils::file_helper::sweep_stale_temp_dirs(&clean_output_dir);
        let temp_dir = create_execution_temp_dir(&clean_output_dir)?;

        if let Err(error) = write_pending_state(&clean_output_dir, &temp_dir, &self.settings, tasks)
        {
            println!(
                "{}",
                format!("  Không thể lưu trạng thái resume: {}", error).yellow()
            );
        }

        println!(
            "\n{} Chuẩn bị tải {} bài hát (song song tối đa {}) vào thư mục: {}\n",
            "🚀".cyan(),
            tasks.len().to_string().green().bold(),
            self.settings.concurrency.to_string().yellow().bold(),
            clean_output_dir.display().to_string().yellow()
        );

        if batch_should_warn_youtube_quality(&self.settings, tasks) {
            println!(
                "{}",
                "ℹ️  YouTube không dùng cookie: YouTube có thể giới hạn chất lượng audio ~128–160 kbps — dùng --cookies để mở khóa chất lượng cao hơn.".yellow()
            );
        }

        if let Some(free) = crate::utils::fs::drive_free_bytes(&clean_output_dir) {
            if let Some(warning) = crate::utils::fs::free_space_warning(free) {
                println!("{}", warning.yellow());
                crate::utils::logger::log_line(&format!("low disk: {free} bytes free"));
            }
        }

        if let Some(cookies) = &self.settings.cookies_file {
            if cookies.is_file() {
                if let Some(warning) = crate::utils::file_helper::cookie_acl_warning(cookies) {
                    println!("{}", warning.yellow());
                    crate::utils::logger::log_line("cookie acl warning shown");
                }
            }
        }

        let removed_parts = crate::utils::file_helper::remove_stale_part_files(
            &clean_output_dir,
            std::time::Duration::from_secs(crate::utils::file_helper::STALE_PART_MAX_AGE_SECS),
        );
        if removed_parts > 0 {
            println!(
                "{}",
                format!(
                    "  🧹 Đã dọn {removed_parts} file .part cũ hơn 24 giờ (sẽ tải lại từ đầu)."
                )
                .yellow()
            );
        }

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
        let cancel = Cancellation::new();
        let started = std::time::Instant::now();
        let total_bar = multi.add(ProgressBar::new(tasks.len() as u64));
        total_bar.set_style(TOTAL_BAR_STYLE.clone());
        total_bar.set_message("đang tải");

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
            let cancel = cancel.clone();

            set.spawn(async move {
                let _permit = match semaphore.acquire_owned().await {
                    Ok(p) => p,
                    Err(_) => {
                        return Ok(TaskOutcome {
                            ok: false,
                            cancelled: false,
                            logs: vec![format!("    {} Mất khóa điều phối tác vụ", "✖".red())],
                            task,
                            task_index: index,
                            source_note: None,
                            produced_bytes: 0,
                        });
                    }
                };

                if cancel.is_cancelled() || global().is_cancelled() {
                    return Ok(TaskOutcome {
                        ok: false,
                        cancelled: true,
                        logs: vec![format!("    {} {}", "✖".red(), "Đã hủy")],
                        task,
                        task_index: index,
                        source_note: None,
                        produced_bytes: 0,
                    });
                }

                let handle = multi.add(ProgressBar::new(0));
                let board = handle.clone();
                let result = indexed_spawn(
                    index,
                    process_task(
                        task,
                        index,
                        total,
                        handle,
                        TaskContext {
                            settings,
                            ytdlp,
                            ffmpeg,
                            tagger,
                            has_node,
                            temp_dir,
                            out_dir,
                            cancel,
                        },
                    ),
                )
                .await;
                if result.is_err() {
                    board.finish_with_message("✖ tác vụ gặp lỗi nghiêm trọng");
                }
                result
            });
        }

        let mut success_count = 0usize;
        let mut total_bytes: u64 = 0;
        let mut failed_tasks: Vec<(usize, DownloadTask)> = Vec::new();
        let mut source_notes: Vec<(usize, Option<String>)> = Vec::new();
        let mut completed_ok: std::collections::HashSet<usize> = std::collections::HashSet::new();
        let logs_to_terminal = std::io::stdout().is_terminal();

        let watcher_handle = {
            let watcher_cancel = cancel.clone();
            tokio::spawn(async move {
                let mut watch_rx = global().subscribe();
                while let Ok(()) = watch_rx.changed().await {
                    if *watch_rx.borrow() {
                        watcher_cancel.cancel();
                        break;
                    }
                }
            })
        };

        while let Some(joined) = set.join_next().await {
            match joined {
                Ok(Ok(outcome)) => {
                    total_bar.inc(1);
                    for line in &outcome.logs {
                        if logs_to_terminal {
                            let _ = multi.println(line);
                        } else {
                            println!("{line}");
                        }
                    }
                    source_notes.push((outcome.task_index, outcome.source_note));
                    if outcome.ok {
                        success_count += 1;
                        total_bytes += outcome.produced_bytes;
                        completed_ok.insert(outcome.task_index);
                        if completed_ok.len().is_multiple_of(PENDING_WRITE_EVERY) {
                            let remaining: Vec<DownloadTask> = tasks
                                .iter()
                                .enumerate()
                                .filter(|(index, _)| !completed_ok.contains(index))
                                .map(|(_, task)| task.clone())
                                .collect();
                            if let Err(error) = write_pending_state(
                                &clean_output_dir,
                                &temp_dir,
                                &self.settings,
                                &remaining,
                            ) {
                                println!(
                                    "{}",
                                    format!("  Không thể cập nhật trạng thái resume: {}", error)
                                        .yellow()
                                );
                            }
                        }
                    } else if !outcome.cancelled {
                        failed_tasks.push((outcome.task_index, outcome.task));
                    }
                }
                Ok(Err((index, e))) => {
                    total_bar.inc(1);
                    let message = format!("    Lỗi tác vụ {}: {}", index + 1, e);
                    if logs_to_terminal {
                        let _ = multi.println(format!("{}", message.red()));
                    } else {
                        println!("{}", message.red());
                    }
                    if !cancel.is_cancelled() {
                        failed_tasks.push((index, tasks[index].clone()));
                    }
                }
                Err(e) => {
                    total_bar.inc(1);
                    let message = format!("    Lỗi điều phối tác vụ: {}", e);
                    if logs_to_terminal {
                        let _ = multi.println(format!("{}", message.red()));
                    } else {
                        println!("{}", message.red());
                    }
                }
            }
        }
        watcher_handle.abort();

        let cancelled = cancel.is_cancelled();

        failed_tasks.sort_by_key(|(index, _)| *index);
        let failed_tasks: Vec<DownloadTask> =
            failed_tasks.into_iter().map(|(_, task)| task).collect();

        if cancelled {
            let remaining: Vec<DownloadTask> = tasks
                .iter()
                .enumerate()
                .filter(|(index, _)| !completed_ok.contains(index))
                .map(|(_, task)| task.clone())
                .collect();
            let remaining_count = remaining.len();
            if let Err(error) =
                write_pending_state(&clean_output_dir, &temp_dir, &self.settings, &remaining)
            {
                println!(
                    "{}",
                    format!("  Không thể lưu trạng thái resume sau khi dừng: {}", error).yellow()
                );
            }
            println!(
                "{}",
                format!(
                    "  Đã dừng theo yêu cầu (Ctrl+C) — {} bài còn lại đã lưu, dùng --resume để tiếp tục",
                    remaining_count
                )
                .yellow()
            );
        }

        if !cancelled {
            clear_pending_state(&clean_output_dir);
        }

        if !failed_tasks.is_empty() {
            let queue = FailedQueue {
                settings: (*settings).clone(),
                tasks: failed_tasks.clone(),
            };
            let queue_path = clean_output_dir.join(FAILED_QUEUE_FILE);
            let temp_queue_path = temp_dir.join(FAILED_QUEUE_FILE);
            let queue_result = match serde_json::to_vec_pretty(&queue) {
                Ok(json) => std::fs::write(&temp_queue_path, json)
                    .and_then(|_| crate::utils::fs::replace_file(&temp_queue_path, &queue_path)),
                Err(e) => Err(std::io::Error::other(e)),
            };
            match queue_result {
                Ok(()) => println!(
                    "{}",
                    format!(
                        "  Đã lưu hàng đợi {} bài lỗi vào: {}",
                        failed_tasks.len(),
                        queue_path.display()
                    )
                    .yellow()
                ),
                Err(e) => {
                    let _ = std::fs::remove_file(&temp_queue_path);
                    let _ = std::fs::remove_dir_all(&temp_dir);
                    println!(
                        "{}",
                        format!("  Không thể lưu hàng đợi bài lỗi: {}", e).red()
                    );
                    return Err(anyhow!("Không thể lưu hàng đợi bài lỗi: {}", e));
                }
            }
        } else {
            let _ = std::fs::remove_file(clean_output_dir.join(FAILED_QUEUE_FILE));
        }
        let _ = std::fs::remove_dir_all(&temp_dir);

        let manifest_result = crate::core::manifest::write_manifest(
            &clean_output_dir,
            tasks,
            &failed_tasks,
            &settings,
            &source_notes,
        );
        match manifest_result {
            Ok(path) => println!(
                "  {} {}",
                "Đã ghi manifest:".cyan(),
                path.display().to_string().yellow()
            ),
            Err(e) => println!("{}", format!("  Không thể ghi manifest: {}", e).red()),
        }

        total_bar.finish_with_message(if cancelled {
            "đã dừng"
        } else {
            "hoàn tất"
        });

        Ok(BatchOutcome {
            success_count,
            failed_tasks,
            elapsed_secs: started.elapsed().as_secs_f64(),
            total_bytes,
        })
    }
}

pub(crate) async fn indexed_spawn<F>(
    index: usize,
    future: F,
) -> std::result::Result<F::Output, (usize, JoinError)>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    match tokio::spawn(future).await {
        Ok(output) => Ok(output),
        Err(e) => Err((index, e)),
    }
}

fn create_execution_temp_dir(output_dir: &Path) -> Result<PathBuf> {
    let base = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for attempt in 0..100u32 {
        let path = output_dir.join(format!(".ghita_temp_{}_{}", base, attempt));
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(anyhow!("Không thể tạo thư mục tạm: {}", e)),
        }
    }
    Err(anyhow!("Không thể tạo thư mục tạm duy nhất"))
}

#[cfg(test)]
mod tests {
    use super::indexed_spawn;
    use super::TOTAL_BAR_STYLE;
    use crate::core::batch::DownloadTask;

    #[test]
    fn total_bar_style_template_parses_and_clones() {
        let style = TOTAL_BAR_STYLE.clone();
        let _ = style.get_tick_str(0);
    }

    fn notice_test_task(source: &str) -> DownloadTask {
        DownloadTask {
            display_title: source.to_string(),
            source_or_search: source.to_string(),
            fallback_searches: Vec::new(),
            expected_duration_secs: None,
            metadata: crate::core::tagger::AudioMetadata {
                title: "t".to_string(),
                artists: Vec::new(),
                album: String::new(),
                release_year: None,
                cover_url: None,
                track_number: None,
                total_tracks: None,
                lyrics: None,
            },
        }
    }

    fn notice_test_settings(
        cookies_file: Option<std::path::PathBuf>,
        cookies_from_browser: Option<&str>,
    ) -> crate::core::config::DownloadSettings {
        crate::core::config::DownloadSettings {
            output_dir: std::env::temp_dir(),
            format: crate::core::config::AudioFormat::Mp3,
            quality: None,
            video_resolution: None,
            concurrency: 3,
            keep_accents: false,
            cookies_file,
            cookies_from_browser: cookies_from_browser.map(str::to_string),
            raw: false,
            fetch_lyrics: false,
            duplicate_mode: "rename".to_string(),
        }
    }

    #[test]
    fn is_youtube_source_target_matches_youtube_searches_and_links() {
        use super::is_youtube_source_target;

        assert!(is_youtube_source_target("ytsearch1:bai hat"));
        assert!(is_youtube_source_target(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
        ));
        assert!(is_youtube_source_target(
            "https://music.youtube.com/playlist?list=PL123"
        ));
        assert!(is_youtube_source_target("https://youtu.be/dQw4w9WgXcQ"));
        assert!(is_youtube_source_target(
            "https://m.youtube.com/watch?v=dQw4w9WgXcQ"
        ));
        assert!(!is_youtube_source_target(
            "https://soundcloud.com/forss/sets/soulhack"
        ));
        assert!(!is_youtube_source_target(
            "https://www.tiktok.com/@user/video/1"
        ));
        assert!(!is_youtube_source_target(
            "suno:3b494117-f372-450e-9c5c-2e48b38e5241"
        ));
        assert!(!is_youtube_source_target(""));
    }

    #[test]
    fn youtube_quality_notice_only_without_cookies_and_only_for_youtube_sources() {
        use super::batch_should_warn_youtube_quality;

        let settings = notice_test_settings(None, None);
        let youtube_tasks = vec![
            notice_test_task("https://soundcloud.com/a/b"),
            notice_test_task("ytsearch1:co nhac"),
        ];
        assert!(batch_should_warn_youtube_quality(&settings, &youtube_tasks));

        let music_tasks = vec![notice_test_task(
            "https://music.youtube.com/watch?v=dQw4w9WgXcQ",
        )];
        assert!(batch_should_warn_youtube_quality(&settings, &music_tasks));

        let other_tasks = vec![
            notice_test_task("https://www.tiktok.com/@user/video/1"),
            notice_test_task("suno:3b494117-f372-450e-9c5c-2e48b38e5241"),
        ];
        assert!(!batch_should_warn_youtube_quality(&settings, &other_tasks));

        let file_cookies =
            notice_test_settings(Some(std::path::PathBuf::from("cookies.txt")), None);
        assert!(!batch_should_warn_youtube_quality(
            &file_cookies,
            &youtube_tasks
        ));

        let browser_cookies = notice_test_settings(None, Some("chrome"));
        assert!(!batch_should_warn_youtube_quality(
            &browser_cookies,
            &youtube_tasks
        ));
    }

    #[tokio::test]
    async fn indexed_spawn_returns_output_on_success() {
        let result = indexed_spawn(3, async move { 42u32 }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn indexed_spawn_maps_panic_to_task_index() {
        let result = indexed_spawn(2, async move { panic!("boom") }).await;
        let (index, error) = result.unwrap_err();
        assert_eq!(index, 2);
        assert!(error.is_panic());
    }

    #[tokio::test]
    async fn indexed_spawn_preserves_distinct_indices() {
        let mut set = tokio::task::JoinSet::new();
        for index in 0..3usize {
            set.spawn(async move {
                if index == 1 {
                    indexed_spawn(index, async move { panic!("fail") }).await
                } else {
                    indexed_spawn(index, async move { index }).await
                }
            });
        }
        let mut panicked = Vec::new();
        let mut succeeded = Vec::new();
        while let Some(joined) = set.join_next().await {
            match joined.unwrap() {
                Ok(value) => succeeded.push(value),
                Err((index, error)) => {
                    assert!(error.is_panic());
                    panicked.push(index);
                }
            }
        }
        succeeded.sort_unstable();
        assert_eq!(succeeded, vec![0, 2]);
        assert_eq!(panicked, vec![1]);
    }
}
