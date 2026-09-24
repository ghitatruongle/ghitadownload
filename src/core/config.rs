use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioFormat {
    Mp3,
    Wav,
    Flac,
    Aac,
    Original,
    Video,
}

impl fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioFormat::Mp3 => write!(
                f,
                "MP3 (.mp3) - Tương thích mọi thiết bị, dung lượng nhỏ gọn"
            ),
            AudioFormat::Wav => write!(
                f,
                "WAV (.wav) - Âm thanh Lossless không nén chuẩn phòng thu"
            ),
            AudioFormat::Flac => write!(
                f,
                "FLAC (.flac) - Âm thanh Lossless nén chất lượng cao cho Audiophile"
            ),
            AudioFormat::Aac => write!(
                f,
                "AAC/M4A (.m4a) - Chuẩn nén cao cấp, tối ưu cho iPhone/Mac/Apple Music"
            ),
            AudioFormat::Original => write!(
                f,
                "ORIGINAL (M4A/Opus) - Giữ nguyên luồng âm thanh gốc, không chuyển mã"
            ),
            AudioFormat::Video => write!(
                f,
                "VIDEO (.mp4) - Giữ nguyên video hình ảnh, không chuyển mã"
            ),
        }
    }
}

impl std::str::FromStr for AudioFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "mp3" => Ok(AudioFormat::Mp3),
            "wav" => Ok(AudioFormat::Wav),
            "flac" => Ok(AudioFormat::Flac),
            "aac" => Ok(AudioFormat::Aac),
            "original" | "m4a" | "opus" => Ok(AudioFormat::Original),
            "video" | "mp4" => Ok(AudioFormat::Video),
            other => Err(format!(
                "Định dạng không hợp lệ: {} (chấp nhận: mp3, wav, flac, aac, original, video)",
                other
            )),
        }
    }
}

impl AudioFormat {
    pub fn default_quality(self) -> AudioQuality {
        match self {
            AudioFormat::Mp3 => AudioQuality::Mp3_320k,
            AudioFormat::Wav => AudioQuality::Wav_24bit_48k,
            AudioFormat::Flac => AudioQuality::Flac_24bit_96k,
            AudioFormat::Aac => AudioQuality::Aac_256k,
            AudioFormat::Original | AudioFormat::Video => AudioQuality::Mp3_320k,
        }
    }

    pub fn is_compatible_quality(self, quality: AudioQuality) -> bool {
        match self {
            AudioFormat::Mp3 => matches!(
                quality,
                AudioQuality::Mp3_320k
                    | AudioQuality::Mp3_256k
                    | AudioQuality::Mp3_192k
                    | AudioQuality::Mp3_128k
                    | AudioQuality::Mp3_96k
                    | AudioQuality::Mp3_64k
            ),
            AudioFormat::Wav => matches!(
                quality,
                AudioQuality::Wav_24bit_48k
                    | AudioQuality::Wav_16bit_44k
                    | AudioQuality::Wav_16bit_22k
                    | AudioQuality::Wav_8bit_11k
            ),
            AudioFormat::Flac => matches!(
                quality,
                AudioQuality::Flac_24bit_96k
                    | AudioQuality::Flac_24bit_48k
                    | AudioQuality::Flac_16bit_44k
            ),
            AudioFormat::Aac => matches!(
                quality,
                AudioQuality::Aac_320k
                    | AudioQuality::Aac_256k
                    | AudioQuality::Aac_192k
                    | AudioQuality::Aac_128k
            ),
            AudioFormat::Original | AudioFormat::Video => false,
        }
    }

    pub fn file_extension(self) -> &'static str {
        match self {
            AudioFormat::Mp3 => "mp3",
            AudioFormat::Wav => "wav",
            AudioFormat::Flac => "flac",
            AudioFormat::Aac | AudioFormat::Original => "m4a",
            AudioFormat::Video => "mp4",
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

    Flac_24bit_96k,
    Flac_24bit_48k,
    Flac_16bit_44k,

    Aac_320k,
    Aac_256k,
    Aac_192k,
    Aac_128k,
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

    pub fn all_flac() -> Vec<AudioQuality> {
        vec![
            AudioQuality::Flac_24bit_96k,
            AudioQuality::Flac_24bit_48k,
            AudioQuality::Flac_16bit_44k,
        ]
    }

    pub fn all_aac() -> Vec<AudioQuality> {
        vec![
            AudioQuality::Aac_320k,
            AudioQuality::Aac_256k,
            AudioQuality::Aac_192k,
            AudioQuality::Aac_128k,
        ]
    }

    pub fn file_extension(&self, format: AudioFormat) -> &'static str {
        format.file_extension()
    }
}

