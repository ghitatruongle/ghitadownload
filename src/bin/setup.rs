use colored::Colorize;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const EMBEDDED_APP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/embedded_app.bin"));
const EMBEDDED_YTDLP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/embedded_ytdlp.bin"));

fn main() {
    #[cfg(windows)]
    let _ = colored::control::set_virtual_terminal(true);

    let args: Vec<String> = std::env::args().collect();
    let is_uninstall = args.iter().any(|a| a == "--uninstall" || a == "-u");
    let is_silent = args.iter().any(|a| a == "--silent" || a == "-s");

    print_banner();

    let default_install_dir = match dirs::data_local_dir() {
        Some(dir) => dir.join("GhitaDownload"),
        None => PathBuf::from(r"C:\GhitaDownload"),
    };

    if is_uninstall {
        handle_uninstall(&default_install_dir);
    } else {
        handle_install(&default_install_dir, is_silent);
    }

    if !is_silent {
        println!(
            "\n{}",
            "Nhấn phím [ENTER] để đóng cửa sổ này...".yellow().bold()
        );
        let mut _pause = String::new();
        let _ = io::stdin().read_line(&mut _pause);
    }
}

fn print_banner() {
    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════════════╗".cyan()
    );
    println!(
        "{}",
        "║             TRÌNH CÀI ĐẶT GHITA DOWNLOADER v0.0.2 (CLI)          ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║    Tải nhạc YouTube, Spotify, Suno AI chất lượng cao mọi lúc     ║".bright_white()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════════════╝".cyan()
    );
    println!();
}

