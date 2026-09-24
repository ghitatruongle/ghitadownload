use anyhow::{anyhow, bail, Context, Result};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const YTDLP_FALLBACK_URL: &str =
    "https://github.com/yt-dlp/yt-dlp/releases/download/2026.08.19/yt-dlp.exe";
const YTDLP_FALLBACK_SHA256: &str =
    "f0a2417a49d9dbbc17d76d2bf37192231eec4930e9d68c45c011788a4641b0b9";

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

fn absolute_existing_file(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() || !path.is_file() {
        return None;
    }
    let canonical = path.canonicalize().ok()?;
    canonical.is_file().then_some(canonical)
}

fn tool_candidates(file_name: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            candidates.push(parent.join(file_name));
            candidates.push(parent.join("bin").join(file_name));
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
        .or_else(|| {
            let local_app_data = std::env::var_os("LOCALAPPDATA")?;
            let path = PathBuf::from(local_app_data).join(
                "Packages/PythonSoftwareFoundation.Python.3.13_qbz5n2kfra8p0/LocalCache/local-packages/Python313/Scripts/yt-dlp.exe",
            );
            validate_tool(&path, "yt-dlp").ok()
        })
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

fn download_file(url: &str, destination: &Path) -> Result<Output> {
    let powershell = powershell_path()?;
    let script = "$ErrorActionPreference='Stop'; $ProgressPreference='SilentlyContinue'; Invoke-WebRequest -UseBasicParsing -Uri $args[0] -OutFile $args[1]";
    let output = Command::new(powershell)
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .arg(url)
        .arg(destination)
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
    let powershell = powershell_path()?;
    let output = Command::new(powershell)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(Get-FileHash -LiteralPath $args[0] -Algorithm SHA256).Hash",
        ])
        .arg(path)
        .output()
        .map_err(|error| anyhow!("Không thể tính SHA-256: {error}"))?;
    if !output.status.success() {
        bail!(
            "Không thể tính SHA-256: {}{}",
            output.status,
            process_error(&output)
        );
    }
    let hash = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_ascii_lowercase();
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
    let output = Command::new(path)
        .arg("--version")
        .output()
        .with_context(|| format!("Không thể chạy {}", path.display()))?;
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
    let target = target_dir.join("ffmpeg.exe");
    let zip = unique_path(&target_dir, "ffmpeg", "ffmpeg")?.with_extension("zip");
    let extracted = unique_path(&target_dir, "ffmpeg", "ffmpeg-extract")?;
    let downloaded = unique_path(&target_dir, "ffmpeg", "ffmpeg-bin")?.with_extension("exe");
    let url = std::env::var("GHITA_FFMPEG_SOURCE_URL")
        .ok()
        .filter(|value| value.starts_with("https://"))
        .ok_or_else(|| {
            anyhow!(
                "Chưa cấu hình nguồn FFmpeg an toàn. Đặt GHITA_FFMPEG_SOURCE_URL và GHITA_FFMPEG_SHA256."
            )
        })?;
    let expected_sha256 = std::env::var("GHITA_FFMPEG_SHA256")
        .ok()
        .filter(|value| value.len() == 64)
        .ok_or_else(|| {
            anyhow!(
                "Chưa cấu hình SHA-256 FFmpeg. Đặt GHITA_FFMPEG_SOURCE_URL và GHITA_FFMPEG_SHA256."
            )
        })?;
    let result = (|| -> Result<PathBuf> {
        download_file(&url, &zip)?;
        verify_sha256(&zip, &expected_sha256)?;
        if std::fs::metadata(&zip).is_ok_and(|metadata| metadata.len() <= 100) {
            bail!("Tệp FFmpeg tải về quá nhỏ hoặc không hợp lệ");
        }
        let powershell = powershell_path()?;
        let expand_script = "$ErrorActionPreference='Stop'; Expand-Archive -LiteralPath $args[0] -DestinationPath $args[1] -Force; $bin = Get-ChildItem -LiteralPath $args[1] -Recurse -Filter 'ffmpeg.exe' | Select-Object -First 1; if ($null -eq $bin) { throw 'ffmpeg.exe not found in archive' }; Copy-Item -LiteralPath $bin.FullName -Destination $args[2] -Force";
        let output = Command::new(powershell)
            .args(["-NoProfile", "-NonInteractive", "-Command", expand_script])
            .arg(&zip)
            .arg(&extracted)
            .arg(&downloaded)
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
        replace_file(&downloaded, &target)?;
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

fn replace_file(source: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        let backup = unique_path(
            destination.parent().unwrap_or_else(|| Path::new(".")),
            destination
                .file_name()
                .and_then(OsStr::to_str)
                .unwrap_or("tool"),
            "backup",
        )?;
        std::fs::rename(destination, &backup)
            .with_context(|| format!("Không thể tạm thời thay thế {}", destination.display()))?;
        match std::fs::rename(source, destination) {
            Ok(()) => {
                let _ = std::fs::remove_file(backup);
                Ok(())
            }
            Err(error) => {
                let _ = std::fs::rename(&backup, destination);
                Err(error).with_context(|| format!("Không thể cài {}", destination.display()))
            }
        }
    } else {
        std::fs::rename(source, destination)
            .with_context(|| format!("Không thể cài {}", destination.display()))
    }
}

pub fn update_ytdlp(ytdlp_path: &Path) -> Result<String> {
    if !ytdlp_path.is_absolute() {
        bail!("Đường dẫn cập nhật yt-dlp phải là đường dẫn tuyệt đối");
    }
    let parent = ytdlp_path.parent().ok_or_else(|| {
        anyhow!(
            "Không xác định được thư mục cài đặt yt-dlp: {}",
            ytdlp_path.display()
        )
    })?;
    let temporary = unique_path(parent, "yt-dlp", "yt-dlp")?.with_extension("exe");
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
        replace_file(&temporary, ytdlp_path)?;
        Ok(version)
    })();
    let _ = std::fs::remove_file(&temporary);
    let version = result?;
    validate_tool(ytdlp_path, "yt-dlp")?;
    Ok(format!(
        "Đã cập nhật yt-dlp {} từ nguồn đã cấu hình.",
        version
    ))
}
