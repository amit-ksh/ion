use super::*;

struct Fixture {
    store: ControlStore,
    directory: PathBuf,
    project: Uuid,
    task: Uuid,
}
impl Fixture {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("ion-retention-tests-{}", Uuid::new_v4()));
        Self {
            store: ControlStore::new(directory.join("control.sqlite")),
            directory,
            project: Uuid::new_v4(),
            task: Uuid::new_v4(),
        }
    }
    fn add(&self) -> Artifact {
        let name = format!("{}.md", Uuid::new_v4());
        let snapshot = self
            .store
            .publish_generated_document(
                self.store.load().unwrap().revision,
                name.clone(),
                "md".into(),
                self.project,
                self.task,
                Uuid::new_v4().to_string(),
                b"retained original bytes".to_vec(),
            )
            .unwrap();
        snapshot
            .records
            .into_iter()
            .find_map(|r| match r {
                ControlRecord::Artifact(a) if a.name == name => Some(a),
                _ => None,
            })
            .unwrap()
    }
    fn request(&self, artifact: &Artifact) -> ArtifactCleanupRequest {
        ArtifactCleanupRequest {
            request_id: Uuid::new_v4(),
            expected: artifact.clone(),
        }
    }
    fn clean(&self, request: &ArtifactCleanupRequest, now: u64) -> Result<ArtifactCleanupReceipt> {
        self.store
            .cleanup_artifact(request, &HashSet::new(), &HashSet::new(), now)
    }
    fn path(&self, artifact: &Artifact) -> PathBuf {
        self.directory
            .join("ion-documents")
            .join(artifact.id.to_string())
    }
    fn page(&self) -> ArtifactRetentionPage {
        self.store
            .list_artifact_retention(
                &ArtifactRetentionQuery {
                    project_id: None,
                    task_id: None,
                    after: None,
                    limit: 32,
                },
                &HashSet::new(),
                &HashSet::new(),
            )
            .unwrap()
    }
    fn state(&self, id: Uuid) -> ArtifactRetentionState {
        self.page()
            .items
            .into_iter()
            .find(|i| i.artifact.id == id)
            .unwrap()
            .state
    }
    fn pending(&self, request: &ArtifactCleanupRequest) {
        self.store
            .connect()
            .unwrap()
            .execute(
                "INSERT INTO ion_artifact_cleanup VALUES(?,?,?,?,?,'pending',23,NULL)",
                params![
                    request.expected.id.to_string(),
                    request.request_id.to_string(),
                    serialize(&request.expected).unwrap(),
                    request.expected.evidence.as_ref().unwrap().bytes,
                    "file"
                ],
            )
            .unwrap();
    }
    fn automation(&self, status_text: &str) -> Uuid {
        let status = serde_json::from_str::<AutomationStatus>(status_text)
            .or_else(|_| serde_json::from_value(serde_json::Value::String(status_text.into())))
            .unwrap_or(AutomationStatus::Running);
        let run = AutomationRun {
            id: Uuid::new_v4(),
            trigger: None,
            name: "Retention workflow".into(),
            project_id: self.project,
            workflow_id: Some(Uuid::new_v4()),
            routine_id: None,
            definition_revision: 1,
            revision: 1,
            created_at: 1,
            scheduled_at: None,
            deadline: 1000,
            status,
            reason: None,
            current_stage: 0,
            stages: vec![],
        };
        // Fixture publication installed the real automation schema through
        // ControlStore::save; use its required columns, not a shadow schema.
        let db = self.store.connect().unwrap();
        db.execute("INSERT INTO ion_automation_runs(id,definition_id,project_id,status,revision,signature,body,pinned) VALUES(?,?,?,?,?,?,?,?)",
            params![run.id.to_string(),run.workflow_id.unwrap().to_string(),self.project.to_string(),status_text,run.revision,"retention-fixture",serde_json::to_string(&run).unwrap(),"{}"])
            .unwrap();
        run.id
    }
    fn team(&self, artifact: &Artifact, run_status: &str, work_status: &str) -> Artifact {
        let run = Uuid::new_v4();
        let work = Uuid::new_v4();
        let agent = Uuid::new_v4();
        let request = Uuid::new_v4();
        let db = self.store.connect().unwrap();
        db.execute_batch("CREATE TABLE ion_team_runs(id TEXT PRIMARY KEY,root_task_id TEXT,project_id TEXT,status TEXT,body TEXT);CREATE TABLE ion_team_work(id TEXT PRIMARY KEY,run_id TEXT,task_id TEXT,agent_id TEXT,status TEXT,body TEXT);CREATE TABLE ion_team_document_receipts(work_id TEXT,request_id TEXT,artifact_id TEXT UNIQUE,name TEXT,format TEXT,content BLOB,PRIMARY KEY(work_id,request_id));CREATE TABLE ion_team_attempts(id TEXT PRIMARY KEY,run_id TEXT,work_id TEXT,live INTEGER,capacity INTEGER);") .unwrap();
        let body = serde_json::json!({"id":run,"rootTaskId":self.task,"projectId":self.project})
            .to_string();
        db.execute(
            "INSERT INTO ion_team_runs VALUES(?,?,?,?,?)",
            params![
                run.to_string(),
                self.task.to_string(),
                self.project.to_string(),
                run_status,
                body
            ],
        )
        .unwrap();
        let body = serde_json::json!({"id":work,"taskId":self.task,"agentId":agent}).to_string();
        db.execute(
            "INSERT INTO ion_team_work VALUES(?,?,?,?,?,?)",
            params![
                work.to_string(),
                run.to_string(),
                self.task.to_string(),
                agent.to_string(),
                work_status,
                body
            ],
        )
        .unwrap();
        db.execute(
            "INSERT INTO ion_team_document_receipts VALUES(?,?,?,?,?,?)",
            params![
                work.to_string(),
                request.to_string(),
                artifact.id.to_string(),
                artifact.name,
                artifact.format,
                b"retained original bytes".as_slice()
            ],
        )
        .unwrap();
        let mut artifact = artifact.clone();
        artifact.evidence.as_mut().unwrap().producer = ArtifactProvenance {
            origin: ArtifactOrigin::TeamAssignment,
            run_id: Some(run),
            assignment_id: Some(work),
            agent_id: Some(agent),
            request_id: Some(request),
            ..Default::default()
        };
        db.execute(
            "UPDATE ion_control_records SET body=? WHERE id=?",
            params![
                serde_json::to_string(&ControlRecord::Artifact(artifact.clone())).unwrap(),
                artifact.id.to_string()
            ],
        )
        .unwrap();
        std::fs::remove_file(self.path(&artifact)).unwrap();
        artifact
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert!(
            self.directory.parent() == Some(std::env::temp_dir().as_path())
                && self
                    .directory
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("ion-retention-tests-")
        );
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn default_keeps_documents_and_has_no_age_based_cleanup() {
    let f = Fixture::new();
    let artifact = f.add();
    assert_eq!(f.state(artifact.id), ArtifactRetentionState::Retained);
    assert_eq!(
        f.store.read_document(artifact.id).unwrap(),
        b"retained original bytes"
    );
    assert_eq!(
        f.store
            .connect()
            .unwrap()
            .query_row("SELECT count(*) FROM ion_artifact_cleanup", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
}
#[test]
fn cleanup_removes_only_private_copy_and_keeps_original_publication() {
    let f = Fixture::new();
    let artifact = f.add();
    let original = f.directory.join("original-project-document.md");
    std::fs::write(&original, b"retained original bytes").unwrap();
    let request = f.request(&artifact);
    let receipt = f.clean(&request, 100).unwrap();
    assert_eq!(receipt.artifact_id, artifact.id);
    assert_eq!(receipt.bytes, 23);
    assert!(!f.path(&artifact).exists());
    assert_eq!(std::fs::read(original).unwrap(), b"retained original bytes");
    assert_eq!(
        serialize(&stored(&f.store.connect().unwrap(), artifact.id).unwrap()).unwrap(),
        serialize(&artifact).unwrap()
    );
    assert_eq!(f.state(artifact.id), ArtifactRetentionState::Cleaned);
    assert!(
        f.store
            .read_document(artifact.id)
            .unwrap_err()
            .to_string()
            .contains("explicitly cleaned")
    );
    assert_eq!(f.page().items[0].cleanup_request, Some(request.request_id));
}
#[test]
fn exact_cleanup_replay_returns_original_receipt_and_rejects_changed_identity() {
    let f = Fixture::new();
    let artifact = f.add();
    let request = f.request(&artifact);
    let first = f.clean(&request, 100).unwrap();
    let replay = f.clean(&request, 200).unwrap();
    assert_eq!(
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&replay).unwrap()
    );
    assert!(f.clean(&f.request(&artifact), 201).is_err());
    let mut changed = request.clone();
    changed.expected.name = "changed.md".into();
    assert!(f.clean(&changed, 202).is_err());
}
#[test]
fn active_task_and_project_protection_never_claim_or_delete() {
    let f = Fixture::new();
    let artifact = f.add();
    let request = f.request(&artifact);
    for project in [false, true] {
        let tasks = if project {
            HashSet::new()
        } else {
            HashSet::from([f.task])
        };
        let projects = if project {
            HashSet::from([f.project])
        } else {
            HashSet::new()
        };
        assert!(
            f.store
                .cleanup_artifact(&request, &tasks, &projects, 100)
                .is_err()
        );
        let page = f
            .store
            .list_artifact_retention(
                &ArtifactRetentionQuery {
                    project_id: None,
                    task_id: None,
                    after: None,
                    limit: 2,
                },
                &tasks,
                &projects,
            )
            .unwrap();
        assert_eq!(page.items[0].state, ArtifactRetentionState::Protected);
    }
    assert!(f.path(&artifact).exists());
    assert!(
        claim(&f.store.connect().unwrap(), artifact.id)
            .unwrap()
            .is_none()
    );
}
#[test]
fn active_workflow_protects_project_documents_until_terminal() {
    let f = Fixture::new();
    let artifact = f.add();
    let db = f.store.connect().unwrap();
    f.automation("waitingDecision");
    assert_eq!(f.state(artifact.id), ArtifactRetentionState::Protected);
    assert!(f.clean(&f.request(&artifact), 100).is_err());
    db.execute("UPDATE ion_automation_runs SET status='completed'", [])
        .unwrap();
    assert_eq!(f.state(artifact.id), ArtifactRetentionState::Retained);
    f.clean(&f.request(&artifact), 101).unwrap();
}
#[test]
fn active_team_and_review_protect_original_blob() {
    for (run, work) in [
        ("active", "completed"),
        ("completed", "inReview"),
        ("completed", "uncertain"),
    ] {
        let f = Fixture::new();
        let artifact = f.add();
        let artifact = f.team(&artifact, run, work);
        assert_eq!(f.state(artifact.id), ArtifactRetentionState::Protected);
        assert!(f.clean(&f.request(&artifact), 100).is_err());
        assert_eq!(
            f.store.read_document(artifact.id).unwrap(),
            b"retained original bytes"
        );
    }
}
#[test]
fn terminal_team_blob_cleanup_is_atomic_and_preserves_request_receipt() {
    let f = Fixture::new();
    let artifact = f.add();
    let artifact = f.team(&artifact, "completed", "completed");
    let request = f.request(&artifact);
    f.clean(&request, 100).unwrap();
    let db = f.store.connect().unwrap();
    assert_eq!(
        db.query_row(
            "SELECT length(content) FROM ion_team_document_receipts WHERE artifact_id=?",
            [artifact.id.to_string()],
            |r| r.get::<_, u64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT request_id FROM ion_team_document_receipts WHERE artifact_id=?",
            [artifact.id.to_string()],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        artifact
            .evidence
            .as_ref()
            .unwrap()
            .producer
            .request_id
            .unwrap()
            .to_string()
    );
    assert_eq!(f.state(artifact.id), ArtifactRetentionState::Cleaned);
    assert!(f.store.read_document(artifact.id).is_err());
    f.clean(&request, 101).unwrap();
}
#[test]
fn live_attempt_is_protected_but_completed_attempt_capacity_limit_is_not() {
    let f = Fixture::new();
    let artifact = f.add();
    let artifact = f.team(&artifact, "completed", "completed");
    let db = f.store.connect().unwrap();
    let p = &artifact.evidence.as_ref().unwrap().producer;
    db.execute(
        "INSERT INTO ion_team_attempts VALUES('attempt',?,?,1,4)",
        params![
            p.run_id.unwrap().to_string(),
            p.assignment_id.unwrap().to_string()
        ],
    )
    .unwrap();
    assert_eq!(f.state(artifact.id), ArtifactRetentionState::Protected);
    assert!(f.clean(&f.request(&artifact), 100).is_err());
    db.execute("UPDATE ion_team_attempts SET live=0", [])
        .unwrap();
    assert_eq!(f.state(artifact.id), ArtifactRetentionState::Retained);
    f.clean(&f.request(&artifact), 101).unwrap();
}
#[test]
fn malformed_original_team_references_are_protected() {
    let f = Fixture::new();
    let artifact = f.add();
    let artifact = f.team(&artifact, "completed", "completed");
    let db = f.store.connect().unwrap();
    db.execute("UPDATE ion_team_work SET body='{bad'", [])
        .unwrap();
    assert_eq!(f.state(artifact.id), ArtifactRetentionState::Protected);
    assert!(f.clean(&f.request(&artifact), 100).is_err());
}
#[test]
fn gpu_and_command_recovery_summaries_are_retained() {
    let f = Fixture::new();
    let artifact = f.add();
    let db = f.store.connect().unwrap();
    db.execute_batch("CREATE TABLE ion_team_gpu_results(request_id TEXT,artifact_id TEXT)")
        .unwrap();
    db.execute(
        "INSERT INTO ion_team_gpu_results VALUES('gpu',?)",
        [artifact.id.to_string()],
    )
    .unwrap();
    assert_eq!(f.state(artifact.id), ArtifactRetentionState::Protected);
    assert!(f.clean(&f.request(&artifact), 100).is_err());
    let f = Fixture::new();
    let artifact = f.add();
    let artifact = f.team(&artifact, "completed", "completed");
    let db = f.store.connect().unwrap();
    let p = &artifact.evidence.as_ref().unwrap().producer;
    db.execute_batch("CREATE TABLE ion_team_jobs(request_id TEXT,work_id TEXT,run_id TEXT)")
        .unwrap();
    db.execute(
        "INSERT INTO ion_team_jobs VALUES(?,?,?)",
        params![
            p.request_id.unwrap().to_string(),
            p.assignment_id.unwrap().to_string(),
            p.run_id.unwrap().to_string()
        ],
    )
    .unwrap();
    assert_eq!(f.state(artifact.id), ArtifactRetentionState::Protected);
    assert!(f.clean(&f.request(&artifact), 100).is_err());
}
#[test]
fn pending_cleanup_survives_restart_and_blocks_reads_until_exact_retry() {
    let f = Fixture::new();
    let artifact = f.add();
    let request = f.request(&artifact);
    f.pending(&request);
    let reopened = ControlStore::new(f.store.path());
    assert!(reopened.read_document(artifact.id).is_err());
    assert_eq!(f.state(artifact.id), ArtifactRetentionState::CleanupPending);
    assert_eq!(f.page().items[0].cleanup_request, Some(request.request_id));
    assert!(f.clean(&f.request(&artifact), 100).is_err());
    reopened
        .cleanup_artifact(&request, &HashSet::new(), &HashSet::new(), 101)
        .unwrap();
    assert!(!f.path(&artifact).exists());
    assert_eq!(f.state(artifact.id), ArtifactRetentionState::Cleaned);
}
#[test]
fn missing_private_copy_requires_durable_pending_claim() {
    let f = Fixture::new();
    let artifact = f.add();
    let request = f.request(&artifact);
    std::fs::remove_file(f.path(&artifact)).unwrap();
    assert!(f.clean(&request, 100).is_err());
    assert!(
        claim(&f.store.connect().unwrap(), artifact.id)
            .unwrap()
            .is_none()
    );
    f.pending(&request);
    f.clean(&request, 101).unwrap();
    assert_eq!(f.state(artifact.id), ArtifactRetentionState::Cleaned);
}
#[test]
fn changed_metadata_or_same_size_content_never_creates_cleanup_claim() {
    let f = Fixture::new();
    let artifact = f.add();
    let mut request = f.request(&artifact);
    request.expected.evidence.as_mut().unwrap().sha256 = "0".repeat(64);
    assert!(f.clean(&request, 100).is_err());
    std::fs::write(f.path(&artifact), b"retained changed! bytes").unwrap();
    assert!(f.clean(&f.request(&artifact), 101).is_err());
    assert!(f.path(&artifact).exists());
    assert!(
        claim(&f.store.connect().unwrap(), artifact.id)
            .unwrap()
            .is_none()
    );
}
#[test]
fn historical_unverified_document_cannot_be_cleaned() {
    let f = Fixture::new();
    let mut artifact = f.add();
    artifact.evidence = None;
    let db = f.store.connect().unwrap();
    db.execute(
        "UPDATE ion_control_records SET body=? WHERE id=?",
        params![
            serde_json::to_string(&ControlRecord::Artifact(artifact.clone())).unwrap(),
            artifact.id.to_string()
        ],
    )
    .unwrap();
    assert_eq!(f.state(artifact.id), ArtifactRetentionState::Protected);
    assert!(f.clean(&f.request(&artifact), 100).is_err());
    assert!(f.path(&artifact).exists());
}
#[test]
fn storage_pages_are_bounded_ordered_and_filter_scoped() {
    let f = Fixture::new();
    for _ in 0..5 {
        f.add();
    }
    let mut query = ArtifactRetentionQuery {
        project_id: Some(f.project),
        task_id: Some(f.task),
        after: None,
        limit: 2,
    };
    let first = f
        .store
        .list_artifact_retention(&query, &HashSet::new(), &HashSet::new())
        .unwrap();
    assert_eq!(first.items.len(), 2);
    query.after = first.next;
    let second = f
        .store
        .list_artifact_retention(&query, &HashSet::new(), &HashSet::new())
        .unwrap();
    assert_eq!(second.items.len(), 2);
    assert!(
        second
            .items
            .iter()
            .all(|b| first.items.iter().all(|a| a.artifact.id != b.artifact.id))
    );
    query.task_id = Some(Uuid::new_v4());
    assert!(
        f.store
            .list_artifact_retention(&query, &HashSet::new(), &HashSet::new())
            .is_err()
    );
    query.limit = 33;
    assert!(
        f.store
            .list_artifact_retention(&query, &HashSet::new(), &HashSet::new())
            .is_err()
    );
}
#[test]
fn managed_directory_cannot_redirect_cleanup_to_foreign_content() {
    let f = Fixture::new();
    let artifact = f.add();
    let managed = f.directory.join("ion-documents");
    let foreign = f.directory.join("foreign-project");
    std::fs::rename(&managed, &foreign).unwrap();
    #[cfg(windows)]
    {
        let output = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&managed)
            .arg(&foreign)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Could not construct owned junction fixture: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&foreign, &managed).unwrap();
    assert!(f.clean(&f.request(&artifact), 100).is_err());
    assert_eq!(
        std::fs::read(foreign.join(artifact.id.to_string())).unwrap(),
        b"retained original bytes"
    );
    assert!(
        claim(&f.store.connect().unwrap(), artifact.id)
            .unwrap()
            .is_none()
    );
    #[cfg(windows)]
    std::fs::remove_dir(&managed).unwrap();
    #[cfg(unix)]
    std::fs::remove_file(&managed).unwrap();
}

#[test]
fn reused_owner_request_never_cleans_another_artifact() {
    let f = Fixture::new();
    let a = f.add();
    let b = f.add();
    let mut request = f.request(&a);
    f.clean(&request, 100).unwrap();
    request.expected = b.clone();
    assert!(f.clean(&request, 101).is_err());
    assert_eq!(
        f.store.read_document(b.id).unwrap(),
        b"retained original bytes"
    );
    assert!(claim(&f.store.connect().unwrap(), b.id).unwrap().is_none());
}
#[test]
fn pending_retry_rechecks_current_task_protection() {
    let f = Fixture::new();
    let artifact = f.add();
    let request = f.request(&artifact);
    f.pending(&request);
    assert!(
        f.store
            .cleanup_artifact(&request, &HashSet::from([f.task]), &HashSet::new(), 100)
            .is_err()
    );
    assert!(f.path(&artifact).exists());
    assert_eq!(f.state(artifact.id), ArtifactRetentionState::CleanupPending);
    f.clean(&request, 101).unwrap();
    assert_eq!(f.state(artifact.id), ArtifactRetentionState::Cleaned);
}
#[test]
fn damaged_team_content_rolls_back_tombstone_and_receipt_zeroing() {
    let f = Fixture::new();
    let artifact = f.add();
    let artifact = f.team(&artifact, "completed", "completed");
    let db = f.store.connect().unwrap();
    db.execute(
        "UPDATE ion_team_document_receipts SET content=?",
        [b"retained changed! bytes".as_slice()],
    )
    .unwrap();
    assert!(f.clean(&f.request(&artifact), 100).is_err());
    assert!(claim(&db, artifact.id).unwrap().is_none());
    assert_eq!(
        db.query_row(
            "SELECT length(content) FROM ion_team_document_receipts",
            [],
            |r| r.get::<_, u64>(0)
        )
        .unwrap(),
        23
    );
}
#[test]
fn pending_claim_can_finish_if_its_entire_private_directory_is_already_absent() {
    let f = Fixture::new();
    let artifact = f.add();
    let request = f.request(&artifact);
    f.pending(&request);
    std::fs::remove_file(f.path(&artifact)).unwrap();
    std::fs::remove_dir(f.directory.join("ion-documents")).unwrap();
    f.clean(&request, 100).unwrap();
    assert_eq!(f.state(artifact.id), ArtifactRetentionState::Cleaned);
}

#[test]
fn native_json_automation_statuses_release_only_terminal_project_protection() {
    for status in [
        AutomationStatus::Running,
        AutomationStatus::WaitingDecision,
        AutomationStatus::Blocked,
        AutomationStatus::Completed,
        AutomationStatus::Canceled,
        AutomationStatus::Skipped,
    ] {
        let f = Fixture::new();
        let artifact = f.add();
        // This is the exact status encoding used by teams::automation::write.
        let encoded = serde_json::to_string(&status).unwrap();
        assert!(encoded.starts_with('"') && encoded.ends_with('"'));
        f.automation(&encoded);
        let terminal = matches!(
            status,
            AutomationStatus::Completed | AutomationStatus::Canceled | AutomationStatus::Skipped
        );
        assert_eq!(
            f.state(artifact.id),
            if terminal {
                ArtifactRetentionState::Retained
            } else {
                ArtifactRetentionState::Protected
            }
        );
        assert_eq!(f.clean(&f.request(&artifact), 100).is_ok(), terminal);
        assert_eq!(f.path(&artifact).exists(), !terminal);
    }
}
#[test]
fn historical_plain_terminal_automation_statuses_still_allow_cleanup() {
    for status in ["completed", "canceled", "skipped"] {
        let f = Fixture::new();
        let artifact = f.add();
        f.automation(status);
        assert_eq!(f.state(artifact.id), ArtifactRetentionState::Retained);
        f.clean(&f.request(&artifact), 100).unwrap();
    }
}
#[test]
fn malformed_or_nonstring_automation_statuses_fail_closed() {
    let f = Fixture::new();
    let artifact = f.add();
    let db = f.store.connect().unwrap();
    f.automation("running");
    // Native status is NOT NULL. Verify damaged historical SQL-null state
    // separately without weakening or replacing the production schema.
    let legacy = Connection::open_in_memory().unwrap();
    legacy
        .execute_batch(
            "CREATE TABLE ion_automation_runs(id TEXT PRIMARY KEY,project_id TEXT,status TEXT)",
        )
        .unwrap();
    legacy
        .execute(
            "INSERT INTO ion_automation_runs VALUES('workflow',?,NULL)",
            [f.project.to_string()],
        )
        .unwrap();
    assert!(
        protected(&legacy, &artifact, &HashSet::new(), &HashSet::new())
            .unwrap()
            .is_some()
    );
    for status in [
        "null",
        "42",
        "true",
        "{}",
        "[]",
        "\"unknown\"",
        "\"completed",
        "\"completed\"junk",
        "COMPLETED",
        "",
        " completed ",
    ] {
        db.execute("UPDATE ion_automation_runs SET status=?", [status])
            .unwrap();
        assert_eq!(
            f.state(artifact.id),
            ArtifactRetentionState::Protected,
            "Unexpected acceptance for {status:?}"
        );
        assert!(f.clean(&f.request(&artifact), 100).is_err());
        assert!(f.path(&artifact).exists());
    }
    assert!(claim(&db, artifact.id).unwrap().is_none());
}

#[test]
fn legacy_publications_missing_from_ai_library_are_listed_as_protected() {
    let f = Fixture::new();
    let mut artifact = f.add();
    artifact.evidence = None;
    let db = f.store.connect().unwrap();
    db.execute(
        "UPDATE ion_control_records SET body=? WHERE id=?",
        params![
            serde_json::to_string(&ControlRecord::Artifact(artifact.clone())).unwrap(),
            artifact.id.to_string()
        ],
    )
    .unwrap();
    // A historical index gap cannot hide retained storage from the owner.
    db.execute(
        "DELETE FROM ion_artifact_library WHERE id=?",
        [artifact.id.to_string()],
    )
    .unwrap();
    let query = ArtifactRetentionQuery {
        project_id: Some(f.project),
        task_id: Some(f.task),
        after: None,
        limit: 2,
    };
    let page = f
        .store
        .list_artifact_retention(&query, &HashSet::new(), &HashSet::new())
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].artifact.id, artifact.id);
    assert_eq!(page.items[0].state, ArtifactRetentionState::Protected);
    assert!(
        page.items[0]
            .reason
            .as_ref()
            .unwrap()
            .contains("lacks verified")
    );
    assert!(f.clean(&f.request(&artifact), 100).is_err());
    assert!(f.path(&artifact).exists());
}
#[test]
fn invalid_publication_json_reports_error_even_when_filters_would_hide_it() {
    let f = Fixture::new();
    let artifact = f.add();
    let db = f.store.connect().unwrap();
    // Only this isolated fixture disables its UPDATE triggers to simulate
    // preexisting historical corruption. Production JSON validation stays intact.
    let update_triggers = db.prepare("SELECT name FROM sqlite_master WHERE type='trigger' AND tbl_name='ion_control_records' AND (upper(sql) LIKE '%AFTER UPDATE%' OR upper(sql) LIKE '%BEFORE UPDATE%' OR upper(sql) LIKE '%INSTEAD OF UPDATE%')")
        .unwrap().query_map([],|row|row.get::<_,String>(0)).unwrap()
        .collect::<std::result::Result<Vec<_>,_>>().unwrap();
    assert!(!update_triggers.is_empty());
    for name in update_triggers {
        db.execute_batch(&format!("DROP TRIGGER \"{}\"", name.replace('\"', "\"\"")))
            .unwrap();
    }
    db.execute(
        "UPDATE ion_control_records SET body='{broken' WHERE id=?",
        [artifact.id.to_string()],
    )
    .unwrap();
    let query = ArtifactRetentionQuery {
        project_id: Some(Uuid::new_v4()),
        task_id: None,
        after: None,
        limit: 2,
    };
    let error = f
        .store
        .list_artifact_retention(&query, &HashSet::new(), &HashSet::new())
        .unwrap_err();
    let message = format!("{error:#}").to_ascii_lowercase();
    assert!(
        message.contains("invalid json") || message.contains("malformed json"),
        "{error:#}"
    );
    assert!(f.path(&artifact).exists());
}

