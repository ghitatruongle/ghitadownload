use ghita_download::core::batch::{DownloadTask, FailedQueue};
use ghita_download::core::config::{AudioFormat, AudioQuality, DownloadSettings};
use ghita_download::core::manifest::write_manifest;
use ghita_download::core::tagger::AudioMetadata;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const GOLDEN_PENDING_TASKS: &str = r#"{
  "settings": {
    "output_dir": "D:\\Music\\Ghita",
    "format": "Mp3",
    "quality": "Mp3_320k",
    "video_resolution": null,
    "concurrency": 3,
    "duplicate_mode": "rename",
    "keep_accents": false,
    "raw": false,
    "fetch_lyrics": true
  },
  "tasks": [
    {
      "display_title": "Never Gonna Give You Up",
      "source_or_search": "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
      "fallback_searches": ["never gonna give you up audio"],
      "expected_duration_secs": 212,
      "metadata": {
        "title": "Never Gonna Give You Up",
        "artists": ["Rick Astley"],
        "album": "Whenever You Need Somebody",
        "release_year": 1987,
        "cover_url": "https://i.ytimg.com/vi/dQw4w9WgXcQ/maxresdefault.jpg",
        "track_number": 1,
        "total_tracks": 10,
        "lyrics": null
      }
    }
  ]
}"#;

const GOLDEN_FAILED_TASKS: &str = r#"{
  "settings": {
    "output_dir": "E:\\Nhac\\Ghita",
    "format": "Flac",
    "quality": null,
    "video_resolution": 1080,
    "concurrency": 5,
    "duplicate_mode": "rename",
    "keep_accents": true,
    "raw": true,
    "fetch_lyrics": false
  },
  "tasks": [
    {
      "display_title": "Me at the zoo",
      "source_or_search": "me at the zoo",
      "fallback_searches": [],
      "expected_duration_secs": null,
      "metadata": {
        "title": "Me at the zoo",
        "artists": [],
        "album": "",
        "release_year": null,
        "cover_url": null,
        "track_number": null,
        "total_tracks": null,
        "lyrics": "jawed"
      }
    }
  ]
}"#;

const GOLDEN_LEGACY_WITH_COOKIES: &str = r#"{
  "settings": {
    "output_dir": "D:\\Music",
    "format": "Mp3",
    "quality": null,
    "cookies_file": "C:\\Users\\me\\cookies.txt",
    "cookies_from_browser": "firefox",
    "video_resolution": null,
    "concurrency": 3,
    "duplicate_mode": "rename",
    "keep_accents": false,
    "raw": false,
    "fetch_lyrics": true
  },
  "tasks": []
}"#;

const QUEUE_ROOT_KEYS: &[&str] = &["settings", "tasks"];
const SETTINGS_KEYS: &[&str] = &[
    "concurrency",
    "duplicate_mode",
    "fetch_lyrics",
    "format",
    "keep_accents",
    "output_dir",
    "quality",
    "raw",
    "video_resolution",
];
const TASK_KEYS: &[&str] = &[
    "display_title",
    "expected_duration_secs",
    "fallback_searches",
    "metadata",
    "source_or_search",
];
const METADATA_KEYS: &[&str] = &[
    "album",
    "artists",
    "cover_url",
    "lyrics",
    "release_year",
    "title",
    "total_tracks",
    "track_number",
];

fn sorted_keys(value: &Value) -> Vec<String> {
    let mut keys: Vec<String> = value
        .as_object()
        .expect("expected a JSON object")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    keys
}

fn assert_keys(value: &Value, expected: &[&str]) {
    let mut expected: Vec<String> = expected.iter().map(|key| key.to_string()).collect();
    expected.sort();
    assert_eq!(sorted_keys(value), expected, "exact key set must match");
}

fn assert_no_cookie_keys(serialized: &str) {
    assert!(
        !serialized.contains("cookies_file"),
        "cookies_file must never be serialized"
    );
    assert!(
        !serialized.contains("cookies_from_browser"),
        "cookies_from_browser must never be serialized"
    );
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "ghita_golden_{label}_{}_{}",
        std::process::id(),
        nanos
    ))
}

fn golden_settings(output_dir: &str) -> DownloadSettings {
    DownloadSettings {
        output_dir: PathBuf::from(output_dir),
        format: AudioFormat::Mp3,
        quality: Some(AudioQuality::Mp3_320k),
        video_resolution: None,
        concurrency: 3,
        keep_accents: false,
        duplicate_mode: "rename".to_string(),
        cookies_file: Some(PathBuf::from("C:\\secret\\cookies.txt")),
        cookies_from_browser: Some("firefox".to_string()),
        raw: false,
        fetch_lyrics: true,
    }
}

