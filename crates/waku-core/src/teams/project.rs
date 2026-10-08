//! Bounded snapshots of an isolated team Git workspace. No profile or ignored tree walk.
use super::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path};

pub(crate) const MAX_ARCHIVE: usize = 34 * 1024 * 1024;
const MAGIC: &[u8; 8] = b"IONPROJ1";
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProjectPolicy {
    pub version: u16,
    pub max_bytes: u64,
    pub max_file_bytes: u64,
    pub max_files: u32,
    pub max_run_bytes: u64,
}
impl ProjectPolicy {
    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(
            self.version == 1
                && (1..=32 * 1024 * 1024).contains(&self.max_bytes)
                && (1..=8 * 1024 * 1024).contains(&self.max_file_bytes)
                && self.max_file_bytes <= self.max_bytes
                && (1..=4096).contains(&self.max_files)
                && (self.max_bytes..=64 * 1024 * 1024).contains(&self.max_run_bytes),
            "Project snapshot policy exceeds its bound"
        );
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProjectSnapshot {
    pub version: u16,
    pub sha256: String,
    pub bytes: u64,
    pub files: u32,
    pub source_bytes: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    path: String,
    bytes: u64,
    sha256: String,
    executable: bool,
}
pub(crate) fn matches(data: &[u8], facts: &ProjectSnapshot) -> Result<()> {
    ensure!(
        facts.version == 1
            && (12..=MAX_ARCHIVE as u64).contains(&facts.bytes)
            && (1..=4096).contains(&facts.files)
            && facts.source_bytes <= 32 * 1024 * 1024
            && data.len() as u64 == facts.bytes
            && format!("{:x}", Sha256::digest(data)) == facts.sha256,
        "Project archive identity changed"
    );
    Ok(())
}
pub(crate) fn snapshot(root: &Path, policy: &ProjectPolicy) -> Result<(ProjectSnapshot, Vec<u8>)> {
    policy.validate()?;
    let canonical = fs::canonicalize(root)?;
    let root = canonical.as_path();
    // Fixed Git inventory: include unignored new files, never recurse into ignored
    // dependencies/caches, and do not invoke filters, hooks or an external fsmonitor.
    let inventory = workspace::git(
        root,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
    )?;
    ensure!(
        inventory.len() <= 2 * 1024 * 1024,
        "Project inventory exceeds its bound"
    );
    let mut names: Vec<&str> = inventory.split('\0').filter(|s| !s.is_empty()).collect();
    ensure!(names.len() <= 8192, "Project inventory exceeds its bound");
    names.sort_unstable();
    names.dedup();
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    let mut content = Vec::new();
    for name in names {
        if jobs::path(name).is_err() {
            continue;
        } // Private paths are never transferred.
        ensure!(
            seen.insert(name.to_ascii_lowercase()),
            "Ambiguous project path"
        );
        let file_path = workspace::safe_edit_path(root, name)?;
        // Git's cached inventory may contain intentionally deleted files.
        let meta = match fs::symlink_metadata(&file_path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        ensure!(
            meta.is_file() && !meta.file_type().is_symlink() && meta.len() <= policy.max_file_bytes,
            "Project contains a link, special file or oversized file"
        );
        ensure!(
            entries.len() < policy.max_files as usize,
            "Project file count exceeds approval"
        );
        let source = fs::File::open(&file_path)?;
        let opened = source.metadata()?;
        ensure!(
            opened.is_file() && fs::canonicalize(&file_path)?.starts_with(root),
            "Project file escaped its root"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            ensure!(opened.nlink() == 1, "Project contains a hard link");
        }
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::Storage::FileSystem::{
                BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
            };
            let mut info = unsafe { std::mem::zeroed::<BY_HANDLE_FILE_INFORMATION>() };
            ensure!(
                unsafe { GetFileInformationByHandle(source.as_raw_handle(), &mut info) } != 0
                    && info.nNumberOfLinks == 1
                    && info.dwFileAttributes & 0x400 == 0,
                "Project contains a hard link or reparse point"
            );
        }
        let mut data = Vec::new();
        source
            .take(policy.max_file_bytes + 1)
            .read_to_end(&mut data)?;
        ensure!(
            data.len() as u64 <= policy.max_file_bytes
                && content.len() as u64 + data.len() as u64 <= policy.max_bytes,
            "Project bytes exceed approval"
        );
        #[cfg(unix)]
        let executable = {
            use std::os::unix::fs::PermissionsExt;
            opened.permissions().mode() & 0o111 != 0
        };
        #[cfg(not(unix))]
        let executable = false;
        entries.push(Entry {
            path: name.into(),
            bytes: data.len() as u64,
            sha256: format!("{:x}", Sha256::digest(&data)),
            executable,
        });
        content.extend(data);
    }
    ensure!(!entries.is_empty(), "Project has no transferable files");
    let index = serde_json::to_vec(&entries)?;
    ensure!(
        index.len() <= 2 * 1024 * 1024,
        "Project index exceeds its bound"
    );
    let source_bytes = content.len() as u64;
    let mut archive = Vec::with_capacity(12 + index.len() + content.len());
    archive.extend_from_slice(MAGIC);
    archive.extend_from_slice(&(index.len() as u32).to_be_bytes());
    archive.extend(index);
    archive.extend(content);
    let facts = ProjectSnapshot {
        version: 1,
        sha256: format!("{:x}", Sha256::digest(&archive)),
        bytes: archive.len() as u64,
        files: entries.len() as u32,
        source_bytes,
    };
    matches(&archive, &facts)?;
    Ok((facts, archive))
}
impl TeamCoordinator {
    pub(crate) fn project_source_context(
        &self,
        lease: &AssignmentLease,
        source: &Path,
        now: u64,
    ) -> Result<super::checks::CommandCheckContext> {
        let db = self.connect()?;
        let run = load(&db, lease.run_id)?;
        active(&run, now)?;
        let work = checked_lease(&run, lease, now, false)?;
        workspace::require_settled_effects(&db, run.id)?;
        let value = verify_source(&db, &run, work)?;
        ensure!(
            value.path.as_ref() == Some(&fs::canonicalize(source)?),
            "Project source differs from its isolated run"
        );
        Ok(value)
    }
}
pub(super) fn verify_source(
    db: &Connection,
    run: &TeamRunSnapshot,
    work: &TeamAssignment,
) -> Result<super::checks::CommandCheckContext> {
    ensure!(
        run.workspace.is_some(),
        "Project upload requires a sealed isolated team workspace"
    );
    super::checks::context(db, run, work)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn team_job_project_inventory_hash_bounds_and_links() {
        let root = std::env::temp_dir().join(format!("ion-project-test-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let result = (|| -> Result<()> {
            workspace::git(&root, &["init"])?;
            fs::write(root.join(".gitignore"), "ignored/\n")?;
            fs::create_dir(root.join("ignored"))?;
            fs::write(root.join("ignored").join("large.bin"), vec![1; 1024 * 1024])?;
            fs::write(root.join(".env"), "not transferred")?;
            fs::write(root.join("source.bin"), vec![7; 512 * 1024])?;
            fs::write(root.join("z-deleted.txt"), "deleted source")?;
            workspace::git(&root, &["add", "z-deleted.txt"])?;
            fs::remove_file(root.join("z-deleted.txt"))?;
            let p = ProjectPolicy {
                version: 1,
                max_bytes: 1024 * 1024,
                max_file_bytes: 1024 * 1024,
                max_files: 4,
                max_run_bytes: 2 * 1024 * 1024,
            };
            let (facts, data) = snapshot(&root, &p)?;
            assert_eq!(facts.files, 2);
            assert_eq!(facts.source_bytes, 512 * 1024 + 9);
            assert_eq!(snapshot(&root, &p)?.0, facts);
            assert_eq!(
                snapshot(
                    &root,
                    &ProjectPolicy {
                        max_files: 2,
                        ..p.clone()
                    }
                )?
                .0,
                facts
            );
            fs::write(root.join("source.bin"), vec![8; 512 * 1024])?;
            assert_ne!(snapshot(&root, &p)?.0, facts);
            assert!(matches(&data[..data.len() - 1], &facts).is_err());
            let tight = ProjectPolicy {
                max_bytes: 64 * 1024,
                max_file_bytes: 64 * 1024,
                ..p.clone()
            };
            assert!(snapshot(&root, &tight).is_err());
            fs::hard_link(root.join("source.bin"), root.join("linked.bin"))?;
            assert!(snapshot(&root, &p).is_err());
            fs::remove_file(root.join("linked.bin"))?;
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(root.join("source.bin"), root.join("linked.bin"))?;
                assert!(snapshot(&root, &p).is_err());
            }
            Ok(())
        })();
        // This test owns a unique child of temp; never a user checkout.
        assert_eq!(
            fs::canonicalize(&root).unwrap().parent(),
            Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
        );
        fs::remove_dir_all(&root).unwrap();
        result.unwrap();
    }
}
