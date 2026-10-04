use anyhow::{Context, Result};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::core::batch::DownloadTask;
use crate::core::config::DownloadSettings;
use crate::utils::file_helper::ensure_unique_path_with_options;

pub fn write_manifest(
    output_dir: &Path,
    tasks: &[DownloadTask],
    failed: &[DownloadTask],
    settings: &DownloadSettings,
) -> Result<PathBuf> {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let (year, month, day, hour, minute, second) = utc_parts(secs);
    let stamp = format!(
        "{:04}{:02}{:02}_{:02}{:02}{:02}",
        year, month, day, hour, minute, second
    );

    let generated_at = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, minute, second
    );

    let results: Vec<serde_json::Value> = tasks
        .iter()
        .enumerate()
        .map(|(index, task)| {
            let failed_hit = failed.iter().any(|f| {
                f.display_title == task.display_title && f.source_or_search == task.source_or_search
            });
            json!({
                "url": task.source_or_search,
                "status": if failed_hit { "failed" } else { "ok" },
                "task_index": index,
            })
        })
        .collect();

    let success_count = results.iter().filter(|r| r["status"] == "ok").count();
    let failed_count = results.len() - success_count;

    let doc = json!({
        "version": "0.0.4",
        "generated_at": generated_at,
        "settings": {
            "format": settings.format.to_string(),
            "quality": settings.quality.as_ref().map(|q| q.to_string()),
            "output_dir": settings.output_dir.to_string_lossy(),
            "concurrency": settings.concurrency,
        },
        "success_count": success_count,
        "failed_count": failed_count,
        "results": results,
    });

    let path = ensure_unique_path_with_options(
        output_dir,
        &format!("ghita_manifest_{}", stamp),
        "json",
        false,
    )?;

    let pretty = serde_json::to_vec_pretty(&doc)?;
    std::fs::write(&path, pretty)
        .with_context(|| format!("Không thể ghi manifest: {}", path.display()))?;
    Ok(path)
}

fn utc_parts(secs: u64) -> (i64, u64, u64, u64, u64, u64) {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let hour = rem / 3_600;
    let minute = (rem % 3_600) / 60;
    let second = rem % 60;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };

    (year, month, day, hour, minute, second)
}
