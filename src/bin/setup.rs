use colored::Colorize;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const EMBEDDED_APP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/embedded_app.bin"));
const EMBEDDED_YTDLP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/embedded_ytdlp.bin"));
const INSTALL_DIR_FILE: &str = "install-dir.txt";

#[derive(Debug, Default, PartialEq, Eq)]
struct SetupArgs {
    uninstall: bool,
    silent: bool,
    install_dir: Option<PathBuf>,
}

fn parse_args<I, S>(arguments: I) -> Result<SetupArgs, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut parsed = SetupArgs::default();
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = argument.as_ref().to_string_lossy();
        match value.as_ref() {
            "--uninstall" | "-u" => {
                if parsed.uninstall {
                    return Err("Không được lặp lại --uninstall".to_string());
                }
                parsed.uninstall = true;
            }
            "--silent" | "-s" => {
                if parsed.silent {
                    return Err("Không được lặp lại --silent".to_string());
                }
                parsed.silent = true;
            }
            "--install-dir" => {
                let raw = iterator
                    .next()
                    .ok_or_else(|| "--install-dir cần một đường dẫn".to_string())?;
                let path = PathBuf::from(raw.as_ref());
                if parsed.install_dir.replace(path).is_some() {
                    return Err("Không được lặp lại --install-dir".to_string());
                }
            }
            unknown if unknown.starts_with('-') => {
                return Err(format!("Tham số không hợp lệ: {unknown}"));
            }
            positional => {
                return Err(format!(
                    "Tham số vị trí không hợp lệ: {positional}; dùng --install-dir <DIR>"
                ));
            }
        }
    }
    if parsed.uninstall && parsed.silent {
        return Err("--uninstall không được kết hợp --silent".to_string());
    }
    Ok(parsed)
}

fn main() {
    let args = match parse_args(std::env::args_os().skip(1)) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("❌ {message}");
            eprintln!(
                "Cú pháp: --install-dir <DIR> [--silent] | --uninstall [--install-dir <DIR>]"
            );
            std::process::exit(2);
        }
    };

    #[cfg(windows)]
    let _ = colored::control::set_virtual_terminal(true);
    print_banner();

    let default_install_dir = dirs::data_local_dir()
        .map(|dir| dir.join("GhitaDownload"))
        .unwrap_or_else(|| PathBuf::from(r"C:\GhitaDownload"));
    let result = if args.uninstall {
        let install_dir = args
            .install_dir
            .unwrap_or_else(|| persisted_install_dir().unwrap_or(default_install_dir));
        handle_uninstall(&install_dir)
    } else {
        handle_install(&default_install_dir, &args)
    };

    if let Err(message) = &result {
        eprintln!("❌ {message}");
    }
    if !args.silent && !args.uninstall {
        println!(
            "\n{}",
            "Nhấn phím [ENTER] để đóng cửa sổ này...".yellow().bold()
        );
        let mut pause = String::new();
        let _ = io::stdin().read_line(&mut pause);
    }
    if result.is_err() {
        std::process::exit(1);
    }
}

fn print_banner() {
    let content_width: usize = 72;
    let title = format!(
        "TRÌNH CÀI ĐẶT GHITA DOWNLOADER v{} (CLI)",
        env!("CARGO_PKG_VERSION")
    );
    let padding = content_width.saturating_sub(title.chars().count());
    println!("{}", format!("╔{}╗", "═".repeat(content_width)).cyan());
    println!(
        "{}",
        format!("║ {}{} ║", title, " ".repeat(padding))
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║    Tải nhạc YouTube, Spotify, Suno AI chất lượng cao mọi lúc     ║".bright_white()
    );
    println!("{}", format!("╚{}╝", "═".repeat(content_width)).cyan());
    println!();
}

fn install_state_path() -> Result<PathBuf, String> {
    let dir = dirs::data_local_dir()
        .ok_or_else(|| "Không xác định được LOCALAPPDATA để lưu đường dẫn cài đặt".to_string())?;
    Ok(dir.join("GhitaDownload").join(INSTALL_DIR_FILE))
}

fn persisted_install_dir() -> Option<PathBuf> {
    let value = std::fs::read_to_string(install_state_path().ok()?).ok()?;
    let trimmed = value.trim();
    let path = PathBuf::from(trimmed);
    (path.is_absolute() && path.file_name().is_some()).then_some(path)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|error| format!("Không tạo được thư mục {}: {error}", parent.display()))?;
    let temporary = parent.join(format!(
        ".{}.{}.{}.tmp",
        path.file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("payload"),
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let write_result = (|| -> io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "Không ghi tệp tạm {}: {error}",
            temporary.display()
        ));
    }
    if let Err(error) = replace_file(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(format!("Không thay thế {}: {error}", path.display()));
    }
    Ok(())
}

fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(_) if destination.exists() => {
            let backup = destination.with_extension(format!(
                "{}.{}.bak",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ));
            fs::rename(destination, &backup)?;
            match fs::rename(source, destination) {
                Ok(()) => {
                    let _ = fs::remove_file(backup);
                    Ok(())
                }
                Err(error) => {
                    let _ = fs::rename(&backup, destination);
                    Err(error)
                }
            }
        }
        Err(error) => Err(error),
    }
}

fn handle_install(default_dir: &Path, args: &SetupArgs) -> Result<(), String> {
    let mut install_dir = args
        .install_dir
        .clone()
        .unwrap_or_else(|| default_dir.to_path_buf());
    if !args.silent {
        println!("📁 {}", "Vị trí cài đặt phần mềm:".bright_white().bold());
        println!("   Mặc định: {}", default_dir.display().to_string().cyan());
        if args.install_dir.is_none() {
            print!(
                "   👉 Nhấn [{}] để cài ngay, hoặc nhập đường dẫn khác: ",
                "ENTER".green().bold()
            );
            let _ = io::stdout().flush();
            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_ok() {
                let trimmed = input.trim().trim_matches('"');
                if !trimmed.is_empty() {
                    install_dir = PathBuf::from(trimmed);
                }
            }
        }
        println!();
    }
    validate_install_dir(&install_dir)?;
    let bin_dir = install_dir.join("bin");
    fs::create_dir_all(&bin_dir)
        .map_err(|error| format!("Lỗi tạo thư mục {}: {error}", bin_dir.display()))?;

    let app_path = install_dir.join("ghitadownload.exe");
    let alias_path = install_dir.join("ghita.exe");
    let ytdlp_path = bin_dir.join("yt-dlp.exe");
    let uninstall_path = install_dir.join("uninstall.bat");
    let ffmpeg_path = bin_dir.join("ffmpeg.exe");
    let state_path = install_state_path()?;
    let staged_state = state_path.with_extension("txt.stage");

    println!("📦 Đang cài đặt tệp thực thi chính (ghitadownload.exe)...");
    if !EMBEDDED_APP.is_empty() && EMBEDDED_APP != b"NO_EMBED" {
        write_atomic(&app_path, EMBEDDED_APP)?;
    } else if let Some(source) = find_local_app() {
        copy_required_file(&source, &app_path)?;
    } else {
        return Err("Không tìm thấy tệp nhúng hoặc tệp ghitadownload.exe để cài đặt!".to_string());
    }
    copy_required_file(&app_path, &alias_path)?;

    if !EMBEDDED_YTDLP.is_empty() && EMBEDDED_YTDLP != b"NO_EMBED" {
        write_atomic(&ytdlp_path, EMBEDDED_YTDLP)?;
    } else if let Some(source) = find_local_ytdlp() {
        copy_required_file(&source, &ytdlp_path)?;
    } else {
        return Err("Không tìm thấy tệp nhúng hoặc tệp yt-dlp.exe để cài đặt!".to_string());
    }

    let persisted = install_dir.to_string_lossy().into_owned();
    write_atomic(&staged_state, persisted.as_bytes())?;
    let uninstall = build_uninstall_script(&install_dir);
    write_atomic(&uninstall_path, uninstall.as_bytes())?;
    replace_file(&staged_state, &state_path)
        .map_err(|error| format!("Không lưu đường dẫn cài đặt: {error}"))?;

    println!("🔍 Đang kiểm tra công cụ chuyển mã FFmpeg...");
    let ffmpeg_exists = find_existing_tool(&ffmpeg_path)
        .or_else(|| ghita_download::utils::env::find_command("ffmpeg"))
        .or_else(ghita_download::utils::env::find_ffmpeg)
        .is_some();
    if !ffmpeg_exists {
        match ghita_download::utils::env::install_ffmpeg_auto() {
            Ok(source) if source != ffmpeg_path => {
                copy_required_file(&source, &ffmpeg_path)?;
            }
            Ok(_) => {}
            Err(error) => {
                println!("⚠️  Chưa cài được FFmpeg: {error}");
            }
        }
    }
    if find_existing_tool(&ffmpeg_path)
        .or_else(|| ghita_download::utils::env::find_command("ffmpeg"))
        .or_else(ghita_download::utils::env::find_ffmpeg)
        .is_some()
    {
        println!("✅ FFmpeg đã sẵn sàng.");
    } else {
        println!(
            "⚠️  Cài ứng dụng vẫn hoàn tất; FFmpeg chưa có. Ứng dụng sẽ yêu cầu cài khi chạy."
        );
    }

    match add_to_user_path(&install_dir) {
        Ok(true) => println!("✅ Đã thêm {} vào PATH người dùng.", install_dir.display()),
        Ok(false) => println!("ℹ️  Thư mục này đã có sẵn trong PATH người dùng."),
        Err(error) => println!("⚠️  Cảnh báo cài đặt PATH: {error}"),
    }
    println!("🎉 CÀI ĐẶT GHITA DOWNLOADER THÀNH CÔNG!");
    println!("📁 Vị trí cài đặt: {}", install_dir.display());
    println!("Đường dẫn tùy chỉnh đã được lưu để --uninstall xác định đúng thư mục.");
    Ok(())
}

