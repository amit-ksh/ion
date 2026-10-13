//! Owner-requested cleanup removes only Ion's copy. Publication identities and
//! receipts remain durable; no clock or scheduler expires artifact content.
use super::*;
use rusqlite::{TransactionBehavior, params_from_iter, types::Value};
use std::{
    fs::{File, Metadata, OpenOptions},
    io::Read,
    path::Path,
};
use waku_protocol::artifact_retention::*;

#[cfg(test)]
mod tests;

const MAX_BYTES: u64 = 16 * 1024 * 1024;

pub(crate) fn schema(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS ion_artifact_cleanup (
        artifact_id TEXT PRIMARY KEY,request_id TEXT NOT NULL UNIQUE,
        expected TEXT NOT NULL,bytes INTEGER NOT NULL CHECK(bytes BETWEEN 1 AND 16777216),
        storage TEXT NOT NULL CHECK(storage IN ('file','blob')),
        state TEXT NOT NULL CHECK(state IN ('pending','cleaned')),
        claimed_at INTEGER NOT NULL,cleaned_at INTEGER,
        CHECK((state='pending' AND cleaned_at IS NULL) OR (state='cleaned' AND cleaned_at IS NOT NULL)));
        CREATE INDEX IF NOT EXISTS ion_artifact_cleanup_state ON ion_artifact_cleanup(state,artifact_id);")?;
    Ok(())
}

pub(crate) fn ensure_available(db: &Connection, id: Uuid) -> Result<()> {
    // Some older standalone fixtures install only the document schema.
    if !has_table(db, "ion_artifact_cleanup")? {
        return Ok(());
    }
    let state: Option<String> = db
        .query_row(
            "SELECT state FROM ion_artifact_cleanup WHERE artifact_id=?",
            [id.to_string()],
            |r| r.get(0),
        )
        .optional()?;
    ensure!(
        state.is_none(),
        "Artifact content was explicitly cleaned or cleanup is pending; publication evidence is retained"
    );
    Ok(())
}

fn has_table(db: &Connection, name: &str) -> Result<bool> {
    Ok(db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?)",
        [name],
        |r| r.get(0),
    )?)
}
fn stored(db: &Connection, id: Uuid) -> Result<Artifact> {
    let body: Option<String> = db.query_row("SELECT CASE WHEN length(body)<=65536 THEN body ELSE NULL END FROM ion_control_records WHERE id=? AND kind='artifact'", [id.to_string()], |r|r.get(0)).context("Artifact publication no longer exists; refresh storage")?;
    let body = body.context("Artifact publication metadata exceeds its 64 KiB bound")?;
    let ControlRecord::Artifact(artifact) = serde_json::from_str(&body)? else {
        anyhow::bail!("Artifact publication identity changed")
    };
    ensure!(artifact.id == id, "Artifact publication identity changed");
    Ok(artifact)
}
fn metadata(artifact: &Artifact) -> Result<&ArtifactEvidence> {
    let evidence = artifact
        .evidence
        .as_ref()
        .context("Historical artifact lacks verified publisher evidence")?;
    let p = &evidence.producer;
    let known = match p.origin {
        ArtifactOrigin::AgentGenerated => p
            .activity_id
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty() && s.len() <= 512),
        ArtifactOrigin::AgentEdit => {
            p.turn_id.is_some_and(|id| !id.is_nil())
                && p.activity_id
                    .as_ref()
                    .is_some_and(|s| !s.trim().is_empty() && s.len() <= 512)
                && p.source_path
                    .as_ref()
                    .is_some_and(|s| !s.trim().is_empty() && s.len() <= 8192)
        }
        ArtifactOrigin::TeamAssignment => [p.run_id, p.assignment_id, p.agent_id, p.request_id]
            .iter()
            .all(|id| id.is_some_and(|id| !id.is_nil())),
    };
    ensure!(
        known
            && !artifact.id.is_nil()
            && artifact.project_id.is_some_and(|id| !id.is_nil())
            && artifact.task_id.is_some_and(|id| !id.is_nil())
            && artifact.reference == format!("ion-document:{}", artifact.id)
            && artifact.created_at > 0
            && artifact.created_at <= i64::MAX as u64
            && !artifact.name.trim().is_empty()
            && artifact.name.len() <= 160
            && !artifact
                .name
                .chars()
                .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
            && artifact.name != "."
            && artifact.name != ".."
            && matches!(
                artifact.format.as_str(),
                "html" | "htm" | "pdf" | "md" | "txt" | "csv" | "json" | "docx"
            )
            && evidence.version == 1
            && evidence.bytes > 0
            && evidence.bytes <= MAX_BYTES
            && evidence.sha256.len() == 64
            && evidence
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "Artifact publisher provenance or original content evidence is invalid"
    );
    Ok(evidence)
}
fn serialize(artifact: &Artifact) -> Result<String> {
    Ok(serde_json::to_string(artifact)?)
}

