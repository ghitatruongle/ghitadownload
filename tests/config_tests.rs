use ghita_download::core::config::{AppConfig, AudioFormat, AudioQuality, DownloadSettings};
use std::path::PathBuf;

fn settings_with_cookies() -> DownloadSettings {
    DownloadSettings {
        output_dir: PathBuf::from(r"D:\Music"),
        format: AudioFormat::Mp3,
        quality: Some(AudioQuality::Mp3_320k),
        video_resolution: Some(1080),
        concurrency: 3,
        keep_accents: false,
        cookies_file: Some(PathBuf::from(r"C:\private\cookies.txt")),
        cookies_from_browser: Some("firefox".to_string()),
        raw: false,
        fetch_lyrics: true,
    }
}

fn test_directory(label: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "ghita_config_{label}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

#[test]
fn initial_output_dir_uses_sensible_current_directory() {
    let config = AppConfig {
        last_output_dir: Some(PathBuf::from(r"D:\Music")),
        ..Default::default()
    };
    assert_eq!(
        config.initial_output_dir(PathBuf::from(r"C:\Workspace")),
        PathBuf::from(r"C:\Workspace")
    );
}

#[test]
fn initial_output_dir_falls_back_for_empty_or_system_value() {
    let config = AppConfig {
        last_output_dir: Some(PathBuf::from(r"D:\SavedMusic")),
        ..Default::default()
    };
    assert_eq!(
        config.initial_output_dir(PathBuf::new()),
        PathBuf::from(r"D:\SavedMusic")
    );
    assert_eq!(
        config.initial_output_dir(PathBuf::from(r"C:\Windows\System32")),
        PathBuf::from(r"D:\SavedMusic")
    );
}

#[test]
fn missing_config_loads_explicit_default() {
    let directory = test_directory("missing");
    let path = directory.join("ghita_config.json");
    let config = AppConfig::try_load_from(&path).unwrap();
    assert_eq!(
        config.initial_output_dir(PathBuf::from("fallback")),
        AppConfig::default().initial_output_dir(PathBuf::from("fallback"))
    );
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn config_round_trip_preserves_saved_directory() {
    let directory = test_directory("round_trip");
    let path = directory.join("ghita_config.json");
    let config = AppConfig {
        last_output_dir: Some(PathBuf::from(r#"  "D:\Saved Music"  "#)),
        last_format: Some(AudioFormat::Flac),
        last_quality: Some(AudioQuality::Flac_24bit_96k),
        ..Default::default()
    };
    config.try_save_to(&path).unwrap();
    let loaded = AppConfig::try_load_from(&path).unwrap();
    assert_eq!(
        loaded.last_output_dir,
        Some(PathBuf::from(r"D:\Saved Music"))
    );
    assert_eq!(loaded.last_format, Some(AudioFormat::Flac));
    assert_eq!(loaded.last_quality, Some(AudioQuality::Flac_24bit_96k));
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn invalid_config_falls_back_to_default_and_backs_up() {
    let directory = test_directory("invalid");
    let path = directory.join("ghita_config.json");
    std::fs::write(&path, b"not json").unwrap();
    let config = AppConfig::try_load_from(&path).unwrap();
    assert_eq!(
        config.initial_output_dir(PathBuf::from("fallback")),
        AppConfig::default().initial_output_dir(PathBuf::from("fallback"))
    );
    assert!(!path.exists(), "tệp hỏng phải được đổi tên tránh va chạm");
    let backup_count = std::fs::read_dir(&directory)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .contains("ghita_config.json.corrupt-")
        })
        .count();
    assert_eq!(backup_count, 1, "phải còn đúng 1 bản sao hỏng");
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn config_round_trip_writes_schema_version() {
    let directory = test_directory("schema_version");
    let path = directory.join("ghita_config.json");
    AppConfig::default().try_save_to(&path).unwrap();
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("schema_version"));
    let loaded = AppConfig::try_load_from(&path).unwrap();
    assert_eq!(loaded.schema_version, 1);
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn serialized_settings_omit_cookie_fields() {
    let settings = settings_with_cookies();
    let json = serde_json::to_string(&settings).unwrap();
    assert!(
        !json.contains("cookies_file"),
        "queue JSON không được chứa cookies_file: {json}"
    );
    assert!(
        !json.contains("cookies_from_browser"),
        "queue JSON không được chứa cookies_from_browser: {json}"
    );
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(value.get("cookies_file").is_none());
    assert!(value.get("cookies_from_browser").is_none());
}

#[test]
fn old_queue_json_with_cookie_fields_still_loads() {
    let json = serde_json::json!({
        "output_dir": r"D:\Music",
        "format": "Mp3",
        "quality": "Mp3_320k",
        "video_resolution": 1080,
        "concurrency": 3,
        "keep_accents": false,
        "cookies_file": r"C:\old\cookies.txt",
        "cookies_from_browser": "chrome",
        "raw": false,
        "fetch_lyrics": true
    });
    let settings: DownloadSettings = serde_json::from_value(json).unwrap();
    assert_eq!(settings.output_dir, PathBuf::from(r"D:\Music"));
    assert_eq!(settings.format, AudioFormat::Mp3);
    assert_eq!(settings.quality, Some(AudioQuality::Mp3_320k));
    assert_eq!(settings.cookies_file, None);
    assert_eq!(settings.cookies_from_browser, None);
}

#[test]
fn old_queue_json_without_cookie_fields_loads_with_defaults() {
    let json = serde_json::json!({
        "output_dir": r"D:\Music",
        "format": "Flac",
        "quality": null
    });
    let settings: DownloadSettings = serde_json::from_value(json).unwrap();
    assert_eq!(settings.output_dir, PathBuf::from(r"D:\Music"));
    assert_eq!(settings.format, AudioFormat::Flac);
    assert!(settings.quality.is_none());
    assert_eq!(settings.cookies_file, None);
    assert_eq!(settings.cookies_from_browser, None);
    assert_eq!(settings.concurrency, 3);
}

#[test]
fn atomic_save_replaces_existing_config() {
    let directory = test_directory("atomic");
    let path = directory.join("ghita_config.json");
    let config = AppConfig {
        last_output_dir: Some(PathBuf::from(r"D:\First")),
        ..Default::default()
    };
    config.try_save_to(&path).unwrap();
    let config = AppConfig {
        last_output_dir: Some(PathBuf::from(r"D:\Second")),
        ..Default::default()
    };
    config.try_save_to(&path).unwrap();
    assert_eq!(
        AppConfig::try_load_from(&path).unwrap().last_output_dir,
        Some(PathBuf::from(r"D:\Second"))
    );
    let temporary_files = std::fs::read_dir(&directory)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".ghita_config.json.")
        })
        .count();
    assert_eq!(temporary_files, 0);
    let _ = std::fs::remove_dir_all(directory);
}
