use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::time::{Duration, Instant};

pub(super) trait Vault: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn set(&self, key: &str, value: &str) -> Result<()>;
    fn delete(&self, key: &str) -> Result<()>;
}
pub(super) struct OsVault;
impl Vault for OsVault {
    fn get(&self, key: &str) -> Result<Option<String>> {
        crate::vault::get(key)
    }
    fn set(&self, key: &str, value: &str) -> Result<()> {
        crate::vault::set(key, value)
    }
    fn delete(&self, key: &str) -> Result<()> {
        if crate::vault::get(key)?.is_some() {
            crate::vault::delete(key)?;
        }
        Ok(())
    }
}
// WinCred limits a blob to 2,560 bytes; vault strings use UTF-16 on Windows.
const PART_SIZE: usize = 1200; // At most 2,400 encoded bytes.
const MAX_PARTS: usize = 32;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u8,
    slot: u8,
    count: usize,
    digest: String,
}

/// Two vault slots make the manifest update the single atomic commit. Each
/// part fits Windows Credential Manager's blob limit. No token is written to disk.
pub(super) struct Store<'a, V: Vault> {
    vault: &'a V,
    prefix: &'a str,
}
impl<'a, V: Vault> Store<'a, V> {
    pub fn new(vault: &'a V, prefix: &'a str) -> Self {
        Self { vault, prefix }
    }
    fn part(&self, slot: u8, index: usize) -> String {
        format!("{}/{slot}/{index}", self.prefix)
    }
    fn manifest(&self) -> Result<Option<Manifest>> {
        let Some(raw) = self.vault.get(self.prefix)? else {
            return Ok(None);
        };
        let value: Manifest = serde_json::from_str(&raw)
            .map_err(|_| anyhow::anyhow!("Cloud vault manifest is invalid"))?;
        if value.version != 1
            || value.slot > 1
            || value.count == 0
            || value.count > MAX_PARTS
            || value.digest.len() != 64
        {
            bail!("Cloud vault manifest is invalid");
        }
        Ok(Some(value))
    }
    pub fn read<T: serde::de::DeserializeOwned>(&self) -> Result<Option<T>> {
        let Some(manifest) = self.manifest()? else {
            return Ok(None);
        };
        let mut raw = String::new();
        for index in 0..manifest.count {
            let part = self
                .vault
                .get(&self.part(manifest.slot, index))?
                .ok_or_else(|| anyhow::anyhow!("Cloud vault transaction is incomplete"))?;
            if part.len() > PART_SIZE {
                bail!("Cloud vault part exceeds its limit");
            }
            raw.push_str(&part);
        }
        if format!("{:x}", Sha256::digest(raw.as_bytes())) != manifest.digest {
            bail!("Cloud vault transaction failed integrity validation");
        }
        serde_json::from_str(&raw)
            .map(Some)
            .map_err(|_| anyhow::anyhow!("Cloud vault state is invalid"))
    }
    pub fn write<T: Serialize>(&self, value: &T) -> Result<()> {
        let raw = serde_json::to_string(value)?;
        if !raw.is_ascii() || raw.is_empty() || raw.len() > PART_SIZE * MAX_PARTS {
            bail!("Cloud account state exceeds the secure vault limit");
        }
        let previous = self.manifest()?;
        self.commit(&raw, previous)
    }
    fn commit(&self, raw: &str, previous: Option<Manifest>) -> Result<()> {
        let slot = previous.as_ref().map_or(0, |previous| 1 - previous.slot);
        let count = raw.len().div_ceil(PART_SIZE);
        for (index, part) in raw.as_bytes().chunks(PART_SIZE).enumerate() {
            self.vault
                .set(&self.part(slot, index), std::str::from_utf8(part)?)?;
        }
        let manifest = Manifest {
            version: 1,
            slot,
            count,
            digest: format!("{:x}", Sha256::digest(raw.as_bytes())),
        };
        self.vault
            .set(self.prefix, &serde_json::to_string(&manifest)?)?;
        // The committed manifest is authoritative even if old-slot cleanup fails.
        if let Some(previous) = previous {
            for index in 0..previous.count {
                let _ = self.vault.delete(&self.part(previous.slot, index));
            }
        }
        Ok(())
    }

    /// Explicit sign-out can recover corrupt metadata and erases orphaned parts
    /// from interrupted transactions. A fresh empty manifest commits first.
    pub fn reset<T: Serialize>(&self, empty: &T) -> Result<()> {
        let previous = self
            .vault
            .get(self.prefix)?
            .and_then(|raw| serde_json::from_str::<Manifest>(&raw).ok())
            .filter(|m| m.version == 1 && m.slot <= 1 && m.count > 0 && m.count <= MAX_PARTS);
        let active_slot = previous.as_ref().map_or(0, |m| 1 - m.slot);
        let raw = serde_json::to_string(empty)?;
        anyhow::ensure!(
            raw.is_ascii() && !raw.is_empty() && raw.len() <= PART_SIZE,
            "Invalid empty cloud account"
        );
        self.commit(&raw, previous)?;
        let mut cleaned = true;
        for slot in 0..=1 {
            for index in 0..MAX_PARTS {
                if slot == active_slot && index == 0 {
                    continue;
                }
                cleaned &= self.vault.delete(&self.part(slot, index)).is_ok();
            }
        }
        anyhow::ensure!(
            cleaned,
            "Signed out locally, but some obsolete vault entries could not be removed"
        );
        Ok(())
    }
}

pub(super) struct AccountLock(File);
impl AccountLock {
    pub fn acquire(directory: &Path, timeout: Duration) -> Result<Self> {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(directory)
            .map_err(|_| anyhow::anyhow!("Cloud coordination directory is unavailable"))?;
        let info = std::fs::symlink_metadata(directory)?;
        if !info.is_dir() || info.file_type().is_symlink() {
            bail!("Cloud coordination directory must not be a link");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            if info.permissions().mode() & 0o077 != 0 || info.uid() != unsafe { libc::geteuid() } {
                bail!("Cloud coordination directory must be private");
            }
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
        }
        let file = options
            .open(directory.join("account.lock"))
            .map_err(|_| anyhow::anyhow!("Cloud account coordination is unavailable"))?;
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            bail!("Cloud account lock is invalid");
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                bail!("Cloud account lock must not be a link");
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            if metadata.nlink() != 1
                || metadata.uid() != unsafe { libc::geteuid() }
                || metadata.permissions().mode() & 0o077 != 0
            {
                bail!("Cloud account lock must be private");
            }
        }
        let start = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Self(file)),
                Err(std::fs::TryLockError::WouldBlock) if start.elapsed() < timeout => {
                    std::thread::sleep(Duration::from_millis(25))
                }
                Err(_) => bail!("Another Ion process is updating cloud access; try again"),
            }
        }
    }
}
impl Drop for AccountLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}
