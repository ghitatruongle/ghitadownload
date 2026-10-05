use clap::Parser;
use ghita_download::cli::args::CliArgs;

fn parse(arguments: &[&str]) -> CliArgs {
    let mut full = vec!["ghitadownload"];
    full.extend_from_slice(arguments);
    CliArgs::try_parse_from(full).unwrap()
}

#[test]
fn update_ytdlp_is_headless_without_links_or_ffmpeg() {
    let args = parse(&["--update-ytdlp"]);
    assert!(args.update_ytdlp);
    assert!(args.is_headless());
    assert!(args.validate().is_ok());
}

#[test]
fn ordinary_invocation_is_interactive() {
    let args = parse(&[]);
    assert!(!args.is_headless());
}

#[test]
fn option_only_invocation_is_rejected_as_headless() {
    let args = parse(&["--format", "mp3"]);
    assert!(args.is_headless());
    assert!(args.validate().is_err());
}

#[test]
fn download_options_without_source_are_rejected() {
    assert!(parse(&["--output", "out", "--concurrency", "2"])
        .validate()
        .is_err());
}

#[test]
fn update_conflicts_with_download_action() {
    assert!(parse(&["--update-ytdlp", "--link", "https://example.com"])
        .validate()
        .is_err());
}

#[test]
fn retry_accepts_only_its_output_directory() {
    assert!(parse(&["--retry-failed", "--output", "out"])
        .validate()
        .is_ok());
    assert!(
        parse(&["--retry-failed", "--output", "out", "--format", "mp3"])
            .validate()
            .is_err()
    );
    assert!(parse(&["--retry-failed", "--concurrency", "2"])
        .validate()
        .is_err());
}

#[test]
fn format_and_quality_must_be_compatible() {
    assert!(parse(&[
        "--link",
        "https://example.com",
        "--format",
        "flac",
        "--quality",
        "320k"
    ])
    .validate()
    .is_err());
    assert!(parse(&[
        "--link",
        "https://example.com",
        "--format",
        "original",
        "--quality",
        "320k"
    ])
    .validate()
    .is_err());
    assert!(parse(&[
        "--link",
        "https://example.com",
        "--format",
        "flac",
        "--quality",
        "24bit96k"
    ])
    .validate()
    .is_ok());
}

#[test]
fn opus_format_quality_validation() {
    assert!(parse(&[
        "--link",
        "https://example.com",
        "--format",
        "opus",
        "--quality",
        "opus128k"
    ])
    .validate()
    .is_ok());
    assert!(parse(&[
        "--link",
        "https://example.com",
        "--format",
        "opus",
        "--quality",
        "320k"
    ])
    .validate()
    .is_err());
    assert!(parse(&[
        "--link",
        "https://example.com",
        "--format",
        "opus",
        "--quality",
        "24bit96k"
    ])
    .validate()
    .is_err());
    assert!(parse(&[
        "--link",
        "https://example.com",
        "--format",
        "opus",
        "--quality",
        "192k"
    ])
    .validate()
    .is_ok());
}

#[test]
fn video_resolution_is_validated_and_restricted() {
    assert!(
        parse(&["--link", "https://example.com", "--resolution", "1080"])
            .validate()
            .is_err()
    );
    assert!(parse(&[
        "--link",
        "https://example.com",
        "--format",
        "video",
        "--resolution",
        "1080"
    ])
    .validate()
    .is_ok());
    assert!(parse(&[
        "--link",
        "https://example.com",
        "--format",
        "video",
        "--resolution",
        "360"
    ])
    .validate()
    .is_err());
}

#[test]
fn cookies_flags_parse_and_validate() {
    let dir = std::env::temp_dir();
    let cookie_path = dir.join(format!("ghita_cookies_{}.txt", std::process::id()));
    std::fs::write(&cookie_path, "cookies").unwrap();
    let path_str = cookie_path.to_string_lossy().into_owned();
    let args = parse(&[
        "--link",
        "https://example.com",
        "--cookies",
        &path_str,
        "--cookies-from-browser",
        "chrome",
    ]);
    assert_eq!(args.cookies.as_deref(), Some(cookie_path.as_path()));
    assert_eq!(args.cookies_from_browser.as_deref(), Some("chrome"));
    assert!(args.is_headless());
    assert!(args.validate().is_ok());
    let _ = std::fs::remove_file(&cookie_path);
}