/// SQL protection is evaluated in the same transaction as the cleanup claim.
/// Runtime task protection comes from the daemon, never the requesting client.
fn protected(
    db: &Connection,
    artifact: &Artifact,
    tasks: &HashSet<Uuid>,
    projects: &HashSet<Uuid>,
) -> Result<Option<String>> {
    let evidence = match metadata(artifact) {
        Ok(e) => e,
        Err(e) => return Ok(Some(e.to_string())),
    };
    if artifact.task_id.is_some_and(|id| tasks.contains(&id)) {
        return Ok(Some("The source task is still active".into()));
    }
    if artifact.project_id.is_some_and(|id| projects.contains(&id)) {
        return Ok(Some("The project still has active work".into()));
    }
    let project = artifact.project_id.unwrap().to_string();
    if has_table(db, "ion_automation_runs")? {
        // Native automation writes JSON strings; older rows used plain enum
        // names. Decode only bounded, valid JSON strings and fail closed for
        // every other representation (including SQL/JSON null).
        let active: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM ion_automation_runs WHERE project_id=? AND COALESCE((CASE
                WHEN typeof(status)!='text' OR length(status)>128 THEN NULL
                WHEN json_valid(status) THEN CASE WHEN json_type(status)='text' THEN json_extract(status,'$') ELSE NULL END
                ELSE status END) NOT IN ('completed','canceled','skipped'),1))",
            [&project], |r|r.get(0))?;
        if active {
            return Ok(Some(
                "An active workflow or routine may need this project document".into(),
            ));
        }
    }
    if has_table(db, "ion_team_gpu_results")?
        && db.query_row(
            "SELECT EXISTS(SELECT 1 FROM ion_team_gpu_results WHERE artifact_id=?)",
            [artifact.id.to_string()],
            |r| r.get::<_, bool>(0),
        )?
    {
        return Ok(Some(
            "The GPU result receipt is retained for original job recovery".into(),
        ));
    }
    if has_table(db, "ion_team_runs")? && has_table(db, "ion_team_work")? {
        let task = artifact.task_id.unwrap().to_string();
        let active: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM ion_team_runs r LEFT JOIN ion_team_work w ON w.run_id=r.id WHERE (r.root_task_id=?1 OR w.task_id=?1) AND (r.status NOT IN ('completed','canceled') OR w.status NOT IN ('completed','superseded','failed','canceled')))", [&task], |r|r.get(0))?;
        if active {
            return Ok(Some("A team task or its review is still active".into()));
        }
    }
    if evidence.producer.origin == ArtifactOrigin::TeamAssignment {
        let p = &evidence.producer;
        if !has_table(db, "ion_team_document_receipts")?
            || !has_table(db, "ion_team_work")?
            || !has_table(db, "ion_team_runs")?
        {
            return Ok(Some(
                "The original team publication cannot be verified".into(),
            ));
        }
        let identity: Option<(String,String,String,String,String,String,String,String,String,Option<String>,Option<String>,String)> = db.query_row("SELECT d.work_id,d.request_id,d.name,d.format,w.run_id,w.task_id,w.agent_id,w.status,r.status,CASE WHEN length(w.body)<=1048576 THEN w.body ELSE NULL END,CASE WHEN length(r.body)<=8388608 THEN r.body ELSE NULL END,r.root_task_id FROM ion_team_document_receipts d JOIN ion_team_work w ON w.id=d.work_id JOIN ion_team_runs r ON r.id=w.run_id WHERE d.artifact_id=? AND r.project_id=?",params![artifact.id.to_string(),project],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?,r.get(10)?,r.get(11)?))).optional()?;
        let Some((
            work,
            request,
            name,
            format,
            run,
            task,
            agent,
            work_status,
            run_status,
            work_body,
            run_body,
            root_task,
        )) = identity
        else {
            return Ok(Some(
                "The original team publication cannot be verified".into(),
            ));
        };
        if work != p.assignment_id.unwrap().to_string()
            || request != p.request_id.unwrap().to_string()
            || run != p.run_id.unwrap().to_string()
            || task != artifact.task_id.unwrap().to_string()
            || agent != p.agent_id.unwrap().to_string()
            || name != artifact.name
            || format != artifact.format
        {
            return Ok(Some(
                "The original team publication identity changed".into(),
            ));
        }
        let identities =
            work_body
                .as_deref()
                .zip(run_body.as_deref())
                .and_then(|(work_body, run_body)| {
                    let work_json: serde_json::Value = serde_json::from_str(work_body).ok()?;
                    let run_json: serde_json::Value = serde_json::from_str(run_body).ok()?;
                    Some(
                        work_json.get("id").and_then(|v| v.as_str()) == Some(work.as_str())
                            && work_json.get("taskId").and_then(|v| v.as_str())
                                == Some(task.as_str())
                            && work_json.get("agentId").and_then(|v| v.as_str())
                                == Some(agent.as_str())
                            && run_json.get("id").and_then(|v| v.as_str()) == Some(run.as_str())
                            && run_json.get("projectId").and_then(|v| v.as_str())
                                == Some(project.as_str())
                            && run_json.get("rootTaskId").and_then(|v| v.as_str())
                                == Some(root_task.as_str()),
                    )
                });
        if identities != Some(true) {
            return Ok(Some(
                "The original team publication references are malformed".into(),
            ));
        }
        if !matches!(run_status.as_str(), "completed" | "canceled")
            || !matches!(
                work_status.as_str(),
                "completed" | "superseded" | "failed" | "canceled"
            )
        {
            return Ok(Some("The team run or its review is still active".into()));
        }
        let unfinished: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM ion_team_work WHERE run_id=? AND status NOT IN ('completed','superseded','failed','canceled'))", [&run], |r|r.get(0))?;
        if unfinished {
            return Ok(Some(
                "The original team run still has unfinished assignments".into(),
            ));
        }
        if has_table(db, "ion_team_attempts")? {
            let live: bool = db.query_row(
                "SELECT EXISTS(SELECT 1 FROM ion_team_attempts WHERE run_id=? AND live<>0)",
                [&run],
                |r| r.get(0),
            )?;
            if live {
                return Ok(Some(
                    "An original team attempt still holds execution capacity".into(),
                ));
            }
        }
        if has_table(db,"ion_team_jobs")? && db.query_row("SELECT EXISTS(SELECT 1 FROM ion_team_jobs WHERE request_id=? AND work_id=? AND run_id=?)",params![request,work,run],|r|r.get::<_,bool>(0))? {
            return Ok(Some("The command result receipt is retained for original job recovery".into()));
        }
        for table in [
            "ion_team_recovery_waits",
            "ion_team_recovery_points",
            "ion_team_decision_waits",
            "ion_team_gpu_waits",
        ] {
            if has_table(db,table)? && db.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE work_id IN (SELECT id FROM ion_team_work WHERE run_id=? AND status NOT IN ('completed','superseded','failed','canceled')))"),[&run],|r|r.get::<_,bool>(0))? {
                return Ok(Some("The original team run still has pending recovery or decision work".into()));
            }
        }
    }
    Ok(None)
}

