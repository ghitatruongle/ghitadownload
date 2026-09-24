use ghita_download::utils::env::{find_command, is_valid_windows_pe};
use std::path::PathBuf;

fn test_directory(label: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "ghita_installer_{label}_{}_{}",
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
fn windows_pe_validation_rejects_short_and_non_pe_files() {
    let directory = test_directory("pe");
    let invalid = directory.join("invalid.exe");
    std::fs::write(&invalid, b"MZ").unwrap();
    assert!(!is_valid_windows_pe(&invalid));
    assert!(!is_valid_windows_pe(&directory.join("missing.exe")));
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn command_discovery_never_returns_relative_paths() {
    if let Some(command) = find_command("cmd") {
        assert!(command.is_absolute());
    }
}
