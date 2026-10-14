//! Amp native request hook, witnessed by the main streaming session.
use super::{
    WorkspaceContext,
    opencode::{Bridge, Hub},
};
use crate::driver::DriverEventSender;
use anyhow::{Context, Result, ensure};
use parking_lot::Mutex;
use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone)]
pub(crate) struct AmpBridge(Arc<Inner>);
struct Inner {
    hub: Hub,
    binding: Mutex<Option<Bridge>>,
    context: WorkspaceContext,
    events: DriverEventSender,
    prefix: String,
    closed: AtomicBool,
    running: AtomicBool,
    _plugin: ProjectPlugin,
}
impl AmpBridge {
    pub(crate) fn start(
        context: WorkspaceContext,
        events: DriverEventSender,
        cwd: &Path,
    ) -> Result<Self> {
        let hub = Hub::start()?;
        let name = format!("ion-teams-{}", context.task_id.simple());
        let prefix = format!("plugin__{name}__");
        let plugin = ProjectPlugin::install(cwd, &format!("{name}.ts"), &hub.amp_plugin(&prefix))?;
        Ok(Self(Arc::new(Inner {
            hub,
            binding: Mutex::new(None),
            context,
            events,
            prefix,
            closed: AtomicBool::new(false),
            running: AtomicBool::new(false),
            _plugin: plugin,
        })))
    }
    pub(crate) fn configure(&self, command: &mut std::process::Command) {
        self.0.hub.configure_amp(command, &self.0.prefix);
    }
    pub(crate) fn allow_turn(&self) {
        if self.0.closed.load(Ordering::Acquire) {
            return;
        }
        self.0.running.store(true, Ordering::Release);
        if let Some(bridge) = self.0.binding.lock().as_ref() {
            bridge.allow_turn();
        }
    }
    pub(crate) fn observe(&self, event: &Value) -> Result<()> {
        if self.0.closed.load(Ordering::Acquire) {
            return Ok(());
        }
        if event["type"] == "system" && event["subtype"] == "init" {
            let native = event["session_id"]
                .as_str()
                .context("Amp native thread ID is missing")?;
            let mut binding = self.0.binding.lock();
            // A native process belongs to one source task/thread. Rebind only via
            // a new driver generation, never from a plugin-selected thread ID.
            if binding.is_none() {
                let bridge = self.0.hub.attach(
                    self.0.context.clone(),
                    format!("amp:{native}"),
                    self.0.events.clone(),
                )?;
                if !self.0.running.load(Ordering::Acquire) {
                    bridge.cancel();
                }
                *binding = Some(bridge);
            }
        }
        if let Some(bridge) = self.0.binding.lock().as_ref() {
            bridge.observe_amp(event, &self.0.prefix);
        }
        Ok(())
    }
    pub(crate) fn respond(&self, id: &str, option: &str) {
        if let Some(bridge) = self.0.binding.lock().as_ref() {
            bridge.respond(id, option);
        }
    }
    pub(crate) fn close(&self) {
        self.0.closed.store(true, Ordering::Release);
        self.0.running.store(false, Ordering::Release);
        if let Some(bridge) = self.0.binding.lock().take() {
            bridge.close();
        }
        self.0.hub.close();
    }
}
impl Drop for Inner {
    fn drop(&mut self) {
        self.hub.close();
    }
}