#[derive(Debug)]
struct Claim {
    request_id: Uuid,
    expected: String,
    bytes: u64,
    storage: String,
    state: String,
    cleaned_at: Option<u64>,
}
fn claim(db: &Connection, id: Uuid) -> Result<Option<Claim>> {
    let raw: Option<(Option<String>, Option<String>, u64, String, String, Option<u64>)> = db.query_row(
        "SELECT CASE WHEN length(request_id)=36 THEN request_id ELSE NULL END,CASE WHEN length(expected)<=65536 THEN expected ELSE NULL END,bytes,storage,state,cleaned_at FROM ion_artifact_cleanup WHERE artifact_id=?",
        [id.to_string()], |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional()?;
    raw.map(
        |(request_id, expected, bytes, storage, state, cleaned_at)| {
            let request_id =
                Uuid::parse_str(&request_id.context("Cleanup request identity is malformed")?)?;
            ensure!(
                !request_id.is_nil()
                    && bytes > 0
                    && bytes <= MAX_BYTES
                    && matches!(storage.as_str(), "file" | "blob")
                    && ((state == "pending" && storage == "file" && cleaned_at.is_none())
                        || (state == "cleaned"
                            && cleaned_at.is_some_and(|t| t > 0 && t <= i64::MAX as u64))),
                "Cleanup receipt state is malformed"
            );
            Ok(Claim {
                request_id,
                expected: expected.context("Cleanup publication evidence exceeds its bound")?,
                bytes,
                storage,
                state,
                cleaned_at,
            })
        },
    )
    .transpose()
}

impl ControlStore {
    pub fn list_artifact_retention(
        &self,
        query: &ArtifactRetentionQuery,
        tasks: &HashSet<Uuid>,
        projects: &HashSet<Uuid>,
    ) -> Result<ArtifactRetentionPage> {
        ensure!(
            (1..=32).contains(&query.limit)
                && [query.project_id, query.task_id, query.after]
                    .iter()
                    .all(|id| id.is_none_or(|id| !id.is_nil())),
            "Use a storage page size between 1 and 32 and valid filter IDs"
        );
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        // Owner cleanup must expose historical publications too, independently
        // of the AI-only library projection or its installation history.
        let invalid: Option<String> = tx.query_row(
            "SELECT id FROM ion_control_records WHERE kind='artifact' AND CASE
                WHEN typeof(body)!='text' OR length(body)>65536 THEN 1
                WHEN NOT json_valid(body) THEN 1
                ELSE COALESCE(json_type(body)!='object' OR json_extract(body,'$.kind')!='artifact' OR json_type(body,'$.id')!='text' OR json_extract(body,'$.id')!=id,1)
             END LIMIT 1", [], |r|r.get(0)).optional()?;
        ensure!(
            invalid.is_none(),
            "An artifact publication has invalid JSON, identity or oversized metadata; repair its storage record before reviewing cleanup"
        );
        let mut sql = "SELECT id FROM ion_control_records WHERE kind='artifact'".to_owned();
        let mut args = Vec::<Value>::new();
        if let Some(id) = query.project_id {
            sql.push_str(" AND CASE WHEN json_valid(body) THEN json_extract(body,'$.projectId') ELSE NULL END=?");
            args.push(Value::Text(id.to_string()));
        }
        if let Some(id) = query.task_id {
            sql.push_str(" AND CASE WHEN json_valid(body) THEN json_extract(body,'$.taskId') ELSE NULL END=?");
            args.push(Value::Text(id.to_string()));
        }
        if let Some(id) = query.after {
            let anchor = stored(&tx, id)?;
            ensure!(
                query
                    .project_id
                    .is_none_or(|p| anchor.project_id == Some(p))
                    && query.task_id.is_none_or(|t| anchor.task_id == Some(t)),
                "Storage page anchor does not match its filters; refresh storage"
            );
            sql.push_str(" AND id>?");
            args.push(Value::Text(id.to_string()));
        }
        sql.push_str(" ORDER BY id LIMIT ?");
        args.push(Value::Integer(i64::from(query.limit) + 1));
        let ids = tx
            .prepare(&sql)?
            .query_map(params_from_iter(args), |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let more = ids.len() > usize::from(query.limit);
        let mut items = Vec::new();
        for id in ids.into_iter().take(usize::from(query.limit)) {
            let artifact = stored(&tx, Uuid::parse_str(&id)?)?;
            let saved = claim(&tx, artifact.id)?;
            let (state, reason, cleanup_request) = match saved {
                Some(c) => {
                    ensure!(
                        c.expected == serialize(&artifact)?,
                        "Cleanup publication evidence changed"
                    );
                    match c.state.as_str() {
                        "pending"=>(ArtifactRetentionState::CleanupPending,Some("Content access is blocked. Retry the original cleanup to finish it".into()),Some(c.request_id)),
                        "cleaned"=>(ArtifactRetentionState::Cleaned,Some("Original publication evidence is retained".into()),Some(c.request_id)),
                        _=>anyhow::bail!("Unknown artifact cleanup state"),
                    }
                }
                None => match protected(&tx, &artifact, tasks, projects)? {
                    Some(reason) => (ArtifactRetentionState::Protected, Some(reason), None),
                    None => (ArtifactRetentionState::Retained, None, None),
                },
            };
            items.push(ArtifactRetentionItem {
                artifact,
                state,
                reason,
                cleanup_request,
            });
        }
        let next = if more {
            items.last().map(|i| i.artifact.id)
        } else {
            None
        };
        tx.commit()?;
        Ok(ArtifactRetentionPage { items, next })
    }

    pub fn cleanup_artifact(
        &self,
        request: &ArtifactCleanupRequest,
        tasks: &HashSet<Uuid>,
        projects: &HashSet<Uuid>,
        now: u64,
    ) -> Result<ArtifactCleanupReceipt> {
        ensure!(
            !request.request_id.is_nil() && now > 0 && now <= i64::MAX as u64,
            "Cleanup requires a valid owner request and timestamp"
        );
        let _guard = self.capture_lock.lock();
        let expected = serialize(&request.expected)?;
        ensure!(
            expected.len() <= 65536,
            "Artifact cleanup metadata exceeds its bound"
        );
        let evidence = metadata(&request.expected)?;
        let mut db = self.connect()?;
        // A successful return must mean the tombstone survived the file unlink.
        db.execute_batch("PRAGMA synchronous=FULL;")?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let artifact = stored(&tx, request.expected.id)?;
        ensure!(
            serialize(&artifact)? == expected,
            "Artifact publication changed; review storage again"
        );
        let saved = claim(&tx, artifact.id)?;
        if let Some(c) = &saved {
            ensure!(
                c.request_id == request.request_id
                    && c.expected == expected
                    && c.bytes == evidence.bytes,
                "Cleanup replay identity or original content evidence changed"
            );
            if c.state == "cleaned" {
                let receipt = ArtifactCleanupReceipt {
                    request_id: c.request_id,
                    artifact_id: artifact.id,
                    bytes: c.bytes,
                    cleaned_at: c.cleaned_at.context("Cleanup receipt is incomplete")?,
                };
                tx.commit()?;
                return Ok(receipt);
            }
            ensure!(
                c.state == "pending" && c.storage == "file",
                "Cleanup receipt has an invalid pending state"
            );
        }
        if let Some(reason) = protected(&tx, &artifact, tasks, projects)? {
            anyhow::bail!("Artifact is protected: {reason}");
        }
        let is_blob = has_table(&tx, "ion_team_document_receipts")?
            && tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM ion_team_document_receipts WHERE artifact_id=?)",
                [artifact.id.to_string()],
                |r| r.get::<_, bool>(0),
            )?;
        if is_blob {
            ensure!(saved.is_none(), "Cleanup storage identity changed");
            let bytes = verified_document(&tx, &artifact)?;
            ensure!(
                evidence.matches(&bytes),
                "Document content changed; original hash no longer matches"
            );
            tx.execute(
                "INSERT INTO ion_artifact_cleanup VALUES(?,?,?,?,?,'cleaned',?,?)",
                params![
                    artifact.id.to_string(),
                    request.request_id.to_string(),
                    expected,
                    evidence.bytes,
                    "blob",
                    now,
                    now
                ],
            )?;
            ensure!(
                tx.execute(
                    "UPDATE ion_team_document_receipts SET content=x'' WHERE artifact_id=?",
                    [artifact.id.to_string()]
                )? == 1,
                "Team document storage changed"
            );
            tx.execute(
                "UPDATE ion_control_revision SET revision=revision+1 WHERE id=1",
                [],
            )?;
            tx.commit()?;
        } else {
            let managed = ManagedDocument::open(&self.path, artifact.id, saved.is_some())?;
            if let Some(document) = &managed {
                document.verify(evidence)?;
            }
            if saved.is_none() {
                ensure!(managed.is_some(), "Original managed document is missing");
                tx.execute(
                    "INSERT INTO ion_artifact_cleanup VALUES(?,?,?,?,?,'pending',?,NULL)",
                    params![
                        artifact.id.to_string(),
                        request.request_id.to_string(),
                        expected,
                        evidence.bytes,
                        "file",
                        now
                    ],
                )?;
                tx.execute(
                    "UPDATE ion_control_revision SET revision=revision+1 WHERE id=1",
                    [],
                )?;
            }
            tx.commit()?;
            if let Some(document) = managed {
                document.unlink()?;
            }
            let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let current = claim(&tx, artifact.id)?.context("Cleanup claim disappeared")?;
            ensure!(
                current.request_id == request.request_id
                    && current.expected == expected
                    && current.storage == "file"
                    && current.state == "pending",
                "Cleanup claim changed while deleting its private copy"
            );
            // Team/automation SQL changes cannot make the already unavailable copy
            // available again; preserve completion after a successful unlink.
            tx.execute("UPDATE ion_artifact_cleanup SET state='cleaned',cleaned_at=? WHERE artifact_id=? AND state='pending'",params![now,artifact.id.to_string()])?;
            tx.execute(
                "UPDATE ion_control_revision SET revision=revision+1 WHERE id=1",
                [],
            )?;
            tx.commit()?;
        }
        Ok(ArtifactCleanupReceipt {
            request_id: request.request_id,
            artifact_id: artifact.id,
            bytes: evidence.bytes,
            cleaned_at: now,
        })
    }
}

fn not_link(metadata: &Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    true
}
struct ManagedDocument {
    file: File,
    directory: File,
    #[cfg(unix)]
    id: String,
}
impl ManagedDocument {
    fn open(database: &Path, id: Uuid, allow_missing: bool) -> Result<Option<Self>> {
        let parent = std::fs::canonicalize(
            database
                .parent()
                .context("Document database parent is missing")?,
        )?;
        let directory_path = parent.join("ion-documents");
        let meta = match std::fs::symlink_metadata(&directory_path) {
            Ok(meta) => meta,
            Err(error) if allow_missing && error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(None);
            }
            Err(error) => return Err(error.into()),
        };
        ensure!(
            meta.is_dir() && not_link(&meta),
            "Managed document directory must be a real private directory"
        );
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::{
                FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
            };
            options
                .access_mode(0x80)
                .share_mode(FILE_SHARE_READ)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        let directory = options.open(&directory_path)?;
        ensure!(
            directory.metadata()?.is_dir() && not_link(&directory.metadata()?),
            "Managed document directory changed"
        );
        #[cfg(not(unix))]
        let path = directory_path.join(id.to_string());
        #[cfg(not(unix))]
        let mut options = OpenOptions::new();
        #[cfg(not(unix))]
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::{
                DELETE, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
            };
            // No write/delete sharing: the verified file and pinned directory
            // cannot be replaced before deletion by this same opened handle.
            options
                .access_mode(0x8000_0000 | DELETE)
                .share_mode(FILE_SHARE_READ)
                .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
        }
        let file_result = {
            #[cfg(unix)]
            {
                use std::os::fd::{AsRawFd, FromRawFd};
                let name = std::ffi::CString::new(id.to_string())?;
                let fd = unsafe {
                    libc::openat(
                        directory.as_raw_fd(),
                        name.as_ptr(),
                        libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    )
                };
                if fd < 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(unsafe { File::from_raw_fd(fd) })
                }
            }
            #[cfg(not(unix))]
            {
                options.open(&path)
            }
        };
        let file = match file_result {
            Ok(f) => f,
            Err(e) if allow_missing && e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let meta = file.metadata()?;
        ensure!(
            meta.is_file() && not_link(&meta) && meta.len() > 0 && meta.len() <= MAX_BYTES,
            "Managed document is not a bounded regular file"
        );
        Ok(Some(Self {
            file,
            directory,
            #[cfg(unix)]
            id: id.to_string(),
        }))
    }
    fn verify(&self, evidence: &ArtifactEvidence) -> Result<()> {
        let mut bytes = Vec::new();
        (&self.file).take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
        ensure!(
            evidence.matches(&bytes),
            "Managed document content changed; original hash no longer matches"
        );
        Ok(())
    }
    fn unlink(self) -> Result<()> {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::Storage::FileSystem::{
                FILE_DISPOSITION_INFO, FileDispositionInfo, SetFileInformationByHandle,
            };
            let info = FILE_DISPOSITION_INFO { DeleteFile: true };
            let success = unsafe {
                SetFileInformationByHandle(
                    self.file.as_raw_handle(),
                    FileDispositionInfo,
                    &info as *const _ as *const _,
                    std::mem::size_of_val(&info) as u32,
                )
            };
            ensure!(
                success != 0,
                "Unable to clean managed document: {}",
                std::io::Error::last_os_error()
            );
            drop(self.file);
            drop(self.directory);
        }
        #[cfg(unix)]
        {
            use std::os::{fd::AsRawFd, unix::fs::MetadataExt};
            let name = std::ffi::CString::new(self.id)?;
            let mut meta = std::mem::MaybeUninit::<libc::stat>::uninit();
            ensure!(
                unsafe {
                    libc::fstatat(
                        self.directory.as_raw_fd(),
                        name.as_ptr(),
                        meta.as_mut_ptr(),
                        libc::AT_SYMLINK_NOFOLLOW,
                    )
                } == 0,
                "Managed document entry changed before cleanup"
            );
            let meta = unsafe { meta.assume_init() };
            let opened = self.file.metadata()?;
            ensure!(
                meta.st_dev as u64 == opened.dev() && meta.st_ino as u64 == opened.ino(),
                "Managed document entry changed before cleanup"
            );
            ensure!(
                unsafe { libc::unlinkat(self.directory.as_raw_fd(), name.as_ptr(), 0) } == 0,
                "Unable to clean managed document: {}",
                std::io::Error::last_os_error()
            );
            self.directory.sync_all()?;
        }
        #[cfg(not(any(windows, unix)))]
        {
            anyhow::bail!("Native artifact cleanup is unavailable on this platform");
        }
        Ok(())
    }
}
