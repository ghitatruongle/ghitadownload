use anyhow::{anyhow, bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::utils::process::hide_console_window;
use std::time::{SystemTime, UNIX_EPOCH};

pub const YTDLP_PINNED_VERSION: &str = "2026.10.26";
const YTDLP_FALLBACK_URL: &str =
    "https://github.com/yt-dlp/yt-dlp/releases/download/2026.10.26/yt-dlp.exe";
const YTDLP_FALLBACK_SHA256: &str =
    "f5389d3698ecde4ef06f4e6b1ce0294fe9ac1396228c8990b8d3416d6d177a7d";
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
            "ffprobe" => "ffprobe.exe",
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

pub fn find_ffprobe() -> Option<PathBuf> {
    tool_candidates("ffprobe.exe")
        .iter()
        .find_map(|candidate| validate_tool(candidate, "ffprobe").ok())
        .or_else(|| find_command("ffprobe"))
        .or_else(|| {
            [
                PathBuf::from(r"C:\ffmpeg\bin\ffprobe.exe"),
                PathBuf::from(r"C:\Program Files\ffmpeg\bin\ffprobe.exe"),
            ]
            .iter()
            .find_map(|candidate| validate_tool(candidate, "ffprobe").ok())
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

pub async fn download_file_stream(url: &str, destination: &Path) -> Result<()> {
    let client = crate::utils::http::download_client()?;
    let mut response = crate::utils::http::with_retry(|| async {
        let response = client.get(url).send().await?.error_for_status()?;
        Ok(response)
    })
    .await
    .with_context(|| format!("Không thể kết nối tới {url}"))?;
    let destination = strip_verbatim_prefix(destination);
    let parent = destination.parent().unwrap_or(Path::new(""));
    if parent.as_os_str().is_empty() {
        bail!(
            "Đường dẫn đích tải về không hợp lệ: {}",
            destination.display()
        );
    }
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("download.bin");
    let temporary = strip_verbatim_prefix(&unique_path(parent, file_name, "download")?);
    let write_result = write_stream_to_file(&mut response, &temporary).await;
    if let Err(error) = write_result {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    if let Err(error) = crate::utils::fs::replace_file(&temporary, &destination) {
        let _ = std::fs::remove_file(&temporary);
        bail!(
            "Không thể thay thế {} bằng tệp vừa tải: {error}",
            destination.display()
        );
    }
    Ok(())
}

async fn write_stream_to_file(response: &mut reqwest::Response, temporary: &Path) -> Result<()> {
    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::File::create(temporary)
        .await
        .with_context(|| format!("Không thể tạo tệp tạm: {}", temporary.display()))?;
    let mut pending: Vec<u8> = Vec::with_capacity(64 * 1024);
    while let Some(chunk) = response.chunk().await? {
        pending.extend_from_slice(&chunk);
        if pending.len() >= 64 * 1024 {
            file.write_all(&pending).await?;
            pending.clear();
        }
    }
    if !pending.is_empty() {
        file.write_all(&pending).await?;
    }
    file.flush().await?;
    drop(file);
    if std::fs::metadata(temporary).is_ok_and(|metadata| metadata.len() <= 1000) {
        bail!(
            "Tệp tải về quá nhỏ hoặc không hợp lệ (dưới 1KB): {}",
            temporary.display()
        );
    }
    Ok(())
}

pub(crate) fn block_on_download<F>(future: F) -> Result<F::Output>
where
    F: std::future::Future,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .with_context(|| "Không khởi tạo được runtime tải xuống")?;
    Ok(runtime.block_on(future))
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
    let mut output = hide_console_window(Command::new(path))
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

pub async fn install_ffmpeg_auto() -> Result<PathBuf> {
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
    let override_url = std::env::var("GHITA_FFMPEG_SOURCE_URL")
        .ok()
        .filter(|value| value.starts_with("https://"));
    let override_sha256 = std::env::var("GHITA_FFMPEG_SHA256")
        .ok()
        .filter(|value| value.len() == 64);
    if override_url.is_some() || override_sha256.is_some() {
        println!(
            "⚠️ Đang dùng nguồn FFmpeg ghi đè qua biến môi trường (GHITA_FFMPEG_SOURCE_URL / GHITA_FFMPEG_SHA256). Hãy đảm bảo bạn tin tưởng nguồn này!"
        );
    }
    let url = override_url.unwrap_or_else(|| FFMPEG_FALLBACK_URL.to_string());
    let expected_sha256 = override_sha256.unwrap_or_else(|| FFMPEG_FALLBACK_SHA256.to_string());
    let temp_zip = strip_verbatim_prefix(
        &unique_path(&std::env::temp_dir(), "ffmpeg", "ffmpeg")?.with_extension("zip"),
    );
    let result = download_and_install_ffmpeg(&url, &expected_sha256, &target_dir, &temp_zip).await;
    let _ = std::fs::remove_file(&temp_zip);
    let target = result?;
    tool_version(&target)?;
    Ok(target)
}

async fn download_and_install_ffmpeg(
    url: &str,
    expected_sha256: &str,
    target_dir: &Path,
    temp_zip: &Path,
) -> Result<PathBuf> {
    let client = crate::utils::http::download_client()?;
    let mut response = crate::utils::http::with_retry(|| async {
        let response = client.get(url).send().await?.error_for_status()?;
        Ok(response)
    })
    .await
    .with_context(|| format!("Không thể tải FFmpeg từ {url}"))?;
    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::File::create(temp_zip)
        .await
        .with_context(|| format!("Không thể tạo tệp tạm: {}", temp_zip.display()))?;
    let mut pending: Vec<u8> = Vec::with_capacity(64 * 1024);
    while let Some(chunk) = response.chunk().await? {
        pending.extend_from_slice(&chunk);
        if pending.len() >= 64 * 1024 {
            file.write_all(&pending).await?;
            pending.clear();
        }
    }
    if !pending.is_empty() {
        file.write_all(&pending).await?;
    }
    file.flush().await?;
    drop(file);
    if std::fs::metadata(temp_zip).is_ok_and(|metadata| metadata.len() <= 100) {
        bail!("Tệp FFmpeg tải về quá nhỏ hoặc không hợp lệ");
    }
    verify_zip_sha256(temp_zip, expected_sha256)?;
    std::fs::create_dir_all(target_dir)
        .with_context(|| format!("Không thể tạo thư mục cài FFmpeg: {}", target_dir.display()))?;
    let ffmpeg_target = target_dir.join("ffmpeg.exe");
    let ffprobe_target = target_dir.join("ffprobe.exe");
    extract_ffmpeg_binaries(temp_zip, &ffmpeg_target, &ffprobe_target).await?;
    validate_tool(&ffmpeg_target, "FFmpeg")?;
    validate_tool(&ffprobe_target, "ffprobe")?;
    tool_version(&ffmpeg_target)?;
    tool_version(&ffprobe_target)?;
    Ok(ffmpeg_target)
}

async fn extract_ffmpeg_binaries(
    zip_path: &Path,
    ffmpeg_target: &Path,
    ffprobe_target: &Path,
) -> Result<()> {
    let zip_path = zip_path.to_path_buf();
    let ffmpeg_target = ffmpeg_target.to_path_buf();
    let ffprobe_target = ffprobe_target.to_path_buf();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let zip_file = std::fs::File::open(&zip_path)
            .with_context(|| format!("Không thể mở tệp zip FFmpeg: {}", zip_path.display()))?;
        let mut archive = zip::ZipArchive::new(zip_file)
            .with_context(|| "Không thể đọc cấu trúc tệp zip FFmpeg")?;
        let mut found_ffmpeg = false;
        let mut found_ffprobe = false;
        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .with_context(|| format!("Không thể đọc mục {index} trong zip FFmpeg"))?;
            if entry.is_dir() {
                continue;
            }
            let entry_name = entry
                .name()
                .ok()
                .and_then(|name| {
                    Path::new(name.as_ref())
                        .file_name()
                        .map(|file| file.to_string_lossy().to_ascii_lowercase())
                })
                .unwrap_or_default();
            let destination = match entry_name.as_str() {
                "ffmpeg.exe" => {
                    found_ffmpeg = true;
                    ffmpeg_target.clone()
                }
                "ffprobe.exe" => {
                    found_ffprobe = true;
                    ffprobe_target.clone()
                }
                _ => continue,
            };
            let mut output = std::fs::File::create(&destination)
                .with_context(|| format!("Không thể tạo {}", destination.display()))?;
            std::io::copy(&mut entry, &mut output)
                .with_context(|| format!("Không thể giải nén {}", destination.display()))?;
            output
                .sync_all()
                .with_context(|| format!("Không thể ghi {}", destination.display()))?;
        }
        if !found_ffmpeg {
            bail!("Không tìm thấy ffmpeg.exe trong gói FFmpeg");
        }
        if !found_ffprobe {
            bail!("Không tìm thấy ffprobe.exe trong gói FFmpeg");
        }
        Ok(())
    })
    .await
    .context("Không thể giải nén gói FFmpeg")??;
    Ok(())
}

fn verify_zip_sha256(path: &Path, expected: &str) -> Result<()> {
    use sha2::{Digest, Sha256};
    let expected = expected.trim().to_ascii_lowercase();
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("Không thể mở tệp để tính SHA-256: {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = std::io::Read::read(&mut file, &mut buffer)
            .with_context(|| format!("Không thể đọc tệp để tính SHA-256: {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = format!("{:064x}", hasher.finalize());
    if actual != expected {
        bail!(
            "SHA-256 không khớp cho {}: expected {expected}, got {actual}",
            path.display()
        );
    }
    Ok(())
}

struct YtdlpManifest {
    #[allow(dead_code)]
    version: String,
    url: String,
    sha256: String,
}

fn is_valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit())
}

fn parse_ytdlp_manifest(payload: &str) -> Result<YtdlpManifest> {
    let value: serde_json::Value = serde_json::from_str(payload)
        .with_context(|| "Manifest yt-dlp.json không phải JSON hợp lệ")?;
    let read_field = |name: &str| -> Result<String> {
        value
            .get(name)
            .and_then(|field| field.as_str())
            .map(str::trim)
            .filter(|field| !field.is_empty())
            .map(str::to_string)
            .ok_or_else(|| anyhow!("Manifest yt-dlp.json thiếu hoặc sai trường {name}"))
    };
    let version = read_field("version")?;
    let url = read_field("url")?;
    let sha256 = read_field("sha256")?;
    if !url.starts_with("https://") {
        bail!("Manifest yt-dlp.json có url không phải https://: {url}");
    }
    if !is_valid_sha256(&sha256) {
        bail!("Manifest yt-dlp.json có sha256 không hợp lệ (cần 64 ký tự hex)");
    }
    Ok(YtdlpManifest {
        version,
        url,
        sha256,
    })
}

fn find_ytdlp_manifest(binary_path: &Path) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(parent) = binary_path.parent() {
        candidates.push(parent.join("yt-dlp.json"));
    }
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            candidates.push(parent.join("yt-dlp.json"));
        }
    }
    candidates.into_iter().find(|candidate| candidate.is_file())
}

