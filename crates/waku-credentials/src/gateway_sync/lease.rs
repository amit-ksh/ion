use anyhow::{Context as _, Result};
use std::{
    fs::{File, OpenOptions},
    path::Path,
};

/// Shared advisory OS lock, held for the paired publisher's lifetime and released on
/// crash/exit. Lock-file existence is never used as proof of a running owner.
pub struct PublisherLease {
    key: String,
    _file: File,
}
impl PublisherLease {
    pub fn matches(&self, key: &str) -> bool {
        self.key == key
    }
    pub fn acquire(key: String) -> Result<Self> {
        anyhow::ensure!(
            key.len() == 64 && key.bytes().all(|b| b.is_ascii_hexdigit()),
            "Invalid publisher lock identity"
        );
        let path = std::env::temp_dir().join(format!("ion-gateway-owner-{key}.lock"));
        Ok(Self {
            _file: lock(&path)?,
            key,
        })
    }
}
fn lock(path: &Path) -> Result<File> {
    if let Ok(metadata) = std::fs::symlink_metadata(path) {
        anyhow::ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "Unsafe gateway publisher lock"
        );
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .context("Gateway publisher lock is unavailable")?;
    file.try_lock()
        .map_err(|_| anyhow::anyhow!("Another Ion gateway publisher owns this pairing"))?;
    Ok(file)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_publisher_is_fenced_and_crash_release_needs_no_stale_file_cleanup() {
        let path = std::env::temp_dir().join(format!(
            "ion-owner-lock-test-{}-{}.lock",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let first = lock(&path).unwrap();
        assert!(lock(&path).is_err());
        drop(first);
        let recovered = lock(&path).unwrap();
        drop(recovered);
        std::fs::remove_file(path).unwrap();
    }
}
