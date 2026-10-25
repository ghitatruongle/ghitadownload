use anyhow::{anyhow, bail, Context, Result};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

pub const YTDLP_PINNED_VERSION: &str = "2026.08.19";
const YTDLP_FALLBACK_URL: &str =
    "https://github.com/yt-dlp/yt-dlp/releases/download/2026.08.19/yt-dlp.exe";
const YTDLP_FALLBACK_SHA256: &str =
    "66674953fe251b89f4d08c5f0e35e0728679bd67ab3d7d05c0562af101dd3e7a";
const YTDLP_LATEST_RELEASE_API: &str = "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest";
const FFMPEG_FALLBACK_URL: &str = "https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-10-07-13-07/ffmpeg-N-127233-g452820cba6-win64-gpl.zip";
const FFMPEG_FALLBACK_SHA256: &str =
    "b85b6ddaf7c8bc58c85d336583c949f8792623431a28bdaea52aa832fd986a59";

pub fn is_valid_windows_pe(path: &Path) -> bool {
    let mut header = [0_u8; 2];
    match std::fs::File::open(path)
        .and_then(|mut file| std::io::Read::read_exact(&mut file, &mut header))
    {
        Ok(()) => {
            header == [0x4d, 0x5a] && std::fs::metadata(path).is_ok_and(|data| data.len() > 100)
        }
        Err(_) => false,
    }
}

pub fn strip_verbatim_prefix(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{stripped}"))
    } else if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

fn absolute_existing_file(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() || !path.is_file() {
        return None;
    }
    let canonical = path.canonicalize().ok()?;
    let stripped = strip_verbatim_prefix(&canonical);
    stripped.is_file().then_some(stripped)
}

fn tool_candidates(file_name: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            candidates.push(parent.join(file_name));
            candidates.push(parent.join("bin").join(file_name));
            candidates.push(parent.join("..").join("..").join("bin").join(file_name));
        }
    }
    if let Some(dir) = dirs::data_local_dir() {
        candidates.push(dir.join("GhitaDownload").join(file_name));
        candidates.push(dir.join("GhitaDownload").join("bin").join(file_name));
    }
    candidates
}

fn command_file_name(command: &str) -> &str {
    if command.ends_with(".exe") {
        command
    } else {
        match command {
            "yt-dlp" => "yt-dlp.exe",
            "ffmpeg" => "ffmpeg.exe",
            "node" => "node.exe",
            "winget" => "winget.exe",
            "powershell" => "powershell.exe",
            _ => command,
        }
    }
}

pub fn find_command(command: &str) -> Option<PathBuf> {
    let file_name = command_file_name(command);
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(file_name))
        .find_map(|candidate| absolute_existing_file(&candidate))
}

pub fn is_command_available(command: &str) -> bool {
    let Some(path) = find_command(command) else {
        return false;
    };
    tool_version(&path).is_ok()
}

fn validate_tool(path: &Path, label: &str) -> Result<PathBuf> {
    let path = absolute_existing_file(path)
        .with_context(|| format!("Không tìm thấy {label} hợp lệ tại {}", path.display()))?;
    if !is_valid_windows_pe(&path) {
        bail!(
            "Tệp {label} không phải Windows PE hợp lệ: {}",
            path.display()
        );
    }
    Ok(path)
}

pub fn find_ytdlp() -> Option<PathBuf> {
    tool_candidates("yt-dlp.exe")
        .iter()
        .find_map(|candidate| validate_tool(candidate, "yt-dlp").ok())
        .or_else(|| find_command("yt-dlp"))
}

pub fn ytdlp_update_path() -> PathBuf {
    find_ytdlp().unwrap_or_else(|| {
        tool_candidates("yt-dlp.exe")
            .into_iter()
            .next()
            .unwrap_or_else(|| PathBuf::from("yt-dlp.exe"))
    })
}

pub fn find_ffmpeg() -> Option<PathBuf> {
    tool_candidates("ffmpeg.exe")
        .iter()
        .find_map(|candidate| validate_tool(candidate, "FFmpeg").ok())
        .or_else(|| find_command("ffmpeg"))
        .or_else(|| {
            [
                PathBuf::from(r"C:\ffmpeg\bin\ffmpeg.exe"),
                PathBuf::from(r"C:\Program Files\ffmpeg\bin\ffmpeg.exe"),
            ]
            .iter()
            .find_map(|candidate| validate_tool(candidate, "FFmpeg").ok())
        })
}

pub fn has_node() -> bool {
    is_command_available("node")
}

fn unique_path(directory: &Path, file_name: &str, label: &str) -> Result<PathBuf> {
    std::fs::create_dir_all(directory)
        .with_context(|| format!("Không thể tạo thư mục tải tạm: {}", directory.display()))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    Ok(directory.join(format!(
        ".{file_name}.{label}.{}.{}.download",
        std::process::id(),
        stamp
    )))
}

