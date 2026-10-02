use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::core::config::DownloadSettings;
use crate::core::tagger::AudioMetadata;

mod execute;
mod resolve;
mod task;

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
    task_index: usize,
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
}