impl fmt::Display for AudioQuality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioQuality::Mp3_320k => write!(
                f,
                "320 kbps [Cực cao / Best] - Chuẩn MP3 phòng thu cao cấp nhất"
            ),
            AudioQuality::Mp3_256k => {
                write!(f, "256 kbps [Rất cao / High] - Chuẩn gốc YouTube Music")
            }
            AudioQuality::Mp3_192k => write!(
                f,
                "192 kbps [Chuẩn / Standard] - Chất lượng nhạc tiêu chuẩn"
            ),
            AudioQuality::Mp3_128k => write!(
                f,
                "128 kbps [Trung bình / Medium] - Nhẹ, tiết kiệm 50% bộ nhớ"
            ),
            AudioQuality::Mp3_96k => {
                write!(f, "96 kbps  [Thấp / Low] - Phù hợp cho nghe nói/podcast")
            }
            AudioQuality::Mp3_64k => {
                write!(f, "64 kbps  [Thấp nhất / Lowest] - Dung lượng siêu nhỏ")
            }

            AudioQuality::Wav_24bit_48k => write!(
                f,
                "24-bit PCM / 48,000 Hz [Master Studio - Cao nhất] - Lossless chuẩn phòng thu"
            ),
            AudioQuality::Wav_16bit_44k => write!(
                f,
                "16-bit PCM / 44,100 Hz [CD Quality - Chuẩn] - Lossless nguyên bản đĩa CD"
            ),
            AudioQuality::Wav_16bit_22k => write!(
                f,
                "16-bit PCM / 22,050 Hz [Radio Quality - Thấp] - Giảm bớt dung lượng WAV"
            ),
            AudioQuality::Wav_8bit_11k => write!(
                f,
                "8-bit PCM / 11,025 Hz  [Lo-Fi - Thấp nhất] - Dung lượng WAV nhỏ nhất"
            ),

            AudioQuality::Flac_24bit_96k => write!(
                f,
                "24-bit / 96,000 Hz [Studio Master Lossless] - Chuẩn FLAC phòng thu cao cấp nhất"
            ),
            AudioQuality::Flac_24bit_48k => write!(
                f,
                "24-bit / 48,000 Hz [Hi-Res Lossless] - Chuẩn FLAC phòng thu cao cấp"
            ),
            AudioQuality::Flac_16bit_44k => write!(
                f,
                "16-bit / 44,100 Hz [CD Quality Lossless] - Chuẩn FLAC nguyên bản đĩa CD"
            ),

            AudioQuality::Aac_320k => {
                write!(f, "320 kbps [Cực cao] - Chuẩn AAC chất lượng cao nhất")
            }
            AudioQuality::Aac_256k => write!(
                f,
                "256 kbps [Rất cao] - Chuẩn Apple Music / YouTube Music chất lượng cao"
            ),
            AudioQuality::Aac_192k => {
                write!(f, "192 kbps [Chuẩn / Standard] - Chất lượng AAC tiêu chuẩn")
            }
            AudioQuality::Aac_128k => {
                write!(f, "128 kbps [Tiết kiệm] - Nhẹ, tiết kiệm dung lượng")
            }
        }
    }
}

impl std::str::FromStr for AudioQuality {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "320k" => Ok(AudioQuality::Mp3_320k),
            "256k" => Ok(AudioQuality::Mp3_256k),
            "192k" => Ok(AudioQuality::Mp3_192k),
            "128k" => Ok(AudioQuality::Mp3_128k),
            "96k" => Ok(AudioQuality::Mp3_96k),
            "64k" => Ok(AudioQuality::Mp3_64k),

            "24bit48k" => Ok(AudioQuality::Wav_24bit_48k),
            "16bit44k" => Ok(AudioQuality::Wav_16bit_44k),
            "16bit22k" => Ok(AudioQuality::Wav_16bit_22k),
            "8bit11k" => Ok(AudioQuality::Wav_8bit_11k),

            "24bit96k" | "flac24bit96k" | "24bit96k_flac" => Ok(AudioQuality::Flac_24bit_96k),
            "flac24bit48k" | "24bit48k_flac" => Ok(AudioQuality::Flac_24bit_48k),
            "flac16bit44k" | "16bit44k_flac" => Ok(AudioQuality::Flac_16bit_44k),

            "aac320k" => Ok(AudioQuality::Aac_320k),
            "aac256k" => Ok(AudioQuality::Aac_256k),
            "aac192k" => Ok(AudioQuality::Aac_192k),
            "aac128k" => Ok(AudioQuality::Aac_128k),

            other => Err(format!("Chất lượng không hợp lệ: {}", other)),
        }
    }
}

fn default_concurrency() -> usize {
    3
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadSettings {
    pub output_dir: PathBuf,
    pub format: AudioFormat,
    pub quality: Option<AudioQuality>,
    #[serde(default)]
    pub video_resolution: Option<u32>,
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,
    #[serde(default)]
    pub keep_accents: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub last_output_dir: Option<PathBuf>,
    pub last_format: Option<AudioFormat>,
    pub last_quality: Option<AudioQuality>,
    #[serde(default = "default_video_resolution")]
    pub video_resolution: Option<u32>,
    #[serde(default)]
    pub keep_accents: bool,
}

fn default_video_resolution() -> Option<u32> {
    Some(1080)
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
            video_resolution: default_video_resolution(),
            keep_accents: false,
        }
    }
}

