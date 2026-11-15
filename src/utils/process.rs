use std::process::Command;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn hide_console_window(mut cmd: Command) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = &mut cmd;
    cmd
}

pub fn hide_console_window_tokio(mut cmd: tokio::process::Command) -> tokio::process::Command {
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = &mut cmd;
    cmd
}

#[cfg(all(test, windows))]
mod tests {
    use super::CREATE_NO_WINDOW;

    #[test]
    fn create_no_window_flag_matches_win32_value() {
        assert_eq!(CREATE_NO_WINDOW, 0x0800_0000);
    }
}
