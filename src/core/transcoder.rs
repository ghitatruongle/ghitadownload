use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

use super::config::{AudioFormat, AudioQuality};
use super::tagger::AudioMetadata;

pub struct Transcoder {
    ffmpeg_path: PathBuf,
}

impl Transcoder {
    pub fn new(ffmpeg_path: &Path) -> Self {
        Self {
            ffmpeg_path: ffmpeg_path.to_path_buf(),
        }
    }

    pub async fn transcode(
        &self,
        input_path: &Path,
        output_path: &Path,
        format: AudioFormat,
        quality: AudioQuality,
        meta: &AudioMetadata,
        mut cancel: tokio::sync::watch::Receiver<bool>,
    ) -> Result<()> {
        if input_path == output_path {
            return Err(anyhow!("Input và output không được trùng đường dẫn"));
        }
        if !format.is_compatible_quality(quality) {
            return Err(anyhow!(
                "Chất lượng {:?} không tương thích với định dạng {:?}",
                quality,
                format
            ));
        }
        if !input_path.is_file() {
            return Err(anyhow!(
                "Không tìm thấy file đầu vào: {}",
                input_path.display()
            ));
        }

        let mut cmd = Self::build_transcode_command(
            &self.ffmpeg_path,
            input_path,
            output_path,
            format,
            quality,
            meta,
        )?;

        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped());
        let mut child = tokio::process::Command::from(cmd)
            .spawn()
            .map_err(|e| anyhow!("Không thể chạy FFmpeg: {}", e))?;
        let mut stderr_pipe = child.stderr.take().expect("stderr phải được pipe");
        let stderr_task = tokio::spawn(async move {
            let mut buf = Vec::new();
            let _ = tokio::io::AsyncReadExt::read_to_end(&mut stderr_pipe, &mut buf).await;
            buf
        });
        let status = tokio::select! {
            status = child.wait() => status?,
            _ = cancel.changed() => {
                let _ = child.kill().await;
                stderr_task.abort();
                return Err(anyhow!("Đã hủy"));
            }
        };
        let stderr_buf = stderr_task.await.unwrap_or_default();
        let stderr = String::from_utf8_lossy(&stderr_buf);
        if !status.success() {
            return Err(anyhow!("FFmpeg chuyển mã thất bại: {}", stderr.trim()));
        }
        if !output_path.is_file()
            || std::fs::metadata(output_path)
                .map(|metadata| metadata.len() == 0)
                .unwrap_or(true)
        {
            return Err(anyhow!(
                "FFmpeg không tạo được file đầu ra: {}",
                output_path.display()
            ));
        }