fn handle_install(default_dir: &Path, is_silent: bool) {
    let mut install_dir = default_dir.to_path_buf();

    if !is_silent {
        println!("📁 {}", "Vị trí cài đặt phần mềm:".bright_white().bold());
        println!("   Mặc định: {}", default_dir.display().to_string().cyan());
        print!(
            "   👉 Nhấn [{}] để cài ngay, hoặc nhập đường dẫn khác: ",
            "ENTER".green().bold()
        );
        let _ = io::stdout().flush();

        let mut user_input = String::new();
        if io::stdin().read_line(&mut user_input).is_ok() {
            let trimmed = user_input.trim().trim_matches('"');
            if !trimmed.is_empty() {
                install_dir = PathBuf::from(trimmed);
            }
        }
        println!();
    }

    println!(
        "⏳ Đang chuẩn bị thư mục cài đặt: {}",
        install_dir.display().to_string().yellow()
    );
    let bin_dir = install_dir.join("bin");

    if let Err(e) = fs::create_dir_all(&bin_dir) {
        println!("❌ {}", format!("Lỗi tạo thư mục: {}", e).red().bold());
        return;
    }

    let target_app_path = install_dir.join("ghitadownload.exe");
    println!("📦 Đang cài đặt tệp thực thi chính (ghitadownload.exe)...");
    if EMBEDDED_APP.len() > 100 && EMBEDDED_APP != b"NO_EMBED" {
        if let Err(e) = fs::write(&target_app_path, EMBEDDED_APP) {
            println!("❌ {}", format!("Lỗi ghi ghitadownload.exe: {}", e).red());
            return;
        }
    } else {
        if let Some(src_app) = find_local_app() {
            if let Err(e) = fs::copy(&src_app, &target_app_path) {
                println!(
                    "❌ {}",
                    format!("Lỗi sao chép từ {}: {}", src_app.display(), e).red()
                );
                return;
            }
        } else {
            println!(
                "{}",
                "❌ Không tìm thấy tệp nhúng hoặc tệp ghitadownload.exe để cài đặt!"
                    .red()
                    .bold()
            );
            return;
        }
    }

    let target_alias_path = install_dir.join("ghita.exe");
    let _ = fs::copy(&target_app_path, &target_alias_path);

    let target_ytdlp_path = bin_dir.join("yt-dlp.exe");
    if EMBEDDED_YTDLP.len() > 100 && EMBEDDED_YTDLP != b"NO_EMBED" {
        println!("📦 Đang cài đặt công cụ hỗ trợ yt-dlp vào bin/...");
        let _ = fs::write(&target_ytdlp_path, EMBEDDED_YTDLP);
    } else if let Some(src_ytdlp) = find_local_ytdlp() {
        println!("📦 Đang sao chép công cụ hỗ trợ yt-dlp vào bin/...");
        let _ = fs::copy(src_ytdlp, &target_ytdlp_path);
    }

    println!("🔍 Đang kiểm tra công cụ chuyển mã FFmpeg...");
    let ffmpeg_exists = Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
        || bin_dir.join("ffmpeg.exe").exists()
        || install_dir.join("ffmpeg.exe").exists();

    if !ffmpeg_exists {
        println!("⚠️  Chưa phát hiện FFmpeg trên hệ thống.");
        println!("⏳ Đang tự động thiết lập FFmpeg cho máy tính của bạn...");
        let target_ffmpeg = bin_dir.join("ffmpeg.exe");
        let ps_script = format!(
            "$ProgressPreference = 'SilentlyContinue'; \
             if (Get-Command winget -ErrorAction SilentlyContinue) {{ \
                 winget install Gyan.FFmpeg --accept-package-agreements --accept-source-agreements --silent \
             }} else {{ \
                 $zip = Join-Path $env:TEMP 'ffmpeg_setup.zip'; \
                 $tmp = Join-Path $env:TEMP 'ffmpeg_setup_tmp'; \
                 Invoke-WebRequest -Uri 'https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip' -OutFile $zip; \
                 Expand-Archive -Path $zip -DestinationPath $tmp -Force; \
                 $bin = Get-ChildItem -Path $tmp -Recurse -Filter 'ffmpeg.exe' | Select-Object -First 1; \
                 if ($bin) {{ Copy-Item $bin.FullName -Destination '{}' -Force }}; \
                 Remove-Item -Path $zip -Force -ErrorAction SilentlyContinue; \
                 Remove-Item -Path $tmp -Recurse -Force -ErrorAction SilentlyContinue; \
             }}",
            target_ffmpeg.display().to_string().replace('\\', "\\\\")
        );
        let _ = Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(&ps_script)
            .status();

        if Command::new("ffmpeg")
            .arg("-version")
            .output()
            .is_ok_and(|o| o.status.success())
            || bin_dir.join("ffmpeg.exe").exists()
        {
            println!("✅ Đã thiết lập FFmpeg thành công!");
        } else {
            println!("⚠️  Chưa tự động cài được FFmpeg. Bạn có thể cài trong app bằng 1-click hoặc lệnh: winget install Gyan.FFmpeg");
        }
    } else {
        println!("✅ Đã phát hiện FFmpeg trên hệ thống.");
    }

    let uninstall_bat = install_dir.join("uninstall.bat");
    let bat_content = format!(
        "@echo off\r\n\
        title Go cai dat Ghita Downloader\r\n\
        echo Dang go cai dat Ghita Downloader khoi he thong...\r\n\
        powershell -NoProfile -ExecutionPolicy Bypass -Command \"$curr = [Environment]::GetEnvironmentVariable('Path', 'User'); $newP = ($curr -split ';' | Where-Object {{ $_.TrimEnd('\\') -ne '{dir}' -and $_.TrimEnd('\\') -ne '{dir}\\bin' }} | Out-String).Trim() -replace '`r`n', ';'; [Environment]::SetEnvironmentVariable('Path', $newP, 'User')\"\r\n\
        del /q \"%~dp0ghitadownload.exe\" 2>nul\r\n\
        del /q \"%~dp0ghita.exe\" 2>nul\r\n\
        del /q \"%~dp0bin\\yt-dlp.exe\" 2>nul\r\n\
        del /q \"%~dp0bin\\ffmpeg.exe\" 2>nul\r\n\
        rd /s /q \"%~dp0bin\" 2>nul\r\n\
        echo Da go bo Ghita khoi bien moi truong PATH va he thong thanh cong!\r\n\
        pause\r\n",
        dir = install_dir.display().to_string().replace('\\', "\\\\")
    );
    let _ = fs::write(uninstall_bat, bat_content);

    println!("⚙️  Đang cấu hình biến môi trường PATH người dùng...");
    match add_to_user_path(&install_dir) {
        Ok(true) => {
            println!(
                "✅ Đã thêm {} vào biến môi trường PATH!",
                install_dir.display().to_string().green()
            );
        }
        Ok(false) => {
            println!("ℹ️  Thư mục này đã có sẵn trong biến môi trường PATH.");
        }
        Err(e) => {
            println!("⚠️  Cảnh báo cài đặt PATH: {}", e.yellow());
        }
    }

    println!();
    println!(
        "{}",
        "══════════════════════════════════════════════════════════════════".green()
    );
    println!(
        "{}",
        "🎉 CÀI ĐẶT GHITA DOWNLOADER THÀNH CÔNG!".green().bold()
    );
    println!(
        "{}",
        "══════════════════════════════════════════════════════════════════".green()
    );
    println!(
        "📁 Vị trí cài đặt  : {}",
        install_dir.display().to_string().cyan()
    );
    println!(
        "🚀 Lệnh khởi chạy  : {} hoặc {}",
        "ghitadownload".yellow().bold(),
        "ghita".yellow().bold()
    );
    println!();
    println!("{}", "💡 CÁCH SỬ DỤNG:".bright_white().bold());
    println!("   1. Mở cửa sổ Terminal hoặc PowerShell/CMD mới tại bất kỳ thư mục nào.");
    println!(
        "   2. Gõ lệnh: {} (hoặc {})",
        "ghitadownload".green().bold(),
        "ghita".green().bold()
    );
    println!("   3. Ứng dụng sẽ mở ngay tại thư mục hiện tại của bạn để lưu bài hát!");
    println!();
    println!(
        "🗑️  Khi cần gỡ cài đặt, bạn chỉ cần chạy: {} hoặc file {}",
        "ghitadownload-setup.exe --uninstall".yellow(),
        "uninstall.bat".yellow()
    );
    println!(
        "{}",
        "══════════════════════════════════════════════════════════════════".green()
    );
}

