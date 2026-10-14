//! Hash-pinned large workspace publications. Bytes stay out of model checkpoints.
use super::*;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Component, Path},
};

pub(super) const MAX_BYTES: usize = 16 * 1024 * 1024;
const FORMATS: &[&str] = &[
    "html", "htm", "md", "txt", "csv", "json", "svg", "xml", "yaml", "yml", "pdf", "docx", "xlsx",
    "png",
];

pub(super) fn read(root: &Path, relative: &str) -> Result<Vec<u8>> {
    ensure!(relative.len() <= 512, "Publication path exceeds its bound");
    let root = fs::canonicalize(root)?;
    let path = workspace::safe_edit_path(&root, relative)?;
    // Pin every parent on Windows; no rename/delete sharing until the read ends.
    #[cfg(windows)]
    let mut parents = Vec::new();
    #[cfg(windows)]
    {
        use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
            FILE_SHARE_READ,
        };
        let mut parent = root.clone();
        let mut components = Path::new(relative).components().peekable();
        loop {
            let handle = fs::OpenOptions::new()
                .read(true)
                .access_mode(0x80)
                .share_mode(FILE_SHARE_READ)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
                .open(&parent)?;
            ensure!(
                handle.metadata()?.is_dir()
                    && handle.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0,
                "Publication parent is a link or not a directory"
            );
            parents.push(handle);
            let Some(Component::Normal(part)) = components.next() else {
                break;
            };
            if components.peek().is_none() {
                break;
            }
            parent.push(part);
        }
    }
    #[cfg(unix)]
    let file = {
        use std::os::fd::{AsRawFd, FromRawFd};
        use std::os::unix::fs::OpenOptionsExt;
        let mut parent = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&root)?;
        let mut components = Path::new(relative).components().peekable();
        let mut output = None;
        while let Some(Component::Normal(part)) = components.next() {
            use std::os::unix::ffi::OsStrExt;
            let name = std::ffi::CString::new(part.as_bytes())?;
            let last = components.peek().is_none();
            let flags = libc::O_RDONLY
                | libc::O_NOFOLLOW
                | libc::O_CLOEXEC
                | libc::O_NONBLOCK
                | if last { 0 } else { libc::O_DIRECTORY };
            let fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags) };
            ensure!(
                fd >= 0,
                "Publication path could not be opened without following links"
            );
            let opened = unsafe { fs::File::from_raw_fd(fd) };
            if last {
                output = Some(opened)
            } else {
                parent = opened
            }
        }
        output.context("Publication file missing")?
    };
    #[cfg(windows)]
    let file = {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
        };
        fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(&path)?
    };
    #[cfg(not(any(unix, windows)))]
    let file = fs::File::open(&path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.len() > 0 && metadata.len() <= MAX_BYTES as u64,
        "Publish a regular document up to 16 MiB"
    );
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "Publication file is a link"
        );
    }
    let mut bytes = Vec::new();
    file.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes)?;
    ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_BYTES && bytes.len() as u64 == metadata.len(),
        "Publication file size changed"
    );
    ensure!(
        workspace::safe_edit_path(&root, relative)? == path,
        "Publication path changed"
    );
    Ok(bytes)
}
fn validate(format: &str, bytes: &[u8]) -> Result<()> {
    ensure!(
        FORMATS.contains(&format),
        "Unsupported file publication format"
    );
    match format {
        "pdf" => ensure!(
            bytes.starts_with(b"%PDF-")
                && bytes
                    .iter()
                    .rposition(|b| !b.is_ascii_whitespace())
                    .is_some_and(|end| bytes[..=end].ends_with(b"%%EOF")),
            "File has no PDF envelope"
        ),
        "docx" | "xlsx" => ensure!(
            bytes.starts_with(b"PK\x03\x04"),
            "Office document has no ZIP envelope"
        ),
        "png" => ensure!(
            bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.get(12..16) == Some(b"IHDR"),
            "File has no PNG envelope"
        ),
        _ => {
            std::str::from_utf8(bytes).context("Text document must be valid UTF-8")?;
        }
    }
    Ok(())
}
fn signature(artifact: &Artifact, relative: &str, summary: &str) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(artifact, relative, summary))?)
    ))
}
pub(in crate::teams) fn verify(
    db: &Connection,
    artifact: &Artifact,
    bytes: &[u8],
) -> Result<Option<(String, String)>> {
    let has_bundles:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='ion_team_html_bundles')",[],|r|r.get(0))?;
    let is_bundle: bool = has_bundles
        && db.query_row(
            "SELECT EXISTS(SELECT 1 FROM ion_team_html_bundles WHERE artifact_id=?)",
            [artifact.id.to_string()],
            |r| r.get(0),
        )?;
    if artifact
        .evidence
        .as_ref()
        .and_then(|e| e.producer.activity_id.as_deref())
        == Some("ion-html-bundle")
        || is_bundle
    {
        return super::html_bundle::verify(db, artifact, bytes).map(Some);
    }
    let has_receipts:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='ion_team_file_documents')",[],|r|r.get(0))?;
    if !has_receipts {
        ensure!(
            artifact
                .evidence
                .as_ref()
                .is_none_or(|e| e.producer.activity_id.as_deref() != Some("ion-file-publication"))
                && !matches!(artifact.format.as_str(), "docx" | "xlsx" | "png")
                && bytes.len() <= 64 * 1024,
            "File publication receipt is missing"
        );
        return Ok(None);
    }
    let row:Option<(String,String,String)>=db.query_row("SELECT path,summary,signature FROM ion_team_file_documents WHERE artifact_id=? AND length(path)<=512 AND length(summary) BETWEEN 1 AND 4096",[artifact.id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    let Some((path, summary, seal)) = row else {
        ensure!(
            artifact
                .evidence
                .as_ref()
                .is_none_or(|e| e.producer.activity_id.as_deref() != Some("ion-file-publication"))
                && !matches!(artifact.format.as_str(), "docx" | "xlsx" | "png")
                && bytes.len() <= 64 * 1024,
            "File publication receipt is missing"
        );
        return Ok(None);
    };
    ensure!(
        signature(artifact, &path, &summary)? == seal
            && artifact.evidence.as_ref().is_some_and(
                |e| e.matches(bytes) && e.producer.origin == ArtifactOrigin::TeamAssignment
            ),
        "File publication evidence changed"
    );
    validate(&artifact.format, bytes)?;
    Ok(Some((path, summary)))
}
impl TeamCoordinator {
    pub(crate) fn inspect_file_document(
        &self,
        lease: &AssignmentLease,
        root: &Path,
        relative: &str,
        now: u64,
    ) -> Result<serde_json::Value> {
        let db = self.connect()?;
        let run = load(&db, lease.run_id)?;
        active(&run, now)?;
        ensure!(
            checked_lease(&run, lease, now, false)?
                .access
                .workspace_read,
            "Workspace read access denied"
        );
        let bytes = read(root, relative)?;
        Ok(
            serde_json::json!({"path":relative,"bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(&bytes))}),
        )
    }
    pub(crate) fn publish_file_document(
        &self,
        lease: &AssignmentLease,
        request: Uuid,
        root: &Path,
        relative: &str,
        name: &str,
        format: &str,
        hash: &str,
        summary: &str,
        now: u64,
    ) -> Result<Artifact> {
        text(summary, 4096, "Document summary")?;
        ensure!(
            !summary
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')),
            "Document summary must be readable text"
        );
        text(relative, 512, "Document path")?;
        ensure!(
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "Use the inspected document SHA-256"
        );
        ensure!(
            FORMATS.contains(&format) && name.to_ascii_lowercase().ends_with(&format!(".{format}")),
            "Use a matching supported document filename"
        );
        let mut db = self.connect()?;
        // Exact durable replay is checked before filesystem I/O; a missing source
        // does not require regenerating, repeating, or resurrecting the artifact.
        {
            let tx = db.transaction()?;
            let run = load(&tx, lease.run_id)?;
            active(&run, now)?;
            let work = checked_lease(&run, lease, now, false)?;
            ensure!(work.access.workspace_read, "Workspace read access denied");
            let old:Option<String>=tx.query_row("SELECT artifact_id FROM ion_team_document_receipts WHERE work_id=? AND request_id=?",params![work.id.to_string(),request.to_string()],|r|r.get(0)).optional()?;
            if let Some(id) = old {
                let body: String = tx.query_row(
                    "SELECT body FROM ion_control_records WHERE id=? AND kind='artifact'",
                    [id],
                    |r| r.get(0),
                )?;
                let ControlRecord::Artifact(artifact) = serde_json::from_str(&body)? else {
                    anyhow::bail!("Publication identity changed")
                };
                let bytes = crate::control::verified_document(&tx, &artifact)?;
                let (old_path, old_summary) =
                    verify(&tx, &artifact, &bytes)?.context("File publication receipt missing")?;
                ensure!(
                    artifact.name == name
                        && artifact.format == format
                        && old_path == relative
                        && old_summary == summary
                        && artifact.evidence.as_ref().is_some_and(|e| e.sha256 == hash),
                    "File publication replay arguments changed"
                );
                return Ok(artifact);
            }
        }
        let bytes = read(root, relative)?;
        ensure!(
            format!("{:x}", Sha256::digest(&bytes)) == hash,
            "Publication content changed since inspection"
        );
        validate(format, &bytes)?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, lease.run_id)?;
        active(&run, now)?;
        let work = checked_lease(&run, lease, now, false)?;
        ensure!(work.access.workspace_read, "Workspace read access denied");
        let existing:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM ion_team_document_receipts WHERE work_id=? AND request_id=?)",params![work.id.to_string(),request.to_string()],|r|r.get(0))?;
        let mut artifact = publish_bytes(&tx, &run, work, request, name, format, &bytes, now)?;
        let old: Option<(String, String, String)> = tx
            .query_row(
                "SELECT path,summary,signature FROM ion_team_file_documents WHERE artifact_id=?",
                [artifact.id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        if !existing {
            artifact
                .evidence
                .as_mut()
                .context("Publication evidence missing")?
                .producer
                .activity_id = Some("ion-file-publication".into());
            tx.execute(
                "UPDATE ion_control_records SET body=? WHERE id=? AND kind='artifact'",
                params![
                    serde_json::to_string(&ControlRecord::Artifact(artifact.clone()))?,
                    artifact.id.to_string()
                ],
            )?;
        }
        let seal = signature(&artifact, relative, summary)?;
        if let Some((old_path, old_summary, old_seal)) = old {
            ensure!(
                old_path == relative && old_summary == summary && old_seal == seal,
                "File publication replay changed"
            );
        } else {
            ensure!(!existing, "File publication receipt was removed");
            tx.execute(
                "INSERT INTO ion_team_file_documents VALUES(?,?,?,?)",
                params![artifact.id.to_string(), relative, summary, seal],
            )?;
        }
        tx.commit()?;
        Ok(artifact)
    }
}
pub(in crate::teams) fn recover(
    db: &Connection,
    artifact: &Artifact,
    args: &serde_json::Value,
    bytes: &[u8],
) -> Result<()> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Args {
        path: String,
        name: String,
        format: String,
        sha256: String,
        summary: String,
    }
    let args: Args = serde_json::from_value(args.clone())?;
    let (path, summary) =
        verify(db, artifact, bytes)?.context("File publication receipt missing")?;
    ensure!(
        args.path == path
            && args.summary == summary
            && args.name == artifact.name
            && args.format == artifact.format
            && artifact
                .evidence
                .as_ref()
                .is_some_and(|e| e.sha256 == args.sha256),
        "Pending file publication differs from its durable receipt"
    );
    Ok(())
}
