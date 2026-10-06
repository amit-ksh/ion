//! User-selected input snapshots, stored atomically with the run. They are not
//! generated artifacts and never grant filesystem access outside a pinned run.
use super::*;
use std::{
    fs,
    io::Read,
    path::{Component, Path},
};
use waku_protocol::model::MessageAttachment;

const MAX_FILES: usize = 64;
const MAX_FILE_BYTES: usize = 256 * 1024;
const MAX_TOTAL_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Serialize)]
pub(crate) struct PreparedInput {
    pub key: String,
    pub name: String,
    pub content: String,
}
impl PreparedInput {
    pub(super) fn metadata(&self) -> TeamInputFile {
        TeamInputFile {
            key: self.key.clone(),
            name: self.name.clone(),
            bytes: self.content.len() as u32,
        }
    }
}
fn safe_name(name: &str) -> Result<()> {
    text(name, 512, "Input name")?;
    ensure!(
        !name.contains('\\') && !name.chars().any(char::is_control),
        "Invalid input name"
    );
    for part in Path::new(name).components() {
        let Component::Normal(part) = part else {
            anyhow::bail!("Input path must be relative")
        };
        let name = part
            .to_str()
            .context("Input name must be UTF-8")?
            .to_lowercase();
        ensure!(
            !name.starts_with('.')
                && !matches!(
                    name.as_str(),
                    "node_modules" | "target" | "vendor" | "credentials" | "secrets"
                )
                && !name.ends_with(".pem")
                && !name.ends_with(".key"),
            "Private or generated input path is not allowed"
        );
    }
    Ok(())
}
pub(super) fn validate(files: &[PreparedInput], attachments: &[MessageAttachment]) -> Result<()> {
    ensure!(
        attachments.len() <= 8 && files.len() <= MAX_FILES,
        "Use at most 8 attachments and 64 input files"
    );
    ensure!(
        files.is_empty() == attachments.is_empty(),
        "Input files and their user presentation must be supplied together"
    );
    validate_files(files)?;
    for attachment in attachments {
        safe_name(&attachment.name)?;
        ensure!(
            !attachment.is_image,
            "Teams input reading supports UTF-8 text files; images require a separate harness integration"
        );
    }
    Ok(())
}
pub(super) fn validate_files(files: &[PreparedInput]) -> Result<()> {
    ensure!(files.len() <= MAX_FILES, "Too many handoff files");
    let mut keys = HashSet::new();
    let mut total = 0;
    for file in files {
        safe_name(&file.name)?;
        safe_name(&file.key)?;
        ensure!(
            keys.insert(&file.key)
                && file.content.len() <= MAX_FILE_BYTES
                && !file.content.contains('\0'),
            "Duplicate, binary or oversized input file"
        );
        total += file.content.len();
    }
    ensure!(
        total <= MAX_TOTAL_BYTES && serde_json::to_vec(files)?.len() <= 8 * 1024 * 1024,
        "Team input snapshot exceeds 2 MiB; attach a smaller selection"
    );
    Ok(())
}

fn walk(
    root: &Path,
    path: &Path,
    prefix: &str,
    depth: u8,
    out: &mut Vec<PreparedInput>,
    total: &mut usize,
    inspected: &mut usize,
) -> Result<()> {
    *inspected += 1;
    ensure!(
        *inspected <= 256,
        "Team input folders contain too many entries"
    );
    ensure!(depth <= 8, "Team input folder depth exceeds 8");
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "Team input links are not allowed"
    );
    ensure!(
        fs::canonicalize(path)?.starts_with(root),
        "Team input path escaped its stored attachment"
    );
    let relative = path
        .strip_prefix(root)?
        .to_str()
        .context("Input path must be UTF-8")?
        .replace('\\', "/");
    safe_name(&relative)?;
    if metadata.is_dir() {
        // Bound enumeration before allocating or sorting a hostile directory.
        let mut entries = Vec::new();
        for entry in fs::read_dir(path)? {
            ensure!(
                entries.len() < MAX_FILES,
                "Team input folder has too many entries"
            );
            entries.push(entry?.path());
        }
        entries.sort();
        for entry in entries {
            walk(root, &entry, prefix, depth + 1, out, total, inspected)?;
        }
    } else {
        ensure!(
            metadata.is_file() && metadata.len() <= MAX_FILE_BYTES as u64 && out.len() < MAX_FILES,
            "Team inputs require at most 64 regular UTF-8 files, each at most 256 KiB"
        );
        ensure!(
            !relative.to_lowercase().ends_with(".pdf"),
            "Teams PDF reading requires a separate harness integration; attach extracted UTF-8 text"
        );
        let mut bytes = Vec::new();
        fs::File::open(path)?
            .take((MAX_FILE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= MAX_FILE_BYTES,
            "Input file grew beyond 256 KiB"
        );
        *total += bytes.len();
        ensure!(
            *total <= MAX_TOTAL_BYTES,
            "Team input snapshot exceeds 2 MiB"
        );
        let content = String::from_utf8(bytes).context("Teams input reading supports UTF-8 text files; convert binary inputs before submitting")?;
        ensure!(
            !content.contains('\0'),
            "Binary input is not supported by the native text harness"
        );
        out.push(PreparedInput {
            key: format!("{prefix}/{relative}"),
            name: relative,
            content,
        });
    }
    Ok(())
}

