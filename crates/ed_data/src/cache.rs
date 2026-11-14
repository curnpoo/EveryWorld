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

/// Top-level cache folders a size cap never evicts: small, whole-world, and slow to rebuild
/// (the coarse canopy layer took about 90 minutes to sweep).
pub const KEEP_DIRS: [&str; 2] = ["canopy_coarse", "ground_coarse"];

/// Cache size cap in bytes: `EW_CACHE_GB` (default 20; 0 = no cap).
pub fn cap_bytes() -> u64 {
    let gb = std::env::var("EW_CACHE_GB").ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(20.0);
    (gb.max(0.0) * 1e9) as u64
}

/// What one [`prune`] did.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PruneReport {
    pub before: u64,
    pub after: u64,
    pub removed: usize,
}

/// Bring the cache under `max_bytes` by deleting the least recently used files first (last
/// read or written, whichever is later). Folders in `keep` (top level) are never touched, and
/// count towards the total. Everything else is re-downloadable or regenerated on demand, so
/// the worst case is a slow first load when a pruned place is visited again.
pub fn prune(root: &Path, max_bytes: u64, keep: &[&str]) -> PruneReport {
    let mut files: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
    let mut total = 0u64;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let Ok(m) = e.metadata() else { continue };
            let p = e.path();
            if m.is_dir() {
                stack.push(p);
                continue;
            }
            total += m.len();
            let top = p.strip_prefix(root).ok().and_then(|r| r.components().next()).and_then(|c| c.as_os_str().to_str()).unwrap_or("");
            if keep.contains(&top) {
                continue;
            }
            let t = m.modified().unwrap_or(SystemTime::UNIX_EPOCH).max(m.accessed().unwrap_or(SystemTime::UNIX_EPOCH));
            files.push((t, m.len(), p));
        }
    }
    let mut r = PruneReport { before: total, after: total, removed: 0 };
    if max_bytes == 0 || total <= max_bytes {
        return r;
    }
    // Oldest first; ties by path so the order is stable.
    files.sort();
    for (_, len, p) in files {
        if r.after <= max_bytes {
            break;
        }
        if std::fs::remove_file(&p).is_ok() {
            r.after -= len;
            r.removed += 1;
        }
    }
    r
}

/// Prune `root` to [`cap_bytes`] on a background thread (call once at startup).
pub fn prune_in_background(root: PathBuf) {
    let cap = cap_bytes();
    if cap == 0 {
        return;
    }
    let _ = std::thread::Builder::new().name("cache-prune".into()).spawn(move || {
        let r = prune(&root, cap, &KEEP_DIRS);
        let gb = |b: u64| b as f64 / 1e9;
        if r.removed > 0 {
            log::info!("cache: {:.1} GB -> {:.1} GB (cap {:.0} GB, {} files removed)", gb(r.before), gb(r.after), gb(cap), r.removed);
            println!("cache: {:.1} GB -> {:.1} GB (cap {:.0} GB, {} files removed)", gb(r.before), gb(r.after), gb(cap), r.removed);
        }
    });
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

    #[test]
    fn prune_drops_oldest_and_keeps_protected() {
        let dir = std::env::temp_dir().join(format!("ed-cache-prune-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let c = DiskCache::new(&dir);
        let old = SystemTime::now() - Duration::from_secs(3600);
        for (i, k) in ["tiles/a.bin", "tiles/b.bin", "tiles/c.bin", "canopy_coarse/x.bin"].iter().enumerate() {
            c.put(k, &[0u8; 100]).unwrap();
            let f = std::fs::File::options().write(true).open(c.path(k)).unwrap();
            // a is oldest, then b, then c; the protected file is the oldest of all.
            let t = if i == 3 { old - Duration::from_secs(9999) } else { old + Duration::from_secs(i as u64 * 60) };
            f.set_times(std::fs::FileTimes::new().set_accessed(t).set_modified(t)).unwrap();
        }
        let r = prune(&dir, 250, &KEEP_DIRS);
        assert_eq!((r.before, r.removed, r.after), (400, 2, 200));
        assert!(c.get("tiles/a.bin", None).is_none() && c.get("tiles/b.bin", None).is_none());
        assert!(c.get("tiles/c.bin", None).is_some() && c.get("canopy_coarse/x.bin", None).is_some());
        assert_eq!(prune(&dir, 0, &KEEP_DIRS).removed, 0);
        let _ = std::fs::remove_dir_all(dir);
    }
}
