use ghita_download::core::config::{AudioFormat, AudioQuality};
use ghita_download::core::tagger::{AudioMetadata, Tagger};
use ghita_download::core::transcoder::Transcoder;
use ghita_download::utils::env::find_ffmpeg;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::process::Command;

#[tokio::test]
async fn test_transcode_and_tagging_pipeline() {
    let ffmpeg_path = find_ffmpeg().expect("FFmpeg must be available for testing");
    let transcoder = Transcoder::new(&ffmpeg_path);
    let tagger = Tagger::new();

    let test_dir = PathBuf::from("./target/test_output");
    std::fs::create_dir_all(&test_dir).unwrap();

    let sample_wav = test_dir.join("input_sample.wav");
    let gen_status = Command::new(&ffmpeg_path)
        .arg("-y")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("sine=frequency=440:duration=2")
        .arg(&sample_wav)
        .status()
        .expect("Failed to generate test tone");
    assert!(gen_status.success());

    let meta = AudioMetadata {
        title: "Test Track Song".to_string(),
        artists: vec!["Ghita Artist".to_string()],
        album: "Ghita Test Album".to_string(),
        release_year: Some(2026),
        cover_url: None,
        track_number: Some(1),
        total_tracks: Some(10),
    };

    let mp3_320_path = test_dir.join("test_320k.mp3");
    let res = transcoder.transcode(
        &sample_wav,
        &mp3_320_path,
        AudioFormat::Mp3,
        AudioQuality::Mp3_320k,
        &meta,
    );
    assert!(res.is_ok(), "MP3 320k transcode failed: {:?}", res.err());
    assert!(mp3_320_path.exists());

    let tag_res = tagger.tag_mp3(&mp3_320_path, &meta).await;
    assert!(tag_res.is_ok());

    let mp3_64_path = test_dir.join("test_64k.mp3");
    let res = transcoder.transcode(
        &sample_wav,
        &mp3_64_path,
        AudioFormat::Mp3,
        AudioQuality::Mp3_64k,
        &meta,
    );
    assert!(res.is_ok(), "MP3 64k transcode failed: {:?}", res.err());
    assert!(mp3_64_path.exists());

    let size_320 = std::fs::metadata(&mp3_320_path).unwrap().len();
    let size_64 = std::fs::metadata(&mp3_64_path).unwrap().len();
    assert!(
        size_64 < size_320,
        "64k ({}) should be smaller than 320k ({})",
        size_64,
        size_320
    );

    let flac_path = test_dir.join("test_flac.flac");
    let res = transcoder.transcode(
        &sample_wav,
        &flac_path,
        AudioFormat::Flac,
        AudioQuality::Flac_24bit_48k,
        &meta,
    );
    assert!(res.is_ok(), "FLAC transcode failed: {:?}", res.err());
    assert!(flac_path.exists());

    let flac_96_path = test_dir.join("test_flac_96k.flac");
    let res_96 = transcoder.transcode(
        &sample_wav,
        &flac_96_path,
        AudioFormat::Flac,
        AudioQuality::Flac_24bit_96k,
        &meta,
    );
    assert!(res_96.is_ok(), "FLAC 96k transcode failed: {:?}", res_96.err());
    assert!(flac_96_path.exists());

    let aac_path = test_dir.join("test_aac.m4a");
    let res = transcoder.transcode(
        &sample_wav,
        &aac_path,
        AudioFormat::Aac,
        AudioQuality::Aac_256k,
        &meta,
    );
    assert!(res.is_ok(), "AAC transcode failed: {:?}", res.err());
    assert!(aac_path.exists());

    let aac_320_path = test_dir.join("test_aac_320k.m4a");
    let res_aac_320 = transcoder.transcode(
        &sample_wav,
        &aac_320_path,
        AudioFormat::Aac,
        AudioQuality::Aac_320k,
        &meta,
    );
    assert!(res_aac_320.is_ok(), "AAC 320k transcode failed: {:?}", res_aac_320.err());
    assert!(aac_320_path.exists());

    let wav_24_path = test_dir.join("test_24bit_48k.wav");
    let res = transcoder.transcode(
        &sample_wav,
        &wav_24_path,
        AudioFormat::Wav,
        AudioQuality::Wav_24bit_48k,
        &meta,
    );
    assert!(res.is_ok(), "WAV 24bit transcode failed: {:?}", res.err());
    assert!(wav_24_path.exists());

    let wav_8_path = test_dir.join("test_8bit_11k.wav");
    let res = transcoder.transcode(
        &sample_wav,
        &wav_8_path,
        AudioFormat::Wav,
        AudioQuality::Wav_8bit_11k,
        &meta,
    );
    assert!(res.is_ok(), "WAV 8bit transcode failed: {:?}", res.err());
    assert!(wav_8_path.exists());

    let size_wav_24 = std::fs::metadata(&wav_24_path).unwrap().len();
    let size_wav_8 = std::fs::metadata(&wav_8_path).unwrap().len();
    assert!(
        size_wav_8 < size_wav_24,
        "8bit WAV should be smaller than 24bit WAV"
    );

    let _ = std::fs::remove_dir_all(&test_dir);
}

