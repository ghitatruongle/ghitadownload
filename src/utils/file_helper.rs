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

pub fn build_output_filename(dir: &Path, title: &str, ext: &str, keep_accents: bool) -> PathBuf {
    dir.join(format!(
        "{}.{}",
        sanitize_name_with_options(title, keep_accents),
        ext
    ))
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
    use super::{sanitize_name, sanitize_name_with_options, sweep_stale_temp_dirs};
    use proptest::prelude::*;
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    const RESERVED_WINDOWS_STEMS: &[&str] = &[
        "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
        "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
    ];

    fn assert_sanitize_invariants(output: &str) {
        assert!(
            !output.is_empty(),
            "sanitize must never return an empty name, input produced {output:?}"
        );
        assert!(
            output.len() <= 255,
            "sanitize output must stay within 255 bytes, got {} bytes: {output:?}",
            output.len()
        );
        for character in output.chars() {
            assert!(
                !matches!(
                    character,
                    '/' | '\\' | '?' | '<' | '>' | ':' | '*' | '|' | '"'
                ),
                "path separator or illegal Windows character {character:?} survived sanitize: {output:?}"
            );
            assert!(
                (character as u32) > 0x1f,
                "control character {character:?} survived sanitize: {output:?}"
            );
            assert!(
                !(0x80..=0x9f).contains(&(character as u32)),
                "C1 control character {character:?} survived sanitize: {output:?}"
            );
        }
        assert!(
            !output.starts_with('.') && !output.ends_with('.'),
            "leading/trailing dots survived sanitize: {output:?}"
        );
        assert!(
            !output.ends_with(' '),
            "trailing space survived sanitize: {output:?}"
        );
    }

    fn nasty_inputs() -> Vec<String> {
        vec![
            String::new(),
            "   ".to_string(),
            "\t".to_string(),
            "\n".to_string(),
            ".".to_string(),
            "..".to_string(),
            "...".to_string(),
            " .... ".to_string(),
            "../..".to_string(),
            "../../etc/passwd".to_string(),
            "..\\..\\windows\\system32\\cmd.exe".to_string(),
            "/absolute/path/to/file.mp3".to_string(),
            "C:\\Windows\\system32\\config.sys".to_string(),
            "C:/Users/Public/..\\..\\secret".to_string(),
            "relative/path/to/some/dir".to_string(),
            "a/b\\c:d".to_string(),
            "a|b<c>d:e\"f?g*h".to_string(),
            "semi;colon.js".to_string(),
            "quote\"and'pipe|".to_string(),
            "\u{0000}a\u{0007}b".to_string(),
            "a\nb\rc".to_string(),
            "a\tb".to_string(),
            "\u{0085}soft\u{009f}break".to_string(),
            "\u{007f}del".to_string(),
            "\u{00ad}hidden\u{00a0}".to_string(),
            "\u{feff}bom\u{2028}separator".to_string(),
            "🎵🎶 Bài Hát 🎶🎵".to_string(),
            "中文テスト файл".to_string(),
            "Nắng Thơ (Official MV).mp3".to_string(),
            "CON".to_string(),
            "con".to_string(),
            "CoM3".to_string(),
            "PRN".to_string(),
            "aux".to_string(),
            "nul".to_string(),
            "com1.txt".to_string(),
            "lpt9.doc".to_string(),
            "con .".to_string(),
            "report?.mp4".to_string(),
            "track name....".to_string(),
            "track name   ".to_string(),
            "...hidden".to_string(),
            "a".repeat(500),
            " Stream Target (- Topic) ".to_string(),
        ]
    }

    #[test]
    fn sanitize_name_holds_invariants_across_adversarial_inputs() {
        for input in nasty_inputs() {
            let output = sanitize_name(&input);
            assert_sanitize_invariants(&output);
        }
    }

    #[test]
    fn sanitize_name_handles_adversarial_inputs_deterministically() {
        assert_eq!(sanitize_name(""), "audio_track");
        assert_eq!(sanitize_name("   "), "_");
        assert_eq!(sanitize_name("."), "_");
        assert_eq!(sanitize_name(".."), "_");
        assert_eq!(sanitize_name("..."), "_");
        assert_eq!(sanitize_name("CON"), "_");
        assert_eq!(sanitize_name("con"), "_");
        assert_eq!(sanitize_name("CoM3"), "_");
        assert_eq!(sanitize_name("PRN"), "_");
        assert_eq!(sanitize_name("aux"), "_");
        assert_eq!(sanitize_name("nul"), "_");
        assert_eq!(sanitize_name("com1.txt"), "_");
        assert_eq!(sanitize_name("lpt9.doc"), "_");
        assert_eq!(sanitize_name("con ."), "con_");
        assert_eq!(sanitize_name("\u{0000}a\u{0007}b"), "_a_b");
        assert_eq!(sanitize_name("a|b<c>d:e\"f?g*h"), "a_b_c_d_e_f_g_h");
        assert_eq!(sanitize_name("a/b\\c:d"), "a_b_c_d");
        assert_eq!(sanitize_name("a\nb\rc"), "a_b_c");
        assert_eq!(sanitize_name("a\tb"), "a_b");
        assert_eq!(sanitize_name("...hidden"), "hidden");
        assert_eq!(sanitize_name("track name...."), "track name_");
        assert_eq!(sanitize_name("track name   "), "track name_");
        assert_eq!(sanitize_name(&"a".repeat(500)), "a".repeat(255));
        assert!(sanitize_name("🎵🎶 Bài Hát 🎶🎵").contains("Bai Hat"));
        assert_eq!(sanitize_name_with_options("Bài Hát", true), "Bài Hát");
        for stem in RESERVED_WINDOWS_STEMS {
            assert_eq!(
                sanitize_name(stem),
                "_",
                "reserved device name {stem} must be neutralized"
            );
        }
        for input in [
            "../..",
            "../../etc/passwd",
            "..\\..\\windows\\system32\\cmd.exe",
        ] {
            let output = sanitize_name(input);
            assert!(
                !output.contains('/') && !output.contains('\\'),
                "path traversal input {input:?} must not survive as a multi-component path: {output:?}"
            );
        }
    }

    proptest! {
        #[test]
        fn sanitize_name_preserves_single_component_invariants(input in any::<String>()) {
            let output = sanitize_name(&input);
            assert_sanitize_invariants(&output);
        }

        #[test]
        fn sanitize_name_keep_accents_preserves_single_component_invariants(input in any::<String>()) {
            let output = sanitize_name_with_options(&input, true);
            assert_sanitize_invariants(&output);
        }
    }

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

pub const STALE_PART_MAX_AGE_SECS: u64 = 24 * 60 * 60;

pub fn remove_stale_part_files(dir: &Path, max_age: std::time::Duration) -> usize {
    use std::time::SystemTime;
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return 0,
    };
    let cutoff = SystemTime::now()
        .checked_sub(max_age)
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let mut removed = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let is_part = path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("part"));
        if !is_part {
            continue;
        }
        let fresh = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .map(|modified| modified > cutoff)
            .unwrap_or(true);
        if !fresh && std::fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