fn powershell_path() -> Result<PathBuf> {
    find_command("powershell")
        .or_else(|| {
            std::env::var_os("SystemRoot").map(|root| {
                PathBuf::from(root)
                    .join("System32")
                    .join("WindowsPowerShell")
                    .join("v1.0")
                    .join("powershell.exe")
            })
        })
        .and_then(|path| absolute_existing_file(&path))
        .ok_or_else(|| anyhow!("Không tìm thấy PowerShell để tải công cụ phụ thuộc"))
}

fn ps_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn download_file(url: &str, destination: &Path) -> Result<Output> {
    let clean_dest = strip_verbatim_prefix(destination);
    let powershell = powershell_path()?;
    let script = format!(
        "$ErrorActionPreference='Stop'; $ProgressPreference='SilentlyContinue'; Invoke-WebRequest -UseBasicParsing -Uri {} -OutFile {}",
        ps_quote(url),
        ps_quote(&clean_dest.to_string_lossy())
    );
    let output = Command::new(powershell)
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .with_context(|| format!("Không thể chạy trình tải từ {url}"))?;
    if !output.status.success() {
        bail!(
            "Tải thất bại với HTTP/client status {}{}",
            output.status,
            process_error(&output)
        );
    }
    Ok(output)
}

fn process_error(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = if stderr.trim().is_empty() {
        stdout.trim()
    } else {
        stderr.trim()
    };
    if detail.is_empty() {
        String::new()
    } else {
        format!(": {detail}")
    }
}

fn file_sha256(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("Không thể mở tệp để tính SHA-256: {}", path.display()))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)
        .with_context(|| format!("Không thể đọc tệp để tính SHA-256: {}", path.display()))?;
    let hash = format!("{:064x}", hasher.finalize());
    if hash.len() != 64 || !hash.chars().all(|value| value.is_ascii_hexdigit()) {
        bail!("SHA-256 không hợp lệ cho {}", path.display());
    }
    Ok(hash)
}

fn verify_sha256(path: &Path, expected: &str) -> Result<()> {
    let expected = expected.trim().to_ascii_lowercase();
    let actual = file_sha256(path)?;
    if actual != expected {
        bail!(
            "SHA-256 không khớp cho {}: expected {expected}, got {actual}",
            path.display()
        );
    }
    Ok(())
}

fn tool_version(path: &Path) -> Result<String> {
    let mut output = Command::new(path)
        .arg("--version")
        .output()
        .with_context(|| format!("Không thể chạy {}", path.display()))?;
    if !output.status.success() {
        output = Command::new(path)
            .arg("-version")
            .output()
            .with_context(|| format!("Không thể chạy {}", path.display()))?;
    }
    if !output.status.success() {
        bail!(
            "Kiểm tra phiên bản thất bại{}{}",
            output.status,
            process_error(&output)
        );
    }
    let version = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    if version.is_empty() {
        bail!("{} không trả về phiên bản hợp lệ", path.display());
    }
    Ok(version)
}

fn configured_url(
    runtime: Option<OsString>,
    compile_time: Option<&'static str>,
    fallback: &str,
) -> String {
    runtime
        .and_then(|value| value.into_string().ok())
        .filter(|value| value.starts_with("https://"))
        .or_else(|| compile_time.map(str::to_string))
        .unwrap_or_else(|| fallback.to_string())
}

