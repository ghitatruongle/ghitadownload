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
