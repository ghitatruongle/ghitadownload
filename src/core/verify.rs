use anyhow::{anyhow, Result};
use regex::Regex;
use std::path::Path;
use std::process::Command;
use std::sync::LazyLock;

static DURATION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Duration:\s*(\d+):(\d+):(\d+(?:\.\d+)?)").unwrap());
static AUDIO_STREAM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)Stream #\d+:\d+.*: Audio:").unwrap());
static VIDEO_STREAM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)Stream #\d+:\d+.*: Video:").unwrap());
static AUDIO_CODEC_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r": Audio: ([A-Za-z0-9_]+)").unwrap());
static AUDIO_BITRATE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+)\s*kb/s").unwrap());

const LOSSY_CODECS: &[&str] = &[
    "aac", "ac3", "eac3", "mp2", "mp3", "mp3float", "mp4a", "opus", "speex", "vorbis",
];

#[derive(Debug, Clone)]
pub struct MediaInfo {
    pub duration_secs: f64,
    pub size_bytes: u64,
    pub has_audio: bool,
    pub has_video: bool,
    pub audio_codec: Option<String>,
    pub audio_bitrate_kbps: Option<u64>,
}

pub fn is_lossy_codec(codec: &str) -> bool {
    codec.starts_with("amr")
        || codec.starts_with("wma")
        || LOSSY_CODECS.contains(&codec.to_ascii_lowercase().as_str())
}

pub fn parse_audio_stream_line(line: &str) -> Option<(String, Option<u64>)> {
    let caps = AUDIO_CODEC_RE.captures(line)?;
    let codec = caps[1].to_ascii_lowercase();
    let bitrate = AUDIO_BITRATE_RE
        .captures(line)
        .and_then(|b| b[1].parse().ok());
    Some((codec, bitrate))
}

fn parse_audio_stream(stderr: &str) -> Option<(String, Option<u64>)> {
    stderr.lines().find_map(|line| {
        AUDIO_STREAM_RE
            .is_match(line)
            .then(|| parse_audio_stream_line(line))
            .flatten()
    })
}

pub fn lossy_transcode_note(
    target_format: &str,
    codec: Option<&str>,
    bitrate_kbps: Option<u64>,
) -> Option<String> {
    let codec = codec?;
    if !is_lossy_codec(codec) {
        return None;
    }
    let bitrate_part = bitrate_kbps
        .map(|kbps| format!(" ~{} kb/s", kbps))
        .unwrap_or_default();
    Some(format!(
        "Nguồn gốc là âm thanh nén có mất dữ liệu (codec {}{}): tệp {} chỉ đóng gói nguyên bản luồng này, không phục hồi chất lượng đã mất khi nén",
        codec, bitrate_part, target_format
    ))
}

pub fn has_media_signature(path: &Path) -> Result<()> {
    let mut head = [0_u8; 16];
    let mut file = std::fs::File::open(path)
        .map_err(|error| anyhow!("Không mở được tệp để kiểm tra định dạng: {}", error))?;
    let n = {
        use std::io::Read;
        file.read(&mut head)
            .map_err(|error| anyhow!("Không đọc được tệp để kiểm tra định dạng: {}", error))?
    };
    if n < 12 {
        return Err(anyhow!(
            "Tệp quá nhỏ hoặc rỗng, không phải media hợp lệ ({} bytes)",
            n
        ));
    }
    if &head[4..8] == b"ftyp"
        || &head[0..3] == b"ID3"
        || &head[0..4] == b"OggS"
        || &head[0..4] == b"RIFF"
        || &head[0..4] == b"fLaC"
        || &head[0..4] == b"FORM"
        || &head[0..4] == b"\x1a\x45\xdf\xa3"
        || (head[0] == 0xFF && (head[1] & 0xE0) == 0xE0)
    {
        return Ok(());
    }
    Err(anyhow!(
        "Tệp không có chữ ký media hợp lệ (MP4/M4A/MP3/WAV/FLAC/OGG/WebM). Nhiều khả năng là HTML lỗi, tệp mã hóa DRM hoặc dữ liệu rác"
    ))
}

