use anyhow::{anyhow, Result};
use regex::Regex;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct MediaInfo {
    pub duration_secs: f64,
    pub size_bytes: u64,
}

pub fn probe_media(path: &Path, ffmpeg: &Path) -> Result<MediaInfo> {
    let size_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    let output = Command::new(ffmpeg)
        .arg("-hide_banner")
        .arg("-i")
        .arg(path)
        .output()
        .map_err(|e| anyhow!("Không thể chạy FFmpeg để kiểm chứng tệp: {}", e))?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    let re = Regex::new(r"Duration:\s*(\d+):(\d+):(\d+(?:\.\d+)?)")?;
    let duration_secs = if let Some(caps) = re.captures(&stderr) {
        let hours: f64 = caps[1].parse().unwrap_or(0.0);
        let minutes: f64 = caps[2].parse().unwrap_or(0.0);
        let seconds: f64 = caps[3].parse().unwrap_or(0.0);
        hours * 3600.0 + minutes * 60.0 + seconds
    } else if stderr.contains("Duration: N/A") {
        -1.0
    } else {
        return Err(anyhow!(
            "FFmpeg không đọc được thời lượng của tệp: {}",
            path.display()
        ));
    };

    Ok(MediaInfo {
        duration_secs,
        size_bytes,
    })
}

pub fn validate_downloaded(
    info: &MediaInfo,
    expected_duration_secs: Option<u64>,
    tolerance_secs: f64,
) -> Result<()> {
    if info.size_bytes < 10_240 {
        return Err(anyhow!(
            "Tệp tải về quá nhỏ ({} bytes), có thể tải thất bại",
            info.size_bytes
        ));
    }
    if info.duration_secs >= 0.0 {
        if info.duration_secs < 3.0 {
            return Err(anyhow!(
                "Tệp tải về có thời lượng quá ngắn ({:.1}s)",
                info.duration_secs
            ));
        }
        if let Some(expected) = expected_duration_secs {
            let diff = (info.duration_secs - expected as f64).abs();
            if diff > tolerance_secs {
                return Err(anyhow!(
                    "Thời lượng lệch {:.1}s so với bản gốc (cho phép {:.0}s), có thể đã tải nhầm bài",
                    diff,
                    tolerance_secs
                ));
            }
        }
    }
    Ok(())
}
