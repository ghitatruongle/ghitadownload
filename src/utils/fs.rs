use std::path::Path;
use std::time::Duration;

const STALE_BACKUP_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

pub fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    if windows_replace_file(source, destination) {
        if let Some(parent) = destination.parent() {
            cleanup_stale_backups(parent, STALE_BACKUP_MAX_AGE);
        }
        return Ok(());
    }
    match std::fs::rename(source, destination) {
        Ok(()) => {
            if let Some(parent) = destination.parent() {
                cleanup_stale_backups(parent, STALE_BACKUP_MAX_AGE);
            }
            Ok(())
        }
        Err(_) if destination.exists() => {
            let extension = destination
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("file");
            let backup = destination.with_extension(format!(
                "{}.{}.{}.bak",
                extension,
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
                    if let Some(parent) = destination.parent() {
                        cleanup_stale_backups(parent, STALE_BACKUP_MAX_AGE);
                    }
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

pub fn cleanup_stale_backups(dir: &Path, max_age: Duration) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let now = std::time::SystemTime::now();
    for entry in entries.flatten() {
        let path = entry.path();
        let is_bak = path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("bak"));
        if !is_bak {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        if now.duration_since(modified).unwrap_or_default() > max_age {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{cleanup_stale_backups, replace_file};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};

    const SEVEN_DAYS: Duration = Duration::from_secs(7 * 24 * 60 * 60);
    const EIGHT_DAYS: Duration = Duration::from_secs(8 * 24 * 60 * 60);

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ghita_fs_{label}_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn backdate(path: &Path, age: Duration) {
        let old = SystemTime::now() - age;
        let file = std::fs::File::options().write(true).open(path).unwrap();
        file.set_modified(old).unwrap();
    }

    fn bak_file_count(dir: &Path) -> usize {
        std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("bak"))
            })
            .count()
    }

    #[test]
    fn cleanup_stale_backups_removes_only_old_bak_files() {
        let dir = temp_dir("cleanup");
        std::fs::write(dir.join("old.bak"), b"old").unwrap();
        std::fs::write(dir.join("fresh.bak"), b"fresh").unwrap();
        std::fs::write(dir.join("old.txt"), b"data").unwrap();
        std::fs::write(dir.join("old.bak2"), b"other").unwrap();
        backdate(&dir.join("old.bak"), EIGHT_DAYS);
        backdate(&dir.join("old.txt"), EIGHT_DAYS);
        backdate(&dir.join("old.bak2"), EIGHT_DAYS);
        cleanup_stale_backups(&dir, SEVEN_DAYS);
        assert!(!dir.join("old.bak").exists());
        assert!(dir.join("fresh.bak").exists());
        assert!(dir.join("old.txt").exists());
        assert!(dir.join("old.bak2").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn replace_file_writes_destination_without_leftover_bak() {
        let dir = temp_dir("replace_fresh");
        let source = dir.join(".incoming.exe");
        let destination = dir.join("app.exe");
        std::fs::write(&source, b"payload").unwrap();
        replace_file(&source, &destination).unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), b"payload");
        assert!(!source.exists());
        assert_eq!(bak_file_count(&dir), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn replace_file_over_existing_destination_leaves_no_bak() {
        let dir = temp_dir("replace_over");
        let source = dir.join(".incoming.exe");
        let destination = dir.join("app.exe");
        std::fs::write(&destination, b"old").unwrap();
        std::fs::write(&source, b"new").unwrap();
        replace_file(&source, &destination).unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), b"new");
        assert_eq!(bak_file_count(&dir), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(windows)]
fn windows_replace_file(source: &Path, destination: &Path) -> bool {
    use windows_sys::Win32::Storage::FileSystem::{ReplaceFileW, REPLACEFILE_IGNORE_MERGE_ERRORS};
    let to_wide = |value: &Path| -> Vec<u16> {
        value
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect()
    };
    let source_wide = to_wide(source);
    let destination_wide = to_wide(destination);
    let mut backup: Vec<u16> = {
        let text = destination.to_string_lossy().into_owned();
        format!(
            "{text}.{}.{}.bak",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        )
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
    };
    let ok = unsafe {
        ReplaceFileW(
            destination_wide.as_ptr(),
            source_wide.as_ptr(),
            backup.as_mut_ptr(),
            REPLACEFILE_IGNORE_MERGE_ERRORS,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if ok != 0 {
        let _ = std::fs::remove_file(std::path::PathBuf::from(String::from_utf16_lossy(
            &backup[..backup.len() - 1],
        )));
        return true;
    }
    let _ = &mut backup;
    false
}

pub const LOW_DISK_WARN_BYTES: u64 = 1024 * 1024 * 1024;

#[cfg(windows)]
pub fn drive_free_bytes(path: &std::path::Path) -> Option<u64> {
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let wide: Vec<u16> = path
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut free: u64 = 0;
    let mut total: u64 = 0;
    let mut total_free: u64 = 0;
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, &mut total, &mut total_free) };
    ok.ne(&0).then_some(free)
}

#[cfg(not(windows))]
pub fn drive_free_bytes(path: &std::path::Path) -> Option<u64> {
    let _ = path;
    None
}

pub fn free_space_warning(free_bytes: u64) -> Option<String> {
    (free_bytes < LOW_DISK_WARN_BYTES).then(|| {
        format!(
            "⚠️  Ổ đĩa đích chỉ còn {:.1} GB trống — cân nhắc dọn dẹp trước khi tải số lượng lớn.",
            free_bytes as f64 / 1024.0 / 1024.0 / 1024.0
        )
    })
}

#[cfg(test)]
mod fs_tests_disk {
    #[cfg(windows)]
    use super::drive_free_bytes;
    use super::free_space_warning;

    #[test]
    fn free_space_warning_only_under_threshold() {
        assert!(free_space_warning(1024 * 1024 * 1024).is_none());
        assert!(free_space_warning(u64::MAX).is_none());
        let warning = free_space_warning(512 * 1024 * 1024);
        let text = warning.expect("512MB must warn");
        assert!(text.contains("0.5 GB") || text.contains("0,5 GB"));
    }

    #[cfg(windows)]
    #[test]
    fn drive_free_bytes_returns_value_for_real_dir() {
        let probe = std::env::temp_dir();
        assert!(drive_free_bytes(&probe).is_some());
        assert!(drive_free_bytes(&probe).unwrap() > 0);
    }
}
