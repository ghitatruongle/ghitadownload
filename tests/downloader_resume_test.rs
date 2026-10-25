#![cfg(windows)]

use std::fs;
use std::path::{Path, PathBuf};

use ghita_download::core::downloader::{DownloadMode, StreamDownloader};

const FAKE_PS1: &str = r#"
$o = $false
$tpl = $null
foreach ($a in $args) {
    if ($o) { $tpl = $a; break }
    if ($a -eq '-o') { $o = $true }
}
$dir = Split-Path -Parent $MyInvocation.MyCommand.Path
$state = Join-Path $dir 'fake_state.txt'
if (-not $tpl) { exit 9 }
if (Test-Path -LiteralPath $state) {
    $final = $tpl.Replace('%(ext)s', 'm4a')
    Set-Content -LiteralPath $final -Value 'COMPLETE'
    Set-Content -LiteralPath (Join-Path $dir 'attempt2_template.txt') -Value $tpl
    exit 0
}
Set-Content -LiteralPath $state -Value 'attempt1'
$part = $tpl.Replace('%(ext)s', 'm4a') + '.part'
Set-Content -LiteralPath $part -Value 'PARTIAL'
Set-Content -LiteralPath (Join-Path $dir 'attempt1_template.txt') -Value $tpl
exit 1
"#;

const FAKE_BAT: &str = "@echo off\r\n@powershell -NoProfile -ExecutionPolicy Bypass -File \"%~dp0fake_ytdlp.ps1\" %*\r\n";

fn prepare_fake_tool(dir: &Path) -> PathBuf {
    let _ = fs::remove_dir_all(dir);
    fs::create_dir_all(dir).expect("create fake tool dir");
    let ps1 = dir.join("fake_ytdlp.ps1");
    let bat = dir.join("fake_ytdlp.bat");
    fs::write(&ps1, FAKE_PS1.trim_start()).expect("write fake ps1");
    fs::write(&bat, FAKE_BAT).expect("write fake bat");
    bat
}

fn read_line(dir: &Path, name: &str) -> String {
    fs::read_to_string(dir.join(name))
        .unwrap_or_else(|e| panic!("missing {name}: {e}"))
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

#[tokio::test]
async fn download_stream_reuses_output_template_across_retry_attempts() {
    let dir = std::env::temp_dir().join("ghita_resume_fake_test");
    let bat = prepare_fake_tool(&dir);
    let downloader = StreamDownloader::new(&bat, false, None, None);
    let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);

    let result = downloader
        .download_stream(
            "https://example.test/media",
            &dir,
            DownloadMode::Audio,
            None,
            cancel_rx,
        )
        .await
        .expect("download_stream must succeed on the resumed attempt");
    let _ = cancel_tx;

    let template_one = read_line(&dir, "attempt1_template.txt");
    let template_two = read_line(&dir, "attempt2_template.txt");
    assert_eq!(
        template_one, template_two,
        "retry attempts must target the identical output template"
    );

    let expected_final = PathBuf::from(template_one.replace("%(ext)s", "m4a"));
    assert_eq!(
        result, expected_final,
        "final file must live at the same stem attempt 1 left its partial data at"
    );

    let partial = expected_final.with_file_name(format!(
        "{}.part",
        expected_final.file_name().unwrap().to_string_lossy()
    ));
    assert_eq!(
        fs::read_to_string(&partial)
            .expect("attempt-1 .part must exist")
            .trim(),
        "PARTIAL",
        "the partial bytes written by attempt 1 prove the resume point"
    );
    let _ = fs::remove_dir_all(&dir);
}
