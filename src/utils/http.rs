use anyhow::Result;
use std::sync::OnceLock;
use std::time::Duration;

static SHARED: OnceLock<reqwest::Client> = OnceLock::new();

const SHARED_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36";

pub fn shared_client() -> Result<&'static reqwest::Client> {
    if let Some(client) = SHARED.get() {
        return Ok(client);
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .user_agent(SHARED_USER_AGENT)
        .build()?;
    let _ = SHARED.set(client);
    SHARED
        .get()
        .ok_or_else(|| anyhow::anyhow!("Không thể khởi tạo HTTP client"))
}

pub fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS
}

pub fn is_retryable_error(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        if let Some(req_err) = cause.downcast_ref::<reqwest::Error>() {
            req_err.is_timeout()
                || req_err.is_connect()
                || req_err.status().map(is_retryable_status).unwrap_or(false)
        } else {
            false
        }
    })
}

pub async fn with_retry<T, F, Fut>(mut op: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut attempt = 0u8;
    loop {
        match op().await {
            Ok(value) => return Ok(value),
            Err(err) => {
                if attempt < 2 && is_retryable_error(&err) {
                    let delay_ms = if attempt == 0 { 500 } else { 1500 };
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    attempt += 1;
                } else {
                    return Err(err);
                }
            }
        }
    }
}
