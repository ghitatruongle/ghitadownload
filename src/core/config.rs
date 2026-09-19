use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioFormat {
    Mp3,
    Wav,
}

impl fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioFormat::Mp3 => write!(f, "MP3 (.mp3) - Tương thích mọi thiết bị, dung lượng nhỏ gọn"),
            AudioFormat::Wav => write!(f, "WAV (.wav) - Âm thanh Lossless không nén chuẩn phòng thu"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub enum AudioQuality {
    Mp3_320k,
    Mp3_256k,
    Mp3_192k,
    Mp3_128k,
    Mp3_96k,
    Mp3_64k,

    Wav_24bit_48k,
    Wav_16bit_44k,
    Wav_16bit_22k,
    Wav_8bit_11k,
}

impl AudioQuality {
    pub fn all_mp3() -> Vec<AudioQuality> {
        vec![
            AudioQuality::Mp3_320k,
            AudioQuality::Mp3_256k,
            AudioQuality::Mp3_192k,
            AudioQuality::Mp3_128k,
            AudioQuality::Mp3_96k,
            AudioQuality::Mp3_64k,
        ]
    }

    pub fn all_wav() -> Vec<AudioQuality> {
        vec![
            AudioQuality::Wav_24bit_48k,
            AudioQuality::Wav_16bit_44k,
            AudioQuality::Wav_16bit_22k,
            AudioQuality::Wav_8bit_11k,
        ]
    }

    pub fn file_extension(&self, format: AudioFormat) -> &'static str {
        match format {
            AudioFormat::Mp3 => "mp3",
            AudioFormat::Wav => "wav",
        }
    }
}

impl fmt::Display for AudioQuality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioQuality::Mp3_320k => write!(f, "320 kbps [Cực cao / Best] - Chuẩn MP3 phòng thu cao cấp nhất"),
            AudioQuality::Mp3_256k => write!(f, "256 kbps [Rất cao / High] - Chuẩn gốc YouTube Music"),
            AudioQuality::Mp3_192k => write!(f, "192 kbps [Chuẩn / Standard] - Chất lượng nhạc tiêu chuẩn"),
            AudioQuality::Mp3_128k => write!(f, "128 kbps [Trung bình / Medium] - Nhẹ, tiết kiệm 50% bộ nhớ"),
            AudioQuality::Mp3_96k  => write!(f, "96 kbps  [Thấp / Low] - Phù hợp cho nghe nói/podcast"),
            AudioQuality::Mp3_64k  => write!(f, "64 kbps  [Thấp nhất / Lowest] - Dung lượng siêu nhỏ"),

            AudioQuality::Wav_24bit_48k => write!(f, "24-bit PCM / 48,000 Hz [Master Studio - Cao nhất] - Lossless chuẩn phòng thu"),
            AudioQuality::Wav_16bit_44k => write!(f, "16-bit PCM / 44,100 Hz [CD Quality - Chuẩn] - Lossless nguyên bản đĩa CD"),
            AudioQuality::Wav_16bit_22k => write!(f, "16-bit PCM / 22,050 Hz [Radio Quality - Thấp] - Giảm bớt dung lượng WAV"),
            AudioQuality::Wav_8bit_11k  => write!(f, "8-bit PCM / 11,025 Hz  [Lo-Fi - Thấp nhất] - Dung lượng WAV nhỏ nhất"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadSettings {
    pub output_dir: PathBuf,
    pub format: AudioFormat,
    pub quality: AudioQuality,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub last_output_dir: Option<PathBuf>,
    pub last_format: Option<AudioFormat>,
    pub last_quality: Option<AudioQuality>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let default_dir = dirs::audio_dir()
            .or_else(dirs::download_dir)
            .unwrap_or_else(|| PathBuf::from("./downloads"));

        Self {
            last_output_dir: Some(default_dir),
            last_format: Some(AudioFormat::Mp3),
            last_quality: Some(AudioQuality::Mp3_320k),
        }
    }
}

impl AppConfig {
    fn config_file_path() -> PathBuf {
        PathBuf::from("ghita_config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_file_path();
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(mut cfg) = serde_json::from_str::<AppConfig>(&content) {
                    if let Some(ref dir) = cfg.last_output_dir {
                        let s = dir.to_string_lossy();
                        let cleaned = s.trim().trim_matches(|c| c == '"' || c == '\'').trim();
                        cfg.last_output_dir = Some(PathBuf::from(cleaned));
                    }
                    return cfg;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = Self::config_file_path();
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, content);
        }
    }

    pub fn update_last_used(&mut self, output_dir: &Path, format: AudioFormat, quality: AudioQuality) {
        let s = output_dir.to_string_lossy();
        let cleaned = s.trim().trim_matches(|c| c == '"' || c == '\'').trim();
        self.last_output_dir = Some(PathBuf::from(cleaned));
        self.last_format = Some(format);
        self.last_quality = Some(quality);
        self.save();
    }
}
