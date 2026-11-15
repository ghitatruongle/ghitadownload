#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GhitaErrorClass {
    Cancelled,
    PinTool,
    Config,
    Network,
    Other,
}

pub fn classify_error(message: &str) -> GhitaErrorClass {
    let lower = message.to_lowercase();
    if lower.contains("đã hủy") || lower.contains("cancelled") || lower.contains("ctrl+c") {
        GhitaErrorClass::Cancelled
    } else if lower.contains("sha-256")
        || lower.contains("sha256")
        || lower.contains("pin desync")
        || lower.contains("nguồn yt-dlp")
    {
        GhitaErrorClass::PinTool
    } else if lower.contains("không đọc được cấu hình")
        || lower.contains("config")
        || lower.contains("json")
    {
        GhitaErrorClass::Config
    } else if lower.contains("timeout")
        || lower.contains("timed out")
        || lower.contains("kết nối")
        || lower.contains("connection")
        || lower.contains("dns")
        || lower.contains("resolve")
    {
        GhitaErrorClass::Network
    } else {
        GhitaErrorClass::Other
    }
}

pub fn classify_exit_code(message: &str) -> i32 {
    match classify_error(message) {
        GhitaErrorClass::Cancelled => 130,
        GhitaErrorClass::PinTool => 20,
        GhitaErrorClass::Config => 10,
        GhitaErrorClass::Network => 30,
        GhitaErrorClass::Other => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_error, classify_exit_code, GhitaErrorClass};

    #[test]
    fn classes_map_to_expected_exit_codes() {
        assert_eq!(classify_exit_code("Đã hủy bởi người dùng"), 130);
        assert_eq!(classify_exit_code("SHA-256 mismatch"), 20);
        assert_eq!(classify_exit_code("config json broken"), 10);
        assert_eq!(classify_exit_code("connection refused"), 30);
        assert_eq!(classify_exit_code("random failure"), 1);
    }

    #[test]
    fn classify_error_distinguishes_network_and_pin() {
        assert_eq!(
            classify_error("request timed out"),
            GhitaErrorClass::Network
        );
        assert_eq!(
            classify_error("yt-dlp pin desync"),
            GhitaErrorClass::PinTool
        );
        assert_eq!(classify_error("all good"), GhitaErrorClass::Other);
    }
}