pub fn install_ffmpeg_auto() -> Result<PathBuf> {
    if let Some(path) = find_ffmpeg() {
        return Ok(path);
    }

    let target_dir = tool_candidates("ffmpeg.exe")
        .into_iter()
        .filter_map(|candidate| candidate.parent().map(Path::to_path_buf))
        .find(|candidate| candidate.join("yt-dlp.exe").is_file())
        .unwrap_or_else(|| {
            dirs::data_local_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join("GhitaDownload")
                .join("bin")
        });
    let target_dir = strip_verbatim_prefix(&target_dir);
    let target = target_dir.join("ffmpeg.exe");
    let zip =
        strip_verbatim_prefix(&unique_path(&target_dir, "ffmpeg", "ffmpeg")?.with_extension("zip"));
    let extracted = strip_verbatim_prefix(&unique_path(&target_dir, "ffmpeg", "ffmpeg-extract")?);
    let downloaded = strip_verbatim_prefix(
        &unique_path(&target_dir, "ffmpeg", "ffmpeg-bin")?.with_extension("exe"),
    );
    let url = std::env::var("GHITA_FFMPEG_SOURCE_URL")
        .ok()
        .filter(|value| value.starts_with("https://"))
        .unwrap_or_else(|| FFMPEG_FALLBACK_URL.to_string());
    let expected_sha256 = std::env::var("GHITA_FFMPEG_SHA256")
        .ok()
        .filter(|value| value.len() == 64)
        .unwrap_or_else(|| FFMPEG_FALLBACK_SHA256.to_string());
    let result = (|| -> Result<PathBuf> {
        download_file(&url, &zip)?;
        verify_sha256(&zip, &expected_sha256)?;
        if std::fs::metadata(&zip).is_ok_and(|metadata| metadata.len() <= 100) {
            bail!("Tệp FFmpeg tải về quá nhỏ hoặc không hợp lệ");
        }
        let powershell = powershell_path()?;
        let expand_script = format!(
            "$ErrorActionPreference='Stop'; Expand-Archive -LiteralPath {} -DestinationPath {} -Force; $bin = Get-ChildItem -LiteralPath {} -Recurse -Filter 'ffmpeg.exe' | Select-Object -First 1; if ($null -eq $bin) {{ throw 'ffmpeg.exe not found in archive' }}; Copy-Item -LiteralPath $bin.FullName -Destination {} -Force",
            ps_quote(&zip.to_string_lossy()),
            ps_quote(&extracted.to_string_lossy()),
            ps_quote(&extracted.to_string_lossy()),
            ps_quote(&downloaded.to_string_lossy())
        );
        let output = Command::new(powershell)
            .args(["-NoProfile", "-NonInteractive", "-Command", &expand_script])
            .output()
            .context("Không thể giải nén gói FFmpeg")?;
        if !output.status.success() {
            bail!(
                "Giải nén FFmpeg thất bại{}{}",
                output.status,
                process_error(&output)
            );
        }
        validate_tool(&downloaded, "FFmpeg")?;
        tool_version(&downloaded)?;
        crate::utils::fs::replace_file(&downloaded, &target)
            .with_context(|| format!("Không thể cài {}", target.display()))?;
        Ok(target)
    })();
    let _ = std::fs::remove_file(&zip);
    let _ = std::fs::remove_file(&downloaded);
    if extracted.exists() {
        let _ = std::fs::remove_dir_all(&extracted);
    }
    let path = result?;
    tool_version(&path)?;
    Ok(path)
}

pub fn update_ytdlp(ytdlp_path: &Path) -> Result<String> {
    let clean_path = strip_verbatim_prefix(ytdlp_path);
    if !clean_path.is_absolute() {
        bail!("Đường dẫn cập nhật yt-dlp phải là đường dẫn tuyệt đối");
    }
    if let Ok(output) = Command::new(&clean_path).arg("--update").output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let version = tool_version(&clean_path).unwrap_or_else(|_| "mới nhất".to_string());
            let summary = stdout
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .unwrap_or("yt-dlp đã được cập nhật thành công.");
            return Ok(format!("{summary} (Phiên bản: {version})"));
        }
    }
    let parent = clean_path.parent().ok_or_else(|| {
        anyhow!(
            "Không xác định được thư mục cài đặt yt-dlp: {}",
            clean_path.display()
        )
    })?;
    let temporary = unique_path(parent, "yt-dlp", "yt-dlp")?.with_extension("exe");
    let temporary = strip_verbatim_prefix(&temporary);
    let url = configured_url(
        std::env::var_os("GHITA_YTDLP_SOURCE_URL"),
        option_env!("GHITA_YTDLP_SOURCE_URL"),
        YTDLP_FALLBACK_URL,
    );
    let expected_sha256 = std::env::var("GHITA_YTDLP_SHA256")
        .ok()
        .filter(|value| value.len() == 64)
        .unwrap_or_else(|| YTDLP_FALLBACK_SHA256.to_string());
    let result = (|| -> Result<String> {
        download_file(&url, &temporary)?;
        verify_sha256(&temporary, &expected_sha256)?;
        validate_tool(&temporary, "yt-dlp tải về")?;
        let version = tool_version(&temporary)?;
        crate::utils::fs::replace_file(&temporary, &clean_path)
            .with_context(|| format!("Không thể cài {}", clean_path.display()))?;
        Ok(version)
    })();
    let _ = std::fs::remove_file(&temporary);
    let version = result?;
    validate_tool(&clean_path, "yt-dlp")?;
    Ok(format!(
        "Đã cập nhật yt-dlp {} từ nguồn đã cấu hình.",
        version
    ))
}

pub fn parse_github_latest_tag(payload: &str) -> Result<String> {
    let value: serde_json::Value = serde_json::from_str(payload)
        .with_context(|| "Phản hồi GitHub Releases không phải JSON hợp lệ")?;
    value
        .get("tag_name")
        .and_then(|tag| tag.as_str())
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("JSON GitHub Releases thiếu trường tag_name"))
}