fn golden_task(title: &str, source: &str) -> DownloadTask {
    DownloadTask {
        display_title: title.to_string(),
        source_or_search: source.to_string(),
        fallback_searches: vec![format!("{title} audio")],
        expected_duration_secs: Some(212),
        metadata: AudioMetadata {
            title: title.to_string(),
            artists: vec!["Rick Astley".to_string()],
            album: "Whenever You Need Somebody".to_string(),
            release_year: Some(1987),
            cover_url: Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/maxresdefault.jpg".to_string()),
            track_number: Some(1),
            total_tracks: Some(10),
            lyrics: None,
        },
    }
}

#[test]
fn pending_tasks_json_deserializes_into_failed_queue() {
    let queue: FailedQueue =
        serde_json::from_str(GOLDEN_PENDING_TASKS).expect("golden pending_tasks.json must parse");
    assert_eq!(queue.settings.output_dir, PathBuf::from("D:\\Music\\Ghita"));
    assert_eq!(queue.settings.format, AudioFormat::Mp3);
    assert_eq!(queue.settings.quality, Some(AudioQuality::Mp3_320k));
    assert_eq!(queue.settings.video_resolution, None);
    assert_eq!(queue.settings.concurrency, 3);
    assert!(!queue.settings.keep_accents);
    assert!(!queue.settings.raw);
    assert!(queue.settings.fetch_lyrics);
    assert_eq!(queue.settings.cookies_file, None);
    assert_eq!(queue.settings.cookies_from_browser, None);
    assert_eq!(queue.tasks.len(), 1);
    let task = &queue.tasks[0];
    assert_eq!(task.display_title, "Never Gonna Give You Up");
    assert_eq!(
        task.source_or_search,
        "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
    );
    assert_eq!(
        task.fallback_searches,
        vec!["never gonna give you up audio"]
    );
    assert_eq!(task.expected_duration_secs, Some(212));
    assert_eq!(task.metadata.title, "Never Gonna Give You Up");
    assert_eq!(task.metadata.artists, vec!["Rick Astley"]);
    assert_eq!(task.metadata.album, "Whenever You Need Somebody");
    assert_eq!(task.metadata.release_year, Some(1987));
    assert_eq!(
        task.metadata.cover_url.as_deref(),
        Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/maxresdefault.jpg"),
    );
    assert_eq!(task.metadata.track_number, Some(1));
    assert_eq!(task.metadata.total_tracks, Some(10));
    assert_eq!(task.metadata.lyrics, None);
}

#[test]
fn pending_tasks_json_round_trip_matches_golden_shape() {
    let queue: FailedQueue =
        serde_json::from_str(GOLDEN_PENDING_TASKS).expect("golden pending_tasks.json must parse");
    let serialized = serde_json::to_string(&queue).expect("serialization must succeed");
    assert_no_cookie_keys(&serialized);
    let doc: Value = serde_json::from_str(&serialized).expect("re-parse must succeed");
    let golden: Value =
        serde_json::from_str(GOLDEN_PENDING_TASKS).expect("golden literal must parse");
    assert_eq!(doc, golden, "round trip must reproduce the golden JSON");
    assert_keys(&doc, QUEUE_ROOT_KEYS);
    assert_keys(&doc["settings"], SETTINGS_KEYS);
    assert_keys(&doc["tasks"][0], TASK_KEYS);
    assert_keys(&doc["tasks"][0]["metadata"], METADATA_KEYS);
    assert!(
        doc["tasks"][0]["expected_duration_secs"].is_u64(),
        "expected_duration_secs must stay an unsigned integer"
    );
    assert!(
        doc["tasks"][0]["metadata"]["release_year"].is_u64(),
        "release_year must stay an unsigned integer"
    );
    assert!(
        doc["tasks"][0]["metadata"]["lyrics"].is_null(),
        "lyrics must stay null-typed in the golden shape"
    );
    assert!(
        doc["settings"]["quality"].is_string(),
        "quality must serialize as a variant-name string"
    );
}

