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
/// Index modes are needed on Windows, whose filesystem has no Unix execute
/// bit. Keep staged and untracked records separate: a filename can contain a
/// tab and must never be mistaken for Git's stage-record prefix.
fn inventory(root: &Path) -> Result<Vec<(String, Option<bool>)>> {
    let tracked = workspace::git(root, &["ls-files", "-z", "--stage", "--cached"])?;
    let untracked = workspace::git(root, &["ls-files", "-z", "--others", "--exclude-standard"])?;
    ensure!(
        tracked
            .len()
            .checked_add(untracked.len())
            .is_some_and(|n| n <= 2 * 1024 * 1024),
        "Project inventory exceeds its bound"
    );
    let mut files = parse_index(&tracked)?;
    for name in untracked.split('\0').filter(|s| !s.is_empty()) {
        if jobs::path(name).is_ok() {
            files.push((name.to_owned(), None));
        }
    }
    ensure!(files.len() <= 8192, "Project inventory exceeds its bound");
    files.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    ensure!(
        files.windows(2).all(|v| v[0].0 != v[1].0),
        "Duplicate project index path"
    );
    Ok(files)
}
fn parse_index(index: &str) -> Result<Vec<(String, Option<bool>)>> {
    let mut files = Vec::new();
    for record in index.split('\0').filter(|s| !s.is_empty()) {
        let (header, name) = record
            .split_once('\t')
            .context("Invalid Git project index record")?;
        if jobs::path(name).is_err() {
            continue;
        } // Private and unsupported paths are never uploaded.
        let fields = header.split(' ').collect::<Vec<_>>();
        ensure!(
            fields.len() == 3
                && matches!(fields[1].len(), 40 | 64)
                && fields[1].bytes().all(|b| b.is_ascii_hexdigit()),
            "Invalid Git project index metadata"
        );
        ensure!(
            fields[2] == "0",
            "Resolve the project's Git merge conflicts before remote execution"
        );
        let executable = match fields[0] {
            "100644" => false,
            "100755" => true,
            _ => anyhow::bail!(
                "Project contains a tracked link, submodule or unsupported index mode"
            ),
        };
        ensure!(files.len() < 8192, "Project inventory exceeds its bound");
        files.push((name.into(), Some(executable)));
    }
    Ok(files)
}
pub(crate) fn snapshot(root: &Path, policy: &ProjectPolicy) -> Result<(ProjectSnapshot, Vec<u8>)> {
    policy.validate()?;
    let canonical = fs::canonicalize(root)?;
    let root = canonical.as_path();
    // Fixed collection reads, never filters/hooks/fsmonitor or an ignored tree walk.
    let names = inventory(root)?;
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    let mut content = Vec::new();
    for (name, indexed_executable) in names {
        ensure!(
            seen.insert(name.to_ascii_lowercase()),
            "Ambiguous project path"
        );
        let file_path = workspace::safe_edit_path(root, &name)?;
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
        let executable = indexed_executable.unwrap_or(false);
        #[cfg(unix)]
        let _ = indexed_executable; // Preserve current local chmod, including unstaged mode edits.
        entries.push(Entry {
            path: name,
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
    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn create(label: &str) -> Result<Self> {
            let path = std::env::temp_dir().join(format!("ion-{label}-{}", Uuid::new_v4()));
            fs::create_dir(&path)?;
            Ok(Self(path))
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            // Exclusive fixture directory, verified inside the native temporary root.
            if let (Ok(path), Ok(temp)) = (
                fs::canonicalize(&self.0),
                fs::canonicalize(std::env::temp_dir()),
            ) {
                if path.parent() == Some(temp.as_path()) {
                    let _ = fs::remove_dir_all(path);
                }
            }
        }
    }
    fn archive_index(data: &[u8]) -> Vec<Entry> {
        let bytes = u32::from_be_bytes(data[8..12].try_into().unwrap()) as usize;
        serde_json::from_slice(&data[12..12 + bytes]).unwrap()
    }
    #[test]
    fn team_job_project_index_modes_conflicts_and_private_paths() {
        let hash = "a".repeat(40);
        let files = parse_index(&format!(
            "100755 {hash} 0\tbin/check\0100644 {hash} 0\tsrc/ordinary space.txt\0"
        ))
        .unwrap();
        assert_eq!(
            files,
            vec![
                ("bin/check".into(), Some(true)),
                ("src/ordinary space.txt".into(), Some(false))
            ]
        );
        assert_eq!(
            parse_index(&format!("100755 {} 0\tunicode-λ.py\0", "b".repeat(64))).unwrap()[0].1,
            Some(true)
        );
        for mode in ["120000", "160000", "100600", "100777"] {
            assert!(parse_index(&format!("{mode} {hash} 0\tpublic.txt\0")).is_err());
        }
        for stage in ["1", "2", "3", "4", ""] {
            assert!(parse_index(&format!("100755 {hash} {stage}\tpublic.txt\0")).is_err());
        }
        for header in ["100755 bad 0", "100755 abc 0 extra", "invalid"] {
            assert!(parse_index(&format!("{header}\tpublic.txt\0")).is_err());
        }
        assert!(parse_index("record-without-tab\0").is_err());
        assert!(
            parse_index(&format!(
                "120000 {hash} 0\t.env\0100644 {hash} 2\t.git/secret\0100644 {hash} 0\todd\tname\0"
            ))
            .unwrap()
            .is_empty()
        );
    }
    fn executable_project(root: &Path) -> Result<(ProjectSnapshot, Vec<u8>)> {
        workspace::git(root, &["init"])?;
        workspace::git(root, &["config", "core.filemode", "false"])?;
        fs::create_dir(root.join("bin"))?;
        fs::write(
            root.join("bin/check"),
            "#!/usr/local/bin/python3\nprint('Executable snapshot passed')\n",
        )?;
        fs::write(
            root.join("launcher.py"),
            "import subprocess\nsubprocess.run(['./bin/check'], check=True)\n",
        )?;
        fs::write(root.join("untracked.txt"), "Ordinary untracked content")?;
        workspace::git(root, &["add", "bin/check", "launcher.py"])?;
        workspace::git(root, &["update-index", "--chmod=+x", "bin/check"])?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(root.join("bin/check"), fs::Permissions::from_mode(0o755))?;
        }
        snapshot(
            root,
            &ProjectPolicy {
                version: 1,
                max_bytes: 65536,
                max_file_bytes: 65536,
                max_files: 8,
                max_run_bytes: 131072,
            },
        )
    }
    #[test]
    fn team_job_project_snapshot_preserves_executable_modes_and_changes_hash() {
        let root = TestDirectory::create("project-executable").unwrap();
        let (facts, data) = executable_project(&root.0).unwrap();
        let entries = archive_index(&data);
        assert!(
            entries
                .iter()
                .find(|e| e.path == "bin/check")
                .unwrap()
                .executable
        );
        assert!(
            !entries
                .iter()
                .find(|e| e.path == "launcher.py")
                .unwrap()
                .executable
        );
        assert!(
            !entries
                .iter()
                .find(|e| e.path == "untracked.txt")
                .unwrap()
                .executable
        );
        workspace::git(&root.0, &["update-index", "--chmod=-x", "bin/check"]).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(root.0.join("bin/check"), fs::Permissions::from_mode(0o644))
                .unwrap();
        }
        let (changed, bytes) = snapshot(
            &root.0,
            &ProjectPolicy {
                version: 1,
                max_bytes: 65536,
                max_file_bytes: 65536,
                max_files: 8,
                max_run_bytes: 131072,
            },
        )
        .unwrap();
        assert_ne!(changed.sha256, facts.sha256);
        assert_eq!(changed.source_bytes, facts.source_bytes);
        assert!(
            !archive_index(&bytes)
                .iter()
                .find(|e| e.path == "bin/check")
                .unwrap()
                .executable
        );
    }
    #[test]
    fn team_job_project_tracked_links_refuse_snapshot() {
        let root = TestDirectory::create("project-index-refusal").unwrap();
        let _ = executable_project(&root.0).unwrap();
        let hash = workspace::git(&root.0, &["hash-object", "bin/check"]).unwrap();
        workspace::git(
            &root.0,
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                "120000",
                hash.trim(),
                "tracked-link",
            ],
        )
        .unwrap();
        // Windows can check symlinks out as regular text. Its index is authoritative.
        fs::write(root.0.join("tracked-link"), "bin/check").unwrap();
        let policy = ProjectPolicy {
            version: 1,
            max_bytes: 65536,
            max_file_bytes: 65536,
            max_files: 8,
            max_run_bytes: 131072,
        };
        assert!(snapshot(&root.0, &policy).is_err());
        workspace::git(&root.0, &["update-index", "--force-remove", "tracked-link"]).unwrap();
        fs::remove_file(root.0.join("tracked-link")).unwrap();
        assert!(snapshot(&root.0, &policy).is_ok());
    }
    #[test]
    fn team_job_project_actual_unmerged_index_refuses_snapshot() {
        let root = TestDirectory::create("project-conflict").unwrap();
        executable_project(&root.0).unwrap();
        workspace::git(&root.0, &["config", "user.name", "Ion Fixture"]).unwrap();
        workspace::git(&root.0, &["config", "user.email", "ion@example.test"]).unwrap();
        workspace::git(&root.0, &["commit", "-m", "baseline"]).unwrap();
        let original = workspace::git(&root.0, &["branch", "--show-current"]).unwrap();
        workspace::git(&root.0, &["checkout", "-b", "other"]).unwrap();
        fs::write(root.0.join("bin/check"), "conflicting side A\n").unwrap();
        workspace::git(&root.0, &["add", "bin/check"]).unwrap();
        workspace::git(&root.0, &["commit", "-m", "side A"]).unwrap();
        workspace::git(&root.0, &["checkout", original.trim()]).unwrap();
        fs::write(root.0.join("bin/check"), "conflicting side B\n").unwrap();
        workspace::git(&root.0, &["add", "bin/check"]).unwrap();
        workspace::git(&root.0, &["commit", "-m", "side B"]).unwrap();
        assert!(workspace::git(&root.0, &["merge", "other"]).is_err());
        assert!(
            snapshot(
                &root.0,
                &ProjectPolicy {
                    version: 1,
                    max_bytes: 65536,
                    max_file_bytes: 65536,
                    max_files: 8,
                    max_run_bytes: 131072
                }
            )
            .unwrap_err()
            .to_string()
            .contains("merge conflicts")
        );
    }
    #[test]
    #[ignore = "Requires actual owner archive executed by the installed Linux worker"]
    fn team_job_project_native_executable_receipt_matches_owner_archive() {
        let owner = PathBuf::from(
            std::env::var_os("ION_PROJECT_EXECUTABLE_EXPORT").expect("Set owner fixture directory"),
        );
        let native = PathBuf::from(
            std::env::var_os("ION_NATIVE_JOB_WORKER_FIXTURE").expect("Set Linux fixture directory"),
        );
        let request: jobs::JobRequest =
            serde_json::from_slice(&fs::read(owner.join("request.json")).unwrap()).unwrap();
        let observed: jobs::JobRequest =
            serde_json::from_slice(&fs::read(native.join("host-executable-request.json")).unwrap())
                .unwrap();
        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            serde_json::to_value(&observed).unwrap()
        );
        let receipt: jobs::JobResponse =
            serde_json::from_slice(&fs::read(native.join("host-executable-receipt.json")).unwrap())
                .unwrap();
        matches(
            &fs::read(owner.join("project.ion")).unwrap(),
            request.project_snapshot.as_ref().unwrap(),
        )
        .unwrap();
        receipt.validate_against(&request).unwrap();
        assert_eq!(receipt.exit_code, 0);
        assert!(!receipt.timed_out && !receipt.canceled);
        assert!(receipt.output_errors.is_empty());
        use base64::{Engine, engine::general_purpose::STANDARD};
        assert!(
            String::from_utf8(STANDARD.decode(receipt.stdout.content_base64).unwrap())
                .unwrap()
                .contains("Executable snapshot passed")
        );
    }
    #[test]
    #[ignore = "Exports an actual host Git snapshot for separate installed Linux worker acceptance"]
    fn team_job_project_export_host_executable_archive() {
        let destination = PathBuf::from(
            std::env::var_os("ION_PROJECT_EXECUTABLE_EXPORT")
                .expect("Set a fresh private output directory"),
        );
        assert!(
            !destination.exists(),
            "Use a fresh private fixture directory"
        );
        let root = TestDirectory::create("project-executable-export").unwrap();
        let (facts, data) = executable_project(&root.0).unwrap();
        let request = jobs::JobRequest {
            version: 1,
            request_id: Uuid::new_v4(),
            argv: vec![
                "/usr/local/bin/python3".into(),
                "-I".into(),
                "-S".into(),
                "launcher.py".into(),
            ],
            timeout_seconds: 5,
            inputs: vec![],
            project_snapshot: Some(facts),
            output_paths: vec![],
        };
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("project.ion"), &data).unwrap();
        fs::write(
            destination.join("request.json"),
            serde_json::to_vec(&request).unwrap(),
        )
        .unwrap();
        assert!(
            archive_index(&data)
                .iter()
                .find(|e| e.path == "bin/check")
                .unwrap()
                .executable
        );
    }
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
