//! Shared HTTP agent. Uses the OS trust store (Keychain on macOS) and honours
//! HTTPS_PROXY.

use std::sync::OnceLock;
use std::time::Duration;

pub const USER_AGENT: &str = concat!("EveryWorld/", env!("CARGO_PKG_VERSION"), " (open-world game; github.com/curnpoo/everyworld)");

static OFFLINE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Refuse every request from now on (cache hits still work). Used by the determinism suite so
/// a test can never depend on the network. `EW_OFFLINE=1` does the same from the environment.
pub fn set_offline(on: bool) {
    OFFLINE.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn offline() -> bool {
    OFFLINE.load(std::sync::atomic::Ordering::Relaxed) || std::env::var_os("EW_OFFLINE").is_some_and(|v| v != "0")
}

/// Requests made (or refused) since start; the determinism suite reads it to prove it stayed offline.
pub static REQUESTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn guard(url: &str) -> anyhow::Result<()> {
    REQUESTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if offline() {
        anyhow::bail!("offline: refused {url}");
    }
    Ok(())
}

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        let tls = ureq::tls::TlsConfig::builder()
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .build();
        ureq::Agent::config_builder()
            .tls_config(tls)
            .timeout_global(Some(Duration::from_secs(30)))
            .user_agent(USER_AGENT)
            .build()
            .into()
    })
}

pub fn get_bytes(url: &str) -> anyhow::Result<Vec<u8>> {
    guard(url)?;
    let mut resp = agent().get(url).call()?;
    let bytes = resp.body_mut().with_config().limit(64 * 1024 * 1024).read_to_vec()?;
    Ok(bytes)
}

/// [`get_bytes`] with extra request headers (some APIs, NVDB among them, ask clients to name themselves).
pub fn get_bytes_with(url: &str, headers: &[(&str, &str)]) -> anyhow::Result<Vec<u8>> {
    guard(url)?;
    let mut req = agent().get(url);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let mut resp = req.call()?;
    Ok(resp.body_mut().with_config().limit(64 * 1024 * 1024).read_to_vec()?)
}

/// Like [`get_bytes`] but a 404/204 (common for empty ocean tiles) is `Ok(None)`.
pub fn get_bytes_opt(url: &str) -> anyhow::Result<Option<Vec<u8>>> {
    guard(url)?;
    match agent().get(url).call() {
        Ok(mut resp) if resp.status() == 204 => {
            let _ = resp.body_mut().read_to_vec();
            Ok(None)
        }
        Ok(mut resp) => Ok(Some(resp.body_mut().with_config().limit(64 * 1024 * 1024).read_to_vec()?)),
        Err(ureq::Error::StatusCode(404)) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Fetch `len` bytes at `start` with an HTTP Range request. Returns the bytes
/// and the total object length (from `Content-Range`); 404 is `Ok(None)`.
/// A server that ignores `Range` (plain 200) is an error, not a silent 400 MB read.
pub fn get_range(url: &str, start: u64, len: u64) -> anyhow::Result<Option<(Vec<u8>, u64)>> {
    guard(url)?;
    let range = format!("bytes={}-{}", start, start + len - 1);
    match agent().get(url).header("Range", range).call() {
        Ok(mut resp) => {
            if resp.status() != 206 {
                anyhow::bail!("{url}: server ignored Range (status {})", resp.status());
            }
            let total = resp
                .headers()
                .get("content-range")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.rsplit('/').next())
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or_else(|| anyhow::anyhow!("{url}: missing Content-Range"))?;
            let bytes = resp.body_mut().with_config().limit(16 * 1024 * 1024).read_to_vec()?;
            Ok(Some((bytes, total)))
        }
        Err(ureq::Error::StatusCode(404)) | Err(ureq::Error::StatusCode(403)) => Ok(None),
        Err(e) => Err(e.into()),
    }
}