fn find_existing_tool(path: &Path) -> Option<PathBuf> {
    if path.is_absolute() && path.is_file() {
        Some(path.canonicalize().ok()?)
    } else {
        None
    }
}

fn build_uninstall_script(_install_dir: &Path) -> String {
    "@echo off\r\nsetlocal\r\npowershell -NoProfile -ExecutionPolicy Bypass -Command \"$dir=[IO.Path]::GetFullPath($args[0]); $p=[Environment]::GetEnvironmentVariable('Path','User'); if ($null -eq $p) { exit 2 }; $root=$dir.TrimEnd('\\'); $n=($p -split ';' | Where-Object { $_ -and $_.TrimEnd('\\') -ne $root -and $_.TrimEnd('\\') -ne ($root + '\\bin').TrimEnd('\\') }) -join ';'; [Environment]::SetEnvironmentVariable('Path',$n,'User')\" \"%~dp0\"\r\nif errorlevel 1 exit /b 1\r\ndel /q \"%~dp0ghitadownload.exe\" 2>nul\r\ndel /q \"%~dp0ghita.exe\" 2>nul\r\ndel /q \"%~dp0uninstall.bat\" 2>nul\r\ndel /q \"%~dp0bin\\yt-dlp.exe\" 2>nul\r\ndel /q \"%~dp0bin\\ffmpeg.exe\" 2>nul\r\nrd \"%~dp0bin\" 2>nul\r\nrd \"%~dp0\" 2>nul\r\nif exist \"%~dp0\" exit /b 1\r\necho Da go bo Ghita khoi he thong thanh cong!\r\npause\r\n"
        .to_string()
}

fn validate_install_dir(path: &Path) -> Result<(), String> {
    if !path.is_absolute() || path.file_name().is_none() {
        return Err("Thư mục cài đặt phải là đường dẫn tuyệt đối".to_string());
    }
    let root = path.parent().unwrap_or_else(|| Path::new(""));
    if root.as_os_str().is_empty() || path == root {
        return Err("Không thể dùng thư mục gốc làm thư mục cài đặt hoặc gỡ cài đặt".to_string());
    }
    Ok(())
}

fn handle_uninstall(install_dir: &Path) -> Result<(), String> {
    validate_install_dir(install_dir)?;
    println!("⏳ Đang tiến hành gỡ cài đặt Ghita Downloader...");
    remove_from_user_path(install_dir)?;
    remove_file_if_exists(&install_dir.join("ghitadownload.exe"))?;
    remove_file_if_exists(&install_dir.join("ghita.exe"))?;
    remove_file_if_exists(&install_dir.join("uninstall.bat"))?;
    remove_file_if_exists(&install_dir.join("bin").join("yt-dlp.exe"))?;
    remove_file_if_exists(&install_dir.join("bin").join("ffmpeg.exe"))?;
    remove_file_if_exists(&install_dir.join("bin").join("ffprobe.exe"))?;
    remove_dir_if_exists(&install_dir.join("bin"))?;
    remove_dir_if_exists(install_dir)?;
    if let Ok(state) = install_state_path() {
        if persisted_install_dir().as_deref() == Some(install_dir) {
            remove_file_if_exists(&state)?;
        }
    }
    println!("🎉 ĐÃ GỠ CÀI ĐẶT THÀNH CÔNG!");
    Ok(())
}

fn copy_required_file(source: &Path, destination: &Path) -> Result<(), String> {
    let bytes =
        fs::read(source).map_err(|error| format!("không thể đọc {}: {error}", source.display()))?;
    if bytes.len() <= 100 {
        return Err(format!("Tệp cài đặt quá nhỏ: {}", source.display()));
    }
    write_atomic(destination, &bytes)
}

fn remove_file_if_exists(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("không thể xóa {}: {error}", path.display())),
    }
}

fn remove_dir_if_exists(path: &Path) -> Result<(), String> {
    match fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("không thể xóa thư mục {}: {error}", path.display())),
    }
}

