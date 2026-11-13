use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

pub fn remove_vietnamese_accents(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    for c in input.chars() {
        if ('\u{0300}'..='\u{036f}').contains(&c) {
            continue;
        }

        let unaccented = match c {
            'à' | 'á' | 'ả' | 'ã' | 'ạ' | 'ă' | 'ằ' | 'ắ' | 'ẳ' | 'ẵ' | 'ặ' | 'â' | 'ầ' | 'ấ'
            | 'ẩ' | 'ẫ' | 'ậ' | 'ä' | 'å' | 'ā' => 'a',
            'À' | 'Á' | 'Ả' | 'Ã' | 'Ạ' | 'Ă' | 'Ằ' | 'Ắ' | 'Ẳ' | 'Ẵ' | 'Ặ' | 'Â' | 'Ầ' | 'Ấ'
            | 'Ẩ' | 'Ẫ' | 'Ậ' | 'Ä' | 'Å' | 'Ā' => 'A',
            'đ' => 'd',
            'Đ' | 'Ð' => 'D',
            'è' | 'é' | 'ẻ' | 'ẽ' | 'ẹ' | 'ê' | 'ề' | 'ế' | 'ể' | 'ễ' | 'ệ' | 'ë' | 'ē' => {
                'e'
            }
            'È' | 'É' | 'Ẻ' | 'Ẽ' | 'Ẹ' | 'Ê' | 'Ề' | 'Ế' | 'Ể' | 'Ễ' | 'Ệ' | 'Ë' | 'Ē' => {
                'E'
            }
            'ì' | 'í' | 'ỉ' | 'ĩ' | 'ị' | 'ï' | 'î' | 'ī' => 'i',
            'Ì' | 'Í' | 'Ỉ' | 'Ĩ' | 'Ị' | 'Ï' | 'Î' | 'Ī' => 'I',
            'ò' | 'ó' | 'ỏ' | 'õ' | 'ọ' | 'ô' | 'ồ' | 'ố' | 'ổ' | 'ỗ' | 'ộ' | 'ơ' | 'ờ' | 'ớ'
            | 'ở' | 'ỡ' | 'ợ' | 'ö' | 'ø' | 'ō' => 'o',
            'Ò' | 'Ó' | 'Ỏ' | 'Õ' | 'Ọ' | 'Ô' | 'Ồ' | 'Ố' | 'Ổ' | 'Ỗ' | 'Ộ' | 'Ơ' | 'Ờ' | 'Ớ'
            | 'Ở' | 'Ỡ' | 'Ợ' | 'Ö' | 'Ø' | 'Ō' => 'O',
            'ù' | 'ú' | 'ủ' | 'ũ' | 'ụ' | 'ư' | 'ừ' | 'ứ' | 'ử' | 'ữ' | 'ự' | 'ü' | 'ū' => {
                'u'
            }
            'Ù' | 'Ú' | 'Ủ' | 'Ũ' | 'Ụ' | 'Ư' | 'Ừ' | 'Ứ' | 'Ử' | 'Ữ' | 'Ự' | 'Ü' | 'Ū' => {
                'U'
            }
            'ỳ' | 'ý' | 'ỷ' | 'ỹ' | 'ỵ' | 'ÿ' => 'y',
            'Ỳ' | 'Ý' | 'Ỷ' | 'Ỹ' | 'Ỵ' | 'Ÿ' => 'Y',
            'ñ' => 'n',
            'Ñ' => 'N',
            'ç' => 'c',
            'Ç' => 'C',
            '–' | '—' => '-',
            '‘' | '’' => '\'',
            '“' | '”' => '"',
            other => other,
        };
        result.push(unaccented);
    }
    result
}

pub fn sanitize_name(name: &str) -> String {
    sanitize_name_with_options(name, false)
}

