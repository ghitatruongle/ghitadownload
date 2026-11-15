use crate::utils::logger::log_line;

pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = format!("PANIC: {info}");
        log_line(&message);
        let path_text = crate::utils::logger::log_file_path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "ghita.log".to_string());
        eprintln!(
            "💥 Ứng dụng gặp lỗi nghiêm trọng, buộc phải dừng. Chi tiết đã ghi vào: {path_text}"
        );
        default_hook(info);
    }));
}

#[cfg(windows)]
pub fn install_shutdown_handlers() {
    use windows_sys::Win32::System::Console::{
        SetConsoleCtrlHandler, CTRL_BREAK_EVENT, CTRL_CLOSE_EVENT, CTRL_LOGOFF_EVENT,
        CTRL_SHUTDOWN_EVENT,
    };

    unsafe extern "system" fn handler(ctrl_type: u32) -> windows_sys::Win32::Foundation::BOOL {
        match ctrl_type {
            CTRL_BREAK_EVENT | CTRL_CLOSE_EVENT | CTRL_LOGOFF_EVENT | CTRL_SHUTDOWN_EVENT => {
                crate::core::cancel::global().cancel();
                log_line("shutdown signal: cancellation requested");
                std::process::exit(130);
            }
            _ => 0,
        }
    }

    unsafe {
        SetConsoleCtrlHandler(Some(handler), 1);
    }
}

#[cfg(not(windows))]
pub fn install_shutdown_handlers() {}
