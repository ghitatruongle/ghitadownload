use ghita_download::core::batch::{BatchOutcome, DownloadTask};
use ghita_download::core::tagger::AudioMetadata;
use ghita_download::core::youtube::YouTubeClient;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "ghita_regression_{label}_{}_{}",
        std::process::id(),
        nanos
    ))
}

fn fake_playlist_payload() -> String {
    [
        "dQw4w9WgXcQ\tNever Gonna Give You Up\tRick Astley\t212.0\thttps://i.ytimg.com/vi/dQw4w9WgXcQ/default.jpg\thttps://www.youtube.com/watch?v=dQw4w9WgXcQ",
        "jNQXAC9IVRw\tMe at the zoo\tjawed\t19.0\tNA\thttps://www.youtube.com/watch?v=jNQXAC9IVRw",
    ]
    .join("\n")
}

fn write_fake_ytdlp(dir: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let script = dir.join("fake-yt-dlp.cmd");
        let lines: Vec<String> = fake_playlist_payload()
            .lines()
            .map(|line| format!("echo {}", line))
            .collect();
        let body = format!("@echo off\r\n{}\r\nexit /b 0\r\n", lines.join("\r\n"));
        std::fs::write(&script, body).unwrap();
        script
    }
    #[cfg(not(windows))]
    {
        let script = dir.join("fake-yt-dlp.sh");
        let body = format!(
            "#!/bin/sh\ncat <<'GHITA_EOF'\n{}\nGHITA_EOF\n",
            fake_playlist_payload()
        );
        std::fs::write(&script, body).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        script
    }
}

fn sample_task(title: &str, source: &str) -> DownloadTask {
    DownloadTask {
        display_title: title.to_string(),
        source_or_search: source.to_string(),
        fallback_searches: Vec::new(),
        expected_duration_secs: None,
        metadata: AudioMetadata {
            title: title.to_string(),
            artists: Vec::new(),
            album: String::new(),
            release_year: None,
            cover_url: None,
            track_number: None,
            total_tracks: None,
            lyrics: None,
        },
    }
}

#[test]
fn extract_playlist_items_collects_flat_playlist_output_through_command_output() {
    let dir = unique_temp_dir("fake_ytdlp");
    std::fs::create_dir_all(&dir).unwrap();
    let fake = write_fake_ytdlp(&dir);
    let client = YouTubeClient::new(fake.as_path(), false, None, None);
    let items = client
        .extract_playlist_items("https://www.youtube.com/playlist?list=PLghita_regression")
        .expect("fake yt-dlp must produce playlist items");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].id, "dQw4w9WgXcQ");
    assert_eq!(items[0].title, "Never Gonna Give You Up");
    assert_eq!(items[0].uploader, "Rick Astley");
    assert_eq!(items[0].duration_secs, Some(212.0));
    assert_eq!(
        items[0].thumbnail_url.as_deref(),
        Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/default.jpg")
    );
    assert_eq!(
        items[0].direct_url,
        "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
    );
    assert_eq!(items[1].id, "jNQXAC9IVRw");
    assert_eq!(items[1].title, "Me at the zoo");
    assert_eq!(items[1].uploader, "jawed");
    assert_eq!(items[1].duration_secs, Some(19.0));
    assert_eq!(items[1].thumbnail_url, None);
    assert_eq!(
        items[1].direct_url,
        "https://www.youtube.com/watch?v=jNQXAC9IVRw"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn extract_playlist_items_reports_spawn_failure_instead_of_hanging() {
    let client = YouTubeClient::new(
        Path::new("ghita-definitely-missing-yt-dlp-binary"),
        false,
        None,
        None,
    );
    let error = client
        .extract_playlist_items("https://www.youtube.com/playlist?list=PLghita_regression")
        .expect_err("missing yt-dlp binary must produce an error");
    assert!(error.to_string().contains("Không thể chạy yt-dlp"));
}

#[test]
fn headless_batch_summary_prints_failed_queue_hint_without_panicking() {
    let dir = unique_temp_dir("summary");
    std::fs::create_dir_all(&dir).unwrap();
    let outcome = BatchOutcome {
        success_count: 1,
        failed_tasks: vec![sample_task("Me at the zoo", "me at the zoo")],
        elapsed_secs: 65.0,
        total_bytes: 4096,
    };
    ghita_download::cli::CliApp::print_batch_summary(&outcome, &dir);
    let outcome_ok = BatchOutcome {
        success_count: 2,
        failed_tasks: Vec::new(),
        elapsed_secs: 12.5,
        total_bytes: 1024,
    };
    ghita_download::cli::CliApp::print_batch_summary(&outcome_ok, &dir);
    let _ = std::fs::remove_dir_all(&dir);
}
