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

    pub fn transcode(
        &self,
        input_path: &Path,
        output_path: &Path,
        format: AudioFormat,
        quality: AudioQuality,
        meta: &AudioMetadata,
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

        let mut cmd = Command::new(&self.ffmpeg_path);
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

        let output = cmd.output()?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("FFmpeg chuyển mã thất bại: {}", err));
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

    pub fn remux_copy(
        &self,
        input_path: &Path,
        output_path: &Path,
        meta: &AudioMetadata,
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

        let output = cmd.output()?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("FFmpeg đóng gói gốc thất bại: {}", err));
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
