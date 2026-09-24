use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::Read;
use std::path::Path;

fn is_windows_pe(path: &Path) -> bool {
    let mut header = [0_u8; 2];
    std::fs::File::open(path)
        .and_then(|mut file| std::io::Read::read_exact(&mut file, &mut header))
        .is_ok_and(|_| header == [0x4d, 0x5a])
}

fn file_sha256(path: &Path) -> String {
    let mut file = fs::File::open(path).expect("payload must be readable");
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file.read(&mut buffer).expect("payload must be readable");
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn expected_ytdlp_sha256() -> String {
    fs::read_to_string("release/yt-dlp.sha256")
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_default()
}

fn embed_file(
    source: &Path,
    destination: &Path,
    label: &str,
    required: bool,
    expected_sha256: Option<&str>,
) {
    let valid_source = source.is_file()
        && is_windows_pe(source)
        && fs::metadata(source)
            .map(|metadata| metadata.len() > 100)
            .unwrap_or(false);

    if valid_source {
        if let Some(expected) = expected_sha256 {
            let actual = file_sha256(source);
            if actual != expected {
                panic!("SHA-256 của {label} không khớp: expected {expected}, got {actual}");
            }
        }
        if let Err(error) = fs::copy(source, destination) {
            panic!("không thể nhúng {} từ {}: {error}", label, source.display());
        }
        if !is_windows_pe(destination)
            || destination
                .metadata()
                .map(|metadata| metadata.len() <= 100)
                .unwrap_or(true)
        {
            panic!("payload {} không hợp lệ sau khi nhúng", label);
        }
    } else if required {
        panic!("không tìm thấy {} bắt buộc cho bản phát hành", label);
    } else if let Err(error) = fs::write(destination, b"NO_EMBED") {
        panic!("không thể tạo payload phát triển cho {}: {error}", label);
    }
}

fn main() {
    println!("cargo:rerun-if-env-changed=GHITA_REQUIRE_EMBED");
    println!("cargo:rerun-if-changed=release/yt-dlp.sha256");
    println!("cargo:rerun-if-changed=target/release/ghitadownload.exe");
    println!("cargo:rerun-if-changed=target/release/ghita_download.exe");
    println!("cargo:rerun-if-changed=bin/yt-dlp.exe");

    let required = env::var("GHITA_REQUIRE_EMBED").as_deref() == Ok("1");
    let out_dir_value = env::var("OUT_DIR").expect("OUT_DIR must be set");
    let out_dir = Path::new(&out_dir_value);
    let app_source = Path::new("target/release/ghitadownload.exe");
    let fallback_app_source = Path::new("target/release/ghita_download.exe");
    let app_destination = out_dir.join("embedded_app.bin");
    let ytdlp_source = Path::new("bin/yt-dlp.exe");
    let ytdlp_destination = out_dir.join("embedded_ytdlp.bin");
    let expected_ytdlp = expected_ytdlp_sha256();

    if required && expected_ytdlp.len() != 64 {
        panic!("release/yt-dlp.sha256 không hợp lệ");
    }

    if app_source.is_file() {
        embed_file(
            app_source,
            &app_destination,
            "ghitadownload.exe",
            required,
            None,
        );
    } else if fallback_app_source.is_file() {
        embed_file(
            fallback_app_source,
            &app_destination,
            "ghita_download.exe",
            required,
            None,
        );
    } else if required {
        panic!("không tìm thấy ghitadownload.exe bắt buộc cho bản phát hành");
    } else if let Err(error) = fs::write(&app_destination, b"NO_EMBED") {
        panic!("không thể tạo payload phát triển cho ghitadownload.exe: {error}");
    }

    embed_file(
        ytdlp_source,
        &ytdlp_destination,
        "bin/yt-dlp.exe",
        required,
        if required {
            Some(expected_ytdlp.as_str())
        } else {
            None
        },
    );
}
