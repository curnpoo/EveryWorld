//! A dumb, durable, content-addressed-by-key disk cache. Map tiles are
//! immutable per release, so "cache forever" is the right policy for them;
//! weather uses a TTL.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone)]
pub struct DiskCache {
    root: PathBuf,
}

impl DiskCache {
    /// `~/Library/Caches/earthdrop` on macOS, `$XDG_CACHE_HOME/earthdrop` on Linux.
    pub fn default_location() -> Self {
        // Default to a temp directory so downloaded tiles do not linger on disk
        // (and are cleared on reboot). Set EARTHDROP_CACHE to persist them.
        let root = match std::env::var_os("EARTHDROP_CACHE") {
            Some(p) => PathBuf::from(p),
            None => {
                // Ephemeral: wiped on every launch so nothing lingers.
                let r = std::env::temp_dir().join("earthdrop");
                let _ = std::fs::remove_dir_all(&r);
                r
            }
        };
        Self::new(root)
    }

    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn path(&self, key: &str) -> PathBuf {
        // Keys look like "terrarium/12/2136/1462.png"; keep them readable.
        let safe: String = key
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || "/._-".contains(c) { c } else { '_' })
            .collect();
        self.root.join(safe)
    }

    pub fn get(&self, key: &str, max_age: Option<Duration>) -> Option<Vec<u8>> {
        let p = self.path(key);
        if let Some(max_age) = max_age {
            let modified = std::fs::metadata(&p).ok()?.modified().ok()?;
            if SystemTime::now().duration_since(modified).unwrap_or_default() > max_age {
                return None;
            }
        }
        std::fs::read(p).ok()
    }

    pub fn put(&self, key: &str, bytes: &[u8]) -> std::io::Result<()> {
        let p = self.path(key);
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // Write-then-rename so a crash never leaves a half tile behind.
        let tmp = p.with_extension(format!("{:?}.partial", std::thread::current().id()).replace('(', "").replace(')', ""));
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(tmp, p)
    }

    /// Fetch through the cache.
    pub fn get_or_fetch(
        &self,
        key: &str,
        max_age: Option<Duration>,
        fetch: impl FnOnce() -> anyhow::Result<Vec<u8>>,
    ) -> anyhow::Result<Vec<u8>> {
        if let Some(b) = self.get(key, max_age) {
            return Ok(b);
        }
        let b = fetch()?;
        if let Err(e) = self.put(key, &b) {
            log::warn!("cache write failed for {key}: {e}");
        }
        Ok(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let dir = std::env::temp_dir().join(format!("ed-cache-test-{}", std::process::id()));
        let c = DiskCache::new(&dir);
        assert!(c.get("a/b c.bin", None).is_none());
        let v = c.get_or_fetch("a/b c.bin", None, || Ok(vec![1, 2, 3])).unwrap();
        assert_eq!(v, vec![1, 2, 3]);
        let v = c.get_or_fetch("a/b c.bin", None, || panic!("should be cached")).unwrap();
        assert_eq!(v, vec![1, 2, 3]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
