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
pub fn ensure_unique_path(dir: &Path, base_name: &str, ext: &str) -> PathBuf {
    ensure_unique_path_with_options(dir, base_name, ext, false)
}

pub fn ensure_unique_path_with_options(
    dir: &Path,
    base_name: &str,
    ext: &str,
    keep_accents: bool,
) -> PathBuf {
    let clean_dir = clean_path(dir);
    let _ = std::fs::create_dir_all(&clean_dir);
    let clean_base = sanitize_name_with_options(base_name, keep_accents);
    let mut candidate = clean_dir.join(format!("{}.{}", clean_base, ext));
    let mut counter = 1u32;

    loop {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(_) => return candidate,
            Err(e) => {
                if counter >= 10_000 || e.kind() == std::io::ErrorKind::PermissionDenied {
                    return candidate;
                }
                candidate = clean_dir.join(format!("{} ({}).{}", clean_base, counter, ext));
                counter += 1;
            }
        }
    }
}

pub fn ensure_dir(path: &Path) -> std::io::Result<PathBuf> {
    let cleaned = clean_path(path);
    if !cleaned.exists() {
        std::fs::create_dir_all(&cleaned)?;
    }
    Ok(cleaned)
}
