use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=target/release/ghitadownload.exe");
    println!("cargo:rerun-if-changed=target/release/ghita_download.exe");
    println!("cargo:rerun-if-changed=bin/yt-dlp.exe");

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR must be set");
    let app_dest = Path::new(&out_dir).join("embedded_app.bin");
    let ytdlp_dest = Path::new(&out_dir).join("embedded_ytdlp.bin");

    let release_app = Path::new("target/release/ghitadownload.exe");
    let fallback_app = Path::new("target/release/ghita_download.exe");

    if release_app.exists() {
        let _ = fs::copy(release_app, &app_dest);
    } else if fallback_app.exists() {
        let _ = fs::copy(fallback_app, &app_dest);
    } else {
        let _ = fs::write(&app_dest, b"NO_EMBED");
    }

    let ytdlp_path = Path::new("bin/yt-dlp.exe");
    if ytdlp_path.exists() {
        let _ = fs::copy(ytdlp_path, &ytdlp_dest);
    } else {
        let _ = fs::write(&ytdlp_dest, b"NO_EMBED");
    }
}
