use anyhow::Result;
use std::sync::OnceLock;
use std::time::Duration;

static SHARED: OnceLock<reqwest::Client> = OnceLock::new();
static DOWNLOAD: OnceLock<reqwest::Client> = OnceLock::new();

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

pub fn download_client() -> Result<&'static reqwest::Client> {
    if let Some(client) = DOWNLOAD.get() {
        return Ok(client);
    }
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .user_agent(SHARED_USER_AGENT)
        .build()?;
    let _ = DOWNLOAD.set(client);
    DOWNLOAD
        .get()
        .ok_or_else(|| anyhow::anyhow!("Không thể khởi tạo HTTP client tải xuống"))
}

pub fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    matches!(status.as_u16(), 408 | 429 | 500 | 502 | 503 | 504)
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
    let cancellation = crate::core::cancel::global();
    let mut cancel = cancellation.subscribe();
    if *cancel.borrow() {
        return Err(anyhow::anyhow!("Đã hủy"));
    }
    let mut attempt = 0u8;
    loop {
        match op().await {
            Ok(value) => return Ok(value),
            Err(err) => {
                if attempt < 2 && is_retryable_error(&err) {
                    let base_ms: u64 = if attempt == 0 { 500 } else { 1500 };
                    let jitter_ms = (std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos()
                        % 250) as u64;
                    let delay_ms = base_ms.saturating_add(jitter_ms);
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_millis(delay_ms)) => {}
                        _ = cancel.changed() => {
                            return Err(anyhow::anyhow!("Đã hủy"));
                        }
                    }
                    attempt += 1;
                } else {
                    return Err(err);
                }
            }
        }
    }
}