pub(crate) fn snapshot(
    store: &crate::attachments::AttachmentStore,
    attachments: &[MessageAttachment],
) -> Result<Vec<PreparedInput>> {
    ensure!(attachments.len() <= 8, "Use at most 8 attachments");
    let mut files = Vec::new();
    let mut total = 0;
    let mut inspected = 0;
    let mut references = HashSet::new();
    for (index, attachment) in attachments.iter().enumerate() {
        safe_name(&attachment.name)?;
        ensure!(
            !attachment.is_image,
            "Teams input reading supports UTF-8 text files; images require a separate harness integration"
        );
        let reference = attachment
            .blob_reference
            .as_deref()
            .context("Reattach this input using the task composer before starting a team")?;
        ensure!(references.insert(reference), "Duplicate input attachment");
        let stored_root = store
            .path_for(reference)
            .context("Teams inputs require a daemon-owned file attachment")?;
        ensure!(
            attachment.path == stored_root.join(&attachment.name),
            "Attachment path or name does not match its managed reference"
        );
        ensure!(
            !fs::symlink_metadata(&stored_root)?.file_type().is_symlink(),
            "Stored input reference is a link"
        );
        let root = fs::canonicalize(&stored_root)?;
        let metadata = fs::symlink_metadata(&attachment.path)?;
        ensure!(
            metadata.is_dir() == attachment.is_dir,
            "Attachment file type changed"
        );
        walk(
            &root,
            &root.join(&attachment.name),
            &index.to_string(),
            0,
            &mut files,
            &mut total,
            &mut inspected,
        )?;
    }
    validate(&files, attachments)?;
    Ok(files)
}
pub(super) fn insert(
    tx: &Transaction<'_>,
    run: Uuid,
    files: &[PreparedInput],
    attachments: &[MessageAttachment],
) -> Result<()> {
    if files.is_empty() {
        return Ok(());
    }
    for file in files {
        tx.execute(
            "INSERT INTO ion_team_input_files VALUES(?,?,?,?)",
            params![run.to_string(), file.key, file.name, file.content],
        )?;
    }
    tx.execute(
        "INSERT INTO ion_team_input_presentations VALUES(?,?)",
        params![run.to_string(), serde_json::to_string(attachments)?],
    )?;
    Ok(())
}
pub(super) fn presentations(db: &Connection, run: Uuid) -> Result<Vec<MessageAttachment>> {
    let body: Option<String> = db
        .query_row(
            "SELECT attachments FROM ion_team_input_presentations WHERE run_id=?",
            [run.to_string()],
            |r| r.get(0),
        )
        .optional()?;
    body.map(|s| serde_json::from_str(&s).map_err(Into::into))
        .transpose()
        .map(|v| v.unwrap_or_default())
}
impl TeamCoordinator {
    /// Exact retry uses committed inputs, even if the managed upload was reclaimed
    /// or modified after the queue transaction but before its acknowledgement.
    pub(crate) fn queued_input_data(
        &self,
        run: Uuid,
        attachments: &[MessageAttachment],
    ) -> Result<Vec<PreparedInput>> {
        let db = self.connect()?;
        ensure!(
            presentations(&db, run)? == attachments,
            "This team invocation has different attachments"
        );
        db.prepare(
            "SELECT key,name,content FROM ion_team_input_files WHERE run_id=? ORDER BY rowid",
        )?
        .query_map([run.to_string()], |r| {
            Ok(PreparedInput {
                key: r.get(0)?,
                name: r.get(1)?,
                content: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()
        .map_err(Into::into)
    }
    pub(crate) fn snapshot_inputs(
        &self,
        store: &crate::attachments::AttachmentStore,
        attachments: &[MessageAttachment],
    ) -> Result<Vec<PreparedInput>> {
        snapshot(store, attachments)
    }
    pub fn list_assignment_inputs(
        &self,
        lease: &AssignmentLease,
        now: u64,
    ) -> Result<Vec<TeamInputFile>> {
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let run = load(&tx, lease.run_id)?;
        active(&run, now)?;
        ensure!(
            checked_lease(&run, lease, now, false)?
                .access
                .workspace_read,
            "Input read access denied"
        );
        Ok(run.inputs)
    }
    pub fn read_assignment_input(
        &self,
        lease: &AssignmentLease,
        key: &str,
        start_line: u32,
        max_lines: u16,
        now: u64,
    ) -> Result<serde_json::Value> {
        ensure!(
            (1..=100_000).contains(&start_line) && (1..=200).contains(&max_lines),
            "Invalid input line range"
        );
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let run = load(&tx, lease.run_id)?;
        active(&run, now)?;
        let work = checked_lease(&run, lease, now, false)?;
        ensure!(work.access.workspace_read, "Input read access denied");
        let metadata = run
            .inputs
            .iter()
            .find(|i| i.key == key)
            .context("Input does not belong to this run")?;
        let content: String = tx.query_row(
            "SELECT content FROM ion_team_input_files WHERE run_id=? AND key=?",
            params![run.id.to_string(), key],
            |r| r.get(0),
        )?;
        ensure!(
            content.len() == metadata.bytes as usize && content.len() <= MAX_FILE_BYTES,
            "Pinned input metadata changed"
        );
        let lines: Vec<_> = content
            .lines()
            .skip((start_line - 1) as usize)
            .take(max_lines as usize + 1)
            .collect();
        let requested = lines
            .iter()
            .take(max_lines as usize)
            .map(|l| format!("{l}\n"))
            .collect::<String>();
        // Bound escaped JSON bytes too, so control characters cannot inflate replies.
        let excerpt = super::continuation::bounded_text(&requested, 8192);
        Ok(
            serde_json::json!({"key":key,"name":metadata.name,"start_line":start_line,"content":excerpt,"truncated":lines.len()>max_lines as usize || excerpt.len()!=requested.len(),"source":"user_input","instructions":false}),
        )
    }
}