pub fn sanitize_name_with_options(name: &str, keep_accents: bool) -> String {
    let processed = if keep_accents {
        name.to_string()
    } else {
        remove_vietnamese_accents(name)
    };
    let sanitized = sanitize_filename::sanitize_with_options(
        &processed,
        sanitize_filename::Options {
            truncate: true,
            windows: true,
            replacement: "_",
        },
    );

    let trimmed = sanitized.trim().trim_matches('.').trim();
    if trimmed.is_empty() {
        "audio_track".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn clean_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    let cleaned = s.trim().trim_matches(|c| c == '"' || c == '\'').trim();
    PathBuf::from(cleaned)
}

#[allow(dead_code)]
pub fn ensure_unique_path(dir: &Path, base_name: &str, ext: &str) -> Result<PathBuf> {
    ensure_unique_path_with_options(dir, base_name, ext, false)
}

pub fn ensure_unique_path_with_options(
    dir: &Path,
    base_name: &str,
    ext: &str,
    keep_accents: bool,
) -> Result<PathBuf> {
    let clean_dir = clean_path(dir);
    std::fs::create_dir_all(&clean_dir)?;
    let clean_base = sanitize_name_with_options(base_name, keep_accents);
    let clean_ext = validate_extension(ext)?;
    let mut counter = 0u32;

    loop {
        let file_name = if counter == 0 {
            format!("{}.{}", clean_base, clean_ext)
        } else {
            format!("{} ({}).{}", clean_base, counter, clean_ext)
        };
        let candidate = clean_dir.join(file_name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(_) => return Ok(candidate),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if counter >= 10_000 {
                    return Err(anyhow!(
                        "Không thể tìm tên tệp duy nhất sau 10.000 lần thử: {}",
                        candidate.display()
                    ));
                }
                counter += 1;
            }
            Err(e) => {
                return Err(anyhow!(
                    "Không thể tạo tệp đầu ra tại {}: {}",
                    candidate.display(),
                    e
                ));
            }
        }
    }
}

fn validate_extension(ext: &str) -> Result<&str> {
    if ext.is_empty()
        || ext == "."
        || ext == ".."
        || !ext
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(anyhow!("Phần mở rộng tệp không hợp lệ: {}", ext));
    }
    Ok(ext)
}

pub fn ensure_dir(path: &Path) -> std::io::Result<PathBuf> {
    let cleaned = clean_path(path);
    if !cleaned.exists() {
        std::fs::create_dir_all(&cleaned)?;
    }
    Ok(cleaned)
}

const STALE_TEMP_DIR_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(30 * 60);

pub fn sweep_stale_temp_dirs(output_dir: &Path) -> std::io::Result<usize> {
    let entries = match std::fs::read_dir(output_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    let cutoff = std::time::SystemTime::now() - STALE_TEMP_DIR_MAX_AGE;
    let mut removed = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with(".ghita_temp_") {
            continue;
        }
        let is_stale = temp_dir_last_activity(&path)
            .map(|last_activity| last_activity <= cutoff)
            .unwrap_or(false);
        if is_stale && std::fs::remove_dir_all(&path).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

fn temp_dir_last_activity(path: &Path) -> Option<std::time::SystemTime> {
    let mut newest = std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok();
    if let Ok(children) = std::fs::read_dir(path) {
        for child in children.flatten() {
            if let Ok(modified) = child.metadata().and_then(|meta| meta.modified()) {
                newest = Some(match newest {
                    Some(current) if current >= modified => current,
                    _ => modified,
                });
            }
        }
    }
    newest
}

#[cfg(test)]
mod tests {
    use super::sweep_stale_temp_dirs;
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    #[cfg(windows)]
    fn open_dir_for_times(path: &Path) -> std::io::Result<std::fs::File> {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        std::fs::OpenOptions::new()
            .write(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)
    }

    #[cfg(not(windows))]
    fn open_dir_for_times(path: &Path) -> std::io::Result<std::fs::File> {
        std::fs::File::open(path)
    }

    fn set_dir_mtime(path: &Path, time: SystemTime) {
        open_dir_for_times(path)
            .and_then(|file| file.set_modified(time))
            .unwrap_or_else(|error| panic!("Không đặt được mtime {}: {error}", path.display()));
    }

    #[test]
    fn sweep_leaves_fresh_temp_dir_and_removes_old_one() {
        let base = std::env::temp_dir().join(format!(
            "ghita_sweep_age_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();

        let fresh = base.join(".ghita_temp_fresh_run");
        std::fs::create_dir_all(&fresh).unwrap();
        std::fs::write(fresh.join("in_progress.tmp"), b"x").unwrap();

        let old = base.join(".ghita_temp_stale_run");
        std::fs::create_dir_all(&old).unwrap();
        let old_time = SystemTime::now() - Duration::from_secs(60 * 60);
        set_dir_mtime(&old, old_time);

        let removed = sweep_stale_temp_dirs(&base).unwrap();
        assert_eq!(removed, 1, "chỉ thư mục cũ bị dọn");
        assert!(fresh.exists(), "thư mục mới phải được giữ nguyên");
        assert!(!old.exists(), "thư mục quá 30 phút phải bị xóa");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn sweep_missing_dir_returns_zero() {
        let missing = std::env::temp_dir().join(format!(
            "ghita_sweep_age_missing_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&missing);
        assert_eq!(sweep_stale_temp_dirs(&missing).unwrap(), 0);
    }
}
