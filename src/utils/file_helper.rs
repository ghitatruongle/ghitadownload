use std::path::{Path, PathBuf};

pub fn remove_vietnamese_accents(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    for c in input.chars() {
        if ('\u{0300}'..='\u{036f}').contains(&c) {
            continue;
        }

        let unaccented = match c {
            'à' | 'á' | 'ả' | 'ã' | 'ạ' | 'ă' | 'ằ' | 'ắ' | 'ẳ' | 'ẵ' | 'ặ' | 'â' | 'ầ' | 'ấ' | 'ẩ' | 'ẫ' | 'ậ' | 'ä' | 'å' | 'ā' => 'a',
            'À' | 'Á' | 'Ả' | 'Ã' | 'Ạ' | 'Ă' | 'Ằ' | 'Ắ' | 'Ẳ' | 'Ẵ' | 'Ặ' | 'Â' | 'Ầ' | 'Ấ' | 'Ẩ' | 'Ẫ' | 'Ậ' | 'Ä' | 'Å' | 'Ā' => 'A',
            'đ' => 'd',
            'Đ' | 'Ð' => 'D',
            'è' | 'é' | 'ẻ' | 'ẽ' | 'ẹ' | 'ê' | 'ề' | 'ế' | 'ể' | 'ễ' | 'ệ' | 'ë' | 'ē' => 'e',
            'È' | 'É' | 'Ẻ' | 'Ẽ' | 'Ẹ' | 'Ê' | 'Ề' | 'Ế' | 'Ể' | 'Ễ' | 'Ệ' | 'Ë' | 'Ē' => 'E',
            'ì' | 'í' | 'ỉ' | 'ĩ' | 'ị' | 'ï' | 'î' | 'ī' => 'i',
            'Ì' | 'Í' | 'Ỉ' | 'Ĩ' | 'Ị' | 'Ï' | 'Î' | 'Ī' => 'I',
            'ò' | 'ó' | 'ỏ' | 'õ' | 'ọ' | 'ô' | 'ồ' | 'ố' | 'ổ' | 'ỗ' | 'ộ' | 'ơ' | 'ờ' | 'ớ' | 'ở' | 'ỡ' | 'ợ' | 'ö' | 'ø' | 'ō' => 'o',
            'Ò' | 'Ó' | 'Ỏ' | 'Õ' | 'Ọ' | 'Ô' | 'Ồ' | 'Ố' | 'Ổ' | 'Ỗ' | 'Ộ' | 'Ơ' | 'Ờ' | 'Ớ' | 'Ở' | 'Ỡ' | 'Ợ' | 'Ö' | 'Ø' | 'Ō' => 'O',
            'ù' | 'ú' | 'ủ' | 'ũ' | 'ụ' | 'ư' | 'ừ' | 'ứ' | 'ử' | 'ữ' | 'ự' | 'ü' | 'ū' => 'u',
            'Ù' | 'Ú' | 'Ủ' | 'Ũ' | 'Ụ' | 'Ư' | 'Ừ' | 'Ứ' | 'Ử' | 'Ữ' | 'Ự' | 'Ü' | 'Ū' => 'U',
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
    let unaccented = remove_vietnamese_accents(name);
    let sanitized = sanitize_filename::sanitize_with_options(
        &unaccented,
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

pub fn ensure_unique_path(dir: &Path, base_name: &str, ext: &str) -> PathBuf {
    let clean_dir = clean_path(dir);
    let clean_base = sanitize_name(base_name);
    let mut candidate = clean_dir.join(format!("{}.{}", clean_base, ext));
    let mut counter = 1;

    while candidate.exists() {
        candidate = clean_dir.join(format!("{} ({}).{}", clean_base, counter, ext));
        counter += 1;
    }

    candidate
}

pub fn ensure_dir(path: &Path) -> std::io::Result<PathBuf> {
    let cleaned = clean_path(path);
    if !cleaned.exists() {
        std::fs::create_dir_all(&cleaned)?;
    }
    Ok(cleaned)
}
