pub const YTDLP_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36";

pub const YTDLP_DEFAULT_ARGS: &[&str] = &[
    "--no-warnings",
    "--extractor-args",
    "youtube:player_client=android,web",
    "--user-agent",
    YTDLP_USER_AGENT,
];
