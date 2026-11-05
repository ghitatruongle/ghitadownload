pub mod env;
pub mod file_helper;
pub mod fs;
pub mod http;

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn SetConsoleOutputCP(code_page: u32) -> i32;
}

pub fn enable_utf8_console() {
    #[cfg(windows)]
    unsafe {
        SetConsoleOutputCP(65001);
    }
}
