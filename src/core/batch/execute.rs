use anyhow::{anyhow, Result};
use colored::Colorize;
use indicatif::{MultiProgress, ProgressBar};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

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
                            task_index: index,
                        };
                    }
                };

                let handle = multi.add(ProgressBar::new(0));
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
                    },
                )
                .await
            });
        }

        let mut success_count = 0usize;
        let mut failed_tasks: Vec<DownloadTask> = Vec::new();
        let mut task_results: Vec<Option<Result<(), ()>>> = vec![None; total];
        let logs_to_terminal = std::io::stdout().is_terminal();

        while let Some(joined) = set.join_next().await {
            match joined {
                Ok(outcome) => {
                    for line in &outcome.logs {
                        if logs_to_terminal {
                            let _ = multi.println(line);
                        } else {
                            println!("{line}");
                        }
                    }
                    task_results[outcome.task_index] = Some(Err(()));
                    if outcome.ok {
                        success_count += 1;
                        task_results[outcome.task_index] = Some(Ok(()));
                    } else {
                        failed_tasks.push(outcome.task);
                    }
                }
                Err(e) => {
                    let message = format!("    Lỗi tác vụ: {}", e);
                    if logs_to_terminal {
                        let _ = multi.println(format!("{}", message.red()));
                    } else {
                        println!("{}", message.red());
                    }
                    let missing = task_results.iter().position(|result| result.is_none());
                    if let Some(index) = missing {
                        task_results[index] = Some(Err(()));
                        failed_tasks.push(tasks[index].clone());
                    }
                }
            }
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
                    .and_then(|_| replace_file(&temp_queue_path, &queue_path)),
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

fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    match std::fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(_) if destination.exists() => {
            let backup = destination.with_extension(format!(
                "json.{}.{}.bak",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ));
            std::fs::rename(destination, &backup)?;
            match std::fs::rename(source, destination) {
                Ok(()) => {
                    let _ = std::fs::remove_file(backup);
                    Ok(())
                }
                Err(error) => {
                    let _ = std::fs::rename(&backup, destination);
                    Err(error)
                }
            }
        }
        Err(error) => Err(error),
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