#[test]
fn cookies_file_missing_is_rejected() {
    assert!(parse(&[
        "--link",
        "https://example.com",
        "--cookies",
        "definitely_missing_cookies.txt"
    ])
    .validate()
    .is_err());
}

#[test]
fn cookies_from_browser_invalid_is_rejected() {
    assert!(parse(&[
        "--link",
        "https://example.com",
        "--cookies-from-browser",
        "netscape"
    ])
    .validate()
    .is_err());
    assert!(parse(&[
        "--link",
        "https://example.com",
        "--cookies-from-browser",
        "Firefox"
    ])
    .validate()
    .is_ok());
}

#[test]
fn settings_json_without_cookie_keys_still_deserializes() {
    let json = r#"{"output_dir":"out","format":"Mp3","quality":null,"concurrency":3}"#;
    let settings: ghita_download::core::config::DownloadSettings =
        serde_json::from_str(json).unwrap();
    assert!(settings.cookies_file.is_none());
    assert!(settings.cookies_from_browser.is_none());
}

#[test]
fn settings_json_with_cookie_keys_roundtrips() {
    let json = r#"{"output_dir":"out","format":"Mp3","quality":null,"concurrency":3,"cookies_file":"cookies.txt","cookies_from_browser":"edge"}"#;
    let settings: ghita_download::core::config::DownloadSettings =
        serde_json::from_str(json).unwrap();
    assert_eq!(
        settings.cookies_file.as_deref(),
        Some(std::path::Path::new("cookies.txt"))
    );
    assert_eq!(settings.cookies_from_browser.as_deref(), Some("edge"));
    let serialized = serde_json::to_string(&settings).unwrap();
    let again: ghita_download::core::config::DownloadSettings =
        serde_json::from_str(&serialized).unwrap();
    assert_eq!(
        again.cookies_file.as_deref(),
        Some(std::path::Path::new("cookies.txt"))
    );
    assert_eq!(again.cookies_from_browser.as_deref(), Some("edge"));
}

#[test]
fn concurrency_must_be_positive() {
    assert!(
        parse(&["--link", "https://example.com", "--concurrency", "0"])
            .validate()
            .is_err()
    );
    assert!(
        parse(&["--link", "https://example.com", "--concurrency", "1"])
            .validate()
            .is_ok()
    );
}

#[test]
fn plain_bitrate_coerces_to_matching_format() {
    assert!(parse(&[
        "--link",
        "https://example.com",
        "--format",
        "aac",
        "--quality",
        "256k"
    ])
    .validate()
    .is_ok());
    assert!(parse(&[
        "--link",
        "https://example.com",
        "--format",
        "opus",
        "--quality",
        "128k"
    ])
    .validate()
    .is_ok());
    assert!(parse(&[
        "--link",
        "https://example.com",
        "--format",
        "opus",
        "--quality",
        "64k"
    ])
    .validate()
    .is_err());
    assert!(parse(&[
        "--link",
        "https://example.com",
        "--format",
        "flac",
        "--quality",
        "320k"
    ])
    .validate()
    .is_err());
    assert!(parse(&[
        "--link",
        "https://example.com",
        "--format",
        "mp3",
        "--quality",
        "24bit96k"
    ])
    .validate()
    .is_err());
    assert!(parse(&[
        "--link",
        "https://example.com",
        "--format",
        "original",
        "--quality",
        "320k"
    ])
    .validate()
    .is_err());
}

#[test]
fn depth_quality_coerces_between_wav_and_flac() {
    assert!(parse(&[
        "--link",
        "https://example.com",
        "--format",
        "flac",
        "--quality",
        "24bit48k"
    ])
    .validate()
    .is_ok());
    assert!(parse(&[
        "--link",
        "https://example.com",
        "--format",
        "flac",
        "--quality",
        "16bit44k"
    ])
    .validate()
    .is_ok());
    assert!(parse(&[
        "--link",
        "https://example.com",
        "--format",
        "wav",
        "--quality",
        "flac24bit48k"
    ])
    .validate()
    .is_ok());
    assert!(parse(&[
        "--link",
        "https://example.com",
        "--format",
        "wav",
        "--quality",
        "24bit96k"
    ])
    .validate()
    .is_err());
    assert!(parse(&[
        "--link",
        "https://example.com",
        "--format",
        "flac",
        "--quality",
        "320k"
    ])
    .validate()
    .is_err());
}