fn parse_duration_secs(stderr: &str) -> Result<f64> {
    if let Some(caps) = DURATION_RE.captures(stderr) {
        let hours: f64 = caps[1].parse().unwrap_or(0.0);
        let minutes: f64 = caps[2].parse().unwrap_or(0.0);
        let seconds: f64 = caps[3].parse().unwrap_or(0.0);
        return Ok(hours * 3600.0 + minutes * 60.0 + seconds);
    }
    if stderr.contains("Duration: N/A") {
        return Ok(-1.0);
    }
    Err(anyhow!("FFmpeg không đọc được thời lượng của tệp"))
}

fn stream_flags(stderr: &str) -> (bool, bool) {
    let has_audio = AUDIO_STREAM_RE.is_match(stderr);
    let has_video = VIDEO_STREAM_RE.is_match(stderr);
    (has_audio, has_video)
}

pub async fn probe_media(
    path: &Path,
    ffmpeg: &Path,
    mut cancel: tokio::sync::watch::Receiver<bool>,
) -> Result<MediaInfo> {
    let size_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if size_bytes == 0 {
        return Err(anyhow!("Tệp media rỗng"));
    }
    has_media_signature(path)?;

    let mut cmd = Command::new(ffmpeg);
    cmd.arg("-hide_banner")
        .arg("-i")
        .arg(path)
        .arg("-f")
        .arg("null")
        .arg("-")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped());
    let mut child = tokio::process::Command::from(cmd)
        .spawn()
        .map_err(|e| anyhow!("Không thể chạy FFmpeg để kiểm chứng tệp: {}", e))?;
    let mut stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("Mất luồng stderr của FFmpeg"))?;
    let stderr_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        let _ = tokio::io::AsyncReadExt::read_to_end(&mut stderr_pipe, &mut buf).await;
        buf
    });
    let status = tokio::select! {
        status = child.wait() => status
            .map_err(|e| anyhow!("Không thể chạy FFmpeg để kiểm chứng tệp: {}", e))?,
        _ = cancel.changed() => {
            let _ = child.kill().await;
            stderr_task.abort();
            return Err(anyhow!("Đã hủy"));
        }
    };
    let stderr_buf = stderr_task.await.unwrap_or_default();
    let stderr = String::from_utf8_lossy(&stderr_buf);
    if stderr.contains("Invalid data found")
        || stderr.contains("does not contain any stream")
        || stderr.contains("Output file is empty")
    {
        return Err(anyhow!(
            "FFmpeg không nhận diện được tệp media hợp lệ (có thể mã hóa DRM/corrupt): {}",
            path.display()
        ));
    }

    let (has_audio, has_video) = stream_flags(&stderr);
    if !has_audio && !has_video {
        return Err(anyhow!(
            "Tệp không chứa luồng âm thanh/video hợp lệ (thường do mã hóa DRM hoặc định dạng không hỗ trợ): {}",
            path.display()
        ));
    }

    if !status.success() && !(has_audio || has_video) {
        return Err(anyhow!(
            "FFmpeg không thể đọc tệp {}: {}",
            path.display(),
            stderr.trim()
        ));
    }

    let duration_secs = parse_duration_secs(&stderr)?;
    let (audio_codec, audio_bitrate_kbps) = match parse_audio_stream(&stderr) {
        Some((codec, bitrate)) => (Some(codec), bitrate),
        None => (None, None),
    };

    Ok(MediaInfo {
        duration_secs,
        size_bytes,
        has_audio,
        has_video,
        audio_codec,
        audio_bitrate_kbps,
    })
}

pub fn validate_downloaded(
    info: &MediaInfo,
    expected_duration_secs: Option<u64>,
    tolerance_secs: f64,
) -> Result<()> {
    if info.size_bytes == 0 || info.duration_secs.is_nan() {
        return Err(anyhow!("Tệp media không có dữ liệu hợp lệ"));
    }
    if !info.has_audio && !info.has_video {
        return Err(anyhow!("Tệp media không có luồng âm thanh/video"));
    }
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