pub fn version_is_newer(candidate: &str, baseline: &str) -> bool {
    let parse_parts = |version: &str| -> Option<Vec<u64>> {
        version
            .trim()
            .trim_start_matches('v')
            .split('.')
            .map(|part| part.parse::<u64>().ok())
            .collect()
    };
    let (Some(candidate_parts), Some(baseline_parts)) =
        (parse_parts(candidate), parse_parts(baseline))
    else {
        let candidate = candidate.trim();
        let baseline = baseline.trim();
        return candidate != baseline && candidate > baseline;
    };
    for index in 0..candidate_parts.len().max(baseline_parts.len()) {
        let left = candidate_parts.get(index).copied().unwrap_or(0);
        let right = baseline_parts.get(index).copied().unwrap_or(0);
        if left != right {
            return left > right;
        }
    }
    false
}

pub async fn fetch_latest_ytdlp_version() -> Result<String> {
    let client = crate::utils::http::shared_client()?;
    let payload = crate::utils::http::with_retry(|| async {
        let response = client
            .get(YTDLP_LATEST_RELEASE_API)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await?
            .error_for_status()?;
        Ok(response.text().await?)
    })
    .await?;
    parse_github_latest_tag(&payload)
}

#[cfg(test)]
mod tests {
    use super::{
        parse_github_latest_tag, version_is_newer, FFMPEG_FALLBACK_SHA256, FFMPEG_FALLBACK_URL,
        YTDLP_FALLBACK_SHA256, YTDLP_PINNED_VERSION,
    };

    #[test]
    fn ps_quote_escapes_single_quotes_for_inline_scripts() {
        use super::ps_quote;
        assert_eq!(
            ps_quote("https://example.com/a.zip"),
            "'https://example.com/a.zip'"
        );
        assert_eq!(ps_quote("it's here"), "'it''s here'");
        assert_eq!(ps_quote(""), "''");
    }

    #[test]
    fn parse_github_latest_tag_reads_top_level_tag_name() {
        let payload =
            r#"{"url":"https://api.github.com/x","tag_name":"2026.09.27","name":"yt-dlp"}"#;
        assert_eq!(
            parse_github_latest_tag(payload).unwrap(),
            "2026.09.27".to_string()
        );
    }

    #[test]
    fn parse_github_latest_tag_rejects_missing_or_invalid_payload() {
        assert!(parse_github_latest_tag(r#"{"message":"Not Found"}"#).is_err());
        assert!(parse_github_latest_tag("not json at all").is_err());
        assert!(parse_github_latest_tag(r#"{"tag_name":""}"#).is_err());
    }

    #[test]
    fn version_is_newer_compares_numeric_segments() {
        assert!(version_is_newer("2026.09.27", "2026.08.19"));
        assert!(version_is_newer("2027.01.01", "2026.12.31"));
        assert!(!version_is_newer("2026.08.19", "2026.08.19"));
        assert!(!version_is_newer("2026.08.18", "2026.08.19"));
        assert!(version_is_newer("2026.08.19.1", "2026.08.19"));
        assert!(!version_is_newer("2026.08.19", "2026.08.19.1"));
        assert!(version_is_newer("2026.9.2", "2026.08.19"));
        assert!(version_is_newer("v2026.09.27", "2026.08.19"));
    }

    #[test]
    fn version_is_newer_falls_back_to_string_compare_for_non_numeric_tags() {
        assert!(version_is_newer("nightly-2", "nightly-1"));
        assert!(!version_is_newer("nightly-1", "nightly-1"));
    }

    #[test]
    fn pinned_constants_are_well_formed() {
        assert_eq!(YTDLP_PINNED_VERSION, "2026.08.19");
        assert_eq!(YTDLP_FALLBACK_SHA256.len(), 64);
        assert!(YTDLP_FALLBACK_SHA256.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(FFMPEG_FALLBACK_SHA256.len(), 64);
        assert!(FFMPEG_FALLBACK_SHA256
            .chars()
            .all(|c| c.is_ascii_hexdigit()));
        assert!(FFMPEG_FALLBACK_URL.starts_with("https://"));
        assert!(FFMPEG_FALLBACK_URL.contains("autobuild-2026-10-07-13-07"));
    }

    #[test]
    fn strip_verbatim_prefix_normalizes_paths() {
        use super::strip_verbatim_prefix;
        use std::path::{Path, PathBuf};

        assert_eq!(
            strip_verbatim_prefix(Path::new(r"\\?\C:\Users\Acer\bin\tool.exe")),
            PathBuf::from(r"C:\Users\Acer\bin\tool.exe")
        );
        assert_eq!(
            strip_verbatim_prefix(Path::new(r"\\?\UNC\server\share\tool.exe")),
            PathBuf::from(r"\\server\share\tool.exe")
        );
        assert_eq!(
            strip_verbatim_prefix(Path::new(r"C:\Users\Acer\bin\tool.exe")),
            PathBuf::from(r"C:\Users\Acer\bin\tool.exe")
        );
    }
}
