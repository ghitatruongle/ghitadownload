use ghita_download::core::tagger::{AudioMetadata, Tagger};
use ghita_download::core::transcoder::embed_cover_ffmpeg;
use ghita_download::utils::env::find_ffmpeg;
use id3::TagLike;
use std::path::PathBuf;
use std::process::Command;

fn test_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ghita_tagger_{}_{}", std::process::id(), label));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn generate_mp3(ffmpeg: &PathBuf, out: &PathBuf) {
    let status = Command::new(ffmpeg)
        .arg("-y")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("sine=frequency=440:duration=1")
        .arg("-b:a")
        .arg("64k")
        .arg(out)
        .status()
        .expect("Failed to generate test tone");
    assert!(status.success());
}

fn sample_metadata() -> AudioMetadata {
    AudioMetadata {
        title: "Bài Hát Kiểm Thử".to_string(),
        artists: vec!["Ca Sĩ A".to_string(), "Ca Sĩ B".to_string()],
        album: "Album Kiểm Thử".to_string(),
        release_year: Some(2020),
        cover_url: None,
        track_number: Some(3),
        total_tracks: Some(12),
    }
}

#[tokio::test]
async fn tag_mp3_writes_all_id3_fields() {
    let Some(ffmpeg) = find_ffmpeg() else {
        eprintln!("Bỏ qua test: FFmpeg khả dụng qua PATH là bắt buộc");
        return;
    };
    let dir = test_dir("write_fields");
    let mp3_path = dir.join("sample.mp3");
    generate_mp3(&ffmpeg, &mp3_path);

    let meta = sample_metadata();
    Tagger::new()
        .tag_mp3(&mp3_path, &meta)
        .await
        .expect("tag_mp3 phải thành công trên tệp MP3 hợp lệ");

    let tag = id3::Tag::read_from_path(&mp3_path).expect("phải đọc lại được tag ID3");
    assert_eq!(tag.title(), Some("Bài Hát Kiểm Thử"));
    assert_eq!(tag.artist(), Some("Ca Sĩ A, Ca Sĩ B"));
    assert_eq!(tag.album(), Some("Album Kiểm Thử"));
    assert_eq!(tag.year(), Some(2020));
    assert_eq!(tag.track(), Some(3));
    assert_eq!(tag.total_tracks(), Some(12));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn embed_cover_ffmpeg_muxes_attached_pic() {
    let Some(ffmpeg) = find_ffmpeg() else {
        eprintln!("Bỏ qua test: FFmpeg khả dụng qua PATH là bắt buộc");
        return;
    };
    let dir = test_dir("embed_cover");
    let flac_path = dir.join("tone.flac");
    let flac_status = Command::new(&ffmpeg)
        .arg("-y")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("sine=frequency=440:duration=1")
        .arg(&flac_path)
        .status()
        .expect("Failed to generate flac");
    assert!(flac_status.success());

    let png_path = dir.join("cover.png");
    let png_status = Command::new(&ffmpeg)
        .arg("-y")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("color=c=red:s=64x64")
        .arg("-frames:v")
        .arg("1")
        .arg(&png_path)
        .status()
        .expect("Failed to generate png");
    assert!(png_status.success());

    let muxed = embed_cover_ffmpeg(&flac_path, &png_path, &ffmpeg)
        .expect("embed_cover_ffmpeg phải thành công trên flac + png");
    assert!(muxed.exists(), "tệp mux phải tồn tại");
    assert!(
        std::fs::metadata(&muxed).unwrap().len() > 0,
        "tệp mux không được rỗng"
    );

    let probe = Command::new(&ffmpeg)
        .arg("-hide_banner")
        .arg("-i")
        .arg(&muxed)
        .output()
        .expect("Failed to probe muxed file");
    let probe_text = String::from_utf8_lossy(&probe.stderr);
    assert!(
        probe_text.contains("Video:"),
        "đầu ra phải có luồng video ảnh bìa: {}",
        probe_text
    );
    assert!(
        probe_text.contains("attached"),
        "đầu ra phải có disposition attached pic: {}",
        probe_text
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn tag_mp3_rejects_missing_file() {
    let dir = test_dir("missing_file");
    let missing = dir.join("khong_ton_tai.mp3");
    let result = Tagger::new().tag_mp3(&missing, &sample_metadata()).await;
    assert!(result.is_err(), "tag_mp3 phải lỗi khi tệp không tồn tại");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn tag_mp3_skips_cover_when_url_absent() {
    let Some(ffmpeg) = find_ffmpeg() else {
        eprintln!("Bỏ qua test: FFmpeg khả dụng qua PATH là bắt buộc");
        return;
    };
    let dir = test_dir("no_cover");
    let mp3_path = dir.join("sample.mp3");
    generate_mp3(&ffmpeg, &mp3_path);

    Tagger::new()
        .tag_mp3(&mp3_path, &sample_metadata())
        .await
        .expect("tag_mp3 không cần mạng khi không có cover_url");

    let tag = id3::Tag::read_from_path(&mp3_path).unwrap();
    assert!(
        tag.pictures().next().is_none(),
        "không được nhúng ảnh bìa khi cover_url trống"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
