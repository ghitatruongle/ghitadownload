use ghita_download::core::config::{AppConfig, AudioFormat, AudioQuality, DownloadSettings};
use ghita_download::core::platform::{PlatformParser, UrlType};
use ghita_download::core::spotify::SpotifyClient;
use ghita_download::utils::file_helper::sanitize_name;

#[test]
fn test_url_parser_platforms() {
    let sp_track = "https://open.spotify.com/track/4cOdK2wGLETKBW3PvgPWqT?si=abc";
    assert_eq!(
        PlatformParser::parse(sp_track),
        UrlType::SpotifyTrack("4cOdK2wGLETKBW3PvgPWqT".to_string())
    );

    let sp_album = "https://open.spotify.com/album/1DFixLWuPkv3KT3TnV35m3";
    assert_eq!(
        PlatformParser::parse(sp_album),
        UrlType::SpotifyAlbum("1DFixLWuPkv3KT3TnV35m3".to_string())
    );

    let sp_pl = "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M";
    assert_eq!(
        PlatformParser::parse(sp_pl),
        UrlType::SpotifyPlaylist("37i9dQZF1DXcBWIGoYBM5M".to_string())
    );

    let yt_video = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
    assert_eq!(
        PlatformParser::parse(yt_video),
        UrlType::YouTubeVideo("https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string())
    );

    let ytm_track = "https://music.youtube.com/watch?v=dQw4w9WgXcQ";
    assert_eq!(
        PlatformParser::parse(ytm_track),
        UrlType::YouTubeMusicTrack("https://music.youtube.com/watch?v=dQw4w9WgXcQ".to_string())
    );

    let yt_pl = "https://www.youtube.com/playlist?list=PL123456789";
    assert_eq!(
        PlatformParser::parse(yt_pl),
        UrlType::YouTubePlaylist("https://www.youtube.com/playlist?list=PL123456789".to_string())
    );
}

#[test]
fn test_filename_sanitization() {
    let raw = "Song: Name / With <Bad> Characters? * \"Yes\" | Done.";
    let clean = sanitize_name(raw);
    assert!(!clean.contains(':'));
    assert!(!clean.contains('/'));
    assert!(!clean.contains('<'));
    assert!(!clean.contains('>'));
    assert!(!clean.contains('?'));
    assert!(!clean.contains('*'));
    assert!(!clean.contains('"'));
    assert!(!clean.contains('|'));
}

#[test]
fn test_vietnamese_accent_removal() {
    let vn_name = "Cháu Lên Ba - Bích Phương (Đêm Sắc Hương Bình) [Official]";
    let clean = sanitize_name(vn_name);
    assert_eq!(
        clean,
        "Chau Len Ba - Bich Phuong (Dem Sac Huong Binh) [Official]"
    );
}

#[test]
fn test_audio_qualities_extensions() {
    assert_eq!(
        AudioQuality::Mp3_320k.file_extension(AudioFormat::Mp3),
        "mp3"
    );
    assert_eq!(
        AudioQuality::Mp3_64k.file_extension(AudioFormat::Mp3),
        "mp3"
    );
    assert_eq!(
        AudioQuality::Wav_24bit_48k.file_extension(AudioFormat::Wav),
        "wav"
    );
    assert_eq!(
        AudioQuality::Wav_8bit_11k.file_extension(AudioFormat::Wav),
        "wav"
    );

    assert_eq!(AudioQuality::all_mp3().len(), 6);
    assert_eq!(AudioQuality::all_wav().len(), 4);
}

#[tokio::test]
async fn test_spotify_metadata_fetch() {
    let client = SpotifyClient::new();
    let meta = client.fetch_track("4cOdK2wGLETKBW3PvgPWqT").await;
    assert!(
        meta.is_ok(),
        "Failed to fetch Spotify metadata: {:?}",
        meta.err()
    );

    let track = meta.unwrap();
    assert!(track.title.contains("Never Gonna Give You Up"));
    assert!(track.artists.iter().any(|a| a.contains("Rick Astley")));
    assert!(track.cover_url.is_some());
}

#[test]
fn test_multi_link_process_and_split() {
    use ghita_download::cli::multi_input::MultiLinkInput;

    let input_lines = vec![
        "https://youtu.be/vid1".to_string(),
        "https://youtu.be/vid2, https://youtu.be/vid3".to_string(),
        "".to_string(),
        "https://open.spotify.com/track/abc;https://open.spotify.com/track/def".to_string(),
    ];

    let processed = MultiLinkInput::process_links(input_lines);
    assert_eq!(processed.len(), 5);
    assert_eq!(processed[0], "https://youtu.be/vid1");
    assert_eq!(processed[1], "https://youtu.be/vid2");
    assert_eq!(processed[2], "https://youtu.be/vid3");
    assert_eq!(processed[3], "https://open.spotify.com/track/abc");
    assert_eq!(processed[4], "https://open.spotify.com/track/def");
}

#[test]
fn test_youtube_url_strips_playlist_and_radio_mix() {
    let mix_url = "https://www.youtube.com/watch?v=uNA3ma4Kmno&list=RDuNA3ma4Kmno&start_radio=1";
    assert_eq!(
        PlatformParser::parse(mix_url),
        UrlType::YouTubeVideo("https://www.youtube.com/watch?v=uNA3ma4Kmno".to_string())
    );

    let short_mix_url = "https://youtu.be/EQfv1-i1X4s?list=RDEQfv1-i1X4s";
    assert_eq!(
        PlatformParser::parse(short_mix_url),
        UrlType::YouTubeVideo("https://www.youtube.com/watch?v=EQfv1-i1X4s".to_string())
    );

    let pl_video_url = "https://www.youtube.com/watch?v=O0xj-JKFaag&list=PLA--0DDHS1NiEvlj68enZZH_uGA_B3COc&index=1";
    assert_eq!(
        PlatformParser::parse(pl_video_url),
        UrlType::YouTubeVideo("https://www.youtube.com/watch?v=O0xj-JKFaag".to_string())
    );
}

#[test]
fn test_clean_path_strips_quotes() {
    use ghita_download::utils::file_helper::clean_path;
    use std::path::{Path, PathBuf};

    let raw = Path::new("\"C:\\Users\\Acer\\Downloads\\mautrain\\New folder\"");
    let cleaned = clean_path(raw);
    assert_eq!(
        cleaned,
        PathBuf::from("C:\\Users\\Acer\\Downloads\\mautrain\\New folder")
    );

    let single_quoted = Path::new("'D:\\Music\\My Songs'");
    let cleaned_single = clean_path(single_quoted);
    assert_eq!(cleaned_single, PathBuf::from("D:\\Music\\My Songs"));
}

#[test]
fn test_social_and_suno_parser() {
    let suno_url = "https://suno.com/song/4bcf2efc-7a98-4c12-9c16-dc562f1dbd75";
    assert_eq!(
        PlatformParser::parse(suno_url),
        UrlType::SunoTrack("4bcf2efc-7a98-4c12-9c16-dc562f1dbd75".to_string())
    );

    let tiktok_url = "https://www.tiktok.com/@user/video/7345678912345678901";
    assert_eq!(
        PlatformParser::parse(tiktok_url),
        UrlType::SocialVideo {
            url: tiktok_url.to_string(),
            platform_name: "TikTok".to_string(),
        }
    );

    let fb_url = "https://www.facebook.com/watch/?v=123456789";
    assert_eq!(
        PlatformParser::parse(fb_url),
        UrlType::SocialVideo {
            url: fb_url.to_string(),
            platform_name: "Facebook".to_string(),
        }
    );

    let ig_url = "https://www.instagram.com/reel/C3abcXYZ123/";
    assert_eq!(
        PlatformParser::parse(ig_url),
        UrlType::SocialVideo {
            url: ig_url.to_string(),
            platform_name: "Instagram".to_string(),
        }
    );

    let threads_url = "https://www.threads.net/@user/post/C4defUVW456";
    assert_eq!(
        PlatformParser::parse(threads_url),
        UrlType::SocialVideo {
            url: threads_url.to_string(),
            platform_name: "Threads".to_string(),
        }
    );
}

#[test]
fn test_extended_accent_removal() {
    use ghita_download::utils::file_helper::remove_vietnamese_accents;

    let text = "Sơn Tùng M-TP – Đừng Về Trễ “Official Audio” (Đêm Nay Đi Đâu?) [Lossless]";
    let unaccented = remove_vietnamese_accents(text);
    assert_eq!(
        unaccented,
        "Son Tung M-TP - Dung Ve Tre \"Official Audio\" (Dem Nay Di Dau?) [Lossless]"
    );

    let clean = sanitize_name(text);
    assert!(!clean.contains('–'));
    assert!(!clean.contains('"'));
    assert!(clean.starts_with("Son Tung M-TP - Dung Ve Tre"));
}

#[test]
fn test_soundcloud_bandcamp_parser() {
    let sc_url = "https://soundcloud.com/artist/set/track-name";
    assert_eq!(
        PlatformParser::parse(sc_url),
        UrlType::SocialVideo {
            url: sc_url.to_string(),
            platform_name: "SoundCloud".to_string(),
        }
    );

    let sc_m_url = "https://m.soundcloud.com/user/tracks";
    assert_eq!(
        PlatformParser::parse(sc_m_url),
        UrlType::SocialVideo {
            url: sc_m_url.to_string(),
            platform_name: "SoundCloud".to_string(),
        }
    );

    let cdn_url = "https://cf-scfwmedia.sndcdn.com/a-1234.mp3";
    assert_eq!(
        PlatformParser::parse(cdn_url),
        UrlType::SocialVideo {
            url: cdn_url.to_string(),
            platform_name: "SoundCloud".to_string(),
        }
    );

    let bc_url = "https://artist.bandcamp.com/track/song-name";
    assert_eq!(
        PlatformParser::parse(bc_url),
        UrlType::SocialVideo {
            url: bc_url.to_string(),
            platform_name: "Bandcamp".to_string(),
        }
    );

    let bc_daily_url = "https://daily.bandcamp.com/features/article";
    assert_eq!(
        PlatformParser::parse(bc_daily_url),
        UrlType::SocialVideo {
            url: bc_daily_url.to_string(),
            platform_name: "Bandcamp".to_string(),
        }
    );
}

#[test]
fn test_audio_format_from_str() {
    assert_eq!("mp3".parse::<AudioFormat>().unwrap(), AudioFormat::Mp3);
    assert_eq!("MP3".parse::<AudioFormat>().unwrap(), AudioFormat::Mp3);
    assert_eq!(" wav ".parse::<AudioFormat>().unwrap(), AudioFormat::Wav);
    assert_eq!(
        "original".parse::<AudioFormat>().unwrap(),
        AudioFormat::Original
    );
    assert_eq!("m4a".parse::<AudioFormat>().unwrap(), AudioFormat::Original);
    assert_eq!(
        "Opus".parse::<AudioFormat>().unwrap(),
        AudioFormat::Original
    );
    assert_eq!("video".parse::<AudioFormat>().unwrap(), AudioFormat::Video);
    assert_eq!("mp4".parse::<AudioFormat>().unwrap(), AudioFormat::Video);
    assert!("flac".parse::<AudioFormat>().is_err());
    assert!("".parse::<AudioFormat>().is_err());
}

#[test]
fn test_audio_quality_from_str() {
    assert_eq!(
        "320k".parse::<AudioQuality>().unwrap(),
        AudioQuality::Mp3_320k
    );
    assert_eq!(
        "256k".parse::<AudioQuality>().unwrap(),
        AudioQuality::Mp3_256k
    );
    assert_eq!(
        "192k".parse::<AudioQuality>().unwrap(),
        AudioQuality::Mp3_192k
    );
    assert_eq!(
        "128k".parse::<AudioQuality>().unwrap(),
        AudioQuality::Mp3_128k
    );
    assert_eq!(
        "96k".parse::<AudioQuality>().unwrap(),
        AudioQuality::Mp3_96k
    );
    assert_eq!(
        "64k".parse::<AudioQuality>().unwrap(),
        AudioQuality::Mp3_64k
    );
    assert_eq!(
        "24bit48k".parse::<AudioQuality>().unwrap(),
        AudioQuality::Wav_24bit_48k
    );
    assert_eq!(
        "16bit44k".parse::<AudioQuality>().unwrap(),
        AudioQuality::Wav_16bit_44k
    );
    assert_eq!(
        "16bit22k".parse::<AudioQuality>().unwrap(),
        AudioQuality::Wav_16bit_22k
    );
    assert_eq!(
        "8bit11k".parse::<AudioQuality>().unwrap(),
        AudioQuality::Wav_8bit_11k
    );
    assert!("320kbps".parse::<AudioQuality>().is_err());
    assert!("999k".parse::<AudioQuality>().is_err());
}

#[test]
fn test_failed_queue_serde_roundtrip_and_retry() {
    use ghita_download::core::batch::{BatchProcessor, DownloadTask, FailedQueue};
    use ghita_download::core::tagger::AudioMetadata;
    use ghita_download::utils::env::find_ffmpeg;
    use std::path::PathBuf;

    let ffmpeg = match find_ffmpeg() {
        Some(f) => f,
        None => {
            eprintln!("Bỏ qua test: FFmpeg khả dụng qua PATH/ytdlp là bắt buộc");
            return;
        }
    };
    let _ = ffmpeg;

    let settings = DownloadSettings {
        output_dir: PathBuf::from("D:\\Music\\Test"),
        format: AudioFormat::Mp3,
        quality: Some(AudioQuality::Mp3_320k),
        video_resolution: None,
        concurrency: 3,
    };

    let task = DownloadTask {
        display_title: "Artist - Track Title".to_string(),
        source_or_search: "https://www.youtube.com/watch?v=abcdef12345".to_string(),
        fallback_searches: vec!["Artist Track Title".to_string()],
        expected_duration_secs: Some(205),
        metadata: AudioMetadata {
            title: "Track Title".to_string(),
            artists: vec!["Artist".to_string()],
            album: "Test Album".to_string(),
            release_year: Some(2026),
            cover_url: None,
        },
    };

    let queue = FailedQueue {
        settings: settings.clone(),
        tasks: vec![task.clone()],
    };

    let json = serde_json::to_string_pretty(&queue).unwrap();
    let parsed: FailedQueue = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.tasks.len(), 1);
    assert_eq!(parsed.tasks[0].display_title, task.display_title);
    assert_eq!(parsed.tasks[0].expected_duration_secs, Some(205));
    assert_eq!(parsed.settings.format, AudioFormat::Mp3);
    assert_eq!(parsed.settings.quality, Some(AudioQuality::Mp3_320k));
    assert_eq!(parsed.settings.concurrency, 3);

    let dir_type = std::thread::current()
        .name()
        .unwrap_or("roundtrip")
        .to_string();
    let tmp_dir = std::env::temp_dir().join(format!("ghita_failed_queue_{}", dir_type));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).unwrap();
    std::fs::write(tmp_dir.join("failed_tasks.json"), &json).unwrap();

    let (loaded_settings, loaded_tasks) = BatchProcessor::retry_from_file(&tmp_dir).unwrap();
    assert_eq!(loaded_settings.format, AudioFormat::Mp3);
    assert_eq!(loaded_tasks.len(), 1);
    assert_eq!(loaded_tasks[0].source_or_search, task.source_or_search);
    assert!(tmp_dir.join("failed_tasks.json").exists());

    let (second_settings, second_tasks) = BatchProcessor::retry_from_file(&tmp_dir).unwrap();
    assert_eq!(second_settings.concurrency, 3);
    assert_eq!(second_tasks.len(), 1);

    let _ = std::fs::remove_dir_all(&tmp_dir);

    let missing_dir = std::env::temp_dir().join("ghita_failed_queue_missing");
    let _ = std::fs::remove_dir_all(&missing_dir);
    assert!(BatchProcessor::retry_from_file(&missing_dir).is_err());
}

