use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/ghitatruongle/ghitadownload/releases/latest";
const CACHE_FILE_NAME: &str = "version.json";
const CACHE_MAX_AGE_SECS: u64 = 86_400;

#[derive(Debug, Serialize, Deserialize)]
struct UpdateCache {
    latest: String,
    checked_at_secs: u64,
}

pub async fn fetch_latest_release_json() -> Result<Value> {
    let client = crate::utils::http::shared_client()?;
    crate::utils::http::with_retry(|| async {
        let response = client
            .get(LATEST_RELEASE_URL)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await?
            .error_for_status()?;
        Ok(response.json::<Value>().await?)
    })
    .await
}

pub fn latest_tag(release: &Value) -> Option<String> {
    let tag = release.get("tag_name")?.as_str()?.trim();
    if tag.is_empty() {
        return None;
    }
    Some(tag.strip_prefix('v').unwrap_or(tag).to_string())
}

pub fn asset_for_version(release: &Value, version: &str) -> Option<String> {
    find_asset(release, version)?
        .get("browser_download_url")?
        .as_str()
        .map(str::to_string)
}

fn find_asset<'a>(release: &'a Value, version: &str) -> Option<&'a Value> {
    let wanted = format!("ghitadownload_{version}.exe");
    release
        .get("assets")?
        .as_array()?
        .iter()
        .find(|asset| asset.get("name").and_then(Value::as_str) == Some(wanted.as_str()))
}

pub fn digest_sha256(asset: &Value) -> Option<String> {
    let digest = asset.get("digest")?.as_str()?.trim();
    let hex = digest.strip_prefix("sha256:")?.trim();
    let lowered = hex.to_ascii_lowercase();
    if lowered.len() == 64 && lowered.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(lowered)
    } else {
        None
    }
}

fn cache_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join("GhitaDownload").join(CACHE_FILE_NAME))
}

fn is_fresh(checked_at_secs: u64, now: SystemTime) -> bool {
    let now_secs = now
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default();
    now_secs.saturating_sub(checked_at_secs) < CACHE_MAX_AGE_SECS
}

fn read_cache(path: &Path) -> Option<UpdateCache> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn write_cache(path: &Path, latest: &str) {
    let cache = UpdateCache {
        latest: latest.to_string(),
        checked_at_secs: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or_default(),
    };
    let Ok(content) = serde_json::to_string(&cache) else {
        return;
    };
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = parent.join(format!(
        ".{}.{}.{}.tmp",
        CACHE_FILE_NAME,
        std::process::id(),
        stamp
    ));
    if std::fs::write(&temporary, content.as_bytes()).is_err() {
        let _ = std::fs::remove_file(&temporary);
        return;
    }
    if crate::utils::fs::replace_file(&temporary, path).is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
}

pub async fn check_for_update(force: bool) -> Option<String> {
    let check = async {
        let now = SystemTime::now();
        if !force {
            if let Some(path) = cache_path() {
                if let Some(cache) = read_cache(&path) {
                    if is_fresh(cache.checked_at_secs, now) {
                        return Ok::<Option<String>, anyhow::Error>(Some(cache.latest));
                    }
                }
            }
        }
        let release = fetch_latest_release_json().await?;
        let latest = latest_tag(&release).context("Phản hồi GitHub Releases thiếu tag_name")?;
        if let Some(path) = cache_path() {
            write_cache(&path, &latest);
        }
        Ok(Some(latest))
    };
    match check.await {
        Ok(Some(latest)) => {
            if crate::utils::env::version_is_newer(&latest, env!("CARGO_PKG_VERSION")) {
                Some(latest)
            } else {
                None
            }
        }
        _ => None,
    }
}

pub async fn run_self_update() -> i32 {
    let release = match fetch_latest_release_json().await {
        Ok(release) => release,
        Err(error) => {
            println!("⚠️ Không kiểm tra được bản mới: {error}");
            return 1;
        }
    };
    let Some(latest) = latest_tag(&release) else {
        println!("⚠️ Không kiểm tra được bản mới: phản hồi GitHub không có tag_name");
        return 1;
    };
    if !crate::utils::env::version_is_newer(&latest, env!("CARGO_PKG_VERSION")) {
        println!(
            "✅ Bạn đang dùng bản mới nhất ({})",
            env!("CARGO_PKG_VERSION")
        );
        return 0;
    }
    let Some(asset_url) = asset_for_version(&release, &latest) else {
        println!("❌ Bản {latest} chưa có file cài đặt trên GitHub Releases");
        return 1;
    };
    let expected_digest = find_asset(&release, &latest).and_then(digest_sha256);
    let destination = std::env::temp_dir().join(format!("ghitadownload_{latest}_setup.exe"));
    if let Err(error) = crate::utils::env::download_file_stream(&asset_url, &destination).await {
        println!("❌ Không tải được bộ cài đặt {latest}: {error}");
        return 1;
    }
    if let Some(expected) = expected_digest {
        if let Err(error) = verify_installer_sha(&destination, &expected).await {
            let _ = std::fs::remove_file(&destination);
            println!("❌ {error}");
            return 1;
        }
    }
    if let Err(error) = std::process::Command::new(&destination)
        .arg("--silent")
        .spawn()
    {
        println!(
            "❌ Không khởi chạy được trình cài đặt {}: {error}",
            destination.display()
        );
        return 1;
    }
    println!("⬆️ Đang chạy trình cài đặt {latest}, ứng dụng sẽ thoát...");
    std::process::exit(0)
}

