use ghita_download::core::verify::{
    has_media_signature, probe_media, validate_downloaded, MediaInfo,
};
use ghita_download::utils::env::find_ffmpeg;
use std::path::{Path, PathBuf};
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

fn media_info(duration_secs: f64, size_bytes: u64) -> MediaInfo {
    MediaInfo {
        duration_secs,
        size_bytes,
        has_audio: true,
        has_video: false,
    }
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
    assert!(info_short.has_audio);

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
fn test_has_media_signature_rejects_encrypted_blob() {
    let dir = test_tmp_dir("sig");
    let encrypted = dir.join("encrypted.m4a");
    let mut bytes = vec![0_u8; 64];
    bytes[0] = 0x5B;
    bytes[1] = 0x53;
    bytes[2] = 0x18;
    bytes[3] = 0xDE;
    for (i, b) in bytes.iter_mut().enumerate().skip(4) {
        *b = (i as u8).wrapping_mul(31).wrapping_add(7);
    }
    std::fs::write(&encrypted, &bytes).unwrap();
    assert!(has_media_signature(&encrypted).is_err());

    let html = dir.join("error.html");
    std::fs::write(&html, b"<!DOCTYPE html><html><body>error</body></html>").unwrap();
    assert!(has_media_signature(&html).is_err());

    let ftyp = dir.join("real.m4a");
    std::fs::write(&ftyp, b"\x00\x00\x00\x20ftypisom\x00\x00\x00\x00isomiso2").unwrap();
    assert!(has_media_signature(&ftyp).is_ok());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_downloaded_rules() {
    let ok_info = media_info(200.0, 5_000_000);
    assert!(validate_downloaded(&ok_info, None, 10.0).is_ok());
    assert!(validate_downloaded(&ok_info, Some(205), 10.0).is_ok());
    assert!(validate_downloaded(&ok_info, Some(195), 10.0).is_ok());
    assert!(validate_downloaded(&ok_info, Some(215), 10.0).is_err());
    assert!(validate_downloaded(&ok_info, Some(185), 10.0).is_err());

    let tiny = media_info(200.0, 10_239);
    assert!(validate_downloaded(&tiny, None, 10.0).is_err());

    let short = media_info(2.9, 50_000);
    assert!(validate_downloaded(&short, None, 10.0).is_err());

    let exact_boundary = media_info(3.0, 10_240);
    assert!(validate_downloaded(&exact_boundary, None, 10.0).is_ok());

    let unknown_duration = media_info(-1.0, 5_000_000);
    assert!(validate_downloaded(&unknown_duration, Some(200), 10.0).is_ok());

    let unknown_but_tiny = media_info(-1.0, 1_000);
    assert!(validate_downloaded(&unknown_but_tiny, None, 10.0).is_err());

    let no_streams = MediaInfo {
        duration_secs: 200.0,
        size_bytes: 5_000_000,
        has_audio: false,
        has_video: false,
    };
    assert!(validate_downloaded(&no_streams, None, 10.0).is_err());
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

#[test]
fn test_probe_rejects_non_media_payload() {
    let ffmpeg = find_ffmpeg().expect("FFmpeg must be available for testing");
    let dir = test_tmp_dir("non_media");
    let path = dir.join("payload.bin");
    let mut bytes = vec![0_u8; 20_000];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = (i as u8).wrapping_mul(13).wrapping_add(29);
    }
    std::fs::write(&path, &bytes).unwrap();
    assert!(probe_media(&path, &ffmpeg).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_has_media_signature_accepts_common_containers() {
    let dir = test_tmp_dir("sig_ok");
    let cases: Vec<(&str, &[u8])> = vec![
        ("id3.mp3", b"ID3\x04\x00\x00\x00\x00\x00\x00\x00\x00\x00"),
        ("ogg.ogg", b"OggS\x00\x02\x00\x00\x00\x00\x00\x00\x00\x00"),
        ("wav.wav", b"RIFF\x24\x00\x00\x00WAVEfmt "),
        ("flac.flac", b"fLaC\x00\x00\x00\x22\x00\x00\x00\x00\x00"),
        (
            "webm.webm",
            b"\x1a\x45\xdf\xa3\x9fB\x86\x81\x01B\xf7\x81\x01",
        ),
        (
            "frame.mp3",
            b"\xff\xfb\x90\x00\x00\x00\x00\x00\x00\x00\x00\x00",
        ),
    ];
    for (name, bytes) in cases {
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        assert!(
            has_media_signature(&path).is_ok(),
            "phải chấp nhận chữ ký {}",
            name
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_probe_media_on_tiny_ftyp_without_decode() {
    let ffmpeg = find_ffmpeg().expect("FFmpeg must be available for testing");
    let dir = test_tmp_dir("ftyp_fail");
    let path = Path::new(&dir).join("broken.m4a");
    std::fs::write(&path, b"\x00\x00\x00\x20ftypisom\x00\x00\x00\x00isomiso2").unwrap();
    assert!(has_media_signature(&path).is_ok());
    assert!(probe_media(&path, &ffmpeg).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
