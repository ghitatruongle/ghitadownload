use anyhow::{anyhow, Result};
use colored::Colorize;
use indicatif::{MultiProgress, ProgressBar};
use std::future::Future;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::{JoinError, JoinSet};

use crate::core::cancel::{global, Cancellation};
use crate::core::tagger::Tagger;
use crate::utils::file_helper::ensure_dir;

use super::task::{process_task, TaskContext};
use super::{
    BatchOutcome, BatchProcessor, DownloadTask, FailedQueue, TaskOutcome, DEFAULT_CONCURRENCY,
    FAILED_QUEUE_FILE,
};

impl BatchProcessor {
    pub async fn execute_batch(&self, tasks: &[DownloadTask]) -> Result<BatchOutcome> {
        if tasks.is_empty() {
            println!("{}", "Không có bài hát nào trong hàng đợi tải!".yellow());
            return Ok(BatchOutcome {
                success_count: 0,
                failed_tasks: Vec::new(),
            });
        }

        let clean_output_dir = ensure_dir(&self.settings.output_dir)?;
        let _ = crate::utils::file_helper::sweep_stale_temp_dirs(&clean_output_dir);
        let temp_dir = create_execution_temp_dir(&clean_output_dir)?;

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
        let cancel = Cancellation::new();
        let global_cancel = global();
        let mut global_rx = global_cancel.subscribe();

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
                            logs: vec![format!("    {} Mất khóa điều phối tác vụ", "✖".red())],
                            task,
                            task_index: index,
                        });
                    }
                };

                if cancel.is_cancelled() || global().is_cancelled() {
                    return Ok(TaskOutcome {
                        ok: false,
                        logs: vec![format!("    {} {}", "✖".red(), "Đã hủy")],
                        task,
                        task_index: index,
                    });
                }

                let handle = multi.add(ProgressBar::new(0));
                indexed_spawn(
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
                .await
            });
        }

        let mut success_count = 0usize;
        let mut failed_tasks: Vec<(usize, DownloadTask)> = Vec::new();
        let logs_to_terminal = std::io::stdout().is_terminal();

        loop {
            tokio::select! {
                joined = set.join_next() => {
                    let Some(joined) = joined else { break };
                    match joined {
                        Ok(Ok(outcome)) => {
                            for line in &outcome.logs {
                                if logs_to_terminal {
                                    let _ = multi.println(line);
                                } else {
                                    println!("{line}");
                                }
                            }
                            if outcome.ok {
                                success_count += 1;
                            } else {
                                failed_tasks.push((outcome.task_index, outcome.task));
                            }
                        }
                        Ok(Err((index, e))) => {
                            let message = format!("    Lỗi tác vụ {}: {}", index + 1, e);
                            if logs_to_terminal {
                                let _ = multi.println(format!("{}", message.red()));
                            } else {
                                println!("{}", message.red());
                            }
                            failed_tasks.push((index, tasks[index].clone()));
                        }
                        Err(e) => {
                            let message = format!("    Lỗi điều phối tác vụ: {}", e);
                            if logs_to_terminal {
                                let _ = multi.println(format!("{}", message.red()));
                            } else {
                                println!("{}", message.red());
                            }
                        }
                    }
                }
                changed = global_rx.changed() => {
                    if changed.is_ok() && *global_rx.borrow() {
                        cancel.cancel();
                    }
                }
            }
        }

        failed_tasks.sort_by_key(|(index, _)| *index);
        let failed_tasks: Vec<DownloadTask> =
            failed_tasks.into_iter().map(|(_, task)| task).collect();

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
        );
        match manifest_result {
            Ok(path) => println!(
                "  {} {}",
                "Đã ghi manifest:".cyan(),
                path.display().to_string().yellow()
            ),
            Err(e) => println!("{}", format!("  Không thể ghi manifest: {}", e).red()),
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