#[test]
fn test_ensure_unique_path_reserves_atomically() {
    use ghita_download::utils::file_helper::{clean_path, ensure_unique_path};
    use std::path::Path;

    let base = std::env::temp_dir().join("ghita_unique_path_test");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();

    let first = ensure_unique_path(&base, "Same Track", "mp3");
    let second = ensure_unique_path(&base, "Same Track", "mp3");
    assert_ne!(
        first, second,
        "hai lần cấp phát cùng tên phải trả về hai đường dẫn khác nhau"
    );
    assert!(
        first.exists(),
        "ensure_unique_path phải giữ chỗ bằng cách tạo tệp"
    );
    assert!(clean_path(Path::new(&first)).exists());

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn test_download_settings_legacy_json_defaults() {
    let legacy = r#"{"output_dir":"D:\\Music","format":"Mp3","quality":"Mp3_320k"}"#;
    let settings: DownloadSettings = serde_json::from_str(legacy).unwrap();
    assert_eq!(settings.format, AudioFormat::Mp3);
    assert_eq!(settings.quality, Some(AudioQuality::Mp3_320k));
    assert_eq!(settings.video_resolution, None);
    assert_eq!(settings.concurrency, 3);
}

#[test]
fn test_app_config_legacy_json_loads() {
    let legacy =
        r#"{"last_output_dir":"D:\\Music","last_format":"Wav","last_quality":"Wav_16bit_44k"}"#;
    let cfg: AppConfig = serde_json::from_str(legacy).unwrap();
    assert_eq!(cfg.last_format, Some(AudioFormat::Wav));
    assert_eq!(cfg.video_resolution, Some(1080));
}

#[test]
fn test_audio_quality_file_extension_new_formats() {
    assert_eq!(
        AudioQuality::Mp3_320k.file_extension(AudioFormat::Original),
        "m4a"
    );
    assert_eq!(
        AudioQuality::Mp3_128k.file_extension(AudioFormat::Original),
        "m4a"
    );
    assert_eq!(
        AudioQuality::Mp3_320k.file_extension(AudioFormat::Video),
        "mp4"
    );
    assert_eq!(
        AudioQuality::Wav_24bit_48k.file_extension(AudioFormat::Video),
        "mp4"
    );
}
