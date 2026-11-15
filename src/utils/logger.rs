use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;
#[allow(dead_code)]
const MAX_LOG_FILES: u32 = 3;
const LOG_FILE_NAME: &str = "ghita.log";

static LOGGER: OnceLock<Mutex<Option<File>>> = OnceLock::new();

pub fn logs_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join("GhitaDownload").join("logs"))
}

pub fn log_file_path() -> Option<PathBuf> {
    logs_dir().map(|dir| dir.join(LOG_FILE_NAME))
}

pub fn format_log_line(now_secs: u64, message: &str) -> String {
    format!("[{now_secs}] {message}")
}

pub fn log_line(message: &str) {
    let _ = std::panic::catch_unwind(|| {
        let file_cell = LOGGER.get_or_init(|| Mutex::new(None));
        let mut guard = match file_cell.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let path = match log_file_path() {
            Some(path) => path,
            None => return,
        };
        if guard
            .as_ref()
            .is_none_or(|_| current_file_needs_rotation(&path))
        {
            *guard = rotate_and_open(&path);
        }
        if let Some(file) = guard.as_mut() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let _ = writeln!(file, "{}", format_log_line(now, message));
            let _ = file.flush();
        }
    });
}

fn current_file_needs_rotation(path: &std::path::Path) -> bool {
    std::fs::metadata(path)
        .map(|metadata| metadata.len() >= MAX_LOG_BYTES)
        .unwrap_or(false)
}

fn rotate_and_open(path: &std::path::Path) -> Option<File> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let path_text = path.to_string_lossy().into_owned();
    let shifted_1 = format!("{path_text}.1");
    let shifted_2 = format!("{path_text}.2");
    if current_file_needs_rotation(path) {
        let _ = std::fs::remove_file(&shifted_2);
        let _ = std::fs::rename(&shifted_1, &shifted_2);
        let _ = std::fs::rename(path, &shifted_1);
    }
    OpenOptions::new().create(true).append(true).open(path).ok()
}

#[cfg(test)]
mod tests {
    use super::{format_log_line, rotate_and_open, MAX_LOG_BYTES, MAX_LOG_FILES};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_log_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ghita_log_test_{label}_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn format_log_line_prefixes_epoch_seconds() {
        assert_eq!(
            format_log_line(1_700_000_000, "hello"),
            "[1700000000] hello"
        );
    }

    #[test]
    fn rotate_and_open_shifts_full_log_and_keeps_history_bounded() {
        let dir = temp_log_dir("rotate");
        let path = dir.join("ghita.log");
        fs::write(&path, vec![b'x'; MAX_LOG_BYTES as usize]).unwrap();
        fs::write(format!("{}.1", path.to_string_lossy()), "old1").unwrap();
        fs::write(format!("{}.2", path.to_string_lossy()), "old2").unwrap();

        let file = rotate_and_open(&path);
        assert!(file.is_some());
        assert_eq!(
            fs::read_to_string(format!("{}.2", path.to_string_lossy())).unwrap(),
            "old1"
        );
        assert_eq!(
            fs::read_to_string(format!("{}.1", path.to_string_lossy())).unwrap(),
            "x".repeat(MAX_LOG_BYTES as usize),
            "full log moves to .1"
        );
        assert!(fs::metadata(&path).unwrap().len() == 0);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rotate_and_open_creates_missing_file_without_touching_history() {
        let dir = temp_log_dir("fresh");
        let path = dir.join("ghita.log");
        fs::write(format!("{}.2", path.to_string_lossy()), "ancient").unwrap();

        assert!(rotate_and_open(&path).is_some());
        assert_eq!(
            fs::read_to_string(format!("{}.2", path.to_string_lossy())).unwrap(),
            "ancient",
            "history untouched when current log absent/under limit"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn max_log_files_constant_is_three() {
        assert_eq!(MAX_LOG_FILES, 3);
    }
}