/// Native project discovery is additive. Pin parents and never overwrite existing
/// plugin/settings/account files. Only our unchanged unique file is cleaned.
struct ProjectPlugin {
    root: PathBuf,
    name: String,
    bytes: Vec<u8>,
    parents: Vec<fs::File>,
    owned: bool,
}
impl ProjectPlugin {
    fn install(cwd: &Path, name: &str, source: &str) -> Result<Self> {
        ensure!(
            name.starts_with("ion-teams-")
                && name.ends_with(".ts")
                && !name.contains(['/', '\\', ':']),
            "Invalid generated plugin filename"
        );
        let root = fs::canonicalize(cwd)?;
        let mut parents = Vec::new();
        #[cfg(windows)]
        {
            use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
            use windows_sys::Win32::Storage::FileSystem::*;
            for path in [&root, &root.join(".amp"), &root.join(".amp/plugins")] {
                if path != &root && !path.try_exists()? {
                    fs::create_dir(path)?;
                }
                let handle = fs::OpenOptions::new()
                    .read(true)
                    .access_mode(0x80)
                    .share_mode(FILE_SHARE_READ)
                    .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
                    .open(path)?;
                let metadata = handle.metadata()?;
                ensure!(
                    metadata.is_dir()
                        && metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0,
                    "Amp plugin directory is a link or not a directory"
                );
                parents.push(handle);
            }
        }
        #[cfg(unix)]
        {
            use std::os::{
                fd::{AsRawFd, FromRawFd},
                unix::fs::OpenOptionsExt,
            };
            parents.push(
                fs::OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                    .open(&root)?,
            );
            for part in [".amp", "plugins"] {
                let part = std::ffi::CString::new(part)?;
                let parent = parents.last().unwrap();
                let created = unsafe { libc::mkdirat(parent.as_raw_fd(), part.as_ptr(), 0o700) };
                ensure!(
                    created == 0
                        || std::io::Error::last_os_error().kind()
                            == std::io::ErrorKind::AlreadyExists,
                    "Could not create private plugin directory"
                );
                let fd = unsafe {
                    libc::openat(
                        parent.as_raw_fd(),
                        part.as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    )
                };
                ensure!(fd >= 0, "Amp plugin directory is a link or unavailable");
                parents.push(unsafe { fs::File::from_raw_fd(fd) });
            }
        }
        #[cfg(not(any(unix, windows)))]
        anyhow::bail!("Native Amp project plugin installation is unsupported on this platform");
        #[cfg(windows)]
        let created = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join(".amp/plugins").join(name));
        #[cfg(unix)]
        let created = (|| -> std::io::Result<fs::File> {
            use std::os::fd::{AsRawFd, FromRawFd};
            let name = std::ffi::CString::new(name)?;
            let fd = unsafe {
                libc::openat(
                    parents.last().unwrap().as_raw_fd(),
                    name.as_ptr(),
                    libc::O_WRONLY
                        | libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_NOFOLLOW
                        | libc::O_CLOEXEC,
                    0o600,
                )
            };
            if fd < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(unsafe { fs::File::from_raw_fd(fd) })
        })();
        let (mut file, owned) = match created {
            Ok(file) => (file, true),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                #[cfg(windows)]
                let existing = {
                    use std::os::windows::fs::OpenOptionsExt;
                    fs::OpenOptions::new()
                        .read(true)
                        .custom_flags(0x00200000)
                        .open(root.join(".amp/plugins").join(name))?
                };
                #[cfg(unix)]
                let existing = {
                    use std::os::fd::{AsRawFd, FromRawFd};
                    let n = std::ffi::CString::new(name)?;
                    let fd = unsafe {
                        libc::openat(
                            parents.last().unwrap().as_raw_fd(),
                            n.as_ptr(),
                            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
                        )
                    };
                    ensure!(fd >= 0, "Existing Amp plugin is unavailable or linked");
                    unsafe { fs::File::from_raw_fd(fd) }
                };
                let metadata = existing.metadata()?;
                ensure!(
                    metadata.is_file() && metadata.len() == source.len() as u64,
                    "Existing Amp plugin differs; preserve it for owner review"
                );
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    ensure!(
                        metadata.file_attributes() & 0x400 == 0,
                        "Existing Amp plugin is linked"
                    );
                }
                let mut bytes = Vec::new();
                existing
                    .take((source.len() + 1) as u64)
                    .read_to_end(&mut bytes)?;
                ensure!(
                    bytes == source.as_bytes(),
                    "Existing Amp plugin differs; preserve it for owner review"
                );
                return Ok(Self {
                    root,
                    name: name.into(),
                    bytes,
                    parents,
                    owned: false,
                });
            }
            Err(error) => return Err(error.into()),
        };
        let plugin = Self {
            root,
            name: name.into(),
            bytes: source.as_bytes().to_vec(),
            parents,
            owned,
        };
        file.write_all(&plugin.bytes)?;
        file.sync_all()?;
        Ok(plugin)
    }
}
impl Drop for ProjectPlugin {
    fn drop(&mut self) {
        // A matching crash-leftover can be reused without taking ownership of
        // an existing project file. Never remove a file this generation did not create.
        if !self.owned {
            return;
        }
        let root = self.root.clone();
        let name = self.name.clone();
        let expected = std::mem::take(&mut self.bytes);
        let parents = std::mem::take(&mut self.parents);
        let _ = std::thread::Builder::new()
            .name("ion-amp-plugin-cleanup".into())
            .spawn(move || {
                #[cfg(windows)]
                let opened = {
                    use std::os::windows::fs::OpenOptionsExt;
                    fs::OpenOptions::new()
                        .read(true)
                        .custom_flags(0x00200000)
                        .open(root.join(".amp/plugins").join(&name))
                };
                #[cfg(unix)]
                let opened = (|| -> std::io::Result<fs::File> {
                    use std::os::fd::{AsRawFd, FromRawFd};
                    let n = std::ffi::CString::new(name.as_str())?;
                    let fd = unsafe {
                        libc::openat(
                            parents.last().unwrap().as_raw_fd(),
                            n.as_ptr(),
                            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
                        )
                    };
                    if fd < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(unsafe { fs::File::from_raw_fd(fd) })
                })();
                let Ok(file) = opened else {
                    return;
                };
                let Ok(metadata) = file.metadata() else {
                    return;
                };
                if !metadata.is_file() || metadata.len() != expected.len() as u64 {
                    return;
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if metadata.file_attributes() & 0x400 != 0 {
                        return;
                    }
                }
                let mut bytes = Vec::new();
                if file
                    .take((expected.len() + 1) as u64)
                    .read_to_end(&mut bytes)
                    .is_err()
                    || bytes != expected
                {
                    return;
                }
                #[cfg(windows)]
                let _ = fs::remove_file(root.join(".amp/plugins").join(name));
                #[cfg(unix)]
                {
                    use std::os::fd::AsRawFd;
                    if let Ok(n) = std::ffi::CString::new(name) {
                        unsafe {
                            libc::unlinkat(parents.last().unwrap().as_raw_fd(), n.as_ptr(), 0)
                        };
                    }
                }
                drop(parents);
            });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn amp_project_plugin_preserves_existing_files_and_cleans_only_unchanged_generated_file() {
        let temp = crate::driver::team_codex::EmptyDirectory::create("amp-plugins").unwrap();
        fs::create_dir_all(temp.0.join(".amp/plugins")).unwrap();
        fs::write(temp.0.join(".amp/plugins/existing.ts"), "existing").unwrap();
        let plugin = ProjectPlugin::install(&temp.0, "ion-teams-test.ts", "generated").unwrap();
        let path = temp.0.join(".amp/plugins/ion-teams-test.ts");
        assert_eq!(fs::read_to_string(&path).unwrap(), "generated");
        assert!(ProjectPlugin::install(&temp.0, "ion-teams-test.ts", "replacement").is_err());
        drop(plugin);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while path.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(!path.exists());
        assert_eq!(
            fs::read_to_string(temp.0.join(".amp/plugins/existing.ts")).unwrap(),
            "existing"
        );
    }
    #[test]
    fn amp_project_plugin_preserves_a_changed_generated_file() {
        let temp = crate::driver::team_codex::EmptyDirectory::create("amp-plugin-changed").unwrap();
        let plugin = ProjectPlugin::install(&temp.0, "ion-teams-changed.ts", "generated").unwrap();
        let path = temp.0.join(".amp/plugins/ion-teams-changed.ts");
        fs::write(&path, "owner changed").unwrap();
        drop(plugin);
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(fs::read_to_string(path).unwrap(), "owner changed");
    }
    #[test]
    fn amp_project_plugin_reuses_identical_preexisting_file_without_owning_it() {
        let temp = crate::driver::team_codex::EmptyDirectory::create("amp-plugin-reuse").unwrap();
        fs::create_dir_all(temp.0.join(".amp/plugins")).unwrap();
        let path = temp.0.join(".amp/plugins/ion-teams-existing.ts");
        fs::write(&path, "generated").unwrap();
        let plugin = ProjectPlugin::install(&temp.0, "ion-teams-existing.ts", "generated").unwrap();
        assert!(!plugin.owned);
        drop(plugin);
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(fs::read_to_string(path).unwrap(), "generated");
    }
    #[cfg(unix)]
    #[test]
    fn amp_project_plugin_refuses_linked_config_directory() {
        let temp = crate::driver::team_codex::EmptyDirectory::create("amp-link").unwrap();
        let external = crate::driver::team_codex::EmptyDirectory::create("amp-external").unwrap();
        std::os::unix::fs::symlink(&external.0, temp.0.join(".amp")).unwrap();
        assert!(ProjectPlugin::install(&temp.0, "ion-teams-link.ts", "generated").is_err());
        assert!(fs::read_dir(&external.0).unwrap().next().is_none());
    }
}
