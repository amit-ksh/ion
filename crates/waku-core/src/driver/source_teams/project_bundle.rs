//! An additive native plugin bundle. Existing owner configuration is never rewritten.
use anyhow::{Result, ensure};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub(super) struct ProjectBundle(Option<Files>);
struct Files {
    root: PathBuf,
    name: String,
    pins: Vec<(PathBuf, fs::File)>,
    files: Vec<(&'static str, Vec<u8>)>,
}
impl ProjectBundle {
    pub(super) fn install(cwd: &Path, name: &str, manifest: &str, mcp: &str) -> Result<Self> {
        ensure!(
            name.starts_with("ion-teams-")
                && name.len() <= 80
                && name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-'),
            "Invalid native bundle name"
        );
        ensure!(
            manifest.len() + mcp.len() < 64 * 1024,
            "Native bundle is too large"
        );
        let root = fs::canonicalize(cwd)?;
        let mut files = Files {
            root: root.clone(),
            name: name.into(),
            pins: Vec::new(),
            files: Vec::new(),
        };
        files.pins.push((root.clone(), open_dir(&root)?));
        for (name, exclusive) in [(".agents", false), ("plugins", false), (name, true)] {
            let (parent, pin) = files.pins.last().unwrap();
            let path = parent.join(name);
            let child = create_child(pin, &path, name, exclusive)?;
            files.pins.push((path, child));
        }
        #[cfg(not(unix))]
        let leaf = files.pins.last().unwrap().0.clone();
        // Publish the marker last; an incomplete MCP file is not a plugin.
        let publication = (|| -> Result<()> {
            for (name, bytes) in [
                ("mcp_config.json", mcp.as_bytes()),
                ("plugin.json", manifest.as_bytes()),
            ] {
                let mut options = fs::OpenOptions::new();
                options.read(true).write(true).create_new(true);
                #[cfg(windows)]
                {
                    use std::os::windows::fs::OpenOptionsExt;
                    options
                        .access_mode(0xC0000000 | 0x10000)
                        .share_mode(0)
                        .custom_flags(0x00200000);
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options
                        .mode(0o600)
                        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
                }
                #[cfg(windows)]
                let mut file = options.open(leaf.join(name))?;
                #[cfg(unix)]
                let mut file = relative_file(
                    &files.pins.last().unwrap().1,
                    name,
                    libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
                )?;
                #[cfg(not(any(windows, unix)))]
                let mut file = options.open(leaf.join(name))?;
                let written = file.write_all(bytes).and_then(|_| file.sync_all());
                if let Err(error) = written {
                    #[cfg(windows)]
                    mark_deleted(&file);
                    #[cfg(unix)]
                    {
                        use std::os::fd::AsRawFd;
                        if let Ok(name) = std::ffi::CString::new(name) {
                            unsafe {
                                libc::unlinkat(
                                    files.pins.last().unwrap().1.as_raw_fd(),
                                    name.as_ptr(),
                                    0,
                                );
                            }
                        }
                    }
                    return Err(error.into());
                }
                files.files.push((name, bytes.to_vec()));
            }
            Ok(())
        })();
        if let Err(error) = publication {
            files.cleanup();
            return Err(error);
        }
        Ok(Self(Some(files)))
    }
    #[cfg(test)]
    pub(super) fn path(&self) -> PathBuf {
        let files = self.0.as_ref().unwrap();
        files.root.join(".agents/plugins").join(&files.name)
    }
}
fn create_child(parent: &fs::File, path: &Path, name: &str, exclusive: bool) -> Result<fs::File> {
    #[cfg(unix)]
    {
        use std::os::fd::{AsRawFd, FromRawFd};
        let _ = path;
        let name = std::ffi::CString::new(name)?;
        let result = unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) };
        if result != 0 {
            let error = std::io::Error::last_os_error();
            ensure!(
                !exclusive && error.kind() == std::io::ErrorKind::AlreadyExists,
                "Native plugin directory exists or is unavailable"
            );
        }
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        ensure!(fd >= 0, "Native plugin directory is linked or unavailable");
        return Ok(unsafe { fs::File::from_raw_fd(fd) });
    }
    #[cfg(not(unix))]
    {
        let _ = (parent, name);
        match fs::create_dir(path) {
            Ok(()) => {}
            Err(e) if !exclusive && e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
        open_dir(path)
    }
}
#[cfg(unix)]
fn relative_file(parent: &fs::File, name: &str, flags: i32) -> std::io::Result<fs::File> {
    use std::os::fd::{AsRawFd, FromRawFd};
    let name = std::ffi::CString::new(name)?;
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { fs::File::from_raw_fd(fd) })
}
fn open_dir(path: &Path) -> Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options
            .access_mode(0x80)
            .share_mode(1)
            .custom_flags(0x02000000 | 0x00200000);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    ensure!(metadata.is_dir(), "Native plugin parent is not a directory");
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "Native plugin parent is linked"
        );
    }
    Ok(file)
}
#[cfg(windows)]
fn mark_deleted(file: &fs::File) {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_DISPOSITION_INFO, FileDispositionInfo, SetFileInformationByHandle,
    };
    let info = FILE_DISPOSITION_INFO { DeleteFile: true };
    unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle() as _,
            FileDispositionInfo,
            &info as *const _ as _,
            std::mem::size_of_val(&info) as u32,
        );
    }
}
impl Files {
    fn cleanup(&mut self) {
        #[cfg(windows)]
        let leaf = self.root.join(".agents/plugins").join(&self.name);
        // On Unix an open directory can be renamed. Preserve adopted/moved trees.
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            for (path, pin) in &self.pins {
                let (Ok(actual), Ok(expected)) = (fs::symlink_metadata(path), pin.metadata())
                else {
                    return;
                };
                if !actual.is_dir()
                    || actual.dev() != expected.dev()
                    || actual.ino() != expected.ino()
                {
                    return;
                }
            }
        }
        if self.pins.len() != 4 {
            return;
        }
        let mut checked = Vec::new();
        for (name, expected) in &self.files {
            let mut options = fs::OpenOptions::new();
            options.read(true);
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                // Delete the exact checked handle, never a subsequently replaced path.
                options
                    .access_mode(0x80000000 | 0x10000)
                    .share_mode(0)
                    .custom_flags(0x00200000);
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
            }
            #[cfg(windows)]
            let opened = options.open(leaf.join(name));
            #[cfg(unix)]
            let opened = relative_file(
                &self.pins.last().unwrap().1,
                name,
                libc::O_RDONLY | libc::O_NONBLOCK,
            );
            #[cfg(not(any(windows, unix)))]
            let opened = options.open(leaf.join(name));
            let Ok(mut file) = opened else {
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
            if (&mut file)
                .take((expected.len() + 1) as u64)
                .read_to_end(&mut bytes)
                .is_err()
                || bytes != *expected
            {
                return;
            }
            checked.push((name, file));
        }
        for (name, file) in checked {
            #[cfg(windows)]
            {
                mark_deleted(&file);
            }
            #[cfg(unix)]
            {
                use std::os::fd::AsRawFd;
                if let Ok(name) = std::ffi::CString::new(*name) {
                    unsafe {
                        libc::unlinkat(self.pins.last().unwrap().1.as_raw_fd(), name.as_ptr(), 0);
                    }
                }
            }
            #[cfg(windows)]
            let _ = name;
            #[cfg(not(any(windows, unix)))]
            {
                let _ = name;
            }
            drop(file);
        }
        // Directory removal is nonrecursive; extra owner/native files are kept.
        self.pins.pop();
        #[cfg(windows)]
        let _ = fs::remove_dir(leaf);
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            if let Ok(name) = std::ffi::CString::new(self.name.as_str()) {
                unsafe {
                    libc::unlinkat(
                        self.pins.last().unwrap().1.as_raw_fd(),
                        name.as_ptr(),
                        libc::AT_REMOVEDIR,
                    );
                }
            }
        }
    }
}
impl Drop for ProjectBundle {
    fn drop(&mut self) {
        if let Some(mut files) = self.0.take() {
            let _ = std::thread::Builder::new()
                .name("ion-native-bundle-cleanup".into())
                .spawn(move || files.cleanup());
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    fn wait(path: &Path) {
        let until = Instant::now() + Duration::from_secs(5);
        while path.exists() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!path.exists(), "Unchanged bundle was not cleaned");
    }
    #[test]
    fn antigravity_bundle_preserves_owner_config_and_overlapping_directories() {
        let root = crate::driver::team_codex::EmptyDirectory::create("antigravity-bundle").unwrap();
        fs::create_dir(root.0.join(".agents")).unwrap();
        let owner = root.0.join(".agents/mcp_config.json");
        fs::write(&owner, "// owner\n{\"mcpServers\":{},}").unwrap();
        let a = ProjectBundle::install(&root.0, "ion-teams-one", "{}", "{}").unwrap();
        let b = ProjectBundle::install(&root.0, "ion-teams-two", "{}", "{}").unwrap();
        let (pa, pb) = (a.path(), b.path());
        drop(a);
        wait(&pa);
        assert!(pb.join("plugin.json").is_file());
        assert_eq!(
            fs::read_to_string(owner).unwrap(),
            "// owner\n{\"mcpServers\":{},}"
        );
        drop(b);
        wait(&pb);
    }
    #[test]
    fn antigravity_bundle_refuses_existing_and_preserves_adopted_bytes() {
        let root = crate::driver::team_codex::EmptyDirectory::create("antigravity-bundle").unwrap();
        let bundle = ProjectBundle::install(&root.0, "ion-teams-one", "{}", "{}").unwrap();
        let path = bundle.path();
        assert!(ProjectBundle::install(&root.0, "ion-teams-one", "{}", "{}").is_err());
        fs::write(path.join("plugin.json"), "{\"name\":\"owner\"}").unwrap();
        drop(bundle);
        std::thread::sleep(Duration::from_millis(150));
        // Cleanup checks each file through an exclusive handle; a read that
        // lands during that check is refused, so retry until it is through.
        let until = Instant::now() + Duration::from_secs(30);
        let owner = loop {
            match fs::read_to_string(path.join("plugin.json")) {
                Ok(bytes) => break bytes,
                Err(error) => {
                    assert!(Instant::now() < until, "{error}");
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        };
        assert_eq!(owner, "{\"name\":\"owner\"}");
        assert!(path.join("mcp_config.json").is_file());
    }
    #[test]
    fn antigravity_bundle_cleanup_preserves_extra_owner_documents() {
        let root = crate::driver::team_codex::EmptyDirectory::create("antigravity-bundle").unwrap();
        let bundle = ProjectBundle::install(&root.0, "ion-teams-one", "{}", "{}").unwrap();
        let path = bundle.path();
        fs::write(path.join("owner-note.txt"), "Keep this document").unwrap();
        drop(bundle);
        let until = Instant::now() + Duration::from_secs(5);
        while path.join("plugin.json").exists() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!path.join("plugin.json").exists());
        assert!(!path.join("mcp_config.json").exists());
        assert_eq!(
            fs::read_to_string(path.join("owner-note.txt")).unwrap(),
            "Keep this document"
        );
    }
}
