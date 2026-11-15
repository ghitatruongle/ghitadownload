use ghita_download::core::platform::{PlatformParser, UrlType};
use proptest::prelude::*;

const YOUTUBE_ID: &str = "[a-zA-Z0-9_-]{11}";
const SOCIAL_PATH: &str = "/[a-zA-Z0-9_@][a-zA-Z0-9_@/-]{0,39}";
const SPOTIFY_URI_RE: &str = "spotify:(track|album|playlist):[a-zA-Z0-9]{22}";
const SPOTIFY_URL_RE: &str =
    "https://open\\.spotify\\.com/(track|album|playlist)/[a-zA-Z0-9]{22}(\\?si=[a-zA-Z0-9]{1,20})?";

fn expected_spotify_variant(kind: &str, id: &str) -> UrlType {
    match kind {
        "track" => UrlType::SpotifyTrack(id.to_string()),
        "album" => UrlType::SpotifyAlbum(id.to_string()),
        _ => UrlType::SpotifyPlaylist(id.to_string()),
    }
}

proptest! {
    #[test]
    fn parse_never_panics_on_arbitrary_input(input in any::<String>()) {
        let _ = PlatformParser::parse(&input);
    }

    #[test]
    fn blank_input_falls_back_to_empty_direct_search(blank in r"\s*") {
        assert_eq!(
            PlatformParser::parse(&blank),
            UrlType::DirectSearch(String::new())
        );
    }

    #[test]
    fn plain_word_queries_stay_direct_search(query in "[a-zà-ø]{1,24}( [a-zà-ø]{1,24}){0,3}") {
        assert_eq!(
            PlatformParser::parse(&query),
            UrlType::DirectSearch(query.clone())
        );
    }

    #[test]
    fn youtube_watch_urls_round_trip_to_video(video_id in YOUTUBE_ID) {
        let input = format!("https://www.youtube.com/watch?v={video_id}");
        assert_eq!(
            PlatformParser::parse(&input),
            UrlType::YouTubeVideo(input.clone())
        );
    }

    #[test]
    fn youtu_be_short_links_parse_to_watch_urls(video_id in YOUTUBE_ID) {
        let input = format!("https://youtu.be/{video_id}");
        assert_eq!(
            PlatformParser::parse(&input),
            UrlType::YouTubeVideo(format!("https://www.youtube.com/watch?v={video_id}"))
        );
    }

    #[test]
    fn music_youtube_watch_urls_parse_to_music_track(video_id in YOUTUBE_ID) {
        let input = format!("https://music.youtube.com/watch?v={video_id}");
        assert_eq!(
            PlatformParser::parse(&input),
            UrlType::YouTubeMusicTrack(format!("https://music.youtube.com/watch?v={video_id}"))
        );
    }

    #[test]
    fn spotify_uris_parse_by_kind(uri in SPOTIFY_URI_RE) {
        let mut parts = uri.split(':');
        let _prefix = parts.next();
        let kind = parts.next().unwrap_or_default();
        let id = parts.next().unwrap_or_default();
        assert_eq!(
            PlatformParser::parse(&uri),
            expected_spotify_variant(kind, id)
        );
    }

    #[test]
    fn spotify_open_urls_parse_by_kind(input in SPOTIFY_URL_RE) {
        let path = input
            .strip_prefix("https://open.spotify.com/")
            .unwrap_or_default();
        let kind = path.split('/').next().unwrap_or_default();
        let id_part = path.split('/').nth(1).unwrap_or_default();
        let id = id_part.split('?').next().unwrap_or_default();
        assert_eq!(
            PlatformParser::parse(&input),
            expected_spotify_variant(kind, id)
        );
    }

    #[test]
    fn social_domains_parse_with_expected_platform_name(
        host_prefix in proptest::sample::select(vec!["", "www.", "m."]),
        path in SOCIAL_PATH,
    ) {
        for (domain, platform_name) in [
            ("threads.net", "Threads"),
            ("threads.com", "Threads"),
            ("x.com", "X"),
            ("twitter.com", "X"),
            ("reddit.com", "Reddit"),
            ("tiktok.com", "TikTok"),
            ("facebook.com", "Facebook"),
            ("instagram.com", "Instagram"),
        ] {
            let input = format!("https://{host_prefix}{domain}{path}");
            assert_eq!(
                PlatformParser::parse(&input),
                UrlType::SocialVideo {
                    url: input.clone(),
                    platform_name: platform_name.to_string(),
                },
                "input {input:?} must parse as {platform_name}"
            );
        }
    }

    #[test]
    fn threads_com_parses_as_threads_social_video(path in SOCIAL_PATH) {
        let input = format!("https://www.threads.com{path}");
        assert_eq!(
            PlatformParser::parse(&input),
            UrlType::SocialVideo {
                url: input.clone(),
                platform_name: "Threads".to_string()
            }
        );
    }

    #[test]
    fn watch_urls_without_valid_id_do_not_claim_video(video_id in "[a-zA-Z0-9_-]{1,10}|[a-zA-Z0-9_-]{12,40}") {
        let input = format!("https://www.youtube.com/watch?v={video_id}");
        match PlatformParser::parse(&input) {
            UrlType::YouTubeVideo(_) => {
                panic!("non-11-char id {video_id:?} must not parse as YouTubeVideo")
            }
            other => {
                assert!(matches!(other, UrlType::DirectSearch(_)));
            }
        }
    }
}
