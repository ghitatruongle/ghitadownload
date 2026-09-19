use std::path::PathBuf;
use std::process::Command;

pub fn find_ytdlp() -> Option<PathBuf> {
    let local_bin = PathBuf::from("./bin/yt-dlp.exe");
    if local_bin.exists() {
        return Some(local_bin);
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let next_to_exe = parent.join("yt-dlp.exe");
            if next_to_exe.exists() {
                return Some(next_to_exe);
            }
            let in_exe_bin = parent.join("bin").join("yt-dlp.exe");
            if in_exe_bin.exists() {
                return Some(in_exe_bin);
            }
        }
    }

    if is_command_available("yt-dlp") {
        return Some(PathBuf::from("yt-dlp"));
    }

    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let pattern = format!(
            r"{}\Packages\PythonSoftwareFoundation.Python.3.13_qbz5n2kfra8p0\LocalCache\local-packages\Python313\Scripts\yt-dlp.exe",
            local_app_data
        );
        let python_path = PathBuf::from(pattern);
        if python_path.exists() {
            return Some(python_path);
        }
    }

    None
}

pub fn find_ffmpeg() -> Option<PathBuf> {
    if is_command_available("ffmpeg") {
        return Some(PathBuf::from("ffmpeg"));
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let next_to_exe = parent.join("ffmpeg.exe");
            if next_to_exe.exists() {
                return Some(next_to_exe);
            }
        }
    }

    None
}

pub fn has_node() -> bool {
    is_command_available("node")
}

fn is_command_available(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("-version")
        .output()
        .or_else(|_| Command::new(cmd).arg("--version").output())
        .map(|out| out.status.success())
        .unwrap_or(false)
}