#[test]
fn test_remux_copy_preserves_codec_and_duration() {
    let ffmpeg_path = find_ffmpeg().expect("FFmpeg must be available for testing");
    let transcoder = Transcoder::new(&ffmpeg_path);

    let test_dir = PathBuf::from("./target/test_output_remux");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();

    let sample_wav = test_dir.join("input_remux.wav");
    let gen_status = Command::new(&ffmpeg_path)
        .arg("-y")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("sine=frequency=880:duration=4")
        .arg(&sample_wav)
        .status()
        .expect("Failed to generate test tone");
    assert!(gen_status.success());

    let meta = AudioMetadata {
        title: "Remux Track".to_string(),
        artists: vec!["Remux Artist".to_string()],
        album: "Remux Album".to_string(),
        release_year: Some(2026),
        cover_url: None,
        track_number: None,
        total_tracks: None,
    };

    let out_wav = test_dir.join("output_remux.wav");
    let res = transcoder.remux_copy(&sample_wav, &out_wav, &meta);
    assert!(res.is_ok(), "remux_copy failed: {:?}", res.err());
    assert!(out_wav.exists());

    let src_info = ffprobe_media(&ffmpeg_path, &sample_wav);
    let out_info = ffprobe_media(&ffmpeg_path, &out_wav);
    assert_eq!(
        src_info.codec, out_info.codec,
        "remux phải giữ nguyên codec"
    );
    assert!(
        (src_info.duration - out_info.duration).abs() <= 0.5,
        "remux làm đổi thời lượng: {} -> {}",
        src_info.duration,
        out_info.duration
    );
    assert!(
        out_info.size >= src_info.size.saturating_sub(1024),
        "Tệp remux bất thường: {} bytes",
        out_info.size
    );

    let mp3_path = test_dir.join("remux_to_mp3.mp3");
    let res_mp3 = transcoder.transcode(
        &sample_wav,
        &mp3_path,
        AudioFormat::Mp3,
        AudioQuality::Mp3_192k,
        &meta,
    );
    assert!(res_mp3.is_ok(), "MP3 transcode failed: {:?}", res_mp3.err());

    let out_mp3 = test_dir.join("remux_mp3_copy.mp3");
    let res_copy_mp3 = transcoder.remux_copy(&mp3_path, &out_mp3, &meta);
    assert!(
        res_copy_mp3.is_ok(),
        "remux_copy MP3 failed: {:?}",
        res_copy_mp3.err()
    );
    let mp3_info = ffprobe_media(&ffmpeg_path, &out_mp3);
    assert_eq!(mp3_info.codec, "mp3", "Remux MP3 phải giữ codec mp3");

    let _ = std::fs::remove_dir_all(&test_dir);
}

struct QuickProbe {
    codec: String,
    duration: f64,
    size: u64,
}

fn ffprobe_media(_ffmpeg: &Path, path: &PathBuf) -> QuickProbe {
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-i")
        .arg(path)
        .output()
        .expect("Failed to run ffmpeg probe");
    let text = String::from_utf8_lossy(&output.stderr).to_string();

    let codec = text
        .lines()
        .find(|l| l.contains("Audio:"))
        .and_then(|l| l.split("Audio:").nth(1))
        .and_then(|rest| rest.split_whitespace().next())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    let duration = Regex::new(r"Duration:\s*(\d+):(\d+):(\d+(?:\.\d+)?)")
        .ok()
        .and_then(|re| re.captures(&text))
        .map(|c| {
            let h: f64 = c[1].parse().unwrap_or(0.0);
            let m: f64 = c[2].parse().unwrap_or(0.0);
            let s: f64 = c[3].parse().unwrap_or(0.0);
            h * 3600.0 + m * 60.0 + s
        })
        .unwrap_or(0.0);

    QuickProbe {
        codec,
        duration,
        size,
    }
}