fn handle_uninstall(install_dir: &Path) {
    println!("⏳ Đang tiến hành gỡ cài đặt Ghita Downloader...");

    match remove_from_user_path(install_dir) {
        Ok(_) => println!("✅ Đã xóa đường dẫn khỏi biến môi trường PATH người dùng."),
        Err(e) => println!("⚠️  Lỗi cập nhật PATH: {e}"),
    }

    let _ = fs::remove_file(install_dir.join("ghitadownload.exe"));
    let _ = fs::remove_file(install_dir.join("ghita.exe"));
    let _ = fs::remove_file(install_dir.join("uninstall.bat"));
    let _ = fs::remove_file(install_dir.join("bin").join("yt-dlp.exe"));
    let _ = fs::remove_dir(install_dir.join("bin"));
    let _ = fs::remove_dir(install_dir);

    println!();
    println!("{}", "🎉 ĐÃ GỠ CÀI ĐẶT THÀNH CÔNG!".green().bold());
}

fn find_local_app() -> Option<PathBuf> {
    let candidates = [
        "ghitadownload.exe",
        "ghita_download.exe",
        "target/release/ghitadownload.exe",
        "target/release/ghita_download.exe",
        "target/debug/ghitadownload.exe",
        "target/debug/ghita_download.exe",
    ];

    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
        if let Ok(current_exe) = std::env::current_exe() {
            if let Some(parent) = current_exe.parent() {
                let adjacent = parent.join(c);
                if adjacent.exists() {
                    return Some(adjacent);
                }
            }
        }
    }
    None
}

fn find_local_ytdlp() -> Option<PathBuf> {
    let candidates = ["bin/yt-dlp.exe", "./yt-dlp.exe"];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
        if let Ok(current_exe) = std::env::current_exe() {
            if let Some(parent) = current_exe.parent() {
                let adjacent = parent.join(c);
                if adjacent.exists() {
                    return Some(adjacent);
                }
            }
        }
    }
    None
}

fn add_to_user_path(dir: &Path) -> Result<bool, String> {
    let dir_str = dir.to_str().ok_or("Đường dẫn không hợp lệ")?;

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "[Environment]::GetEnvironmentVariable('Path', 'User')",
        ])
        .output()
        .map_err(|e| e.to_string())?;

    let current_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let parts: Vec<&str> = current_path.split(';').map(str::trim).collect();

    let already_in = parts.iter().any(|&p| {
        p.eq_ignore_ascii_case(dir_str)
            || p.trim_end_matches('\\')
                .eq_ignore_ascii_case(dir_str.trim_end_matches('\\'))
    });

    if already_in {
        return Ok(false);
    }

    let new_path = if current_path.is_empty() {
        dir_str.to_string()
    } else {
        format!("{};{}", current_path.trim_end_matches(';'), dir_str)
    };

    let set_cmd = format!(
        "[Environment]::SetEnvironmentVariable('Path', '{}', 'User')",
        new_path.replace('\'', "''")
    );

    let status = Command::new("powershell")
        .args(["-NoProfile", "-Command", &set_cmd])
        .status()
        .map_err(|e| e.to_string())?;

    if status.success() {
        Ok(true)
    } else {
        Err("Không thể cập nhật PATH qua PowerShell".to_string())
    }
}

fn remove_from_user_path(dir: &Path) -> Result<(), String> {
    let dir_str = dir.to_str().ok_or("Đường dẫn không hợp lệ")?;

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "[Environment]::GetEnvironmentVariable('Path', 'User')",
        ])
        .output()
        .map_err(|e| e.to_string())?;

    let current_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let parts: Vec<&str> = current_path
        .split(';')
        .map(str::trim)
        .filter(|&p| {
            !p.is_empty()
                && !p.eq_ignore_ascii_case(dir_str)
                && !p
                    .trim_end_matches('\\')
                    .eq_ignore_ascii_case(dir_str.trim_end_matches('\\'))
        })
        .collect();

    let new_path = parts.join(";");
    let set_cmd = format!(
        "[Environment]::SetEnvironmentVariable('Path', '{}', 'User')",
        new_path.replace('\'', "''")
    );

    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", &set_cmd])
        .status();

    Ok(())
}
