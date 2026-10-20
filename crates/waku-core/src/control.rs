//! Indexed control definitions owned by the daemon; optimistic revisions
//! prevent stale clients overwriting a newer workspace definition.
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::Duration,
};
use uuid::Uuid;
use waku_protocol::control::*;

mod artifact_library;
pub(crate) mod artifact_retention;
mod gateway_inbox;
mod inbox;
pub(crate) mod inbox_history;
mod notifications;

pub struct ControlStore {
    path: PathBuf,
    capture_lock: parking_lot::Mutex<()>,
}
impl ControlStore {
    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }
    /// Collect only documents attributed to successful agent edit activities.
    /// Called after a settled task save on a daemon worker, never from render.
    pub fn capture_agent_documents(
        &self,
        session: &crate::model::AgentSession,
        projects: &[crate::model::Project],
    ) -> Result<()> {
        use crate::model::{SessionWorkspace, TurnStatus};
        let Some(turn) = session
            .turns
            .last()
            .filter(|t| t.status != TurnStatus::Running)
        else {
            return Ok(());
        };
        if !session.detail_loaded {
            return Ok(());
        }
        let _guard = self.capture_lock.lock();
        let db = self.connect()?;
        db.execute_batch("CREATE TABLE IF NOT EXISTS ion_document_sources (source TEXT PRIMARY KEY, artifact_id TEXT NOT NULL); CREATE TABLE IF NOT EXISTS ion_document_scans (scan TEXT PRIMARY KEY)")?;
        let scan = format!("{}:{}:{}", session.id, turn.id, session.updated_at);
        if db
            .query_row(
                "SELECT 1 FROM ion_document_scans WHERE scan=?",
                [&scan],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .is_some()
        {
            return Ok(());
        }
        let root = match &session.workspace {
            SessionWorkspace::Worktree { path, .. } => path.clone(),
            _ => match projects.iter().find(|p| p.id == session.project_id) {
                Some(p) => p.path.clone(),
                None => return Ok(()),
            },
        };
        let root = match std::fs::canonicalize(root) {
            Ok(root) => root,
            Err(_) => return Ok(()),
        };
        for activity in session
            .transcript_blocks
            .iter()
            .filter(|b| b.turn_id == Some(turn.id))
            .flat_map(|b| &b.activities)
            .filter(|a| a.complete && !a.failed)
        {
            for change in &activity.file_changes {
                let source = format!("{}:{}:{}:{}", session.id, turn.id, activity.id, change.path);
                if db
                    .query_row(
                        "SELECT 1 FROM ion_document_sources WHERE source=?",
                        [&source],
                        |row| row.get::<_, i64>(0),
                    )
                    .optional()?
                    .is_some()
                {
                    continue;
                }
                let path = match std::fs::canonicalize(root.join(&change.path)) {
                    Ok(path) => path,
                    Err(_) => continue,
                };
                if !path.starts_with(&root) {
                    continue;
                }
                let relative = path.strip_prefix(&root)?;
                if relative.components().any(|c| {
                    c.as_os_str() == ".git"
                        || c.as_os_str() == "node_modules"
                        || c.as_os_str() == ".env"
                }) {
                    continue;
                }
                let format = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if !matches!(
                    format.as_str(),
                    "html"
                        | "htm"
                        | "pdf"
                        | "md"
                        | "txt"
                        | "docx"
                        | "xlsx"
                        | "pptx"
                        | "png"
                        | "csv"
                        | "svg"
                        | "xml"
                        | "yaml"
                        | "yml"
                ) {
                    continue;
                }
                let metadata = std::fs::metadata(&path)?;
                if !metadata.is_file() || metadata.len() == 0 || metadata.len() > 16 * 1024 * 1024 {
                    continue;
                }
                use std::io::Read;
                let mut bytes = Vec::new();
                std::fs::File::open(&path)?
                    .take(16 * 1024 * 1024 + 1)
                    .read_to_end(&mut bytes)?;
                if bytes.len() > 16 * 1024 * 1024 {
                    continue;
                }
                let name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Document")
                    .to_owned();
                let snapshot = self.load()?;
                let existing: HashSet<_> = snapshot.records.iter().map(ControlRecord::id).collect();
                let snapshot = self.publish_document(
                    snapshot.revision,
                    name,
                    format,
                    Some(session.project_id),
                    Some(session.id),
                    bytes,
                    ArtifactProvenance {
                        origin: ArtifactOrigin::AgentEdit,
                        turn_id: Some(turn.id),
                        activity_id: Some(activity.id.to_string()),
                        source_path: Some(relative.to_string_lossy().into_owned()),
                        ..Default::default()
                    },
                )?;
                let artifact = snapshot
                    .records
                    .iter()
                    .find_map(|record| match record {
                        ControlRecord::Artifact(a) if !existing.contains(&a.id) => Some(a),
                        _ => None,
                    })
                    .context("Generated artifact was not saved")?;
                db.execute(
                    "INSERT OR IGNORE INTO ion_document_sources VALUES(?,?)",
                    params![source, artifact.id.to_string()],
                )?;
            }
        }
        db.execute("INSERT OR IGNORE INTO ion_document_scans VALUES(?)", [scan])?;
        Ok(())
    }
    pub fn add_agent_action(&self, action: WorkspaceAction) -> Result<ControlSnapshot> {
        for _ in 0..4 {
            let snapshot = self.load()?;
            if snapshot.records.iter().any(|r| matches!(r, ControlRecord::Action(a) if a.task_id==action.task_id && a.title==action.title && a.detail==action.detail)) { return Ok(snapshot); }
            match self.save(snapshot.revision, ControlRecord::Action(action.clone())) {
                Ok(saved) => return Ok(saved),
                Err(error) if self.load()?.revision == snapshot.revision => return Err(error),
                Err(_) => {}
            }
        }
        anyhow::bail!("Workspace changed while saving proposal. Try again.")
    }
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            capture_lock: parking_lot::Mutex::new(()),
        }
    }
    pub(crate) fn connect(&self) -> Result<Connection> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let db = Connection::open(&self.path)?;
        db.busy_timeout(Duration::from_secs(5))?;
        db.execute_batch("CREATE TABLE IF NOT EXISTS ion_control_records (id TEXT PRIMARY KEY, kind TEXT NOT NULL, body TEXT NOT NULL); CREATE INDEX IF NOT EXISTS ion_control_kind ON ion_control_records(kind); CREATE TABLE IF NOT EXISTS ion_control_revision (id INTEGER PRIMARY KEY CHECK(id=1), revision INTEGER NOT NULL); INSERT OR IGNORE INTO ion_control_revision VALUES(1,0);")?;
        inbox_history::install_control(&db)?;
        artifact_library::schema(&db)?;
        artifact_retention::schema(&db)?;
        crate::usage_ledger::schema(&db)?;
        crate::cloud_spending::schema(&db)?;
        crate::cloud_billing::schema(&db)?;
        Ok(db)
    }
    pub(crate) fn read(db: &Connection) -> Result<ControlSnapshot> {
        let revision = db.query_row(
            "SELECT revision FROM ion_control_revision WHERE id=1",
            [],
            |row| row.get(0),
        )?;
        let mut query =
            db.prepare("SELECT body FROM ion_control_records ORDER BY kind,id LIMIT 1001")?;
        let mut records = Vec::new();
        for body in query.query_map([], |row| row.get::<_, String>(0))? {
            records.push(serde_json::from_str(&body?)?);
        }
        ensure!(
            records.len() <= 1000,
            "Workspace control library exceeded its 1000 record limit"
        );
        Ok(ControlSnapshot { revision, records })
    }
    pub fn load(&self) -> Result<ControlSnapshot> {
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let snapshot = Self::read(&tx)?;
        tx.commit()?;
        Ok(snapshot)
    }
    fn document_dir(&self) -> PathBuf {
        self.path.with_file_name("ion-documents")
    }
    pub fn import_document(
        &self,
        revision: u64,
        name: String,
        format: String,
        project_id: Option<Uuid>,
        task_id: Option<Uuid>,
        bytes: Vec<u8>,
    ) -> Result<ControlSnapshot> {
        self.publish_document(
            revision,
            name,
            format,
            project_id,
            task_id,
            bytes,
            ArtifactProvenance::default(),
        )
    }
    /// Only the trusted native tool call supplies publisher provenance.
    pub(crate) fn publish_generated_document(
        &self,
        revision: u64,
        name: String,
        format: String,
        project: Uuid,
        task: Uuid,
        call_id: String,
        bytes: Vec<u8>,
    ) -> Result<ControlSnapshot> {
        ensure!(
            !project.is_nil() && !task.is_nil() && !call_id.is_empty() && call_id.len() <= 512,
            "Generated document needs trusted task/tool provenance"
        );
        self.publish_document(
            revision,
            name,
            format,
            Some(project),
            Some(task),
            bytes,
            ArtifactProvenance {
                origin: ArtifactOrigin::AgentGenerated,
                activity_id: Some(call_id),
                ..Default::default()
            },
        )
    }
    fn publish_document(
        &self,
        revision: u64,
        name: String,
        format: String,
        project_id: Option<Uuid>,
        task_id: Option<Uuid>,
        bytes: Vec<u8>,
        producer: ArtifactProvenance,
    ) -> Result<ControlSnapshot> {
        ensure!(
            !name.contains(['/', '\\', ':', '\0']),
            "Use a document filename without directories"
        );
        ensure!(
            matches!(
                format.as_str(),
                "html"
                    | "htm"
                    | "pdf"
                    | "md"
                    | "txt"
                    | "docx"
                    | "xlsx"
                    | "pptx"
                    | "png"
                    | "csv"
                    | "svg"
                    | "xml"
                    | "yaml"
                    | "yml"
                    | "json"
            ),
            "Choose HTML, PDF, Markdown, text, Word, CSV, JSON, SVG, XML or YAML"
        );
        ensure!(
            !bytes.is_empty() && bytes.len() <= 16 * 1024 * 1024,
            "Choose a document smaller than 16 MB"
        );
        let id = Uuid::new_v4();
        std::fs::create_dir_all(self.document_dir())?;
        let path = self.document_dir().join(id.to_string());
        let evidence = ArtifactEvidence::from_bytes(&bytes, producer);
        std::fs::write(&path, bytes)?;
        let record = ControlRecord::Artifact(Artifact {
            id,
            name,
            project_id,
            task_id,
            reference: format!("ion-document:{id}"),
            format,
            created_at: crate::model::unix_time(),
            evidence: Some(evidence),
        });
        match self.save(revision, record) {
            Ok(snapshot) => Ok(snapshot),
            Err(error) => {
                let _ = std::fs::remove_file(path);
                Err(error)
            }
        }
    }
    pub fn read_document(&self, id: Uuid) -> Result<Vec<u8>> {
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let body: String = tx
            .query_row(
                "SELECT body FROM ion_control_records WHERE id=? AND kind='artifact'",
                [id.to_string()],
                |r| r.get(0),
            )
            .context("Document no longer exists")?;
        let ControlRecord::Artifact(artifact) = serde_json::from_str(&body)? else {
            anyhow::bail!("Document identity changed");
        };
        ensure!(
            artifact.id == id && artifact.reference == format!("ion-document:{id}"),
            "Document identity changed"
        );
        let bytes = verified_document(&tx, &artifact)?;
        tx.commit()?;
        Ok(bytes)
    }
    pub fn save_client_record(
        &self,
        revision: u64,
        record: ControlRecord,
    ) -> Result<ControlSnapshot> {
        ensure!(
            !matches!(record, ControlRecord::Artifact(_)),
            "Artifacts are published by agents; their original evidence cannot be edited by clients"
        );
        self.save(revision, record)
    }
    pub fn save(&self, revision: u64, record: ControlRecord) -> Result<ControlSnapshot> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let mut snapshot = Self::read(&tx)?;
        ensure!(
            snapshot.revision == revision,
            "Workspace changed on another client. Refresh before saving."
        );
        if let ControlRecord::Artifact(artifact) = &record {
            artifact_retention::ensure_available(&tx, artifact.id)?;
        }
        let previous: Option<String> = tx
            .query_row(
                "SELECT kind FROM ion_control_records WHERE id=?",
                [record.id().to_string()],
                |row| row.get(0),
            )
            .optional()?;
        ensure!(
            previous.as_deref().is_none_or(|kind| kind == record.kind()),
            "Record kind cannot change"
        );
        snapshot.records.retain(|entry| entry.id() != record.id());
        snapshot.records.push(record.clone());
        validate(&snapshot)?;
        crate::teams::validate_automation_definition(&record, &snapshot)?;
        crate::cloud_spending::validate_change(&tx, &record)?;
        tx.execute("INSERT INTO ion_control_records VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET body=excluded.body", params![record.id().to_string(),record.kind(),serde_json::to_string(&record)?])?;
        crate::teams::sync_automation_definition(&tx, &record)?;
        tx.execute(
            "UPDATE ion_control_revision SET revision=revision+1 WHERE id=1",
            [],
        )?;
        let result = Self::read(&tx)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn remove(&self, revision: u64, id: Uuid) -> Result<ControlSnapshot> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let mut snapshot = Self::read(&tx)?;
        ensure!(
            snapshot.revision == revision,
            "Workspace changed. Refresh before deleting."
        );
        ensure!(
            !snapshot
                .records
                .iter()
                .any(|record| matches!(record, ControlRecord::Artifact(a) if a.id == id)),
            "Use artifact storage cleanup; original publication evidence must be preserved"
        );
        snapshot.records.retain(|record| record.id() != id);
        validate(&snapshot)?;
        crate::cloud_spending::ensure_removable(&tx, id)?;
        crate::teams::remove_automation_definition(&tx, id)?;
        tx.execute(
            "DELETE FROM ion_control_records WHERE id=?",
            [id.to_string()],
        )?;
        tx.execute(
            "UPDATE ion_control_revision SET revision=revision+1 WHERE id=1",
            [],
        )?;
        let snapshot = Self::read(&tx)?;
        tx.commit()?;
        Ok(snapshot)
    }
}
/// Verify stored bytes at every result, handoff, model read and export boundary.
pub(crate) fn verified_document(db: &Connection, artifact: &Artifact) -> Result<Vec<u8>> {
    artifact_retention::ensure_available(db, artifact.id)?;
    ensure!(
        artifact.reference == format!("ion-document:{}", artifact.id),
        "Document identity changed"
    );
    // Team documents commit their payload, provenance and tool receipt in
    // the same control transaction. Older imported documents remain files.
    let has_team_documents = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='ion_team_document_receipts')",
            [], |row| row.get::<_, bool>(0),
        )?;
    let stored: Option<(String, String, Vec<u8>)> = if has_team_documents {
        let length: Option<u64> = db
            .query_row(
                "SELECT length(content) FROM ion_team_document_receipts WHERE artifact_id=?",
                [artifact.id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        ensure!(
            length.is_none_or(|bytes| bytes > 0 && bytes <= 16 * 1024 * 1024),
            "Team document exceeds its bound"
        );
        db.query_row(
            "SELECT name,format,content FROM ion_team_document_receipts WHERE artifact_id=?",
            [artifact.id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
    } else {
        None
    };
    let bytes = match stored {
        Some((name, format, bytes)) => {
            ensure!(
                artifact.name == name && artifact.format == format,
                "Document metadata changed"
            );
            crate::teams::verify_binary_document(db, artifact, &bytes)?;
            bytes
        }
        None => {
            use std::io::Read;
            let path = PathBuf::from(db.path().context("Document database path is unavailable")?)
                .with_file_name("ion-documents")
                .join(artifact.id.to_string());
            let meta = std::fs::symlink_metadata(&path)?;
            ensure!(
                meta.is_file() && !meta.file_type().is_symlink() && meta.len() <= 16 * 1024 * 1024,
                "Document storage is not a bounded regular file"
            );
            let mut bytes = Vec::new();
            std::fs::File::open(path)?
                .take(16 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            bytes
        }
    };
    ensure!(bytes.len() <= 16 * 1024 * 1024, "Document is too large");
    ensure!(
        artifact.evidence.as_ref().is_none_or(|e| e.matches(&bytes)),
        "Document content changed; its original hash no longer matches"
    );
    Ok(bytes)
}

pub fn validate(snapshot: &ControlSnapshot) -> Result<()> {
    ensure!(
        snapshot.records.len() <= 1000,
        "Keep the workspace library below 1000 records"
    );
    let mut budget_projects = HashSet::new();
    for budget in snapshot.records.iter().filter_map(|r| {
        if let ControlRecord::CloudBudget(b) = r {
            Some(b)
        } else {
            None
        }
    }) {
        ensure!(
            budget_projects.insert(&budget.cloud_project),
            "Only one shared GPU budget can authorize a Nebius project"
        );
    }
    ensure!(
        budget_projects.len() <= 64,
        "Keep at most 64 shared GPU budgets"
    );
    let agents: HashSet<_> = snapshot
        .records
        .iter()
        .filter_map(|entry| {
            if let ControlRecord::Agent(agent) = entry {
                Some(agent.id)
            } else {
                None
            }
        })
        .collect();
    for record in &snapshot.records {
        crate::teams::validate_automation_definition(record, snapshot)?;
        ensure!(
            !record.name().trim().is_empty() && record.name().len() <= 160,
            "Enter a name of at most 160 characters"
        );
        ensure!(
            serde_json::to_vec(record)?.len() <= 128 * 1024,
            "Definition is too large"
        );
        match record {
            ControlRecord::Team(team) => {
                if let Some(limits) = &team.limits {
                    crate::teams::validate_limits(limits)?;
                }
                ensure!(
                    !team.members.is_empty() && team.members.len() <= 32,
                    "A team needs 1–32 agents"
                );
                let parents: HashMap<_, _> = team
                    .members
                    .iter()
                    .map(|member| (member.agent_id, member.parent_id))
                    .collect();
                ensure!(
                    parents.len() == team.members.len(),
                    "An agent can only appear once per team"
                );
                ensure!(
                    team.members
                        .iter()
                        .filter(|member| member.parent_id.is_none())
                        .count()
                        == 1,
                    "A team needs exactly one lead"
                );
                for member in &team.members {
                    ensure!(
                        agents.contains(&member.agent_id),
                        "Team agent no longer exists"
                    );
                    let mut path = HashSet::new();
                    let mut id = Some(member.agent_id);
                    while let Some(current) = id {
                        ensure!(path.insert(current), "Team hierarchy contains a cycle");
                        id = *parents
                            .get(&current)
                            .context("Parent must belong to the same team")?;
                    }
                }
            }
            ControlRecord::Workflow(workflow) => {
                workflow.validate_graph().map_err(anyhow::Error::msg)?;
                for stage in &workflow.stages {
                    stage.document_contract().map_err(anyhow::Error::msg)?;
                }
                ensure!(
                    !workflow.stages.is_empty()
                        && workflow.stages.len() <= 24
                        && workflow
                            .stages
                            .iter()
                            .all(|stage| !stage.title.trim().is_empty()
                                && !stage.instructions.trim().is_empty()),
                    "A workflow needs 1–24 stages with a title and instructions"
                );
            }
            ControlRecord::Routine(routine) => {
                ensure!(
                    !routine.instructions.trim().is_empty(),
                    "A routine needs instructions"
                );
                crate::routine_schedule::validate(&RoutineScheduleRequest {
                    interval_minutes: routine.interval_minutes,
                    calendar: routine.calendar.clone(),
                })?;
            }
            ControlRecord::CloudBudget(budget) => crate::cloud_spending::validate(budget)?,
            ControlRecord::Agent(agent) => {
                ensure!(
                    !agent.instructions.trim().is_empty(),
                    "An agent needs instructions"
                );
                crate::teams::validate_profile(agent)?;
            }
            ControlRecord::Artifact(artifact) => ensure!(
                artifact.reference == format!("ion-document:{}", artifact.id),
                "Artifacts must reference stored documents"
            ),
            ControlRecord::Action(action) => ensure!(
                !action.detail.trim().is_empty() && action.detail.len() <= 32 * 1024,
                "An action needs details of at most 32 KiB"
            ),
        }
    }
    Ok(())
}

pub fn generate_workflow(project_id: Uuid, prompt: String) -> Result<Workflow> {
    ensure!(
        !prompt.trim().is_empty() && prompt.len() <= 16000,
        "Describe the workflow in at most 16000 characters"
    );
    let transport = waku_credentials::inference::Inference::runtime()?;
    let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(90)).redirect(reqwest::redirect::Policy::none()).build()?;
    let response: serde_json::Value=transport.request(&client, "chat/completions")?
        .json(&serde_json::json!({"model":"nvidia/NVIDIA-Nemotron-3-Super-120B-A12B","messages":[{"role":"system","content":"Design an editable forward-only workflow draft. Reply with only JSON: {\"name\":string,\"stages\":[{\"title\":string,\"instructions\":string,\"requiresDecision\":boolean,\"requiredFormats\":[]}]}. 1 to 24 stages. The requiredFormats array may contain html, md, txt, csv, json, pdf, svg, xml, yaml, yml, docx, xlsx, pptx and png, with no duplicates. Each stage may include transition: {type: next}, {type: goTo, target: {type: stage, index: zero-based-index} or {type: finish}}, or {type: documentCondition, document: exact-file.json, pointer: /ready, equals: JSON scalar, onMatch: target, onMismatch: target}. Links must target a later stage or finish. Every stage must be reachable from stage zero. Conditions require that stage to produce the exact named JSON document and the selected scalar pointer. Use next by default; no loops or parallel effects. Set inheritWorkspace: true on later stages only when they need the preceding completed stage's receipted code. Keep it false for the first stage and independent work. Code stays isolated until human source review. Require formats only when the requested output needs them; use an empty array otherwise. Use user review gates before publishing, deploying, merging, spending, destructive changes or external communication. Treat apps as requirements, never claim they are connected. Do not output executable code."},{"role":"user","content":prompt}]})).send()?.error_for_status()?.json()?;
    let content = response["choices"][0]["message"]["content"]
        .as_str()
        .context("Model returned no workflow")?;
    let json = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    #[derive(serde::Deserialize)]
    struct Draft {
        name: String,
        stages: Vec<WorkflowStage>,
    }
    let draft: Draft = serde_json::from_str(json)
        .context("Model returned an invalid workflow. Try again or define stages manually.")?;
    let workflow = Workflow {
        id: Uuid::new_v4(),
        name: draft.name,
        project_id,
        prompt,
        stages: draft.stages,
        limits: None,
    };
    validate(&ControlSnapshot {
        revision: 0,
        records: vec![ControlRecord::Workflow(workflow.clone())],
    })?;
    Ok(workflow)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn agent_proposals_survive_restart_and_stale_acceptance_is_rejected() {
        let directory = std::env::temp_dir().join(format!("ion-actions-{}", Uuid::new_v4()));
        let path = directory.join("control.db");
        let store = ControlStore::new(path.clone());
        let mut action = WorkspaceAction {
            id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            task_id: Uuid::new_v4(),
            title: "Review release".into(),
            detail: "Review the generated release documents".into(),
            action_kind: WorkspaceActionKind::ProposedTask,
            status: WorkspaceActionStatus::Pending,
            created_at: 1,
        };
        let snapshot = store.add_agent_action(action.clone()).unwrap();
        assert_eq!(
            store.add_agent_action(action.clone()).unwrap().revision,
            snapshot.revision
        );
        action.status = WorkspaceActionStatus::Accepted;
        let accepted = store
            .save(snapshot.revision, ControlRecord::Action(action.clone()))
            .unwrap();
        assert!(
            store
                .save(snapshot.revision, ControlRecord::Action(action))
                .is_err()
        );
        let reopened = ControlStore::new(path).load().unwrap();
        assert_eq!(reopened.revision, accepted.revision);
        assert!(
            matches!(&reopened.records[0],ControlRecord::Action(a) if a.status==WorkspaceActionStatus::Accepted)
        );
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn collects_agent_documents_once_and_never_imports_outside_project_or_unrelated_files() {
        use crate::model::{
            ActivityFileChange, ActivityItem, ActivityKind, AgentSession, AgentTurn, Project,
            ProviderKind, TranscriptBlock, TurnStatus,
        };
        let directory = std::env::temp_dir().join(format!("ion-artifacts-{}", Uuid::new_v4()));
        let project_dir = directory.join("project");
        std::fs::create_dir_all(&project_dir).unwrap();
        std::fs::write(project_dir.join("agent.html"), b"<h1>Agent report</h1>").unwrap();
        std::fs::write(project_dir.join("user.html"), b"User document").unwrap();
        std::fs::write(directory.join("outside.html"), b"Outside document").unwrap();
        let project = Project {
            id: Uuid::new_v4(),
            name: "Project".into(),
            path: project_dir,
            created_at: 1,
        };
        let mut session = AgentSession::new(project.id, ProviderKind::Claude);
        let turn_id = Uuid::new_v4();
        session.turns.push(AgentTurn {
            id: turn_id,
            turn_count: 1,
            status: TurnStatus::Completed,
            provider_turn_started: true,
            provider_resume_at: None,
            started_at: 1,
            completed_at: Some(2),
            checkpoint: None,
        });
        let mut activity = ActivityItem::new(None, ActivityKind::Tool, "Write report", None, true);
        activity.file_changes = ["agent.html", "../outside.html"]
            .into_iter()
            .map(|path| ActivityFileChange {
                path: path.into(),
                additions: None,
                deletions: None,
                status: None,
                diff: None,
            })
            .collect();
        session.transcript_blocks.push(TranscriptBlock {
            after_message: 0,
            turn_id: Some(turn_id),
            activities: vec![activity],
        });
        let store = ControlStore::new(directory.join("control.db"));
        // A later-sorting record must not be mistaken for the newly saved artifact.
        store
            .save(
                0,
                ControlRecord::Routine(Routine {
                    id: Uuid::new_v4(),
                    name: "Daily report".into(),
                    project_id: project.id,
                    instructions: "Summarize updates".into(),
                    interval_minutes: 60,
                    calendar: None,
                    missed_run_policy: MissedRunPolicy::default(),
                    overlap_policy: OverlapPolicy::default(),
                    paused: true,
                    next_run: 0,
                    provider: ProviderKind::Claude,
                    team_id: None,
                    workflow_id: None,
                    last_error: None,
                }),
            )
            .unwrap();
        store
            .capture_agent_documents(&session, &[project.clone()])
            .unwrap();
        session.updated_at += 1;
        store.capture_agent_documents(&session, &[project]).unwrap();
        let snapshot = store.load().unwrap();
        assert_eq!(snapshot.records.len(), 2);
        assert!(
            matches!(&snapshot.records[0],ControlRecord::Artifact(a) if a.name=="agent.html"&&a.task_id==Some(session.id))
        );
        assert_eq!(
            store.read_document(snapshot.records[0].id()).unwrap(),
            b"<h1>Agent report</h1>"
        );
        let ControlRecord::Artifact(artifact) = &snapshot.records[0] else {
            panic!("artifact");
        };
        let evidence = artifact.evidence.as_ref().unwrap();
        assert!(evidence.matches(b"<h1>Agent report</h1>"));
        assert_eq!(evidence.producer.origin, ArtifactOrigin::AgentEdit);
        assert_eq!(evidence.producer.turn_id, Some(turn_id));
        assert_eq!(evidence.producer.source_path.as_deref(), Some("agent.html"));
        assert!(evidence.producer.activity_id.is_some());
        std::fs::remove_dir_all(directory).unwrap();
    }
    fn agent(id: Uuid) -> ControlRecord {
        ControlRecord::Agent(AgentProfile {
            id,
            name: "Agent".into(),
            expertise: "Review".into(),
            instructions: "Review code".into(),
            provider: crate::model::ProviderKind::Nebius,
            model: None,
            expected_output: String::new(),
            execution: Default::default(),
            access: Default::default(),
            max_parallel_tasks: 1,
        })
    }
    #[test]
    fn rejects_cycles_and_cross_team_parents() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();
        let team = Team {
            id: Uuid::new_v4(),
            name: "Team".into(),
            objective: "Ship".into(),
            limits: None,
            members: vec![
                TeamMember {
                    agent_id: a,
                    parent_id: None,
                },
                TeamMember {
                    agent_id: b,
                    parent_id: Some(c),
                },
                TeamMember {
                    agent_id: c,
                    parent_id: Some(b),
                },
            ],
        };
        let mut snapshot = ControlSnapshot {
            revision: 0,
            records: vec![agent(a), agent(b), agent(c), ControlRecord::Team(team)],
        };
        assert!(validate(&snapshot).is_err());
        if let ControlRecord::Team(team) = snapshot.records.last_mut().unwrap() {
            team.members[2].parent_id = Some(a);
        }
        assert!(validate(&snapshot).is_ok());
    }
    #[test]
    fn persists_definitions_and_rejects_stale_writes() {
        let path = std::env::temp_dir().join(format!("ion-control-test-{}.db", Uuid::new_v4()));
        let store = ControlStore::new(path.clone());
        let id = Uuid::new_v4();
        assert_eq!(store.save(0, agent(id)).unwrap().revision, 1);
        assert!(store.save(0, agent(id)).is_err());
        assert_eq!(store.load().unwrap().records.len(), 1);
        assert_eq!(store.remove(1, id).unwrap().records.len(), 0);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn artifact_integrity_rejects_changed_bytes_unbounded_files_and_client_metadata_edits() {
        let dir = std::env::temp_dir().join(format!("ion-artifact-integrity-{}", Uuid::new_v4()));
        let store = ControlStore::new(dir.join("control.db"));
        let bytes = b"<h1>Original generated report</h1>".to_vec();
        let snapshot = store
            .import_document(
                0,
                "report.html".into(),
                "html".into(),
                Some(Uuid::new_v4()),
                Some(Uuid::new_v4()),
                bytes.clone(),
            )
            .unwrap();
        let ControlRecord::Artifact(artifact) = snapshot.records[0].clone() else {
            panic!("artifact");
        };
        assert!(artifact.evidence.as_ref().unwrap().matches(&bytes));
        let reopened = ControlStore::new(dir.join("control.db"));
        assert_eq!(reopened.read_document(artifact.id).unwrap(), bytes);
        let mut edited = artifact.clone();
        edited.name = "Forged name.html".into();
        edited.evidence = None;
        assert!(
            reopened
                .save_client_record(snapshot.revision, ControlRecord::Artifact(edited))
                .is_err()
        );
        assert_eq!(reopened.load().unwrap().revision, snapshot.revision);
        let path = store.document_dir().join(artifact.id.to_string());
        std::fs::write(&path, b"<h1>Modified bytes</h1>").unwrap();
        assert!(reopened.read_document(artifact.id).is_err());
        std::fs::File::create(&path)
            .unwrap()
            .set_len(16 * 1024 * 1024 + 1)
            .unwrap();
        assert!(reopened.read_document(artifact.id).is_err());
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(reopened.read_document(artifact.id).unwrap(), bytes);
        let mut old = serde_json::to_value(&artifact).unwrap();
        old.as_object_mut().unwrap().remove("evidence");
        let legacy: Artifact = serde_json::from_value(old).unwrap();
        assert!(legacy.evidence.is_none());
        assert_eq!(
            std::fs::canonicalize(&dir).unwrap().parent(),
            Some(
                std::fs::canonicalize(std::env::temp_dir())
                    .unwrap()
                    .as_path()
            )
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn imports_document_bytes_and_cleans_a_rejected_import() {
        let directory = std::env::temp_dir().join(format!("ion-documents-test-{}", Uuid::new_v4()));
        let store = ControlStore::new(directory.join("control.db"));
        let bytes = b"<h1>ion</h1>".to_vec();
        let snapshot = store
            .import_document(
                0,
                "report.html".into(),
                "html".into(),
                None,
                None,
                bytes.clone(),
            )
            .unwrap();
        assert_eq!(
            store.read_document(snapshot.records[0].id()).unwrap(),
            bytes
        );
        assert!(
            store
                .import_document(
                    0,
                    "stale.html".into(),
                    "html".into(),
                    None,
                    None,
                    b"draft".to_vec()
                )
                .is_err()
        );
        assert!(
            store
                .import_document(
                    1,
                    "unsafe.exe".into(),
                    "exe".into(),
                    None,
                    None,
                    b"draft".to_vec()
                )
                .is_err()
        );
        assert_eq!(std::fs::read_dir(store.document_dir()).unwrap().count(), 1);
        std::fs::remove_dir_all(directory).unwrap();
    }
}

/// Bound serialized JSON string cost while preserving UTF-8 boundaries.
pub(crate) fn json_excerpt(value: &str, max: usize) -> &str {
    let mut cost = 0;
    let mut end = 0;
    for (offset, character) in value.char_indices() {
        let bytes = match character {
            '"' | '\\' => 2,
            '\u{0}'..='\u{1f}' => 6,
            _ => character.len_utf8(),
        };
        if cost + bytes > max {
            break;
        }
        cost += bytes;
        end = offset + character.len_utf8();
    }
    &value[..end]
}