fn load_ytdlp_manifest(binary_path: &Path) -> Result<Option<YtdlpManifest>> {
    let Some(manifest_path) = find_ytdlp_manifest(binary_path) else {
        return Ok(None);
    };
    let payload = match std::fs::read_to_string(&manifest_path) {
        Ok(payload) => payload,
        Err(error) => bail!(
            "Không thể đọc manifest yt-dlp.json {}: {error}",
            manifest_path.display()
        ),
    };
    let manifest = match parse_ytdlp_manifest(&payload) {
        Ok(manifest) => manifest,
        Err(error) => bail!("yt-dlp.json không hợp lệ, chặn cập nhật (fail-closed): {error}"),
    };
    Ok(Some(manifest))
}

fn resolve_ytdlp_source(
    manifest: Option<&YtdlpManifest>,
    env_url: Option<&str>,
    env_sha256: Option<&str>,
) -> Result<(String, String)> {
    let pinned = match manifest {
        Some(entry) => (entry.url.clone(), entry.sha256.clone()),
        None => (
            YTDLP_FALLBACK_URL.to_string(),
            YTDLP_FALLBACK_SHA256.to_string(),
        ),
    };
    let Some(url) = env_url else {
        return Ok(pinned);
    };
    let url = url.trim().to_string();
    if !url.starts_with("https://") {
        bail!(
            "GHITA_YTDLP_SOURCE_URL chỉ chấp nhận địa chỉ https://, nhận được: {url}. Chính sách fail-closed: từ chối tải yt-dlp từ nguồn không an toàn."
        );
    }
    let Some(expected_sha256) = env_sha256 else {
        bail!(
            "GHITA_YTDLP_SOURCE_URL đã được đặt nhưng thiếu GHITA_YTDLP_SHA256 (64 ký tự hex). Chính sách fail-closed: không tải yt-dlp khi không có SHA-256 để xác thực."
        );
    };
    let expected_sha256 = expected_sha256.trim().to_string();
    if !is_valid_sha256(&expected_sha256) {
        bail!(
            "GHITA_YTDLP_SHA256 không hợp lệ (cần đúng 64 ký tự hex). Chính sách fail-closed: không tải yt-dlp khi SHA-256 không xác thực được."
        );
    }
    Ok((url, expected_sha256))
}