fn source_copy_fixture(f: &Fixture, artifact: Uuid) {
    let db = f.store.connect().unwrap();
    db.execute_batch(
        "CREATE TABLE ion_team_html_sources(artifact_id TEXT,path TEXT,content BLOB);",
    )
    .unwrap();
    for (id, path) in [
        (artifact, "index.html"),
        (artifact, "sprite.svg"),
        (Uuid::new_v4(), "unrelated.svg"),
    ] {
        db.execute(
            "INSERT INTO ion_team_html_sources VALUES(?,?,?)",
            params![id.to_string(), path, b"retained pinned source".as_slice()],
        )
        .unwrap();
    }
}
#[test]
fn html_source_cleanup_keeps_active_protection_and_releases_only_parent_copies() {
    for run in ["active", "completed"] {
        let f = Fixture::new();
        let artifact = f.add();
        let artifact = f.team(&artifact, run, "completed");
        source_copy_fixture(&f, artifact.id);
        let db = f.store.connect().unwrap();
        let request = f.request(&artifact);
        if run == "active" {
            assert!(f.clean(&request, 100).is_err());
            assert!(
                db.query_row(
                    "SELECT sum(length(content)) FROM ion_team_html_sources WHERE artifact_id=?",
                    [artifact.id.to_string()],
                    |r| r.get::<_, u64>(0)
                )
                .unwrap()
                    > 0
            );
        } else {
            let receipt = f.clean(&request, 100).unwrap();
            assert_eq!(f.clean(&request, 101).unwrap().bytes, receipt.bytes);
            assert_eq!(
                db.query_row(
                    "SELECT sum(length(content)) FROM ion_team_html_sources WHERE artifact_id=?",
                    [artifact.id.to_string()],
                    |r| r.get::<_, u64>(0)
                )
                .unwrap(),
                0
            );
            assert_eq!(
                db.query_row(
                    "SELECT count(*) FROM ion_team_html_sources WHERE artifact_id=?",
                    [artifact.id.to_string()],
                    |r| r.get::<_, u32>(0)
                )
                .unwrap(),
                2
            );
            assert!(f.store.read_document(artifact.id).is_err());
        }
        assert!(
            db.query_row(
                "SELECT length(content) FROM ion_team_html_sources WHERE path='unrelated.svg'",
                [],
                |r| r.get::<_, u64>(0)
            )
            .unwrap()
                > 0
        );
    }
}
#[test]
fn html_source_cleanup_failure_rolls_back_original_bytes_and_cleanup_receipt() {
    let f = Fixture::new();
    let artifact = f.add();
    let artifact = f.team(&artifact, "completed", "completed");
    source_copy_fixture(&f, artifact.id);
    let db = f.store.connect().unwrap();
    db.execute_batch("CREATE TRIGGER refuse_html_cleanup BEFORE UPDATE ON ion_team_html_sources BEGIN SELECT RAISE(ABORT,'fixture cleanup failure');END;").unwrap();
    let request = f.request(&artifact);
    assert!(f.clean(&request, 100).is_err());
    assert_eq!(
        f.store.read_document(artifact.id).unwrap(),
        b"retained original bytes"
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM ion_artifact_cleanup", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert!(
        db.query_row(
            "SELECT sum(length(content)) FROM ion_team_html_sources WHERE artifact_id=?",
            [artifact.id.to_string()],
            |r| r.get::<_, u64>(0)
        )
        .unwrap()
            > 0
    );
    db.execute_batch("DROP TRIGGER refuse_html_cleanup")
        .unwrap();
    f.clean(&request, 101).unwrap();
    assert!(f.store.read_document(artifact.id).is_err());
}
