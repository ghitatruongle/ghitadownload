#![cfg(windows)]

use colored::Colorize;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use winreg::enums::{RegType, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
use winreg::types::{FromRegValue, ToRegValue};
use winreg::{RegKey, RegValue};

const EMBEDDED_APP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/embedded_app.bin"));
const EMBEDDED_YTDLP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/embedded_ytdlp.bin"));
const INSTALL_DIR_FILE: &str = "install-dir.txt";
const CONFIG_FILE: &str = "ghita_config.json";

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn SetConsoleOutputCP(code_page: u32) -> i32;
}

fn enable_utf8_console() {
    #[cfg(windows)]
    unsafe {
        SetConsoleOutputCP(65001);
    }
}

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
                let raw = raw.as_ref().to_string_lossy().into_owned();
                if raw.starts_with('-') {
                    return Err(format!(
                        "--install-dir cần một đường dẫn, không nhận tham số: {raw}"
                    ));
                }
                let path = PathBuf::from(raw);
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
    enable_utf8_console();
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

fn state_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join("GhitaDownload"))
}

fn global_config_path() -> Option<PathBuf> {
    state_dir().map(|dir| dir.join(CONFIG_FILE))
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
    if let Err(error) = ghita_download::utils::fs::replace_file(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(format!("Không thay thế {}: {error}", path.display()));
    }
    Ok(())
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
    let state_path = match install_state_path() {
        Ok(path) => Some(path),
        Err(error) => {
            println!("⚠️  Bỏ qua lưu đường dẫn cài đặt: {error}");
            None
        }
    };
    let staged_state = state_path
        .as_ref()
        .map(|path| path.with_extension("txt.stage"));

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

    let uninstall = build_uninstall_script(&install_dir);
    write_atomic(&uninstall_path, uninstall.as_bytes())?;
    if let (Some(staged), Some(state)) = (staged_state, state_path) {
        let persisted = install_dir.to_string_lossy().into_owned();
        write_atomic(&staged, persisted.as_bytes())
            .and_then(|()| {
                ghita_download::utils::fs::replace_file(&staged, &state)
                    .map_err(|error| format!("Không lưu đường dẫn cài đặt: {error}"))
            })
            .unwrap_or_else(|error| {
                let _ = std::fs::remove_file(&staged);
                println!("⚠️  Không lưu được đường dẫn cài đặt: {error}");
                println!("   --uninstall sẽ dùng thư mục cài đặt mặc định.");
            });
    }

    println!("🔍 Đang kiểm tra công cụ chuyển mã FFmpeg...");
    let ffmpeg_exists = find_existing_tool(&ffmpeg_path)
        .or_else(|| ghita_download::utils::env::find_command("ffmpeg"))
        .or_else(ghita_download::utils::env::find_ffmpeg)
        .is_some();
    if !ffmpeg_exists {
        match install_ffmpeg_blocking() {
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
    println!("⚖️  Chỉ tải nội dung bạn có quyền sử dụng. Tải từ nền tảng có thể vi phạm điều khoản dịch vụ của họ. Xem mục 'Lưu Ý Pháp Lý' trong README.");
    Ok(())
}

fn strip_verbatim_prefix(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{stripped}"))
    } else if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

fn find_existing_tool(path: &Path) -> Option<PathBuf> {
    if path.is_absolute() && path.is_file() {
        let canonical = path.canonicalize().ok()?;
        Some(strip_verbatim_prefix(&canonical))
    } else {
        None
    }
}

fn build_uninstall_script(install_dir: &Path) -> String {
    let dir_literal = install_dir.to_string_lossy().replace('\'', "''");
    format!(
        "@echo off\r\nsetlocal\r\npowershell -NoProfile -ExecutionPolicy Bypass -Command \"$dir=[IO.Path]::GetFullPath('{}'); $p=[Environment]::GetEnvironmentVariable('Path','User'); if ($null -eq $p) {{ exit 2 }}; $root=$dir.TrimEnd('\\'); $n=($p -split ';' | Where-Object {{ $cur=$_.Replace('/','\\'); $cur -and $cur.TrimEnd('\\') -ne $root -and $cur.TrimEnd('\\') -ne ($root + '\\bin').TrimEnd('\\') }}) -join ';'; [Environment]::SetEnvironmentVariable('Path',$n,'User')\"\r\nif errorlevel 1 exit /b 1\r\nfor %%F in (\"%~dp0ghita_config.json\" \"%LOCALAPPDATA%\\GhitaDownload\\ghita_config.json\") do if exist %%F del /q %%F 2>nul\r\ndel /q \"%~dp0ghitadownload.exe\" 2>nul\r\ndel /q \"%~dp0ghita.exe\" 2>nul\r\ndel /q \"%~dp0uninstall.bat\" 2>nul\r\ndel /q \"%~dp0bin\\yt-dlp.exe\" 2>nul\r\ndel /q \"%~dp0bin\\ffmpeg.exe\" 2>nul\r\nrd \"%~dp0bin\" 2>nul\r\nrd \"%~dp0\" 2>nul\r\nrd \"%LOCALAPPDATA%\\GhitaDownload\" 2>nul\r\nif exist \"%~dp0\" exit /b 1\r\necho Da go bo Ghita khoi he thong thanh cong!\r\npause\r\n",
        dir_literal
    )
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
    for name in uninstall_config_names(install_dir) {
        remove_file_if_exists(&install_dir.join(name))?;
    }
    remove_dir_if_exists(install_dir)?;
    if let Ok(state) = install_state_path() {
        if persisted_install_dir().as_deref() == Some(install_dir) {
            remove_file_if_exists(&state)?;
        }
    }
    if let Some(global_config) = global_config_path() {
        if let Some(state_dir) = state_dir() {
            for name in uninstall_config_names(&state_dir) {
                remove_file_if_exists(&global_config.with_file_name(name))?;
            }
            if state_dir == install_dir {
                remove_dir_if_exists(&state_dir)?;
            } else {
                let _ = std::fs::remove_dir(&state_dir);
            }
        }
    }
    println!("🎉 ĐÃ GỠ CÀI ĐẶT THÀNH CÔNG!");
    Ok(())
}

fn uninstall_config_names(dir: &Path) -> Vec<String> {
    let mut names = vec![CONFIG_FILE.to_string()];
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(&format!("{CONFIG_FILE}.")) {
                names.push(name);
            }
        }
    }
    names
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
        .find(|path| path.is_file())
}

