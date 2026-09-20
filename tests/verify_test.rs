use ghita_download::core::verify::{probe_media, validate_downloaded, MediaInfo};
use ghita_download::utils::env::find_ffmpeg;
use std::path::PathBuf;
use std::process::Command;

fn test_tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ghita_verify_test_{}", tag));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn gen_tone(ffmpeg: &PathBuf, secs: u64, out: &PathBuf) {
    let status = Command::new(ffmpeg)
        .arg("-y")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg(format!("sine=frequency=440:duration={}", secs))
        .arg(out)
        .status()
        .expect("Failed to generate test tone");
    assert!(status.success(), "FFmpeg không sinh được tone {secs}s");
}

#[test]
fn test_probe_media_duration_accuracy() {
    let ffmpeg = find_ffmpeg().expect("FFmpeg must be available for testing");
    let dir = test_tmp_dir("probe");

    let tone_short = dir.join("tone_2s.wav");
    gen_tone(&ffmpeg, 2, &tone_short);
    let info_short = probe_media(&tone_short, &ffmpeg).expect("probe 2s failed");
    assert!(
        (info_short.duration_secs - 2.0).abs() <= 0.5,
        "Duration 2s lệch quá nhiều: {}",
        info_short.duration_secs
    );
    assert!(info_short.size_bytes > 0);

    let tone_long = dir.join("tone_30s.wav");
    gen_tone(&ffmpeg, 30, &tone_long);
    let info_long = probe_media(&tone_long, &ffmpeg).expect("probe 30s failed");
    assert!(
        (info_long.duration_secs - 30.0).abs() <= 0.5,
        "Duration 30s lệch quá nhiều: {}",
        info_long.duration_secs
    );
    assert!(info_long.size_bytes > info_short.size_bytes);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_probe_media_empty_file_fails() {
    let ffmpeg = find_ffmpeg().expect("FFmpeg must be available for testing");
    let dir = test_tmp_dir("empty");

    let broken = dir.join("broken.wav");
    std::fs::write(&broken, []).unwrap();
    assert!(probe_media(&broken, &ffmpeg).is_err());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_downloaded_rules() {
    let ok_info = MediaInfo {
        duration_secs: 200.0,
        size_bytes: 5_000_000,
    };
    assert!(validate_downloaded(&ok_info, None, 10.0).is_ok());
    assert!(validate_downloaded(&ok_info, Some(205), 10.0).is_ok());
    assert!(validate_downloaded(&ok_info, Some(195), 10.0).is_ok());
    assert!(validate_downloaded(&ok_info, Some(215), 10.0).is_err());
    assert!(validate_downloaded(&ok_info, Some(185), 10.0).is_err());

    let tiny = MediaInfo {
        duration_secs: 200.0,
        size_bytes: 10_239,
    };
    assert!(validate_downloaded(&tiny, None, 10.0).is_err());

    let short = MediaInfo {
        duration_secs: 2.9,
        size_bytes: 50_000,
    };
    assert!(validate_downloaded(&short, None, 10.0).is_err());

    let exact_boundary = MediaInfo {
        duration_secs: 3.0,
        size_bytes: 10_240,
    };
    assert!(validate_downloaded(&exact_boundary, None, 10.0).is_ok());

    let unknown_duration = MediaInfo {
        duration_secs: -1.0,
        size_bytes: 5_000_000,
    };
    assert!(validate_downloaded(&unknown_duration, Some(200), 10.0).is_ok());

    let unknown_but_tiny = MediaInfo {
        duration_secs: -1.0,
        size_bytes: 1_000,
    };
    assert!(validate_downloaded(&unknown_but_tiny, None, 10.0).is_err());
}

#[test]
fn test_validate_end_to_end_with_generated_tone() {
    let ffmpeg = find_ffmpeg().expect("FFmpeg must be available for testing");
    let dir = test_tmp_dir("e2e");

    let tone = dir.join("tone_5s.wav");
    gen_tone(&ffmpeg, 5, &tone);
    let info = probe_media(&tone, &ffmpeg).expect("probe 5s failed");

    assert!(validate_downloaded(&info, Some(5), 10.0).is_ok());
    assert!(validate_downloaded(&info, Some(60), 10.0).is_err());

    let _ = std::fs::remove_dir_all(&dir);
}