async fn verify_installer_sha(path: &Path, expected: &str) -> Result<()> {
    let path = path.to_path_buf();
    let expected = expected.trim().to_ascii_lowercase();
    let actual = tokio::task::spawn_blocking(move || -> Result<String> {
        use sha2::{Digest, Sha256};
        let mut file = std::fs::File::open(&path)
            .with_context(|| format!("Không thể mở bộ cài để tính SHA-256: {}", path.display()))?;
        let mut hasher = Sha256::new();
        std::io::copy(&mut file, &mut hasher)
            .with_context(|| format!("Không thể đọc bộ cài để tính SHA-256: {}", path.display()))?;
        Ok(format!("{:064x}", hasher.finalize()))
    })
    .await
    .map_err(|error| anyhow::anyhow!("Không tính được SHA-256: {error}"))??;
    if actual != expected {
        anyhow::bail!("SHA-256 không khớp: mong đợi {expected}, thực tế {actual}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{asset_for_version, digest_sha256, is_fresh, latest_tag};
    use serde_json::json;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn latest_tag_strips_leading_v() {
        assert_eq!(
            latest_tag(&json!({"tag_name": "v0.0.6"})),
            Some("0.0.6".to_string())
        );
        assert_eq!(
            latest_tag(&json!({"tag_name": "0.0.7"})),
            Some("0.0.7".to_string())
        );
        assert!(latest_tag(&json!({})).is_none());
        assert!(latest_tag(&json!({"tag_name": ""})).is_none());
        assert!(latest_tag(&json!({"tag_name": 42})).is_none());
    }

    #[test]
    fn asset_for_version_matches_exact_name() {
        let release = json!({
            "assets": [
                {
                    "name": "ghitadownload_0.0.5.exe",
                    "browser_download_url": "https://example.com/old.exe"
                },
                {
                    "name": "ghitadownload_0.0.6.exe",
                    "browser_download_url": "https://example.com/new.exe"
                },
                {
                    "name": "ghitadownload_0.0.6.exe.sha256",
                    "browser_download_url": "https://example.com/wrong.exe"
                }
            ]
        });
        assert_eq!(
            asset_for_version(&release, "0.0.6"),
            Some("https://example.com/new.exe".to_string())
        );
        assert!(asset_for_version(&release, "9.9.9").is_none());
        assert!(asset_for_version(&json!({}), "0.0.6").is_none());
        let missing_url = json!({"assets": [{"name": "ghitadownload_0.0.6.exe"}]});
        assert!(asset_for_version(&missing_url, "0.0.6").is_none());
    }

    #[test]
    fn digest_sha256_parses_lowercase_hex() {
        let hex = "ABCDEF0123456789".repeat(4);
        let asset = json!({"digest": format!("sha256:{hex}")});
        assert_eq!(digest_sha256(&asset), Some(hex.to_ascii_lowercase()));
    }

    #[test]
    fn digest_sha256_rejects_non_sha_or_malformed() {
        let hex = "ABCDEF0123456789".repeat(2);
        assert!(
            digest_sha256(&json!({"digest": format!("md5:{hex}")})).is_none(),
            "digest không phải sha256 phải bị bỏ qua"
        );
        assert!(digest_sha256(&json!({"digest": "sha256:khongphihex"})).is_none());
        assert!(digest_sha256(&json!({"digest": "sha256:abcd"})).is_none());
        assert!(digest_sha256(&json!({"digest": 123})).is_none());
        assert!(digest_sha256(&json!({})).is_none());
    }

    #[test]
    fn is_fresh_respects_24h_boundary() {
        let now = UNIX_EPOCH + Duration::from_secs(100_000);
        assert!(is_fresh(100_000, now));
        assert!(is_fresh(100_000 - 86_399, now));
        assert!(!is_fresh(100_000 - 86_400, now));
        assert!(!is_fresh(100_000 - 90_000, now));
    }
}