fn find_local_ytdlp() -> Option<PathBuf> {
    trusted_candidates("yt-dlp.exe")
        .into_iter()
        .find(|path| path.is_file())
}

fn install_ffmpeg_blocking() -> Result<PathBuf, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Không khởi tạo được runtime tải FFmpeg: {error}"))?;
    runtime
        .block_on(ghita_download::utils::env::install_ffmpeg_auto())
        .map_err(|error| error.to_string())
}

fn open_user_environment_key() -> Result<RegKey, String> {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .map_err(|error| format!("Không mở được registry HKCU\\Environment: {error}"))
}

fn read_user_path_value(key: &RegKey) -> Result<(Option<RegValue>, String), String> {
    match key.get_raw_value("Path") {
        Ok(value) => {
            if matches!(value.vtype, RegType::REG_SZ | RegType::REG_EXPAND_SZ) {
                let text = String::from_reg_value(&value)
                    .map_err(|error| format!("Không giải mã được PATH người dùng: {error}"))?;
                Ok((Some(value), text))
            } else {
                Err("PATH người dùng có kiểu registry không hỗ trợ".to_string())
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok((None, String::new())),
        Err(error) => Err(format!("Không đọc PATH người dùng: {error}")),
    }
}

fn make_user_path_value(text: &str, value_type: RegType) -> RegValue {
    let mut value = text.to_reg_value();
    value.vtype = value_type;
    value
}

fn broadcast_environment_change() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
    };
    let parameter: Vec<u16> = "Environment\0".encode_utf16().collect();
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            parameter.as_ptr() as _,
            SMTO_ABORTIFHUNG,
            1000,
            std::ptr::null_mut(),
        );
    }
}

fn try_update_user_path<F>(transform: &F) -> Result<bool, String>
where
    F: Fn(&str) -> Option<String>,
{
    let key = open_user_environment_key()?;
    let (existing, current) = read_user_path_value(&key)?;
    let Some(updated) = transform(&current) else {
        return Ok(false);
    };
    let value_type = match &existing {
        Some(value) if matches!(value.vtype, RegType::REG_SZ) => RegType::REG_SZ,
        _ => RegType::REG_EXPAND_SZ,
    };
    key.set_raw_value("Path", &make_user_path_value(&updated, value_type))
        .map_err(|error| format!("Không ghi PATH người dùng: {error}"))?;
    broadcast_environment_change();
    Ok(true)
}

fn update_user_path<F>(transform: F) -> Result<bool, String>
where
    F: Fn(&str) -> Option<String>,
{
    let mut last_error: Option<String> = None;
    for _ in 0..3 {
        match try_update_user_path(&transform) {
            Ok(changed) => return Ok(changed),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| "Không cập nhật được PATH người dùng".to_string()))
}

fn normalize_path_entry(entry: &str) -> String {
    entry
        .trim()
        .trim_matches('"')
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

fn path_entry_present(current: &str, dir_str: &str) -> bool {
    let target = normalize_path_entry(dir_str);
    current.split(';').any(|part| {
        let normalized = normalize_path_entry(part);
        !normalized.is_empty() && normalized == target
    })
}

fn append_path_entry(current: &str, dir_str: &str) -> String {
    if current.trim().is_empty() {
        dir_str.to_string()
    } else {
        format!("{};{}", current.trim_end_matches(';'), dir_str)
    }
}

fn remove_path_entry(current: &str, dir_str: &str) -> Option<String> {
    let target = normalize_path_entry(dir_str);
    let updated = current
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty() && normalize_path_entry(part) != target)
        .collect::<Vec<_>>()
        .join(";");
    (updated != current).then_some(updated)
}

