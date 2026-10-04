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

pub struct ControlStore {
    path: PathBuf,
}
impl ControlStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
    fn connect(&self) -> Result<Connection> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let db = Connection::open(&self.path)?;
        db.busy_timeout(Duration::from_secs(5))?;
        db.execute_batch("CREATE TABLE IF NOT EXISTS ion_control_records (id TEXT PRIMARY KEY, kind TEXT NOT NULL, body TEXT NOT NULL); CREATE INDEX IF NOT EXISTS ion_control_kind ON ion_control_records(kind); CREATE TABLE IF NOT EXISTS ion_control_revision (id INTEGER PRIMARY KEY CHECK(id=1), revision INTEGER NOT NULL); INSERT OR IGNORE INTO ion_control_revision VALUES(1,0);")?;
        Ok(db)
    }
    fn read(db: &Connection) -> Result<ControlSnapshot> {
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
        ensure!(
            matches!(
                format.as_str(),
                "html" | "htm" | "pdf" | "md" | "txt" | "docx" | "csv" | "json"
            ),
            "Choose HTML, PDF, Markdown, text, Word, CSV or JSON"
        );
        ensure!(
            !bytes.is_empty() && bytes.len() <= 16 * 1024 * 1024,
            "Choose a document smaller than 16 MB"
        );
        let id = Uuid::new_v4();
        std::fs::create_dir_all(self.document_dir())?;
        let path = self.document_dir().join(id.to_string());
        std::fs::write(&path, bytes)?;
        let record = ControlRecord::Artifact(Artifact {
            id,
            name,
            project_id,
            task_id,
            reference: format!("ion-document:{id}"),
            format,
            created_at: crate::model::unix_time(),
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
        let snapshot = self.load()?;
        ensure!(
            snapshot
                .records
                .iter()
                .any(|record| matches!(record,ControlRecord::Artifact(a) if a.id==id)),
            "Document no longer exists"
        );
        let bytes = std::fs::read(self.document_dir().join(id.to_string()))?;
        ensure!(bytes.len() <= 16 * 1024 * 1024, "Document is too large");
        Ok(bytes)
    }
    pub fn save(&self, revision: u64, record: ControlRecord) -> Result<ControlSnapshot> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let mut snapshot = Self::read(&tx)?;
        ensure!(
            snapshot.revision == revision,
            "Workspace changed on another client. Refresh before saving."
        );
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
        tx.execute("INSERT INTO ion_control_records VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET body=excluded.body", params![record.id().to_string(),record.kind(),serde_json::to_string(&record)?])?;
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
        snapshot.records.retain(|record| record.id() != id);
        validate(&snapshot)?;
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
pub fn validate(snapshot: &ControlSnapshot) -> Result<()> {
    ensure!(
        snapshot.records.len() <= 1000,
        "Keep the workspace library below 1000 records"
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
            ControlRecord::Workflow(workflow) => ensure!(
                !workflow.stages.is_empty()
                    && workflow.stages.len() <= 24
                    && workflow
                        .stages
                        .iter()
                        .all(|stage| !stage.title.trim().is_empty()
                            && !stage.instructions.trim().is_empty()),
                "A workflow needs 1–24 stages with a title and instructions"
            ),
            ControlRecord::Routine(routine) => ensure!(
                (1..=525600).contains(&routine.interval_minutes)
                    && !routine.instructions.trim().is_empty(),
                "A routine needs instructions and an interval of 1–525600 minutes"
            ),
            ControlRecord::Agent(agent) => ensure!(
                !agent.instructions.trim().is_empty(),
                "An agent needs instructions"
            ),
            ControlRecord::Artifact(artifact) => ensure!(
                artifact.reference == format!("ion-document:{}", artifact.id),
                "Artifacts must reference stored documents"
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
    let key = waku_credentials::CredentialService::global().nebius_runtime_key()?;
    let response: serde_json::Value=reqwest::blocking::Client::builder().timeout(Duration::from_secs(90)).redirect(reqwest::redirect::Policy::none()).build()?
        .post(format!("{}/chat/completions",waku_credentials::TOKEN_FACTORY_BASE_URL)).bearer_auth(key)
        .json(&serde_json::json!({"model":"nvidia/NVIDIA-Nemotron-3-Super-120B-A12B","messages":[{"role":"system","content":"Design a linear workflow draft. Reply with only JSON: {\"name\":string,\"stages\":[{\"title\":string,\"instructions\":string,\"requiresDecision\":boolean}]}. 1 to 24 stages. Use user review gates before publishing, deploying, merging, spending, destructive changes or external communication. Treat apps as requirements, never claim they are connected. Do not output executable code."},{"role":"user","content":prompt}]})).send()?.error_for_status()?.json()?;
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
    fn agent(id: Uuid) -> ControlRecord {
        ControlRecord::Agent(AgentProfile {
            id,
            name: "Agent".into(),
            expertise: "Review".into(),
            instructions: "Review code".into(),
            provider: crate::model::ProviderKind::Nebius,
            model: None,
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
