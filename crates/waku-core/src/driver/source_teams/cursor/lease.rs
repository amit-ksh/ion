//! Own an otherwise unused native project hook file. Owner files are never
//! merged or replaced. Concurrent Ion drivers share only a file created here.
use anyhow::{Context, Result, ensure};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::os::windows::{
    fs::{MetadataExt, OpenOptionsExt},
    io::AsRawHandle,
};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    sync::{Arc, OnceLock, Weak},
};
use windows_sys::Win32::Storage::FileSystem::*;
const LIMIT: usize = 256 * 1024;
const MAX_ENTRIES: usize = 64;
type Registry = Mutex<HashMap<PathBuf, Weak<Mutex<Group>>>>;
fn registry() -> &'static Registry {
    static REG: OnceLock<Registry> = OnceLock::new();
    REG.get_or_init(Default::default)
}
pub(super) struct HookLease {
    key: PathBuf,
    id: String,
    group: Option<Arc<Mutex<Group>>>,
}
struct Group {
    path: PathBuf,
    id: (u32, u32, u32),
    entries: BTreeMap<String, Value>,
    expected: Vec<u8>,
    cursor: bool,
    _pins: Vec<fs::File>,
}
fn pin(path: &Path) -> Result<fs::File> {
    let file = fs::OpenOptions::new()
        .read(true)
        .access_mode(FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_dir() && metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0,
        "Native hook parent is linked or not a directory"
    );
    Ok(file)
}
fn pin_chain(path: &Path) -> Result<Vec<fs::File>> {
    ensure!(path.is_absolute(), "Native hook cache must be absolute");
    let mut current = PathBuf::new();
    let mut pins = Vec::new();
    for part in path.components() {
        ensure!(
            !matches!(part, Component::ParentDir | Component::CurDir),
            "Native hook cache has relative components"
        );
        current.push(part.as_os_str());
        if !current.has_root() {
            continue;
        }
        if !current.try_exists()? {
            fs::create_dir(&current)?;
        }
        pins.push(pin(&current)?);
    }
    ensure!(!pins.is_empty(), "Native hook cache has no directory");
    Ok(pins)
}
fn identity(file: &fs::File) -> Result<(u32, u32, u32)> {
    let mut info = unsafe { std::mem::zeroed::<BY_HANDLE_FILE_INFORMATION>() };
    ensure!(
        unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } != 0
            && info.nNumberOfLinks == 1
            && info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT == 0,
        "Could not inspect native hook identity"
    );
    Ok((
        info.dwVolumeSerialNumber,
        info.nFileIndexHigh,
        info.nFileIndexLow,
    ))
}
fn opened(path: &Path, write: bool) -> Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    if write {
        options
            .access_mode(FILE_GENERIC_READ | FILE_GENERIC_WRITE | DELETE)
            .share_mode(FILE_SHARE_READ);
    } else {
        options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file()
            && metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0
            && metadata.len() <= LIMIT as u64,
        "Native hook file is linked or exceeds its bound"
    );
    Ok(file)
}
fn same(file: &mut fs::File, id: (u32, u32, u32), expected: &[u8]) -> Result<()> {
    ensure!(identity(file)? == id, "Native hook file was replaced");
    let mut bytes = Vec::new();
    file.seek(SeekFrom::Start(0))?;
    file.take((LIMIT + 1) as u64).read_to_end(&mut bytes)?;
    ensure!(
        bytes == expected,
        "Native hook file changed; preserve owner changes"
    );
    Ok(())
}
impl Group {
    fn bytes(&self, entries: &BTreeMap<String, Value>) -> Result<Vec<u8>> {
        let hooks = entries.values().cloned().collect::<Vec<_>>();
        let value = if self.cursor {
            json!({"version":1,"hooks":{"preToolUse":hooks}})
        } else {
            json!({"hooks":{"PreToolUse":hooks}})
        };
        let bytes = serde_json::to_vec(&value)?;
        ensure!(
            bytes.len() <= LIMIT,
            "Native hook configuration exceeds its bound"
        );
        Ok(bytes)
    }
    fn update(&mut self, entries: BTreeMap<String, Value>) -> Result<()> {
        let bytes = self.bytes(&entries)?;
        let mut file = opened(&self.path, true)?;
        same(&mut file, self.id, &self.expected)?;
        file.seek(SeekFrom::Start(0))?;
        if let Err(error) = file
            .write_all(&bytes)
            .and_then(|_| file.set_len(bytes.len() as u64))
            .and_then(|_| file.sync_all())
        {
            let _ = file.seek(SeekFrom::Start(0));
            let _ = file.write_all(&self.expected);
            let _ = file.set_len(self.expected.len() as u64);
            let _ = file.sync_all();
            return Err(error.into());
        }
        self.expected = bytes;
        self.entries = entries;
        Ok(())
    }
    fn valid(&self) -> Result<()> {
        let mut file = opened(&self.path, false)?;
        same(&mut file, self.id, &self.expected)
    }
    fn remove(&mut self, id: &str) -> Result<bool> {
        let mut entries = self.entries.clone();
        entries.remove(id);
        if entries.is_empty() {
            let mut file = opened(&self.path, true)?;
            same(&mut file, self.id, &self.expected)?;
            let info = FILE_DISPOSITION_INFO { DeleteFile: true };
            ensure!(
                unsafe {
                    SetFileInformationByHandle(
                        file.as_raw_handle(),
                        FileDispositionInfo,
                        &info as *const _ as _,
                        std::mem::size_of_val(&info) as u32,
                    )
                } != 0,
                "Could not remove owned native hook file"
            );
            self.entries.clear();
            return Ok(true);
        }
        self.update(entries)?;
        Ok(false)
    }
}
impl HookLease {
    pub(super) fn install(root: &Path, id: &str, matcher: &str, command: &str) -> Result<Self> {
        ensure!(
            !id.is_empty()
                && id.len() <= 80
                && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_'),
            "Invalid native hook lease ID"
        );
        ensure!(
            !command.contains("//") && !command.contains("/*") && command.len() < 16 * 1024,
            "Native hook command is incompatible with the pinned JSONC loader"
        );
        let mut reg = registry().lock();
        let key = root.to_owned();
        let cursor_entry =
            json!({"matcher":matcher,"command":command,"failClosed":true,"timeout":5});
        let claude_entry = json!({"matcher":matcher,"hooks":[{"type":"command","command":command,"timeout":5,"failClosed":true}]});
        let group = if let Some(group) = reg.get(&key).and_then(Weak::upgrade) {
            {
                let mut owned = group.lock();
                ensure!(
                    owned.entries.len() < MAX_ENTRIES && !owned.entries.contains_key(id),
                    "Native hook lease capacity or identity conflict"
                );
                let mut entries = owned.entries.clone();
                entries.insert(
                    id.into(),
                    if owned.cursor {
                        cursor_entry
                    } else {
                        claude_entry
                    },
                );
                owned.update(entries)?;
            }
            group
        } else {
            let root_pins = pin_chain(root)?;
            let mut selected = None;
            // Prefer the separately loaded local file, leaving an owner's Cursor
            // project hooks intact. Every candidate uses exclusive creation.
            for (folder, name, cursor) in [
                (".claude", "settings.local.json", false),
                (".cursor", "hooks.json", true),
                (".claude", "settings.json", false),
            ] {
                let dir = root.join(folder);
                if !dir.try_exists()? {
                    match fs::create_dir(&dir) {
                        Ok(()) => {}
                        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                        Err(_) => continue,
                    }
                }
                let Ok(dir_pin) = pin(&dir) else { continue };
                let path = dir.join(name);
                let mut options = fs::OpenOptions::new();
                options
                    .read(true)
                    .write(true)
                    .create_new(true)
                    .access_mode(FILE_GENERIC_READ | FILE_GENERIC_WRITE | DELETE)
                    .share_mode(FILE_SHARE_READ)
                    .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
                let Ok(mut file) = options.open(&path) else {
                    continue;
                };
                let file_id = identity(&file)?;
                let entry = if cursor {
                    cursor_entry.clone()
                } else {
                    claude_entry.clone()
                };
                let mut owned = Group {
                    path,
                    id: file_id,
                    entries: BTreeMap::from([(id.into(), entry)]),
                    expected: Vec::new(),
                    cursor,
                    _pins: Vec::new(),
                };
                let bytes = owned.bytes(&owned.entries)?;
                if let Err(error) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
                    let info = FILE_DISPOSITION_INFO { DeleteFile: true };
                    let _ = unsafe {
                        SetFileInformationByHandle(
                            file.as_raw_handle(),
                            FileDispositionInfo,
                            &info as *const _ as _,
                            std::mem::size_of_val(&info) as u32,
                        )
                    };
                    return Err(error.into());
                }
                owned.expected = bytes;
                owned._pins.push(dir_pin);
                selected = Some(owned);
                break;
            }
            let mut owned = selected.context(
                "All native hook files are owned or unavailable; existing files were preserved",
            )?;
            owned._pins.extend(root_pins);
            let group = Arc::new(Mutex::new(owned));
            reg.insert(key.clone(), Arc::downgrade(&group));
            group
        };
        Ok(Self {
            key,
            id: id.into(),
            group: Some(group),
        })
    }
    pub(super) fn valid(&self) -> Result<()> {
        self.group
            .as_ref()
            .context("Native hook lease is closed")?
            .lock()
            .valid()
    }
    #[cfg(test)]
    fn path(&self) -> PathBuf {
        self.group.as_ref().unwrap().lock().path.clone()
    }
    #[cfg(test)]
    fn cleanup(&mut self) {
        let Some(group) = self.group.take() else {
            return;
        };
        let mut reg = registry().lock();
        let removed = group.lock().remove(&self.id);
        if removed.is_ok_and(|empty| empty) {
            reg.remove(&self.key);
        }
    }
}
impl Drop for HookLease {
    fn drop(&mut self) {
        let Some(group) = self.group.take() else {
            return;
        };
        let key = self.key.clone();
        let id = self.id.clone();
        let _ = std::thread::Builder::new()
            .name("ion-cursor-hook-cleanup".into())
            .spawn(move || {
                let mut reg = registry().lock();
                if group.lock().remove(&id).is_ok_and(|empty| empty) {
                    reg.remove(&key);
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> crate::driver::team_codex::EmptyDirectory {
        crate::driver::team_codex::EmptyDirectory::create("cursor-hook-lease").unwrap()
    }
    #[test]
    fn cursor_hook_lease_preserves_owner_config_and_shares_only_its_own_file() {
        let dir = root();
        fs::create_dir(dir.0.join(".cursor")).unwrap();
        let owner = dir.0.join(".cursor/hooks.json");
        fs::write(&owner, b"owner bytes").unwrap();
        let mut first = HookLease::install(&dir.0, "first", "^MCP:first$", "exit 0").unwrap();
        assert!(first.path().ends_with(".claude/settings.local.json"));
        let mut second = HookLease::install(&dir.0, "second", "^MCP:second$", "exit 0").unwrap();
        let path = second.path();
        assert_eq!(fs::read(&owner).unwrap(), b"owner bytes");
        let parsed: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(parsed["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
        first.cleanup();
        second.valid().unwrap();
        let parsed: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(parsed["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
        second.cleanup();
        assert!(!path.exists());
        assert_eq!(fs::read(owner).unwrap(), b"owner bytes");
    }
    #[test]
    fn cursor_hook_lease_never_rewrites_existing_files_and_refuses_when_all_slots_are_owned() {
        let dir = root();
        for (folder, file) in [
            (".claude", "settings.local.json"),
            (".cursor", "hooks.json"),
            (".claude", "settings.json"),
        ] {
            fs::create_dir_all(dir.0.join(folder)).unwrap();
            fs::write(dir.0.join(folder).join(file), b"owner").unwrap();
        }
        assert!(HookLease::install(&dir.0, "one", "^MCP:one$", "exit 0").is_err());
        for (folder, file) in [
            (".claude", "settings.local.json"),
            (".cursor", "hooks.json"),
            (".claude", "settings.json"),
        ] {
            assert_eq!(fs::read(dir.0.join(folder).join(file)).unwrap(), b"owner");
        }
    }
    #[test]
    fn cursor_hook_lease_owner_edits_or_file_replacement_disable_mutation_and_cleanup() {
        for replace in [false, true] {
            let dir = root();
            let mut lease = HookLease::install(&dir.0, "one", "^MCP:one$", "exit 0").unwrap();
            let path = lease.path();
            let bytes = fs::read(&path).unwrap();
            if replace {
                fs::rename(&path, path.with_extension("old")).unwrap();
                fs::write(&path, &bytes).unwrap();
            } else {
                fs::write(&path, b"owner changed").unwrap();
            }
            assert!(lease.valid().is_err());
            assert!(HookLease::install(&dir.0, "two", "^MCP:two$", "exit 0").is_err());
            let keep = fs::read(&path).unwrap();
            lease.cleanup();
            assert_eq!(fs::read(&path).unwrap(), keep);
        }
    }
    #[test]
    fn cursor_hook_lease_preserves_files_when_an_owner_adds_a_hard_link() {
        let dir = root();
        let mut lease = HookLease::install(&dir.0, "one", "^MCP:one$", "exit 0").unwrap();
        let path = lease.path();
        let bytes = fs::read(&path).unwrap();
        let alias = path.with_extension("owner-alias");
        fs::hard_link(&path, &alias).unwrap();
        assert!(lease.valid().is_err());
        assert!(HookLease::install(&dir.0, "two", "^MCP:two$", "exit 0").is_err());
        lease.cleanup();
        assert_eq!(fs::read(path).unwrap(), bytes);
        assert_eq!(fs::read(alias).unwrap(), bytes);
    }
    #[test]
    fn cursor_hook_lease_rejects_jsonc_comment_markers_without_creating_a_hook_file() {
        let dir = root();
        assert!(HookLease::install(&dir.0, "one", "^MCP:one$", "literal http://endpoint").is_err());
        assert!(fs::read_dir(&dir.0).unwrap().next().is_none());
    }
}