fn replace_config_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    match std::fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(_) if destination.exists() => {
            let backup = destination.with_extension(format!(
                "json.{}.{}.bak",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ));
            std::fs::rename(destination, &backup)?;
            match std::fs::rename(source, destination) {
                Ok(()) => {
                    let _ = std::fs::remove_file(backup);
                    Ok(())
                }
                Err(error) => {
                    let _ = std::fs::rename(&backup, destination);
                    Err(error)
                }
            }
        }
        Err(error) => Err(error),
    }
}

impl AppConfig {
    fn local_config_path() -> PathBuf {
        PathBuf::from("ghita_config.json")
    }

    fn global_config_path() -> PathBuf {
        if let Some(dir) = dirs::data_local_dir() {
            dir.join("GhitaDownload").join("ghita_config.json")
        } else {
            PathBuf::from("ghita_config.json")
        }
    }

    pub fn config_file_path() -> PathBuf {
        let local = Self::local_config_path();
        if local.exists() {
            local
        } else {
            Self::global_config_path()
        }
    }

    pub fn system_music_dir() -> PathBuf {
        dirs::audio_dir()
            .or_else(dirs::download_dir)
            .unwrap_or_else(|| PathBuf::from("./downloads"))
    }

    pub fn is_sensible_user_dir(path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        let trimmed = path_str
            .trim()
            .trim_matches(|c| c == '"' || c == '\'')
            .trim();
        if trimmed.is_empty() || trimmed == "." {
            return false;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower == r"c:\"
            || lower == "/"
            || lower == r"c:\windows"
            || lower.starts_with(r"c:\windows\system32")
            || lower.starts_with(r"c:\windows\syswow64")
        {
            return false;
        }
        if lower.contains(r"appdata\local\ghitadownload") {
            return false;
        }
        true
    }

    pub fn initial_output_dir(&self, current_dir: PathBuf) -> PathBuf {
        if Self::is_sensible_user_dir(&current_dir) {
            current_dir
        } else {
            self.last_output_dir
                .clone()
                .filter(|path| Self::is_sensible_user_dir(path))
                .unwrap_or_else(Self::system_music_dir)
        }
    }

    pub fn try_load() -> Result<Self, String> {
        Self::try_load_from(&Self::config_file_path())
    }

    pub fn try_load_from(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)
            .map_err(|error| format!("Không đọc được cấu hình {}: {error}", path.display()))?;
        let mut config = serde_json::from_str::<AppConfig>(&content)
            .map_err(|error| format!("Cấu hình {} không hợp lệ: {error}", path.display()))?;
        if let Some(ref dir) = config.last_output_dir {
            let value = dir.to_string_lossy();
            let cleaned = value.trim().trim_matches(|c| c == '"' || c == '\'').trim();
            config.last_output_dir = Some(PathBuf::from(cleaned));
        }
        Ok(config)
    }

    #[allow(dead_code)]
    pub fn load() -> Self {
        match Self::try_load() {
            Ok(config) => config,
            Err(message) => {
                eprintln!("Cảnh báo: {message}. Đang dùng cấu hình mặc định.");
                Self::default()
            }
        }
    }

    pub fn try_save(&self) -> Result<(), String> {
        self.try_save_to(&Self::config_file_path())
    }

    pub fn try_save_to(&self, path: &Path) -> Result<(), String> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Không tạo được thư mục cấu hình {}: {error}",
                parent.display()
            )
        })?;
        let content = serde_json::to_string_pretty(self)
            .map_err(|error| format!("Không chuyển cấu hình sang JSON: {error}"))?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temporary = parent.join(format!(
            ".{}.{}.{}.tmp",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("ghita_config.json"),
            std::process::id(),
            stamp
        ));
        let write_result = (|| -> std::io::Result<()> {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(content.as_bytes())?;
            file.sync_all()?;
            Ok(())
        })();
        if let Err(error) = write_result {
            let _ = std::fs::remove_file(&temporary);
            return Err(format!(
                "Không ghi cấu hình tạm {}: {error}",
                temporary.display()
            ));
        }
        if let Err(error) = replace_config_file(&temporary, path) {
            let _ = std::fs::remove_file(&temporary);
            return Err(format!("Không lưu cấu hình {}: {error}", path.display()));
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn save(&self) {
        if let Err(message) = self.try_save() {
            eprintln!("Cảnh báo: {message}");
        }
    }

    pub fn update_last_used(
        &mut self,
        output_dir: &Path,
        format: AudioFormat,
        quality: AudioQuality,
    ) -> Result<(), String> {
        let value = output_dir.to_string_lossy();
        let cleaned = value.trim().trim_matches(|c| c == '"' || c == '\'').trim();
        self.last_output_dir = Some(PathBuf::from(cleaned));
        self.last_format = Some(format);
        self.last_quality = Some(quality);
        self.try_save()
    }

    pub fn set_video_resolution(&mut self, resolution: Option<u32>) -> Result<(), String> {
        self.video_resolution = resolution;
        self.try_save()
    }

    pub fn set_keep_accents(&mut self, keep: bool) -> Result<(), String> {
        self.keep_accents = keep;
        self.try_save()
    }
}