        Ok(())
    }

    pub fn build_transcode_command(
        ffmpeg_path: &Path,
        input_path: &Path,
        output_path: &Path,
        format: AudioFormat,
        quality: AudioQuality,
        meta: &AudioMetadata,
    ) -> Result<Command> {
        if !format.is_compatible_quality(quality) {
            return Err(anyhow!(
                "Chất lượng {:?} không tương thích với định dạng {:?}",
                quality,
                format
            ));
        }
        let mut cmd = Command::new(ffmpeg_path);
        cmd.arg("-y").arg("-i").arg(input_path).arg("-vn");

        match format {
            AudioFormat::Mp3 => {
                cmd.arg("-c:a").arg("libmp3lame");
                let bitrate = match quality {
                    AudioQuality::Mp3_320k | AudioQuality::Aac_320k => "320k",
                    AudioQuality::Mp3_256k | AudioQuality::Aac_256k => "256k",
                    AudioQuality::Mp3_192k | AudioQuality::Aac_192k => "192k",
                    AudioQuality::Mp3_128k | AudioQuality::Aac_128k => "128k",
                    AudioQuality::Mp3_96k => "96k",
                    AudioQuality::Mp3_64k => "64k",
                    _ => "320k",
                };
                cmd.arg("-b:a").arg(bitrate);
            }
            AudioFormat::Wav => match quality {
                AudioQuality::Wav_24bit_48k | AudioQuality::Flac_24bit_48k => {
                    cmd.arg("-c:a").arg("pcm_s24le").arg("-ar").arg("48000");
                }
                AudioQuality::Wav_16bit_44k | AudioQuality::Flac_16bit_44k => {
                    cmd.arg("-c:a").arg("pcm_s16le").arg("-ar").arg("44100");
                }
                AudioQuality::Wav_16bit_22k => {
                    cmd.arg("-c:a").arg("pcm_s16le").arg("-ar").arg("22050");
                }
                AudioQuality::Wav_8bit_11k => {
                    cmd.arg("-c:a").arg("pcm_u8").arg("-ar").arg("11025");
                }
                _ => {
                    cmd.arg("-c:a").arg("pcm_s16le").arg("-ar").arg("44100");
                }
            },
            AudioFormat::Flac => match quality {
                AudioQuality::Flac_24bit_96k => {
                    cmd.arg("-c:a")
                        .arg("flac")
                        .arg("-sample_fmt")
                        .arg("s32")
                        .arg("-ar")
                        .arg("96000");
                }
                AudioQuality::Flac_24bit_48k | AudioQuality::Wav_24bit_48k => {
                    cmd.arg("-c:a")
                        .arg("flac")
                        .arg("-sample_fmt")
                        .arg("s32")
                        .arg("-ar")
                        .arg("48000");
                }
                AudioQuality::Flac_16bit_44k | AudioQuality::Wav_16bit_44k => {
                    cmd.arg("-c:a")
                        .arg("flac")
                        .arg("-sample_fmt")
                        .arg("s16")
                        .arg("-ar")
                        .arg("44100");
                }
                _ => {
                    cmd.arg("-c:a").arg("flac").arg("-ar").arg("44100");
                }
            },
            AudioFormat::Aac => {
                cmd.arg("-c:a").arg("aac");
                let bitrate = match quality {
                    AudioQuality::Aac_320k | AudioQuality::Mp3_320k => "320k",
                    AudioQuality::Aac_256k | AudioQuality::Mp3_256k => "256k",
                    AudioQuality::Aac_192k | AudioQuality::Mp3_192k => "192k",
                    AudioQuality::Aac_128k | AudioQuality::Mp3_128k => "128k",
                    _ => "256k",
                };
                cmd.arg("-b:a").arg(bitrate);
            }
            AudioFormat::Opus => {
                cmd.arg("-c:a").arg("libopus");
                let bitrate = match quality {
                    AudioQuality::Opus_192k => "192k",
                    AudioQuality::Opus_128k => "128k",
                    AudioQuality::Opus_96k => "96k",
                    _ => "128k",
                };
                cmd.arg("-b:a").arg(bitrate);
            }
            AudioFormat::Original | AudioFormat::Video => {
                return Err(anyhow!("Không hỗ trợ chuyển mã cho định dạng {:?}", format));
            }
        }

        cmd.arg("-metadata").arg(format!("title={}", meta.title));
        cmd.arg("-metadata")
            .arg(format!("artist={}", meta.artists.join(", ")));
        cmd.arg("-metadata").arg(format!("album={}", meta.album));

        if let Some(year) = meta.release_year {
            cmd.arg("-metadata").arg(format!("date={}", year));
        }

        if let Some(tr) = meta.track_number {
            cmd.arg("-metadata").arg(format!("track={}", tr));
        }

        cmd.arg(output_path);

        Ok(cmd)
    }

    pub async fn remux_copy(
        &self,
        input_path: &Path,
        output_path: &Path,
        meta: &AudioMetadata,
        mut cancel: tokio::sync::watch::Receiver<bool>,
    ) -> Result<()> {
        if input_path == output_path {
            return Err(anyhow!("Input và output không được trùng đường dẫn"));
        }
        if !input_path.is_file() {
            return Err(anyhow!(
                "Không tìm thấy file đầu vào: {}",
                input_path.display()
            ));
        }

        let mut cmd = Command::new(&self.ffmpeg_path);
        cmd.arg("-y")
            .arg("-i")
            .arg(input_path)
            .arg("-c")
            .arg("copy");

        cmd.arg("-metadata").arg(format!("title={}", meta.title));
        cmd.arg("-metadata")
            .arg(format!("artist={}", meta.artists.join(", ")));
        cmd.arg("-metadata").arg(format!("album={}", meta.album));

        if let Some(year) = meta.release_year {
            cmd.arg("-metadata").arg(format!("date={}", year));
        }

        if let Some(tr) = meta.track_number {
            cmd.arg("-metadata").arg(format!("track={}", tr));
        }

        cmd.arg(output_path);

        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped());
        let mut child = tokio::process::Command::from(cmd)
            .spawn()
            .map_err(|e| anyhow!("Không thể chạy FFmpeg: {}", e))?;
        let mut stderr_pipe = child.stderr.take().expect("stderr phải được pipe");
        let stderr_task = tokio::spawn(async move {
            let mut buf = Vec::new();
            let _ = tokio::io::AsyncReadExt::read_to_end(&mut stderr_pipe, &mut buf).await;
            buf
        });
        let status = tokio::select! {
            status = child.wait() => status?,
            _ = cancel.changed() => {
                let _ = child.kill().await;
                stderr_task.abort();
                return Err(anyhow!("Đã hủy"));
            }
        };
        let stderr_buf = stderr_task.await.unwrap_or_default();
        let stderr = String::from_utf8_lossy(&stderr_buf);
        if !status.success() {
            return Err(anyhow!("FFmpeg đóng gói gốc thất bại: {}", stderr.trim()));
        }
        if !output_path.is_file()
            || std::fs::metadata(output_path)
                .map(|metadata| metadata.len() == 0)
                .unwrap_or(true)
        {
            return Err(anyhow!(
                "FFmpeg không tạo được file đầu ra: {}",
                output_path.display()
            ));
        }

        Ok(())
    }
}

