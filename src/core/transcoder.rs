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
        let mut cmd = Command::new(&self.ffmpeg_path);
        cmd.arg("-y").arg("-i").arg(input_path).arg("-vn");

        match format {
            AudioFormat::Mp3 => {
                cmd.arg("-c:a").arg("libmp3lame");
                let bitrate = match quality {
                    AudioQuality::Mp3_320k => "320k",
                    AudioQuality::Mp3_256k => "256k",
                    AudioQuality::Mp3_192k => "192k",
                    AudioQuality::Mp3_128k => "128k",
                    AudioQuality::Mp3_96k => "96k",
                    AudioQuality::Mp3_64k => "64k",
                    _ => "320k",
                };
                cmd.arg("-b:a").arg(bitrate);
            }
            AudioFormat::Wav => match quality {
                AudioQuality::Wav_24bit_48k => {
                    cmd.arg("-c:a").arg("pcm_s24le").arg("-ar").arg("48000");
                }
                AudioQuality::Wav_16bit_44k => {
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

        cmd.arg(output_path);

        let output = cmd.output()?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("FFmpeg chuyển mã thất bại: {}", err));
        }

        Ok(())
    }

    pub fn remux_copy(
        &self,
        input_path: &Path,
        output_path: &Path,
        meta: &AudioMetadata,
    ) -> Result<()> {
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

        cmd.arg(output_path);

        let output = cmd.output()?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("FFmpeg đóng gói gốc thất bại: {}", err));
        }

        Ok(())
    }
}