#[cfg(test)]
mod part_tests {
    use super::remove_stale_part_files;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ghita_part_test_{label}_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn set_old(path: &Path, secs: u64) {
        let file = fs::OpenOptions::new().append(true).open(path).unwrap();
        let _ = file.set_modified(SystemTime::now() - Duration::from_secs(secs));
    }

    #[test]
    fn removes_only_stale_part_files() {
        let dir = temp_dir("stale");
        let stale = dir.join("song.mp3.part");
        let fresh = dir.join("other.flac.part");
        let untouched = dir.join("keep.txt");
        fs::write(&stale, b"partial").unwrap();
        fs::write(&fresh, b"partial").unwrap();
        fs::write(&untouched, b"data").unwrap();
        set_old(&stale, 25 * 60 * 60);

        let removed = remove_stale_part_files(&dir, Duration::from_secs(24 * 60 * 60));

        assert_eq!(removed, 1);
        assert!(!stale.exists());
        assert!(fresh.exists());
        assert!(untouched.exists());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn empty_or_missing_dir_is_noop() {
        assert_eq!(
            remove_stale_part_files(&temp_dir("missing"), Duration::from_secs(3600)),
            0
        );
    }
}

#[cfg(test)]
mod dup_tests {
    use super::build_output_filename;

    #[test]
    fn build_output_filename_sanitizes_and_appends_ext() {
        let dir = std::env::temp_dir();
        let path = build_output_filename(&dir, "Bài Hát: Tự Nhiên", "mp3", false);
        assert_eq!(path, dir.join("Bai Hat_ Tu Nhien.mp3"));
        let path2 = build_output_filename(&dir, "Bài Hát: Tự Nhiên", "mp3", true);
        assert_eq!(path2, dir.join("Bài Hát_ Tự Nhiên.mp3"));
    }
}

#[cfg(windows)]
pub fn cookie_acl_warning(path: &std::path::Path) -> Option<String> {
    use windows_sys::Win32::Security::{
        GetAce, GetFileSecurityW, GetSecurityDescriptorDacl, ACCESS_ALLOWED_ACE,
        DACL_SECURITY_INFORMATION,
    };
    const GENERIC_READ: u32 = 0x8000_0000;
    const FILE_GENERIC_READ: u32 = 0x0012_0089;
    const EVERYONE_SID_BYTES: [u8; 12] = [1, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0];
    const ACE_TYPE_ACCESS_ALLOWED: u8 = 0;
    let wide: Vec<u16> = path
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut needed = 0u32;
    unsafe {
        GetFileSecurityW(
            wide.as_ptr(),
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            0,
            &mut needed,
        );
    };
    if needed == 0 {
        return None;
    }
    let mut descriptor_buf: Vec<u8> = vec![0u8; needed as usize];
    let ok = unsafe {
        GetFileSecurityW(
            wide.as_ptr(),
            DACL_SECURITY_INFORMATION,
            descriptor_buf.as_mut_ptr() as *mut core::ffi::c_void,
            needed,
            &mut needed,
        )
    };
    if ok == 0 {
        return None;
    }
    let descriptor = descriptor_buf.as_ptr() as *const core::ffi::c_void;
    let mut dacl: *mut windows_sys::Win32::Security::ACL = std::ptr::null_mut();
    let mut dacl_present = 0i32;
    let mut dacl_defaulted = 0i32;
    let dacl_ok = unsafe {
        GetSecurityDescriptorDacl(
            descriptor as *mut core::ffi::c_void,
            &mut dacl_present,
            &mut dacl,
            &mut dacl_defaulted,
        )
    };
    if dacl_ok == 0 || dacl_present == 0 || dacl.is_null() {
        return None;
    }
    let dacl_typed = dacl;
    let mut found = false;
    let mut index = 0u32;
    while index < unsafe { (*dacl_typed).AceCount } as u32 {
        let mut ace: *mut core::ffi::c_void = std::ptr::null_mut();
        if unsafe { GetAce(dacl_typed, index, &mut ace) } == 0 {
            break;
        }
        let allowed = unsafe { &*(ace as *const ACCESS_ALLOWED_ACE) };
        let readable = allowed.Mask & (GENERIC_READ | FILE_GENERIC_READ) != 0;
        let is_allow = allowed.Header.AceType == ACE_TYPE_ACCESS_ALLOWED;
        let big_enough = allowed.Header.AceSize as usize >= 8 + 12;
        if is_allow && readable && big_enough {
            let sid_bytes = unsafe {
                core::slice::from_raw_parts(&allowed.SidStart as *const u32 as *const u8, 12)
            };
            if sid_bytes == EVERYONE_SID_BYTES {
                found = true;
                break;
            }
        }
        index += 1;
    }
    found.then(|| {
        "⚠️  File cookie có quyền đọc cho nhóm Everyone (ACL) — người dùng khác trên máy có thể đọc được. "
            .to_string()
            + "Hãy đặt file vào thư mục chỉ mình bạn đọc được, hoặc dùng --cookies-from-browser."
    })
}

#[cfg(not(windows))]
pub fn cookie_acl_warning(_path: &std::path::Path) -> Option<String> {
    None
}
