use ghita_download::core::platform::{PlatformParser, UrlType};

#[test]
fn youtu_be_short_links_parse_to_watch_urls() {
    assert_eq!(
        PlatformParser::parse("https://youtu.be/dQw4w9WgXcQ"),
        UrlType::YouTubeVideo("https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string())
    );
    assert_eq!(
        PlatformParser::parse("https://youtu.be/dQw4w9WgXcQ?t=30"),
        UrlType::YouTubeVideo("https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string())
    );
}

#[test]
fn youtu_be_extra_segments_are_rejected_as_search() {
    assert_eq!(
        PlatformParser::parse("https://youtu.be/dQw4w9WgXcQ/extra"),
        UrlType::DirectSearch("https://youtu.be/dQw4w9WgXcQ/extra".to_string())
    );
}

#[test]
fn embed_and_v_paths_parse_to_watch_urls() {
    assert_eq!(
        PlatformParser::parse("https://www.youtube.com/embed/dQw4w9WgXcQ"),
        UrlType::YouTubeVideo("https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string())
    );
    assert_eq!(
        PlatformParser::parse("https://www.youtube.com/v/dQw4w9WgXcQ"),
        UrlType::YouTubeVideo("https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string())
    );
}

#[test]
fn music_youtube_playlist_is_recognized() {
    assert_eq!(
        PlatformParser::parse("https://music.youtube.com/playlist?list=PL123456789"),
        UrlType::YouTubeMusicPlaylist(
            "https://music.youtube.com/playlist?list=PL123456789".to_string()
        )
    );
}

#[test]
fn spotify_uri_album_and_playlist_parse() {
    assert_eq!(
        PlatformParser::parse("spotify:album:1DFixLWuPkv3KT3TnV35m3"),
        UrlType::SpotifyAlbum("1DFixLWuPkv3KT3TnV35m3".to_string())
    );
    assert_eq!(
        PlatformParser::parse("spotify:playlist:37i9dQZF1DXcBWIGoYBM5M"),
        UrlType::SpotifyPlaylist("37i9dQZF1DXcBWIGoYBM5M".to_string())
    );
}

#[test]
fn spotify_uri_with_invalid_id_falls_back_to_search() {
    assert_eq!(
        PlatformParser::parse("spotify:track:too-short"),
        UrlType::DirectSearch("spotify:track:too-short".to_string())
    );
}

#[test]
fn direct_media_extensions_and_uppercase_paths_are_classified() {
    for suffix in [
        ".flac", ".opus", ".ogg", ".wav", ".webm", ".mp4", ".aiff", ".oga",
    ] {
        let url = format!("https://cdn.example.com/audio/file{suffix}");
        assert_eq!(
            PlatformParser::parse(&url),
            UrlType::DirectMedia(url.clone()),
            "phần mở rộng {suffix} phải được nhận diện DirectMedia"
        );
    }
    assert_eq!(
        PlatformParser::parse("https://cdn.example.com/audio/file.MP3"),
        UrlType::DirectMedia("https://cdn.example.com/audio/file.MP3".to_string())
    );
}

#[test]
fn suno_alt_hosts_are_recognized() {
    assert_eq!(
        PlatformParser::parse("https://suno.ai/song/4bcf2efc-7a98-4c12-9c16-dc562f1dbd75"),
        UrlType::SunoTrack("4bcf2efc-7a98-4c12-9c16-dc562f1dbd75".to_string())
    );
    assert_eq!(
        PlatformParser::parse(
            "https://studio-api-prod.suno.com/api/clip/3b494117-f372-450e-9c5c-2e48b38e5241"
        ),
        UrlType::SunoTrack("3b494117-f372-450e-9c5c-2e48b38e5241".to_string())
    );
}

#[test]
fn fb_watch_links_are_classified_as_facebook() {
    assert_eq!(
        PlatformParser::parse("https://fb.watch/abcd12345/"),
        UrlType::SocialVideo {
            url: "https://fb.watch/abcd12345/".to_string(),
            platform_name: "Facebook".to_string(),
        }
    );
}

#[test]
fn empty_input_becomes_empty_search() {
    assert_eq!(
        PlatformParser::parse(""),
        UrlType::DirectSearch(String::new())
    );
    assert_eq!(
        PlatformParser::parse("   \t  "),
        UrlType::DirectSearch(String::new())
    );
}

#[test]
fn playlist_classification_covers_playlist_and_direct_media_variants() {
    use ghita_download::core::platform::PlatformParser as Parser;
    assert!(Parser::is_playlist_or_album(&UrlType::SunoPlaylist(
        "uuid".to_string()
    )));
    assert!(Parser::is_playlist_or_album(&UrlType::YouTubePlaylist(
        "url".to_string()
    )));
    assert!(Parser::is_playlist_or_album(
        &UrlType::YouTubeMusicPlaylist("url".to_string())
    ));
    assert!(Parser::is_playlist_or_album(&UrlType::DirectMedia(
        "url".to_string()
    )));
    assert!(!Parser::is_playlist_or_album(&UrlType::YouTubeVideo(
        "url".to_string()
    )));
    assert!(!Parser::is_playlist_or_album(&UrlType::SpotifyTrack(
        "id".to_string()
    )));
}
