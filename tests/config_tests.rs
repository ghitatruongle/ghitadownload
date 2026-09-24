use ghita_download::core::config::{AppConfig, AudioFormat, AudioQuality};
use std::path::PathBuf;

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
fn invalid_config_returns_error() {
    let directory = test_directory("invalid");
    let path = directory.join("ghita_config.json");
    std::fs::write(&path, b"not json").unwrap();
    assert!(AppConfig::try_load_from(&path).is_err());
    let _ = std::fs::remove_dir_all(directory);
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