fn trusted_candidates(file_name: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join(file_name));
            candidates.push(parent.join("bin").join(file_name));
        }
    }
    if let Some(dir) = dirs::data_local_dir() {
        candidates.push(dir.join("GhitaDownload").join(file_name));
        candidates.push(dir.join("GhitaDownload").join("bin").join(file_name));
    }
    candidates
}

fn find_local_app() -> Option<PathBuf> {
    trusted_candidates("ghitadownload.exe")
        .into_iter()
        .chain([
            PathBuf::from("target/release/ghitadownload.exe"),
            PathBuf::from("target/debug/ghitadownload.exe"),
        ])
        .find(|path| path.is_absolute() && path.is_file())
}

fn find_local_ytdlp() -> Option<PathBuf> {
    trusted_candidates("yt-dlp.exe")
        .into_iter()
        .find(|path| path.is_absolute() && path.is_file())
}

fn powershell_path() -> Result<PathBuf, String> {
    ghita_download::utils::env::find_command("powershell")
        .or_else(|| {
            std::env::var_os("SystemRoot").map(|root| {
                PathBuf::from(root)
                    .join("System32")
                    .join("WindowsPowerShell")
                    .join("v1.0")
                    .join("powershell.exe")
            })
        })
        .filter(|path| path.is_absolute() && path.is_file())
        .ok_or_else(|| "Không tìm thấy PowerShell".to_string())
}

fn read_user_path() -> Result<Option<String>, String> {
    let output = Command::new(powershell_path()?)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Environment]::GetEnvironmentVariable('Path', 'User')",
        ])
        .output()
        .map_err(|error| format!("Không đọc PATH người dùng: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Không đọc PATH người dùng (status {})",
            output.status
        ));
    }
    Ok(String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_string()))
}

fn set_user_path(value: &str) -> Result<(), String> {
    let escaped = value.replace('\'', "''");
    let script = format!(
        "[Environment]::SetEnvironmentVariable('Path', '{}', 'User')",
        escaped
    );
    let output = Command::new(powershell_path()?)
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|error| format!("Không cập nhật PATH người dùng: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Không cập nhật PATH người dùng (status {})",
            output.status
        ))
    }
}

fn add_to_user_path(dir: &Path) -> Result<bool, String> {
    let dir_str = dir.to_str().ok_or("Đường dẫn cài đặt không hợp lệ")?;
    let current = read_user_path()?.ok_or("Không đọc được PATH người dùng")?;
    let parts: Vec<&str> = current.split(';').map(str::trim).collect();
    if parts.iter().any(|part| {
        part.eq_ignore_ascii_case(dir_str)
            || part
                .trim_end_matches('\\')
                .eq_ignore_ascii_case(dir_str.trim_end_matches('\\'))
    }) {
        return Ok(false);
    }
    let updated = if current.trim().is_empty() {
        dir_str.to_string()
    } else {
        format!("{};{}", current.trim_end_matches(';'), dir_str)
    };
    set_user_path(&updated)?;
    Ok(true)
}

fn remove_from_user_path(dir: &Path) -> Result<(), String> {
    let dir_str = dir.to_str().ok_or("Đường dẫn cài đặt không hợp lệ")?;
    let current =
        read_user_path()?.ok_or("Không đọc được PATH người dùng; PATH không được ghi đè")?;
    let updated = current
        .split(';')
        .map(str::trim)
        .filter(|part| {
            !part.is_empty()
                && !part.eq_ignore_ascii_case(dir_str)
                && !part
                    .trim_end_matches('\\')
                    .eq_ignore_ascii_case(dir_str.trim_end_matches('\\'))
        })
        .collect::<Vec<_>>()
        .join(";");
    if updated != current {
        set_user_path(&updated)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_args, validate_install_dir, write_atomic};
    use std::path::{Path, PathBuf};

    #[test]
    fn parses_supported_arguments() {
        let args = parse_args(["--install-dir", r"D:\Apps\Ghita", "--silent"]).unwrap();
        assert!(args.silent);
        assert_eq!(args.install_dir, Some(PathBuf::from(r"D:\Apps\Ghita")));
    }

    #[test]
    fn rejects_unknown_and_duplicate_arguments() {
        assert!(parse_args(["--unknown"]).is_err());
        assert!(parse_args(["--silent", "--silent"]).is_err());
        assert!(parse_args(["positional"]).is_err());
    }

    #[test]
    fn rejects_root_install_directories() {
        assert!(validate_install_dir(Path::new(r"C:\")).is_err());
        assert!(validate_install_dir(Path::new("relative")).is_err());
    }

    #[test]
    fn writes_and_replaces_atomically() {
        let dir = std::env::temp_dir().join(format!("ghita_setup_atomic_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("payload.bin");
        write_atomic(&path, b"first").unwrap();
        write_atomic(&path, b"second").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
