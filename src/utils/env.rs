use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn find_ytdlp() -> Option<PathBuf> {
    let local_bin = PathBuf::from("./bin/yt-dlp.exe");
    if local_bin.exists() {
        return Some(local_bin);
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let next_to_exe = parent.join("yt-dlp.exe");
            if next_to_exe.exists() {
                return Some(next_to_exe);
            }
            let in_exe_bin = parent.join("bin").join("yt-dlp.exe");
            if in_exe_bin.exists() {
                return Some(in_exe_bin);
            }
        }
    }

    if let Some(dir) = dirs::data_local_dir() {
        let ghita_bin = dir.join("GhitaDownload").join("bin").join("yt-dlp.exe");
        if ghita_bin.exists() {
            return Some(ghita_bin);
        }
    }

    if is_command_available("yt-dlp") {
        return Some(PathBuf::from("yt-dlp"));
    }

    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let pattern = format!(
            r"{}\Packages\PythonSoftwareFoundation.Python.3.13_qbz5n2kfra8p0\LocalCache\local-packages\Python313\Scripts\yt-dlp.exe",
            local_app_data
        );
        let python_path = PathBuf::from(pattern);
        if python_path.exists() {
            return Some(python_path);
        }
    }

    None
}

pub fn find_ffmpeg() -> Option<PathBuf> {
    if is_command_available("ffmpeg") {
        return Some(PathBuf::from("ffmpeg"));
    }

    let local_bin = PathBuf::from("./bin/ffmpeg.exe");
    if local_bin.exists() {
        return Some(local_bin);
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let next_to_exe = parent.join("ffmpeg.exe");
            if next_to_exe.exists() {
                return Some(next_to_exe);
            }
            let in_exe_bin = parent.join("bin").join("ffmpeg.exe");
            if in_exe_bin.exists() {
                return Some(in_exe_bin);
            }
        }
    }

    if let Some(dir) = dirs::data_local_dir() {
        let ghita_bin = dir.join("GhitaDownload").join("bin").join("ffmpeg.exe");
        if ghita_bin.exists() {
            return Some(ghita_bin);
        }
        let ghita_root = dir.join("GhitaDownload").join("ffmpeg.exe");
        if ghita_root.exists() {
            return Some(ghita_root);
        }
    }

    let default_paths = [
        PathBuf::from(r"C:\ffmpeg\bin\ffmpeg.exe"),
        PathBuf::from(r"C:\Program Files\ffmpeg\bin\ffmpeg.exe"),
    ];
    for p in &default_paths {
        if p.exists() {
            return Some(p.clone());
        }
    }

    None
}

pub fn has_node() -> bool {
    is_command_available("node")
}

pub fn is_command_available(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("-version")
        .output()
        .or_else(|_| Command::new(cmd).arg("--version").output())
        .map(|out| out.status.success())
        .unwrap_or(false)
}

pub fn install_ffmpeg_auto() -> Result<PathBuf> {
    if let Some(p) = find_ffmpeg() {
        return Ok(p);
    }

    if is_command_available("winget") {
        let status = Command::new("winget")
            .arg("install")
            .arg("Gyan.FFmpeg")
            .arg("--accept-package-agreements")
            .arg("--accept-source-agreements")
            .arg("--silent")
            .status();

        if let Ok(s) = status {
            if s.success() {
                if let Some(p) = find_ffmpeg() {
                    return Ok(p);
                }
            }
        }
    }

    let target_dir = match dirs::data_local_dir() {
        Some(d) => d.join("GhitaDownload").join("bin"),
        None => PathBuf::from("./bin"),
    };
    let _ = std::fs::create_dir_all(&target_dir);
    let target_exe = target_dir.join("ffmpeg.exe");

    let ps_script = format!(
        "$ProgressPreference = 'SilentlyContinue'; \
         $zip = Join-Path $env:TEMP 'ffmpeg_ghita.zip'; \
         $tmp = Join-Path $env:TEMP 'ffmpeg_ghita_extracted'; \
         Invoke-WebRequest -Uri 'https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip' -OutFile $zip; \
         Expand-Archive -Path $zip -DestinationPath $tmp -Force; \
         $bin = Get-ChildItem -Path $tmp -Recurse -Filter 'ffmpeg.exe' | Select-Object -First 1; \
         if ($bin) {{ Copy-Item $bin.FullName -Destination '{}' -Force }}; \
         Remove-Item -Path $zip -Force -ErrorAction SilentlyContinue; \
         Remove-Item -Path $tmp -Recurse -Force -ErrorAction SilentlyContinue;",
        target_exe.display().to_string().replace('\\', "\\\\")
    );

    let ps_status = Command::new("powershell")
        .arg("-NoProfile")
        .arg("-Command")
        .arg(&ps_script)
        .status();

    if let Ok(s) = ps_status {
        if s.success() && target_exe.exists() {
            return Ok(target_exe);
        }
    }

    if let Some(p) = find_ffmpeg() {
        Ok(p)
    } else {
        Err(anyhow!(
            "Không thể tự động cài đặt FFmpeg. Vui lòng cài đặt thủ công hoặc dùng lệnh: winget install Gyan.FFmpeg"
        ))
    }
}

pub fn update_ytdlp(ytdlp_path: &Path) -> Result<String> {
    let output = Command::new(ytdlp_path).arg("-U").output();

    if let Ok(out) = output {
        let msg = String::from_utf8_lossy(&out.stdout).to_string();
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        let combined = format!("{}{}", msg, err);
        if out.status.success()
            || combined.to_ascii_lowercase().contains("up to date")
            || combined.to_ascii_lowercase().contains("updated")
        {
            let last_meaningful = combined
                .lines()
                .rfind(|l| !l.trim().is_empty())
                .unwrap_or("Đã cập nhật yt-dlp thành công");
            return Ok(last_meaningful.trim().to_string());
        }
    }

    let target_dest = if ytdlp_path.is_absolute() || ytdlp_path.exists() {
        ytdlp_path.to_path_buf()
    } else if let Some(d) = dirs::data_local_dir() {
        let bin = d.join("GhitaDownload").join("bin");
        let _ = std::fs::create_dir_all(&bin);
        bin.join("yt-dlp.exe")
    } else {
        let bin = PathBuf::from("./bin");
        let _ = std::fs::create_dir_all(&bin);
        bin.join("yt-dlp.exe")
    };

    let download_url = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
    let ps_script = format!(
        "$ProgressPreference = 'SilentlyContinue'; \
         Invoke-WebRequest -Uri '{}' -OutFile '{}';",
        download_url,
        target_dest.display().to_string().replace('\\', "\\\\")
    );

    let ps_status = Command::new("powershell")
        .arg("-NoProfile")
        .arg("-Command")
        .arg(&ps_script)
        .status();

    match ps_status {
        Ok(s) if s.success() => {
            Ok("Đã tải và cập nhật phiên bản yt-dlp mới nhất từ GitHub Releases.".to_string())
        }
        _ => Err(anyhow!("Không thể cập nhật yt-dlp tự động qua GitHub")),
    }
}