fn add_to_user_path(dir: &Path) -> Result<bool, String> {
    let dir_str = dir.to_str().ok_or("Đường dẫn cài đặt không hợp lệ")?;
    update_user_path(|current| {
        if path_entry_present(current, dir_str) {
            None
        } else {
            Some(append_path_entry(current, dir_str))
        }
    })
}

fn remove_from_user_path(dir: &Path) -> Result<(), String> {
    let dir_str = dir.to_str().ok_or("Đường dẫn cài đặt không hợp lệ")?;
    update_user_path(|current| remove_path_entry(current, dir_str)).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::{
        append_path_entry, build_uninstall_script, normalize_path_entry, parse_args,
        path_entry_present, remove_path_entry, uninstall_config_names, validate_install_dir,
        write_atomic,
    };
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
    fn install_dir_flag_does_not_swallow_next_flag() {
        assert!(parse_args(["--install-dir", "--silent"]).is_err());
        assert!(parse_args(["--install-dir", "-s"]).is_err());
        assert!(parse_args(["--install-dir", "--uninstall"]).is_err());
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

    #[test]
    fn uninstall_script_embeds_install_dir_without_args_placeholder() {
        let script = build_uninstall_script(Path::new(r"C:\Apps\Ghita"));
        assert!(script.contains("[IO.Path]::GetFullPath('C:\\Apps\\Ghita')"));
        assert!(!script.contains("$args[0]"));
        assert!(!script.contains("\"%~dp0\"\r\nif errorlevel"));
        assert!(script.contains("SetEnvironmentVariable('Path',$n,'User')"));
    }

    #[test]
    fn uninstall_script_escapes_single_quotes_in_install_dir() {
        let script = build_uninstall_script(Path::new(r"C:\Apps\Ghita's Music"));
        assert!(script.contains("[IO.Path]::GetFullPath('C:\\Apps\\Ghita''s Music')"));
    }

    #[test]
    fn uninstall_script_normalizes_forward_slash_path_entries() {
        let script = build_uninstall_script(Path::new("C:/Apps/Ghita"));
        assert!(script.contains("[IO.Path]::GetFullPath('C:/Apps/Ghita')"));
        assert!(script.contains(".Replace('/','\\')"));
    }

    #[test]
    fn uninstall_script_cleans_config_and_state_dir() {
        let script = build_uninstall_script(Path::new(r"C:\Apps\Ghita"));
        assert!(script.contains("ghita_config.json"));
        assert!(script.contains("rd \"%LOCALAPPDATA%\\GhitaDownload\" 2>nul"));
    }

    #[test]
    fn path_entries_normalize_case_slashes_and_quotes() {
        assert_eq!(
            normalize_path_entry(r#"  "C:/Apps/Ghita/"  "#),
            r"c:\apps\ghita".to_string()
        );
        assert_eq!(
            normalize_path_entry(r"C:\Apps\Ghita"),
            r"c:\apps\ghita".to_string()
        );
    }

    #[test]
    fn user_path_add_and_remove_are_idempotent() {
        let dir = r"C:\Apps\Ghita";
        let add_once = |current: &str| -> Option<String> {
            if path_entry_present(current, dir) {
                None
            } else {
                Some(append_path_entry(current, dir))
            }
        };
        let once = add_once("").unwrap();
        assert_eq!(once, r"C:\Apps\Ghita");
        assert!(add_once(&once).is_none());
        assert!(path_entry_present(&once, r"c:/apps/ghita/"));
        assert!(path_entry_present(
            &format!("C:\\Other;{}", once),
            r"C:\Apps\Ghita"
        ));
        let removed = remove_path_entry(&format!("C:\\Other;{}", once), dir).unwrap();
        assert_eq!(removed, r"C:\Other");
        assert!(remove_path_entry(&once, dir).unwrap().is_empty());
        assert!(remove_path_entry(&once, dir).is_some());
        assert!(remove_path_entry("", dir).is_none());
    }

    #[test]
    fn uninstall_config_names_includes_corrupt_backups() {
        let dir = std::env::temp_dir().join(format!("ghita_setup_config_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ghita_config.json"), b"{}").unwrap();
        std::fs::write(dir.join("ghita_config.json.corrupt-123"), b"bad").unwrap();
        std::fs::write(dir.join("other.json"), b"keep").unwrap();
        let names = uninstall_config_names(&dir);
        assert!(names.iter().any(|name| name == "ghita_config.json"));
        assert!(names
            .iter()
            .any(|name| name == "ghita_config.json.corrupt-123"));
        assert!(!names.iter().any(|name| name == "other.json"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Trình cài đặt Ghita Downloader chỉ hỗ trợ Windows.");
    std::process::exit(1);
}