pub fn embed_cover_ffmpeg(input: &Path, cover: &Path, ffmpeg: &Path) -> Result<PathBuf> {
    if !input.is_file() {
        return Err(anyhow!("Không tìm thấy file đầu vào: {}", input.display()));
    }
    if !cover.is_file() {
        return Err(anyhow!("Không tìm thấy ảnh bìa: {}", cover.display()));
    }
    let extension = input
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("m4a");
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let parent = input.parent().unwrap_or_else(|| Path::new("."));
    let stem = input
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("audio");
    let output = parent.join(format!("{}.cover_{}.{}", stem, stamp, extension));
    let image_codec = if extension.eq_ignore_ascii_case("flac") {
        "png"
    } else {
        "mjpeg"
    };
    let result = Command::new(ffmpeg)
        .arg("-y")
        .arg("-i")
        .arg(input)
        .arg("-i")
        .arg(cover)
        .arg("-map")
        .arg("0:a")
        .arg("-map")
        .arg("1:v")
        .arg("-c:a")
        .arg("copy")
        .arg("-c:v")
        .arg(image_codec)
        .arg("-disposition:v:0")
        .arg("attached_pic")
        .arg(&output)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| anyhow!("Không thể chạy FFmpeg: {}", e))?;
    if !result.status.success() {
        let _ = std::fs::remove_file(&output);
        return Err(anyhow!(
            "FFmpeg nhúng ảnh bìa thất bại: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        ));
    }
    if !output.is_file()
        || std::fs::metadata(&output)
            .map(|metadata| metadata.len() == 0)
            .unwrap_or(true)
    {
        return Err(anyhow!(
            "FFmpeg không tạo được file đầu ra: {}",
            output.display()
        ));
    }
    Ok(output)
}
