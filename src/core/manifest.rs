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
    source_notes: &[(usize, Option<String>)],
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

    let failed_keys: std::collections::HashSet<(&str, &str)> = failed
        .iter()
        .map(|f| (f.display_title.as_str(), f.source_or_search.as_str()))
        .collect();
    let note_map: std::collections::HashMap<usize, Option<String>> =
        source_notes.iter().cloned().collect();

    let results: Vec<serde_json::Value> = tasks
        .iter()
        .enumerate()
        .map(|(index, task)| {
            let failed_hit = failed_keys
                .contains(&(task.display_title.as_str(), task.source_or_search.as_str()));
            let note = note_map.get(&index).cloned().unwrap_or(None);
            let mut entry = json!({
                "url": task.source_or_search,
                "status": if failed_hit { "failed" } else { "ok" },
                "task_index": index,
            });
            if let Some(note) = note {
                entry["source_note"] = json!(note);
            }
            entry
        })
        .collect();

    let success_count = results.iter().filter(|r| r["status"] == "ok").count();
    let failed_count = results.len() - success_count;

    let doc = json!({
        "version": env!("CARGO_PKG_VERSION"),
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

#[derive(Debug, Clone)]
pub struct RecentManifest {
    pub timestamp_label: String,
    pub success_count: usize,
    pub failed_count: usize,
    pub format: String,
    pub output_dir: String,
    pub path: String,
}

#[derive(Default, serde::Deserialize)]
struct ManifestSnapshot {
    #[serde(default)]
    generated_at: String,
    #[serde(default)]
    success_count: usize,
    #[serde(default)]
    failed_count: usize,
    #[serde(default)]
    settings: ManifestSnapshotSettings,
}

#[derive(Default, serde::Deserialize)]
struct ManifestSnapshotSettings {
    #[serde(default)]
    format: String,
    #[serde(default)]
    output_dir: String,
}

pub fn read_recent_manifests(output_dir: &Path, limit: usize) -> Vec<RecentManifest> {
    let mut candidates: Vec<(String, PathBuf)> = std::fs::read_dir(output_dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return None;
            }
            let name = path.file_name()?.to_string_lossy().to_string();
            let stem = name.strip_suffix(".json")?.to_string();
            if stem.starts_with("ghita_manifest_") {
                Some((stem, path))
            } else {
                None
            }
        })
        .collect();
    candidates.sort_by(|a, b| b.0.cmp(&a.0));

    candidates
        .into_iter()
        .filter_map(|(stem, path)| {
            let content = std::fs::read_to_string(&path).ok()?;
            let snapshot: ManifestSnapshot = serde_json::from_str(&content).ok()?;
            Some(RecentManifest {
                timestamp_label: format_timestamp_label(&stem, &snapshot.generated_at),
                success_count: snapshot.success_count,
                failed_count: snapshot.failed_count,
                format: snapshot.settings.format,
                output_dir: snapshot.settings.output_dir,
                path: path.to_string_lossy().to_string(),
            })
        })
        .take(limit)
        .collect()
}

fn format_timestamp_label(stem: &str, generated_at: &str) -> String {
    if let Some(stamp) = stem.strip_prefix("ghita_manifest_") {
        let compact: String = stamp.chars().filter(|c| c.is_ascii_digit()).collect();
        if compact.len() >= 14 {
            return format!(
                "{}-{}-{} {}:{}:{}",
                &compact[0..4],
                &compact[4..6],
                &compact[6..8],
                &compact[8..10],
                &compact[10..12],
                &compact[12..14]
            );
        }
    }
    generated_at
        .trim_end_matches('Z')
        .replace('T', " ")
        .trim()
        .to_string()
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

#[cfg(test)]
mod tests {
    use super::read_recent_manifests;
    use std::path::{Path, PathBuf};

    fn temp_test_dir(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "ghita_manifest_test_{}_{}_{}",
            label,
            std::process::id(),
            nanos
        ))
    }

    fn write_manifest_file(dir: &Path, stamp: &str, success: usize, failed: usize) {
        std::fs::create_dir_all(dir).unwrap();
        let doc = serde_json::json!({
            "version": "test",
            "generated_at": format!("{}T00:00:00Z", &stamp.replace('_', "-")),
            "settings": {
                "format": "MP3 (.mp3) - Tương thích mọi thiết bị, dung lượng nhỏ gọn",
                "quality": null,
                "output_dir": dir.display().to_string(),
                "concurrency": 3
            },
            "success_count": success,
            "failed_count": failed,
            "results": []
        });
        std::fs::write(
            dir.join(format!("ghita_manifest_{}.json", stamp)),
            serde_json::to_vec_pretty(&doc).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn read_recent_manifests_sorts_by_name_desc_and_limits() {
        let dir = temp_test_dir("sort_limit");
        write_manifest_file(&dir, "20260101_000001", 1, 0);
        write_manifest_file(&dir, "20260101_000002", 2, 1);
        write_manifest_file(&dir, "20260101_000003", 3, 2);
        std::fs::write(
            dir.join("ghita_manifest_20260101_000004.json"),
            b"{broken json",
        )
        .unwrap();

        let recent = read_recent_manifests(&dir, 8);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].timestamp_label, "2026-01-01 00:00:03");
        assert_eq!(recent[0].success_count, 3);
        assert_eq!(recent[0].failed_count, 2);
        assert_eq!(recent[1].success_count, 2);
        assert_eq!(recent[2].success_count, 1);
        assert!(recent[0]
            .path
            .ends_with("ghita_manifest_20260101_000003.json"));
        assert!(recent[0].path.contains(&dir.display().to_string()));

        let limited = read_recent_manifests(&dir, 2);
        assert_eq!(limited.len(), 2);
        assert_eq!(limited[0].timestamp_label, "2026-01-01 00:00:03");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_recent_manifests_tolerates_partial_fields() {
        let dir = temp_test_dir("partial");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("ghita_manifest_20260101_000009.json"),
            b"{\"success_count\": 4}",
        )
        .unwrap();

        let recent = read_recent_manifests(&dir, 8);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].success_count, 4);
        assert_eq!(recent[0].failed_count, 0);
        assert_eq!(recent[0].timestamp_label, "2026-01-01 00:00:09");
        assert!(recent[0].format.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_recent_manifests_handles_missing_dir() {
        let dir = temp_test_dir("missing");
        let recent = read_recent_manifests(&dir, 8);
        assert!(recent.is_empty());
    }
}