#[test]
fn failed_tasks_json_deserializes_and_round_trips() {
    let queue: FailedQueue =
        serde_json::from_str(GOLDEN_FAILED_TASKS).expect("golden failed_tasks.json must parse");
    assert_eq!(queue.settings.format, AudioFormat::Flac);
    assert_eq!(queue.settings.quality, None);
    assert_eq!(queue.settings.video_resolution, Some(1080));
    assert_eq!(queue.settings.concurrency, 5);
    assert!(queue.settings.keep_accents);
    assert!(queue.settings.raw);
    assert!(!queue.settings.fetch_lyrics);
    assert_eq!(queue.tasks.len(), 1);
    assert_eq!(queue.tasks[0].source_or_search, "me at the zoo");
    assert_eq!(queue.tasks[0].expected_duration_secs, None);
    assert_eq!(queue.tasks[0].metadata.lyrics.as_deref(), Some("jawed"));
    let serialized = serde_json::to_string(&queue).expect("serialization must succeed");
    assert_no_cookie_keys(&serialized);
    let doc: Value = serde_json::from_str(&serialized).expect("re-parse must succeed");
    let golden: Value =
        serde_json::from_str(GOLDEN_FAILED_TASKS).expect("golden literal must parse");
    assert_eq!(doc, golden, "round trip must reproduce the golden JSON");
    assert_keys(&doc, QUEUE_ROOT_KEYS);
    assert_keys(&doc["settings"], SETTINGS_KEYS);
    assert_keys(&doc["tasks"][0], TASK_KEYS);
    assert!(
        doc["settings"]["quality"].is_null(),
        "absent quality must serialize as null"
    );
    assert!(
        doc["settings"]["video_resolution"].is_u64(),
        "video_resolution must stay an unsigned integer"
    );
}

#[test]
fn legacy_queue_files_with_cookie_keys_still_load_and_reserialize_clean() {
    let queue: FailedQueue = serde_json::from_str(GOLDEN_LEGACY_WITH_COOKIES)
        .expect("old queue files carrying cookie keys must still load");
    assert_eq!(queue.settings.cookies_file, None);
    assert_eq!(queue.settings.cookies_from_browser, None);
    let serialized = serde_json::to_string(&queue).expect("serialization must succeed");
    assert_no_cookie_keys(&serialized);
    let doc: Value = serde_json::from_str(&serialized).expect("re-parse must succeed");
    assert_keys(&doc["settings"], SETTINGS_KEYS);
    assert!(queue.tasks.is_empty());
}

#[test]
fn manifest_json_matches_golden_shape() {
    let dir = unique_temp_dir("manifest");
    std::fs::create_dir_all(&dir).unwrap();
    let output_dir = dir.join("out");
    let settings = golden_settings(output_dir.to_string_lossy().as_ref());
    let ok_task = golden_task(
        "Never Gonna Give You Up",
        "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
    );
    let failed_task = golden_task("Me at the zoo", "me at the zoo");
    let path = write_manifest(
        &dir,
        &[ok_task, failed_task.clone()],
        std::slice::from_ref(&failed_task),
        &settings,
        &[(1, Some("re-ranked fallback".to_string()))],
    )
    .expect("manifest must write");
    let content = std::fs::read_to_string(&path).expect("manifest file must exist");
    assert_no_cookie_keys(&content);
    assert!(
        Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("ghita_manifest_") && name.ends_with(".json")),
        "manifest file name must keep the ghita_manifest_*.json pattern"
    );
    let mut doc: Value = serde_json::from_str(&content).expect("manifest must parse");
    let generated_at = doc["generated_at"]
        .as_str()
        .expect("generated_at must be a string")
        .to_string();
    assert!(
        generated_at.len() == 20
            && generated_at.ends_with('Z')
            && generated_at.as_bytes()[4] == b'-'
            && generated_at.as_bytes()[10] == b'T',
        "generated_at must keep the ISO-8601 UTC shape, got {generated_at:?}"
    );
    doc["generated_at"] = serde_json::json!("<generated_at>");
    doc["settings"]["output_dir"] = serde_json::json!("<output_dir>");
    let expected = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "generated_at": "<generated_at>",
        "settings": {
            "duplicate_mode": "rename",
            "format": "MP3 (.mp3) - Tương thích mọi thiết bị, dung lượng nhỏ gọn",
            "quality": "320 kbps [Cực cao / Best] - Chuẩn MP3 phòng thu cao cấp nhất",
            "output_dir": "<output_dir>",
            "concurrency": 3,
            "duplicate_mode": "rename",
        },
        "success_count": 1,
        "failed_count": 1,
        "results": [
            {
                "url": "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
                "status": "ok",
                "task_index": 0,
            },
            {
                "url": "me at the zoo",
                "status": "failed",
                "task_index": 1,
                "source_note": "re-ranked fallback",
            },
        ],
    });
    assert_eq!(doc, expected, "manifest shape must match the golden form");
    assert_keys(
        &doc,
        &[
            "version",
            "generated_at",
            "settings",
            "success_count",
            "failed_count",
            "results",
        ],
    );
    assert_keys(
        &doc["settings"],
        &[
            "format",
            "quality",
            "output_dir",
            "concurrency",
            "duplicate_mode",
        ],
    );
    assert_keys(&doc["results"][0], &["url", "status", "task_index"]);
    assert_keys(
        &doc["results"][1],
        &["url", "status", "task_index", "source_note"],
    );
    assert!(doc["results"][0]["task_index"].is_u64());
    assert!(doc["success_count"].is_u64() && doc["failed_count"].is_u64());
    let _ = std::fs::remove_dir_all(&dir);
}
