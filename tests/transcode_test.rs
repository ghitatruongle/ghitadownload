use ghita_download::core::config::{AudioFormat, AudioQuality};
use ghita_download::core::tagger::{AudioMetadata, Tagger};
use ghita_download::core::transcoder::Transcoder;
use ghita_download::utils::env::find_ffmpeg;
use std::path::PathBuf;
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
        .arg("-f").arg("lavfi")
        .arg("-i").arg("sine=frequency=440:duration=2")
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
    assert!(size_64 < size_320, "64k ({}) should be smaller than 320k ({})", size_64, size_320);

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
    assert!(size_wav_8 < size_wav_24, "8bit WAV should be smaller than 24bit WAV");

    let _ = std::fs::remove_dir_all(&test_dir);
}
