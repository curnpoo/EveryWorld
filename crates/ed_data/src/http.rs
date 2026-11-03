//! Shared HTTP agent. Uses the OS trust store (Keychain on macOS) and honours
//! HTTPS_PROXY.

use std::sync::OnceLock;
use std::time::Duration;

pub const USER_AGENT: &str = concat!("EveryWorld/", env!("CARGO_PKG_VERSION"), " (open-world game; github.com/curnpoo/everyworld)");

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
    let mut resp = agent().get(url).call()?;
    let bytes = resp.body_mut().with_config().limit(64 * 1024 * 1024).read_to_vec()?;
    Ok(bytes)
}

/// Like [`get_bytes`] but a 404/204 (common for empty ocean tiles) is `Ok(None)`.
pub fn get_bytes_opt(url: &str) -> anyhow::Result<Option<Vec<u8>>> {
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
