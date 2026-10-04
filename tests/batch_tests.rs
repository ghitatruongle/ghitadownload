use ghita_download::core::batch::{BatchProcessor, DownloadTask, FailedQueue};
use ghita_download::core::config::{AudioFormat, DownloadSettings};
use ghita_download::core::tagger::AudioMetadata;
use std::path::PathBuf;

fn test_dir(label: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("ghita_batch_exec_{}_{}", std::process::id(), label));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn sample_settings(output_dir: PathBuf) -> DownloadSettings {
    DownloadSettings {
        output_dir,
        format: AudioFormat::Mp3,
        quality: None,
        video_resolution: None,
        concurrency: 3,
        keep_accents: false,
        cookies_file: None,
        cookies_from_browser: None,
    }
}

fn sample_task(title: &str) -> DownloadTask {
    DownloadTask {
        display_title: title.to_string(),
        source_or_search: "https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string(),
        fallback_searches: Vec::new(),
        expected_duration_secs: None,
        metadata: AudioMetadata {
            title: title.to_string(),
            artists: vec!["Artist".to_string()],
            album: "Album".to_string(),
            release_year: None,
            cover_url: None,
            track_number: None,
            total_tracks: None,
        },
    }
}

#[tokio::test]
async fn execute_batch_with_empty_queue_returns_zero() {
    let dir = test_dir("empty_queue");
    let processor = BatchProcessor::new(
        sample_settings(dir.clone()),
        std::path::Path::new("bin/yt-dlp.exe"),
        std::path::Path::new("ffmpeg"),
        false,
    );

    let outcome = processor.execute_batch(&[]).await.unwrap();
    assert_eq!(outcome.success_count, 0);
    assert!(outcome.failed_tasks.is_empty());
    assert!(
        !dir.join("failed_tasks.json").exists(),
        "hàng đợi rỗng không được tạo tệp failed_tasks.json"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn retry_from_file_rejects_corrupt_queue_json() {
    let dir = test_dir("corrupt_json");
    std::fs::write(dir.join("failed_tasks.json"), "{ not valid json").unwrap();
    assert!(BatchProcessor::retry_from_file(&dir).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn retry_from_file_rejects_wrong_json_shape() {
    let dir = test_dir("wrong_shape");
    std::fs::write(dir.join("failed_tasks.json"), "[]").unwrap();
    assert!(BatchProcessor::retry_from_file(&dir).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn failed_queue_roundtrip_preserves_many_tasks() {
    let dir = test_dir("roundtrip_many");
    let queue = FailedQueue {
        settings: sample_settings(dir.clone()),
        tasks: vec![
            sample_task("Bài 1"),
            sample_task("Bài 2"),
            sample_task("Bài 3"),
        ],
    };
    let json = serde_json::to_string_pretty(&queue).unwrap();
    std::fs::write(dir.join("failed_tasks.json"), &json).unwrap();

    let (loaded_settings, loaded_tasks) = BatchProcessor::retry_from_file(&dir).unwrap();
    assert_eq!(loaded_settings.format, AudioFormat::Mp3);
    assert_eq!(loaded_settings.concurrency, 3);
    assert_eq!(loaded_tasks.len(), 3);
    assert_eq!(loaded_tasks[2].display_title, "Bài 3");

    let _ = std::fs::remove_dir_all(&dir);
}
