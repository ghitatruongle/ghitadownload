use ghita_download::core::config::{AudioFormat, AudioQuality};
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
    assert_eq!(clean, "Chau Len Ba - Bich Phuong (Dem Sac Huong Binh) [Official]");
}

#[test]
fn test_audio_qualities_extensions() {
    assert_eq!(AudioQuality::Mp3_320k.file_extension(AudioFormat::Mp3), "mp3");
    assert_eq!(AudioQuality::Mp3_64k.file_extension(AudioFormat::Mp3), "mp3");
    assert_eq!(AudioQuality::Wav_24bit_48k.file_extension(AudioFormat::Wav), "wav");
    assert_eq!(AudioQuality::Wav_8bit_11k.file_extension(AudioFormat::Wav), "wav");

    assert_eq!(AudioQuality::all_mp3().len(), 6);
    assert_eq!(AudioQuality::all_wav().len(), 4);
}

#[tokio::test]
async fn test_spotify_metadata_fetch() {
    let client = SpotifyClient::new();
    let meta = client.fetch_track("4cOdK2wGLETKBW3PvgPWqT").await;
    assert!(meta.is_ok(), "Failed to fetch Spotify metadata: {:?}", meta.err());

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
    assert_eq!(cleaned, PathBuf::from("C:\\Users\\Acer\\Downloads\\mautrain\\New folder"));

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