pub fn update_ytdlp(ytdlp_path: &Path) -> Result<String> {
    let clean_path = strip_verbatim_prefix(ytdlp_path);
    if !clean_path.is_absolute() {
        bail!("Đường dẫn cập nhật yt-dlp phải là đường dẫn tuyệt đối");
    }
    let manifest = load_ytdlp_manifest(&clean_path)?;
    let env_url = std::env::var("GHITA_YTDLP_SOURCE_URL").ok();
    let env_sha256 = std::env::var("GHITA_YTDLP_SHA256").ok();
    let (url, expected_sha256) =
        resolve_ytdlp_source(manifest.as_ref(), env_url.as_deref(), env_sha256.as_deref())?;
    let parent = clean_path.parent().ok_or_else(|| {
        anyhow!(
            "Không xác định được thư mục cài đặt yt-dlp: {}",
            clean_path.display()
        )
    })?;
    let temporary = unique_path(parent, "yt-dlp", "yt-dlp")?.with_extension("exe");
    let temporary = strip_verbatim_prefix(&temporary);
    let result = block_on_download(async {
        download_file_stream(&url, &temporary).await?;
        verify_sha256(&temporary, &expected_sha256)?;
        validate_tool(&temporary, "yt-dlp tải về")?;
        let version = tool_version(&temporary)?;
        crate::utils::fs::replace_file(&temporary, &clean_path)
            .with_context(|| format!("Không thể cài {}", clean_path.display()))?;
        Ok::<String, anyhow::Error>(version)
    });
    let _ = std::fs::remove_file(&temporary);
    let version = result??;
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
    fn split_version(version: &str) -> (Option<Vec<u64>>, bool) {
        let trimmed = version.trim().trim_start_matches('v');
        let (core, prerelease) = match trimmed.split_once('-') {
            Some((core, suffix)) => (core, !suffix.is_empty()),
            None => (trimmed, false),
        };
        let parts: Option<Vec<u64>> = core
            .split('.')
            .map(|part| part.parse::<u64>().ok())
            .collect();
        (parts, prerelease)
    }
    let (candidate_parts, candidate_pre) = split_version(candidate);
    let (baseline_parts, baseline_pre) = split_version(baseline);
    let (Some(candidate_parts), Some(baseline_parts)) = (candidate_parts, baseline_parts) else {
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
    baseline_pre && !candidate_pre
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
        YTDLP_FALLBACK_SHA256, YTDLP_FALLBACK_URL, YTDLP_PINNED_VERSION,
    };

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
        assert!(version_is_newer("0.0.7", "0.0.6"));
        assert!(!version_is_newer("0.0.6-preview", "0.0.6"));
        assert!(!version_is_newer("0.0.6-beta", "0.0.6"));
        assert!(version_is_newer("0.0.6", "0.0.6-preview"));
        assert!(version_is_newer("0.0.7-preview", "0.0.6"));
        assert!(version_is_newer("v0.0.7", "0.0.6"));
        assert!(version_is_newer("v2026.09.27", "2026.08.19"));
    }

    #[test]
    fn version_is_newer_falls_back_to_string_compare_for_non_numeric_tags() {
        assert!(version_is_newer("nightly-2", "nightly-1"));
        assert!(!version_is_newer("nightly-1", "nightly-1"));
    }

    #[test]
    fn pinned_constants_are_well_formed() {
        assert_eq!(YTDLP_PINNED_VERSION, "2026.10.26");
        assert!(YTDLP_FALLBACK_URL.contains(YTDLP_PINNED_VERSION));
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

    #[test]
    fn parse_ytdlp_manifest_accepts_release_schema() {
        use super::parse_ytdlp_manifest;
        let payload = r#"{"version":"2026.10.26","url":"https://github.com/yt-dlp/yt-dlp/releases/download/2026.10.26/yt-dlp.exe","sha256":"f5389d3698ecde4ef06f4e6b1ce0294fe9ac1396228c8990b8d3416d6d177a7d"}"#;
        let manifest = parse_ytdlp_manifest(payload).unwrap();
        assert_eq!(manifest.version, "2026.10.26");
        assert_eq!(
            manifest.url,
            "https://github.com/yt-dlp/yt-dlp/releases/download/2026.10.26/yt-dlp.exe"
        );
        assert_eq!(
            manifest.sha256,
            "f5389d3698ecde4ef06f4e6b1ce0294fe9ac1396228c8990b8d3416d6d177a7d"
        );
    }

    #[test]
    fn parse_ytdlp_manifest_rejects_invalid_payloads() {
        use super::parse_ytdlp_manifest;
        let missing_sha256 = r#"{"version":"1","url":"https://x/y.exe"}"#;
        let missing_url = r#"{"version":"1","sha256":"abc"}"#;
        let missing_version = r#"{"url":"https://x/y.exe","sha256":"abc"}"#;
        let empty_version = r#"{"version":"","url":"https://x/y.exe","sha256":""}"#;
        let http_url = r#"{"version":"1","url":"http://x/y.exe","sha256":"abc"}"#;
        let short_sha = r#"{"version":"1","url":"https://x/y.exe","sha256":"abc"}"#;
        let non_hex_sha = r#"{"version":"1","url":"https://x/y.exe","sha256":"zz"}"#;
        assert!(parse_ytdlp_manifest("not json").is_err());
        assert!(parse_ytdlp_manifest(missing_sha256).is_err());
        assert!(parse_ytdlp_manifest(missing_url).is_err());
        assert!(parse_ytdlp_manifest(missing_version).is_err());
        assert!(parse_ytdlp_manifest(empty_version).is_err());
        assert!(parse_ytdlp_manifest(http_url).is_err());
        assert!(parse_ytdlp_manifest(short_sha).is_err());
        assert!(parse_ytdlp_manifest(non_hex_sha).is_err());
    }

    #[test]
    fn resolve_ytdlp_source_uses_manifest_then_pinned_constants() {
        use super::{resolve_ytdlp_source, YtdlpManifest};
        let manifest = YtdlpManifest {
            version: "2026.10.26".to_string(),
            url: "https://mirror.example.com/yt-dlp.exe".to_string(),
            sha256: "a".repeat(64),
        };
        let resolved = resolve_ytdlp_source(Some(&manifest), None, None);
        let (url, sha256) = resolved.unwrap();
        assert_eq!(url, "https://mirror.example.com/yt-dlp.exe");
        assert_eq!(sha256, "a".repeat(64));
        let resolved = resolve_ytdlp_source(None, None, None);
        let (url, sha256) = resolved.unwrap();
        assert_eq!(url, YTDLP_FALLBACK_URL);
        assert_eq!(sha256, YTDLP_FALLBACK_SHA256);
    }

    #[test]
    fn resolve_ytdlp_source_env_override_requires_matching_sha256() {
        use super::{resolve_ytdlp_source, YtdlpManifest};
        let manifest = YtdlpManifest {
            version: "2026.10.26".to_string(),
            url: "https://mirror.example.com/yt-dlp.exe".to_string(),
            sha256: "a".repeat(64),
        };
        let override_url = "https://override.example.com/yt-dlp.exe";
        let sha = "b".repeat(64);
        let res = resolve_ytdlp_source(Some(&manifest), Some(override_url), Some(sha.as_str()));
        let (url, sha256) = res.unwrap();
        assert_eq!(url, override_url);
        assert_eq!(sha256, sha);
        let missing_sha = resolve_ytdlp_source(Some(&manifest), Some(override_url), None);
        assert!(missing_sha.is_err());
        let bad_sha = resolve_ytdlp_source(Some(&manifest), Some(override_url), Some("xyz"));
        assert!(bad_sha.is_err());
        let insecure_url = "http://insecure.example.com/yt-dlp.exe";
        let insecure = resolve_ytdlp_source(None, Some(insecure_url), Some(sha.as_str()));
        assert!(insecure.is_err());
    }

    #[test]
    fn resolve_ytdlp_source_ignores_sha256_without_url() {
        use super::resolve_ytdlp_source;
        let stray_sha = "c".repeat(64);
        let resolved = resolve_ytdlp_source(None, None, Some(stray_sha.as_str()));
        let (url, sha256) = resolved.unwrap();
        assert_eq!(url, YTDLP_FALLBACK_URL);
        assert_eq!(sha256, YTDLP_FALLBACK_SHA256);
    }
}
