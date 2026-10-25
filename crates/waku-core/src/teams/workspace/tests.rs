use super::*;
use waku_protocol::model::ProviderKind;
struct Fixture {
    dir: PathBuf,
    source: PathBuf,
    store: TeamCoordinator,
    run: TeamRunSnapshot,
    agents: [Uuid; 3],
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("ion-team-workspace-test-{}", Uuid::new_v4()));
        let source = dir.join("project");
        fs::create_dir_all(&source).unwrap();
        git(&source, &["init", "-b", "main"]).unwrap();
        git(&source, &["config", "core.autocrlf", "false"]).unwrap();
        fs::write(source.join("main.rs"), "fn value() -> i32 { 1 }\n").unwrap();
        git(&source, &["add", "main.rs"]).unwrap();
        git(
            &source,
            &[
                "-c",
                "user.name=Ion Tests",
                "-c",
                "user.email=ion@example.test",
                "commit",
                "-m",
                "baseline",
            ],
        )
        .unwrap();
        let definitions = crate::control::ControlStore::new(dir.join("control.db"));
        let agents = std::array::from_fn(|_| Uuid::new_v4());
        let access = AgentAccess {
            workspace_write: true,
            ..Default::default()
        };
        for id in agents {
            definitions
                .save(
                    definitions.load().unwrap().revision,
                    ControlRecord::Agent(AgentProfile {
                        id,
                        name: id.to_string(),
                        provider: ProviderKind::Nebius,
                        model: Some("fixture-model".into()),
                        expertise: "Coding".into(),
                        instructions: "Make reviewed changes".into(),
                        expected_output: "Change and evidence".into(),
                        execution: Default::default(),
                        access: access.clone(),
                        max_parallel_tasks: 4,
                    }),
                )
                .unwrap();
        }
        let team = Team {
            id: Uuid::new_v4(),
            name: "Engineering".into(),
            objective: "Reviewed code".into(),
            limits: None,
            members: vec![
                TeamMember {
                    agent_id: agents[0],
                    parent_id: None,
                },
                TeamMember {
                    agent_id: agents[1],
                    parent_id: Some(agents[0]),
                },
                TeamMember {
                    agent_id: agents[2],
                    parent_id: Some(agents[0]),
                },
            ],
        };
        definitions
            .save(
                definitions.load().unwrap().revision,
                ControlRecord::Team(team.clone()),
            )
            .unwrap();
        let store = TeamCoordinator::new(definitions.path());
        let run = store
            .prepare(
                PrepareRun {
                    account_scope: None,
                    id: Uuid::new_v4(),
                    root_task_id: Uuid::new_v4(),
                    project_id: Uuid::new_v4(),
                    team_id: team.id,
                    expected_definition_revision: definitions.load().unwrap().revision,
                    objective: "Change value to 2".into(),
                    limits: Default::default(),
                    access,
                    input_data: vec![],
                    source_attachments: vec![],
                    source_prompt: None,
                    origin: None,
                },
                crate::model::unix_time(),
            )
            .unwrap();
        Self {
            dir,
            source,
            store,
            run,
            agents,
        }
    }
    fn claim(&self, work: Uuid) -> ClaimedAssignment {
        for p in self.store.pending_materializations(128).unwrap() {
            self.store
                .acknowledge_materialized(
                    p.run_id,
                    p.assignment.id,
                    p.assignment.task_id,
                    crate::model::unix_time(),
                )
                .unwrap();
        }
        self.store
            .claim(
                self.run.id,
                work,
                DispatchHost::Local,
                crate::model::unix_time(),
                180,
            )
            .unwrap()
    }
    fn workspace(&self) -> PathBuf {
        self.store
            .prepare_local_workspace(self.run.id, &self.source, crate::model::unix_time())
            .unwrap()
    }
    fn child(&self, lease: &AssignmentLease, agent: usize) -> TeamAssignment {
        self.store
            .delegate(
                lease,
                Delegation {
                    request_id: Uuid::new_v4(),
                    agent_id: self.agents[agent],
                    objective: "Implement code".into(),
                    expected_output: "Changes".into(),
                    dependencies: vec![],
                    access: AgentAccess {
                        workspace_write: true,
                        ..Default::default()
                    },
                },
                crate::model::unix_time(),
            )
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert!(
            self.dir.starts_with(std::env::temp_dir())
                && self
                    .dir
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("ion-team-workspace-test-")
        );
        // Disposable fixture is verified to stay in its exact test directory.
        let _ = fs::remove_dir_all(&self.dir);
    }
}
#[test]
fn team_workspace_pins_baseline_and_reuses_edits_across_restart() {
    let f = Fixture::new();
    let workspace = f.workspace();
    let root = f.claim(f.run.work[0].id);
    let request = Uuid::new_v4();
    let now = crate::model::unix_time();
    let old = fs::read_to_string(workspace.join("main.rs")).unwrap();
    let receipt = f
        .store
        .edit_workspace_file(
            &root.lease,
            request,
            "main.rs",
            Some(&old),
            "fn value() -> i32 { 2 }\n",
            now,
        )
        .unwrap();
    assert_eq!(fs::read_to_string(f.source.join("main.rs")).unwrap(), old);
    assert_eq!(
        f.store
            .edit_workspace_file(
                &root.lease,
                request,
                "main.rs",
                Some(&old),
                "fn value() -> i32 { 2 }\n",
                now
            )
            .unwrap(),
        receipt
    );
    assert!(
        f.store
            .edit_workspace_file(&root.lease, request, "main.rs", Some(&old), "changed", now)
            .is_err()
    );
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    assert_eq!(
        reopened
            .prepare_local_workspace(f.run.id, &f.source, now)
            .unwrap(),
        workspace
    );
    fs::write(f.source.join("main.rs"), "User edit after team start").unwrap();
    assert_eq!(f.workspace(), workspace);
    let doc = f
        .store
        .publish_workspace_change(&root.lease, Uuid::new_v4(), "main.rs", now)
        .unwrap();
    let content = f
        .store
        .read_assignment_document(&root.lease, doc.id, now)
        .unwrap();
    let content: serde_json::Value = serde_json::from_slice(&content).unwrap();
    assert_eq!(content["before"], old);
    assert_eq!(content["after"], "fn value() -> i32 { 2 }\n");
    assert_eq!(content["source_checkout_changed"], false);
    assert_eq!(content["edit_ids"].as_array().unwrap().len(), 1);
    let snapshot = reopened.snapshot(f.run.id).unwrap();
    assert!(snapshot.workspace.unwrap().base_commit.len() >= 40);
}
#[test]
fn team_workspace_refuses_dirty_or_redirected_baselines() {
    let f = Fixture::new();
    fs::write(f.source.join("draft.txt"), "Do not lose this").unwrap();
    let err = f
        .store
        .prepare_local_workspace(f.run.id, &f.source, crate::model::unix_time())
        .unwrap_err()
        .to_string();
    assert!(err.contains("Commit or stash"));
    assert!(f.store.snapshot(f.run.id).unwrap().workspace.is_none());
    fs::remove_file(f.source.join("draft.txt")).unwrap();
    let workspace = f.workspace();
    git(&workspace, &["checkout", "-b", "different"]).unwrap();
    assert!(
        f.store
            .prepare_local_workspace(f.run.id, &f.source, crate::model::unix_time())
            .is_err()
    );
}
#[test]
fn team_workspace_serializes_writers_and_releases_when_parent_yields() {
    let f = Fixture::new();
    f.workspace();
    let root = f.claim(f.run.work[0].id);
    let a = f.child(&root.lease, 1);
    let b = f.child(&root.lease, 2);
    for p in f.store.pending_materializations(128).unwrap() {
        f.store
            .acknowledge_materialized(
                p.run_id,
                p.assignment.id,
                p.assignment.task_id,
                crate::model::unix_time(),
            )
            .unwrap();
    }
    assert!(
        f.store
            .claim(
                f.run.id,
                a.id,
                DispatchHost::Local,
                crate::model::unix_time(),
                180
            )
            .is_err()
    );
    f.store
        .suspend_for_children(&root.lease, &[a.id, b.id], "{}", crate::model::unix_time())
        .unwrap();
    let first = f.claim(a.id);
    assert!(
        f.store
            .claim(
                f.run.id,
                b.id,
                DispatchHost::Local,
                crate::model::unix_time(),
                180
            )
            .is_err()
    );
    f.store
        .report(
            &first.lease,
            AssignmentOutcome::Blocked("Known setup failure".into()),
            crate::model::unix_time(),
        )
        .unwrap();
    let second = f.claim(b.id);
    assert_eq!(second.assignment.id, b.id);
    f.store
        .report(
            &second.lease,
            AssignmentOutcome::Uncertain("Host outcome unknown".into()),
            crate::model::unix_time(),
        )
        .unwrap();
    assert!(
        f.store
            .resume_after_children(f.run.id, root.assignment.id, crate::model::unix_time(), 180)
            .is_err()
    );
}
#[test]
fn team_workspace_denies_stale_private_link_and_conflicting_writes() {
    let f = Fixture::new();
    let workspace = f.workspace();
    let root = f.claim(f.run.work[0].id);
    let now = crate::model::unix_time();
    for path in [
        "../escape",
        ".git/config",
        ".env",
        ".env.test",
        "credentials.json",
        "main.rs:stream",
        "main.rs.",
        "NUL",
        "nested/../../escape",
    ] {
        assert!(
            f.store
                .edit_workspace_file(&root.lease, Uuid::new_v4(), path, None, "x", now)
                .is_err(),
            "{path}"
        );
    }
    assert!(
        f.store
            .edit_workspace_file(
                &root.lease,
                Uuid::new_v4(),
                "main.rs",
                Some("wrong"),
                "x",
                now
            )
            .is_err()
    );
    assert!(!f.store.workspace_has_pending_effect(f.run.id).unwrap());
    let r = f
        .store
        .edit_workspace_file(
            &root.lease,
            Uuid::new_v4(),
            "new.rs",
            None,
            "fn new() {}\n",
            now,
        )
        .unwrap();
    assert!(r.contains("applied"));
    assert!(
        f.store
            .edit_workspace_file(&root.lease, Uuid::new_v4(), "new.rs", None, "bad", now)
            .is_err()
    );
    f.store
        .stop(f.run.id, f.store.snapshot(f.run.id).unwrap().revision, now)
        .unwrap();
    assert!(
        f.store
            .edit_workspace_file(
                &root.lease,
                Uuid::new_v4(),
                "new.rs",
                Some("fn new() {}\n"),
                "x",
                now
            )
            .is_err()
    );
    assert_eq!(
        fs::read_to_string(workspace.join("new.rs")).unwrap(),
        "fn new() {}\n"
    );
}

#[test]
fn team_workspace_coding_adapter_completes_delegation_edit_handoff_and_review() {
    use crate::driver::{NativeTeamExit, NativeTeamStart};
    use serde_json::{Value, json};
    let f = Fixture::new();
    let workspace = f.workspace();
    let root = f.claim(f.run.work[0].id);
    let (events, _) = crate::driver::test_event_channel();
    let invoke = |start: NativeTeamStart, mut action: Box<dyn FnMut(&[Value], Value) -> Value>| {
        crate::driver::run_scripted_team_assignment(&f.store,start,&workspace,&events,&AtomicBool::new(false),|_,_,messages,tools|{
            let call=action(messages,tools);let chunk=json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":format!("coding-{}",messages.len()),"function":{"name":call["name"],"arguments":call["args"].to_string()}}]}}]});
            Ok(format!("data: {chunk}\n\ndata: [DONE]\n\n"))
        }).unwrap()
    };
    let specialist = f.agents[1];
    let mut step = 0;
    assert_eq!(
        invoke(
            NativeTeamStart::Fresh(root),
            Box::new(move |messages, tools| {
                assert!(
                    tools
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|t| t["function"]["name"] == "write_file")
                );
                step += 1;
                match step {
                    1 => {
                        json!({"name":"delegate","args":{"agent_id":specialist,"objective":"Change main.rs value to 2","expected_output":"Exact change document"}})
                    }
                    _ => {
                        let receipt: Value = serde_json::from_str(
                            messages.last().unwrap()["content"].as_str().unwrap(),
                        )
                        .unwrap();
                        json!({"name":"await_children","args":{"assignment_ids":[receipt["assignment_id"]]}})
                    }
                }
            })
        ),
        NativeTeamExit::Yielded
    );
    let run = f.store.snapshot(f.run.id).unwrap();
    let child = run.work.iter().find(|w| w.parent_id.is_some()).unwrap();
    let child_id = child.id;
    let claim = f.claim(child_id);
    let mut step = 0;
    assert_eq!(
        invoke(
            NativeTeamStart::Fresh(claim),
            Box::new(move |messages, _| {
                step += 1;
                match step {
                    1 => {
                        json!({"name":"write_file","args":{"path":"main.rs","old_content":"fn value() -> i32 { 1 }\n","new_content":"fn value() -> i32 { 2 }\n"}})
                    }
                    2 => json!({"name":"publish_change","args":{"path":"main.rs"}}),
                    _ => {
                        let receipt: Value = serde_json::from_str(
                            messages.last().unwrap()["content"].as_str().unwrap(),
                        )
                        .unwrap();
                        json!({"name":"report_result","args":{"summary":"Implemented value 2 in the isolated workspace","artifact_ids":[receipt["artifact_id"]]}})
                    }
                }
            })
        ),
        NativeTeamExit::Settled
    );
    let run = f.store.snapshot(f.run.id).unwrap();
    let child = run.work.iter().find(|w| w.id == child_id).unwrap();
    assert_eq!(child.status, AssignmentStatus::InReview);
    let artifact = child.result.as_ref().unwrap().artifact_ids[0];
    let resumed = f
        .store
        .resume_after_children(f.run.id, f.run.work[0].id, crate::model::unix_time(), 180)
        .unwrap();
    let mut step = 0;
    assert_eq!(
        invoke(
            NativeTeamStart::Resumed(resumed),
            Box::new(move |_, _| {
                step += 1;
                match step {
                    1 => json!({"name":"read_document","args":{"artifact_id":artifact}}),
                    2 => {
                        json!({"name":"review_child","args":{"assignment_id":child_id,"verdict":"accepted","feedback":"Read exact before/after document and verified requested value 2","artifact_ids":[artifact]}})
                    }
                    _ => {
                        json!({"name":"report_result","args":{"summary":"Reviewed coding change; isolated branch is ready for user integration","artifact_ids":[artifact]}})
                    }
                }
            })
        ),
        NativeTeamExit::Settled
    );
    assert_eq!(
        f.store.snapshot(f.run.id).unwrap().status,
        TeamRunStatus::Completed
    );
    assert_eq!(
        fs::read_to_string(workspace.join("main.rs")).unwrap(),
        "fn value() -> i32 { 2 }\n"
    );
    assert_eq!(
        fs::read_to_string(f.source.join("main.rs")).unwrap(),
        "fn value() -> i32 { 1 }\n"
    );
}

#[test]
fn team_workspace_pending_edit_survives_restart_and_fences_effects_and_completion() {
    let f = Fixture::new();
    let workspace = f.workspace();
    let root = f.claim(f.run.work[0].id);
    let child = f.child(&root.lease, 1);
    let now = crate::model::unix_time();
    let request = Uuid::new_v4();
    let signature=serde_json::json!({"path":"main.rs","old":"fn value() -> i32 { 1 }\n","new":"interrupted edit"}).to_string();
    f.store.connect().unwrap().execute("INSERT INTO ion_team_workspace_edits(id,run_id,work_id,attempt_id,request_id,signature,state) VALUES(?,?,?,?,?,?,'pending')",params![Uuid::new_v4().to_string(),f.run.id.to_string(),root.assignment.id.to_string(),root.lease.attempt_id().to_string(),request.to_string(),signature]).unwrap();
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    assert!(reopened.workspace_has_pending_effect(f.run.id).unwrap());
    assert!(
        reopened
            .edit_workspace_file(
                &root.lease,
                request,
                "main.rs",
                Some("fn value() -> i32 { 1 }\n"),
                "interrupted edit",
                now
            )
            .is_err()
    );
    assert!(
        reopened
            .edit_workspace_file(
                &root.lease,
                Uuid::new_v4(),
                "other.txt",
                None,
                "do not execute",
                now
            )
            .is_err()
    );
    assert!(
        reopened
            .suspend_for_children(&root.lease, &[child.id], "{}", now)
            .is_err()
    );
    assert!(
        reopened
            .suspend_for_decision(
                &root.lease,
                Uuid::new_v4(),
                "Review",
                "Exact proposal",
                "{}",
                now
            )
            .is_err()
    );
    assert!(
        reopened
            .reserve_model_call(
                &root.lease,
                1,
                ModelCallAllowance {
                    model: "fixture-model".into(),
                    max_input_tokens: 100,
                    max_output_tokens: 100
                },
                now
            )
            .is_err()
    );
    let doc = reopened
        .publish_document(
            &root.lease,
            Uuid::new_v4(),
            "Incomplete.md",
            "md",
            "Work is not verified",
            now,
        )
        .unwrap();
    assert!(
        reopened
            .report(
                &root.lease,
                AssignmentOutcome::Completed(AssignmentResult {
                    executed_checks: vec![],
                    summary: "Do not accept".into(),
                    artifact_ids: vec![doc.id],
                    checks: vec![]
                }),
                now
            )
            .is_err()
    );
    assert_eq!(
        fs::read_to_string(workspace.join("main.rs")).unwrap(),
        "fn value() -> i32 { 1 }\n"
    );
    assert!(!workspace.join("other.txt").exists());
}

#[test]
fn team_workspace_structural_edits_preserve_exact_receipts_backups_and_change_documents() {
    let f = Fixture::new();
    let workspace = f.workspace();
    let root = f.claim(f.run.work[0].id);
    let now = crate::model::unix_time();
    let request = Uuid::new_v4();
    let receipt = f
        .store
        .create_workspace_directory(&root.lease, request, "src", now)
        .unwrap();
    assert!(workspace.join("src").is_dir());
    assert!(!f.source.join("src").exists());
    assert_eq!(
        f.store
            .create_workspace_directory(&root.lease, request, "src", now)
            .unwrap(),
        receipt
    );
    assert!(
        f.store
            .create_workspace_directory(&root.lease, request, "other", now)
            .is_err()
    );
    assert!(
        f.store
            .create_workspace_directory(&root.lease, Uuid::new_v4(), "src", now)
            .is_err()
    );
    f.store
        .edit_workspace_file(
            &root.lease,
            Uuid::new_v4(),
            "src/lib.rs",
            None,
            "pub fn answer() -> i32 { 42 }\n",
            now,
        )
        .unwrap();
    let old = fs::read_to_string(workspace.join("main.rs")).unwrap();
    let deletion = Uuid::new_v4();
    let deleted = f
        .store
        .delete_workspace_file(&root.lease, deletion, "main.rs", &old, now)
        .unwrap();
    assert!(!workspace.join("main.rs").exists());
    assert_eq!(fs::read_to_string(f.source.join("main.rs")).unwrap(), old);
    assert_eq!(
        f.store
            .delete_workspace_file(&root.lease, deletion, "main.rs", &old, now)
            .unwrap(),
        deleted
    );
    let value: serde_json::Value = serde_json::from_str(&deleted).unwrap();
    assert_eq!(value["backup"], "durable_edit_receipt");
    let edit_signature: String = f
        .store
        .connect()
        .unwrap()
        .query_row(
            "SELECT signature FROM ion_team_workspace_edits WHERE id=?",
            [value["edit_id"].as_str().unwrap()],
            |r| r.get(0),
        )
        .unwrap();
    let edit_signature: serde_json::Value = serde_json::from_str(&edit_signature).unwrap();
    assert_eq!(edit_signature["old"], old);
    assert!(
        !git(&workspace, &["status", "--porcelain"])
            .unwrap()
            .contains(".ion-edit-")
    );
    let doc = f
        .store
        .publish_workspace_change(&root.lease, Uuid::new_v4(), "main.rs", now)
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(
        &f.store
            .read_assignment_document(&root.lease, doc.id, now)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["before"], old);
    assert!(body["after"].is_null());
    assert_eq!(body["edit_ids"].as_array().unwrap().len(), 1);
    assert!(
        f.store
            .publish_workspace_change(&root.lease, Uuid::new_v4(), "src", now)
            .is_err()
    );
    f.store
        .edit_workspace_file(
            &root.lease,
            Uuid::new_v4(),
            "main.rs",
            None,
            "// Recreated deliberately\n",
            now,
        )
        .unwrap();
    let doc = f
        .store
        .publish_workspace_change(&root.lease, Uuid::new_v4(), "main.rs", now)
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(
        &f.store
            .read_assignment_document(&root.lease, doc.id, now)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["before"], old);
    assert_eq!(body["after"], "// Recreated deliberately\n");
    assert_eq!(body["edit_ids"].as_array().unwrap().len(), 2);
    assert!(!f.store.workspace_has_pending_effect(f.run.id).unwrap());
}
#[test]
fn team_workspace_structural_edits_reject_scope_private_paths_conflicts_and_recursive_deletion() {
    let f = Fixture::new();
    let workspace = f.workspace();
    let root = f.claim(f.run.work[0].id);
    let now = crate::model::unix_time();
    for path in [
        "../outside",
        ".git",
        ".aws",
        ".ion-edit-test",
        ".env/config",
        "NUL",
        "folder/missing",
    ] {
        assert!(
            f.store
                .create_workspace_directory(&root.lease, Uuid::new_v4(), path, now)
                .is_err(),
            "{path}"
        );
    }
    assert!(
        f.store
            .create_workspace_directory(&root.lease, Uuid::nil(), "src", now)
            .is_err()
    );
    assert!(
        f.store
            .delete_workspace_file(&root.lease, Uuid::new_v4(), "main.rs", "Wrong content", now)
            .is_err()
    );
    f.store
        .create_workspace_directory(&root.lease, Uuid::new_v4(), "src", now)
        .unwrap();
    assert!(
        f.store
            .delete_workspace_file(&root.lease, Uuid::new_v4(), "src", "", now)
            .is_err()
    );
    let child = f
        .store
        .delegate(
            &root.lease,
            Delegation {
                request_id: Uuid::new_v4(),
                agent_id: f.agents[1],
                objective: "Review".into(),
                expected_output: "Document".into(),
                dependencies: vec![],
                access: AgentAccess::default(),
            },
            now,
        )
        .unwrap();
    f.store
        .suspend_for_children(&root.lease, &[child.id], "{}", now)
        .unwrap();
    let reader = f.claim(child.id);
    assert!(
        f.store
            .create_workspace_directory(&reader.lease, Uuid::new_v4(), "other", now)
            .is_err()
    );
    let old = fs::read_to_string(workspace.join("main.rs")).unwrap();
    assert!(
        f.store
            .delete_workspace_file(&reader.lease, Uuid::new_v4(), "main.rs", &old, now)
            .is_err()
    );
    assert!(
        f.store
            .create_workspace_directory(&root.lease, Uuid::new_v4(), "stale", now)
            .is_err()
    );
    assert_eq!(fs::read_to_string(workspace.join("main.rs")).unwrap(), old);
    assert!(!f.store.workspace_has_pending_effect(f.run.id).unwrap());
}
#[test]
fn team_workspace_pending_structural_effect_is_never_replayed_after_restart() {
    let f = Fixture::new();
    let workspace = f.workspace();
    let root = f.claim(f.run.work[0].id);
    let now = crate::model::unix_time();
    let request = Uuid::new_v4();
    let id = Uuid::new_v4();
    let signature = serde_json::json!({"path":"src","operation":"createDirectory"}).to_string();
    f.store
        .connect()
        .unwrap()
        .execute(
            "INSERT INTO ion_team_workspace_edits VALUES(?,?,?,?,?,?,'pending',NULL)",
            params![
                id.to_string(),
                f.run.id.to_string(),
                root.assignment.id.to_string(),
                root.lease.attempt_id().to_string(),
                request.to_string(),
                signature
            ],
        )
        .unwrap();
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    assert!(
        reopened
            .create_workspace_directory(&root.lease, request, "src", now)
            .is_err()
    );
    assert!(
        reopened
            .delete_workspace_file(
                &root.lease,
                Uuid::new_v4(),
                "main.rs",
                &fs::read_to_string(workspace.join("main.rs")).unwrap(),
                now
            )
            .is_err()
    );
    assert!(!workspace.join("src").exists());
    reopened.reconcile_owner_restart(now).unwrap();
    assert!(reopened.workspace_has_pending_effect(f.run.id).unwrap());
    assert!(workspace.join("main.rs").exists());
}
#[test]
fn team_workspace_native_adapter_creates_directory_deletes_file_and_submits_reviewable_changes() {
    use crate::driver::{NativeTeamExit, NativeTeamStart};
    use serde_json::json;
    let f = Fixture::new();
    let workspace = f.workspace();
    let root = f.claim(f.run.work[0].id);
    let old = fs::read_to_string(workspace.join("main.rs")).unwrap();
    let (events, _) = crate::driver::test_event_channel();
    let mut step = 0;
    let mut artifact = None;
    let exit=crate::driver::run_scripted_team_assignment(&f.store,NativeTeamStart::Fresh(root),&workspace,&events,&AtomicBool::new(false),|_,_,messages,tools| {
        let names:Vec<_>=tools.as_array().unwrap().iter().filter_map(|t|t["function"]["name"].as_str()).collect();
        assert!(names.contains(&"create_directory") && names.contains(&"delete_file"));
        step+=1;
        let (name,args)=match step {
            1=>("create_directory",json!({"path":"src"})),
            2=>("write_file",json!({"path":"src/lib.rs","old_content":null,"new_content":"pub fn value() -> i32 { 2 }\n"})),
            3=>("write_file",json!({"path":"src/marker.txt","old_content":"","new_content":"MARKER"})),
            4=>("delete_file",json!({"path":"main.rs","old_content":old})),
            5=>("publish_change",json!({"path":"main.rs"})),
            _=> {let reply:serde_json::Value=serde_json::from_str(messages.iter().rev().find(|m|m["role"]=="tool").unwrap()["content"].as_str().unwrap()).unwrap(); artifact=Some(reply["artifact_id"].clone()); ("report_result",json!({"summary":"Structural changes submitted; no executed tests claimed","artifact_ids":[reply["artifact_id"]]}))}
        };
        let chunk=json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":format!("structural-{step}"),"function":{"name":name,"arguments":args.to_string()}}]}}]});
        Ok(format!("data: {chunk}\n\ndata: [DONE]\n\n"))
    }).unwrap();
    assert_eq!(exit, NativeTeamExit::Settled);
    let completed = f.store.snapshot(f.run.id).unwrap();
    assert_eq!(completed.status, TeamRunStatus::Completed);
    assert_eq!(
        completed.work[0].result.as_ref().unwrap().artifact_ids[0].to_string(),
        artifact.unwrap().as_str().unwrap()
    );
    assert!(workspace.join("src/lib.rs").exists());
    assert_eq!(fs::read_to_string(workspace.join("src/marker.txt")).unwrap(), "MARKER");
    assert!(!workspace.join("main.rs").exists());
    assert_eq!(fs::read_to_string(f.source.join("main.rs")).unwrap(), old);
}

fn finish_coding(f: &Fixture, root: &ClaimedAssignment) -> TeamRunSnapshot {
    let document = f
        .store
        .publish_workspace_change(
            &root.lease,
            Uuid::new_v4(),
            "main.rs",
            crate::model::unix_time(),
        )
        .unwrap();
    f.store
        .report(
            &root.lease,
            AssignmentOutcome::Completed(AssignmentResult {
                executed_checks: vec![],
                summary: "Reviewed code complete".into(),
                artifact_ids: vec![document.id],
                checks: vec![],
            }),
            crate::model::unix_time(),
        )
        .unwrap()
}
fn source_review(f: &Fixture) -> TeamIntegration {
    f.store
        .prepare_source_integration(
            f.run.id,
            f.store.snapshot(f.run.id).unwrap().revision,
            &f.source,
            crate::model::unix_time(),
        )
        .unwrap()
}
#[test]
fn team_source_integration_reviews_combined_creations_deletions_and_receipts_before_apply() {
    let f = Fixture::new();
    let workspace = f.workspace();
    let root = f.claim(f.run.work[0].id);
    let now = crate::model::unix_time();
    assert!(
        f.store
            .prepare_source_integration(
                f.run.id,
                f.store.snapshot(f.run.id).unwrap().revision,
                &f.source,
                now
            )
            .is_err()
    );
    f.store
        .create_workspace_directory(&root.lease, Uuid::new_v4(), "src", now)
        .unwrap();
    f.store
        .edit_workspace_file(
            &root.lease,
            Uuid::new_v4(),
            "src/lib.rs",
            None,
            "pub fn answer() -> i32 { 2 }\n",
            now,
        )
        .unwrap();
    f.store
        .edit_workspace_file(
            &root.lease,
            Uuid::new_v4(),
            "src/lib.rs",
            Some("pub fn answer() -> i32 { 2 }\n"),
            "pub fn answer() -> i32 { 3 }\n",
            now,
        )
        .unwrap();
    let before = fs::read_to_string(workspace.join("main.rs")).unwrap();
    f.store
        .delete_workspace_file(&root.lease, Uuid::new_v4(), "main.rs", &before, now)
        .unwrap();
    finish_coding(&f, &root);
    let review = source_review(&f);
    assert_eq!(review.changes.len(), 3);
    assert!(review.conflicts.is_empty());
    assert_eq!(review.changes[0].path, "src");
    assert!(review.changes[0].directory);
    let addition = review
        .changes
        .iter()
        .find(|c| c.path == "src/lib.rs")
        .unwrap();
    assert!(addition.before.is_none());
    assert_eq!(addition.edit_ids.len(), 2);
    assert_eq!(
        addition.after.as_deref(),
        Some("pub fn answer() -> i32 { 3 }\n")
    );
    assert_eq!(
        fs::read_to_string(f.source.join("main.rs")).unwrap(),
        before
    );
    assert!(!f.source.join("src").exists());
    assert_eq!(source_review(&f).id, review.id);
    assert!(
        f.store
            .apply_source_integration(review.id, review.revision + 1, &f.source, now)
            .is_err()
    );
    let applied = f
        .store
        .apply_source_integration(review.id, review.revision, &f.source, now)
        .unwrap();
    assert_eq!(applied.status, TeamIntegrationStatus::Applied);
    assert_eq!(applied.applied_changes, 3);
    assert!(!f.source.join("main.rs").exists());
    assert_eq!(
        fs::read_to_string(f.source.join("src/lib.rs")).unwrap(),
        addition.after.clone().unwrap()
    );
    assert!(git(&f.source, &["diff", "--cached"]).unwrap().is_empty());
    // Repeating a successful request returns its receipt, preserving newer edits.
    fs::write(f.source.join("src/lib.rs"), "User follow-up").unwrap();
    assert_eq!(
        f.store
            .apply_source_integration(review.id, review.revision, &f.source, now)
            .unwrap()
            .revision,
        applied.revision
    );
    assert_eq!(
        fs::read_to_string(f.source.join("src/lib.rs")).unwrap(),
        "User follow-up"
    );
    let restarted = TeamCoordinator::new(f.store.control.path());
    assert_eq!(
        restarted
            .prepare_source_integration(
                f.run.id,
                restarted.snapshot(f.run.id).unwrap().revision,
                &f.source,
                now
            )
            .unwrap()
            .status,
        TeamIntegrationStatus::Applied
    );
}
#[test]
fn team_source_integration_detects_changed_source_workspace_and_project_binding() {
    let f = Fixture::new();
    let workspace = f.workspace();
    let root = f.claim(f.run.work[0].id);
    let now = crate::model::unix_time();
    let before = fs::read_to_string(workspace.join("main.rs")).unwrap();
    f.store
        .edit_workspace_file(
            &root.lease,
            Uuid::new_v4(),
            "main.rs",
            Some(&before),
            "Reviewed replacement",
            now,
        )
        .unwrap();
    finish_coding(&f, &root);
    let plan = source_review(&f);
    fs::write(f.source.join("main.rs"), "User edit").unwrap();
    assert!(!source_review(&f).conflicts.is_empty());
    assert!(
        f.store
            .apply_source_integration(plan.id, plan.revision, &f.source, now)
            .is_err()
    );
    assert_eq!(
        fs::read_to_string(f.source.join("main.rs")).unwrap(),
        "User edit"
    );
    fs::write(f.source.join("main.rs"), &before).unwrap();
    assert!(
        f.store
            .apply_source_integration(plan.id, plan.revision, &workspace, now)
            .is_err()
    );
    fs::write(workspace.join("main.rs"), "Workspace drift").unwrap();
    assert!(
        f.store
            .apply_source_integration(plan.id, plan.revision, &f.source, now)
            .is_err()
    );
    assert_eq!(
        fs::read_to_string(f.source.join("main.rs")).unwrap(),
        before
    );
}
#[test]
fn team_source_integration_rejects_unreceipted_files_and_changed_commit() {
    let f = Fixture::new();
    let workspace = f.workspace();
    let root = f.claim(f.run.work[0].id);
    let now = crate::model::unix_time();
    let before = fs::read_to_string(workspace.join("main.rs")).unwrap();
    f.store
        .edit_workspace_file(
            &root.lease,
            Uuid::new_v4(),
            "main.rs",
            Some(&before),
            "Reviewed replacement",
            now,
        )
        .unwrap();
    finish_coding(&f, &root);
    fs::write(workspace.join("untracked.txt"), "Outside tool boundary").unwrap();
    assert!(
        f.store
            .prepare_source_integration(
                f.run.id,
                f.store.snapshot(f.run.id).unwrap().revision,
                &f.source,
                now
            )
            .unwrap_err()
            .to_string()
            .contains("Unreceipted")
    );
    fs::remove_file(workspace.join("untracked.txt")).unwrap();
    let plan = source_review(&f);
    git(
        &f.source,
        &[
            "-c",
            "user.name=Ion Tests",
            "-c",
            "user.email=ion@example.test",
            "commit",
            "--allow-empty",
            "-m",
            "New baseline",
        ],
    )
    .unwrap();
    assert!(!source_review(&f).conflicts.is_empty());
    assert!(
        f.store
            .apply_source_integration(plan.id, plan.revision, &f.source, now)
            .is_err()
    );
    assert_eq!(
        fs::read_to_string(f.source.join("main.rs")).unwrap(),
        before
    );
}
#[test]
fn team_source_integration_partial_apply_never_replays_and_requires_exact_reconciliation() {
    let f = Fixture::new();
    let workspace = f.workspace();
    let root = f.claim(f.run.work[0].id);
    let now = crate::model::unix_time();
    let before = fs::read_to_string(workspace.join("main.rs")).unwrap();
    f.store
        .create_workspace_directory(&root.lease, Uuid::new_v4(), "new", now)
        .unwrap();
    f.store
        .edit_workspace_file(
            &root.lease,
            Uuid::new_v4(),
            "main.rs",
            Some(&before),
            "Reviewed replacement",
            now,
        )
        .unwrap();
    finish_coding(&f, &root);
    let plan = source_review(&f);
    let path = f.source.join("main.rs");
    let original = fs::metadata(&path).unwrap().permissions();
    let mut permissions = original.clone();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();
    let partial = f
        .store
        .apply_source_integration(plan.id, plan.revision, &f.source, now)
        .unwrap();
    assert_eq!(partial.status, TeamIntegrationStatus::Uncertain);
    assert_eq!(partial.applied_changes, 1);
    assert!(f.source.join("new").is_dir());
    assert_eq!(fs::read_to_string(&path).unwrap(), before);
    assert!(
        f.store
            .apply_source_integration(plan.id, partial.revision, &f.source, now)
            .is_err()
    );
    assert!(
        f.store
            .reconcile_source_integration(plan.id, partial.revision, true, &f.source, now)
            .is_err()
    );
    fs::set_permissions(&path, original).unwrap();
    fs::write(&path, "Reviewed replacement").unwrap();
    let stage = f
        .source
        .join(format!(".ion-edit-{}-1.integration", plan.id));
    assert!(stage.exists());
    assert!(
        f.store
            .reconcile_source_integration(plan.id, partial.revision, true, &f.source, now)
            .is_err()
    );
    fs::remove_file(stage).unwrap();
    let applied = f
        .store
        .reconcile_source_integration(plan.id, partial.revision, true, &f.source, now)
        .unwrap();
    assert_eq!(applied.status, TeamIntegrationStatus::Applied);
    assert_eq!(applied.applied_changes, 2);
}
#[test]
fn team_source_integration_restart_retains_unknown_effect_and_revokes_apply() {
    let f = Fixture::new();
    let workspace = f.workspace();
    let root = f.claim(f.run.work[0].id);
    let now = crate::model::unix_time();
    let before = fs::read_to_string(workspace.join("main.rs")).unwrap();
    f.store
        .edit_workspace_file(
            &root.lease,
            Uuid::new_v4(),
            "main.rs",
            Some(&before),
            "Reviewed replacement",
            now,
        )
        .unwrap();
    finish_coding(&f, &root);
    let plan = source_review(&f);
    f.store
        .connect()
        .unwrap()
        .execute(
            "UPDATE ion_team_integrations SET state='applying',revision=revision+1 WHERE id=?",
            [plan.id.to_string()],
        )
        .unwrap();
    let restarted = TeamCoordinator::new(f.store.control.path());
    restarted.reconcile_owner_restart(now).unwrap();
    let recovered = restarted
        .prepare_source_integration(
            f.run.id,
            restarted.snapshot(f.run.id).unwrap().revision,
            &f.source,
            now,
        )
        .unwrap();
    assert_eq!(recovered.status, TeamIntegrationStatus::Uncertain);
    assert!(
        restarted
            .apply_source_integration(plan.id, recovered.revision, &f.source, now)
            .is_err()
    );
    assert_eq!(
        fs::read_to_string(f.source.join("main.rs")).unwrap(),
        before
    );
    fs::write(f.source.join("main.rs"), "Reviewed replacement").unwrap();
    assert_eq!(
        restarted
            .reconcile_source_integration(plan.id, recovered.revision, true, &f.source, now)
            .unwrap()
            .status,
        TeamIntegrationStatus::Applied
    );
}

#[test]
fn team_source_integration_can_confirm_restored_source_after_interruption() {
    let f = Fixture::new();
    let workspace = f.workspace();
    let root = f.claim(f.run.work[0].id);
    let now = crate::model::unix_time();
    let before = fs::read_to_string(workspace.join("main.rs")).unwrap();
    f.store
        .edit_workspace_file(
            &root.lease,
            Uuid::new_v4(),
            "main.rs",
            Some(&before),
            "Reviewed replacement",
            now,
        )
        .unwrap();
    finish_coding(&f, &root);
    let plan = source_review(&f);
    f.store
        .connect()
        .unwrap()
        .execute(
            "UPDATE ion_team_integrations SET state='applying',revision=revision+1 WHERE id=?",
            [plan.id.to_string()],
        )
        .unwrap();
    f.store.reconcile_owner_restart(now).unwrap();
    let recovered = source_review(&f);
    fs::write(f.source.join("main.rs"), "Unresolved user edit").unwrap();
    assert!(
        f.store
            .reconcile_source_integration(plan.id, recovered.revision, false, &f.source, now)
            .is_err()
    );
    fs::write(f.source.join("main.rs"), &before).unwrap();
    let reverted = f
        .store
        .reconcile_source_integration(plan.id, recovered.revision, false, &f.source, now)
        .unwrap();
    assert_eq!(reverted.status, TeamIntegrationStatus::Reverted);
    assert_eq!(reverted.applied_changes, 0);
    assert!(
        f.store
            .apply_source_integration(plan.id, reverted.revision, &f.source, now)
            .is_err()
    );
    assert!(
        f.store
            .reconcile_source_integration(plan.id, reverted.revision, true, &f.source, now)
            .is_err()
    );
    assert_eq!(
        fs::read_to_string(f.source.join("main.rs")).unwrap(),
        before
    );
}
#[test]
fn team_source_integration_rejects_checkout_filters_before_git_status() {
    let f = Fixture::new();
    git(
        &f.source,
        &["config", "filter.fixture.clean", "echo forbidden"],
    )
    .unwrap();
    assert!(
        f.store
            .prepare_local_workspace(f.run.id, &f.source, crate::model::unix_time())
            .unwrap_err()
            .to_string()
            .contains("custom clean")
    );
}

#[test]
fn team_file_recovery_verifies_applied_restored_and_structural_outcomes_without_replay() {
    use serde_json::{Value, json};
    for operation in ["writeFile", "createDirectory", "deleteFile"] {
        for applied in [true, false] {
            let f = Fixture::new();
            let workspace = f.workspace();
            let root = f.claim(f.run.work[0].id);
            let now = crate::model::unix_time();
            let request = Uuid::new_v4();
            let relative = if operation == "createDirectory" {
                "created"
            } else {
                "main.rs"
            };
            let old = "fn value() -> i32 { 1 }\n";
            let new = "fn value() -> i32 { 2 }\n";
            let (signature, args, name) = match operation {
                "writeFile" => (
                    json!({"path":relative,"old":old,"new":new}),
                    json!({"path":relative,"old_content":old,"new_content":new}),
                    "write_file",
                ),
                "deleteFile" => (
                    json!({"path":relative,"operation":operation,"old":old,"new":null}),
                    json!({"path":relative,"old_content":old}),
                    "delete_file",
                ),
                _ => (
                    json!({"path":relative,"operation":operation}),
                    json!({"path":relative}),
                    "create_directory",
                ),
            };
            let ReservedWorkspaceEdit::Pending { id, path } = f
                .store
                .reserve_workspace_edit(
                    &root.lease,
                    request,
                    relative,
                    &signature.to_string(),
                    |_| Ok(()),
                    now,
                )
                .unwrap()
            else {
                panic!("Expected reservation")
            };
            if applied {
                match operation {
                    "writeFile" => fs::write(&path, new).unwrap(),
                    "deleteFile" => fs::remove_file(&path).unwrap(),
                    _ => fs::create_dir(&path).unwrap(),
                }
            }
            let checkpoint=json!({"version":1,"steps":1,"messages":[{"role":"user","content":"Change value"},{"role":"assistant","content":null,"tool_calls":[{"id":"file-effect","type":"function","function":{"name":name,"arguments":args.to_string()}}]}],"pending":[{"id":"file-effect","name":name,"arguments":args,"request_id":request,"activity_id":"file-effect"}]}).to_string();
            f.store
                .save_runtime_checkpoint(&root.lease, &checkpoint, now)
                .unwrap();
            let blocked = f
                .store
                .report(
                    &root.lease,
                    AssignmentOutcome::Blocked("Effect acknowledgement lost".into()),
                    now,
                )
                .unwrap();
            let (review, _) = f
                .store
                .recovery_review(f.run.id, root.assignment.id)
                .unwrap();
            assert_eq!(review.files.len(), 1);
            assert_eq!(review.files[0].id, id);
            let confirmation = if applied {
                TeamRecoveryConfirmation::FileApplied { id }
            } else {
                TeamRecoveryConfirmation::FileRestored { id }
            };
            // The other outcome must fail against actual bytes/structure.
            let wrong = if applied {
                TeamRecoveryConfirmation::FileRestored { id }
            } else {
                TeamRecoveryConfirmation::FileApplied { id }
            };
            assert!(
                f.store
                    .confirm_recovery(
                        f.run.id,
                        root.assignment.id,
                        root.lease.attempt_id(),
                        blocked.revision,
                        wrong,
                        now
                    )
                    .is_err()
            );
            let (_, settled) = f
                .store
                .confirm_recovery(
                    f.run.id,
                    root.assignment.id,
                    root.lease.attempt_id(),
                    blocked.revision,
                    confirmation.clone(),
                    now,
                )
                .unwrap();
            let repeated = f
                .store
                .confirm_recovery(
                    f.run.id,
                    root.assignment.id,
                    root.lease.attempt_id(),
                    blocked.revision,
                    confirmation,
                    now,
                )
                .unwrap();
            assert_eq!(repeated.1.revision, settled.revision);
            let (_, ready) = f
                .store
                .confirm_recovery(
                    f.run.id,
                    root.assignment.id,
                    root.lease.attempt_id(),
                    settled.revision,
                    TeamRecoveryConfirmation::PrepareContinuation,
                    now,
                )
                .unwrap();
            assert!(ready.work[0].can_resume);
            let db = f.store.connect().unwrap();
            let (state,checkpoint):(String,String)=db.query_row("SELECT e.state,c.checkpoint FROM ion_team_workspace_edits e JOIN ion_team_continuations c ON c.work_id=e.work_id WHERE e.id=?",[id.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
            assert_eq!(state, if applied { "applied" } else { "restored" });
            let journal: Value = serde_json::from_str(&checkpoint).unwrap();
            assert_eq!(journal["pending"], json!([]));
            let reply: Value = serde_json::from_str(
                journal["messages"].as_array().unwrap().last().unwrap()["content"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(reply["outcome"], state);
            assert_eq!(reply["replayed"], false);
            assert_eq!(fs::read_to_string(f.source.join("main.rs")).unwrap(), old);
            if operation == "writeFile" {
                assert_eq!(
                    fs::read_to_string(workspace.join(relative)).unwrap(),
                    if applied { new } else { old }
                );
            }
            drop(db);
            f.store
                .queue_checkpoint_resume(f.run.id, root.assignment.id, ready.revision, now)
                .unwrap();
            let resumed = f
                .store
                .resume_after_children(f.run.id, root.assignment.id, now, 180)
                .unwrap();
            let (events, _) = crate::driver::test_event_channel();
            let mut step = 0;
            crate::driver::run_scripted_team_assignment(&f.store,crate::driver::NativeTeamStart::Resumed(resumed),&workspace,&events,&AtomicBool::new(false),|_,_,messages,_|{
                step+=1;
                let (name,args)=if step==1 {
                    let receipt:Value=messages.iter().filter(|m|m["role"]=="tool").filter_map(|m|serde_json::from_str(m["content"].as_str()?).ok()).find(|v:&Value|v.get("outcome").is_some()).expect("Recovered file receipt reached the model");
                    assert_eq!(receipt["outcome"],if applied{"applied"}else{"restored"});assert_eq!(receipt["replayed"],false);
                    ("save_document",json!({"name":"Recovery.md","format":"md","content":"Recovered outcome reviewed; no edit repeated"}))
                }else{
                    let receipt:Value=serde_json::from_str(messages.iter().rev().find(|m|m["role"]=="tool").unwrap()["content"].as_str().unwrap()).unwrap();
                    ("report_result",json!({"summary":"Recovered file outcome reviewed","artifact_ids":[receipt["artifact_id"]]}))
                };
                let chunk=json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":format!("recovery-{step}"),"function":{"name":name,"arguments":args.to_string()}}]}}]});
                Ok(format!("data: {chunk}\n\ndata: [DONE]\n\n"))
            }).unwrap();
            assert_eq!(
                f.store.snapshot(f.run.id).unwrap().work[0].attempts.len(),
                1
            );
            if applied {
                assert_eq!(source_review(&f).changes.len(), 1);
            } else {
                let done = f.store.snapshot(f.run.id).unwrap();
                assert!(
                    f.store
                        .prepare_source_integration(f.run.id, done.revision, &f.source, now)
                        .unwrap_err()
                        .to_string()
                        .contains("No receipted changes remain")
                );
            }
        }
    }
}
#[test]
fn team_file_recovery_retains_private_backups_and_rejects_unauthorized_changed_bytes() {
    use serde_json::json;
    let f = Fixture::new();
    let workspace = f.workspace();
    let root = f.claim(f.run.work[0].id);
    let now = crate::model::unix_time();
    let request = Uuid::new_v4();
    let old = "fn value() -> i32 { 1 }\n";
    let new = "fn value() -> i32 { 2 }\n";
    let signature = json!({"path":"main.rs","old":old,"new":new}).to_string();
    let ReservedWorkspaceEdit::Pending { id, path } = f
        .store
        .reserve_workspace_edit(&root.lease, request, "main.rs", &signature, |_| Ok(()), now)
        .unwrap()
    else {
        panic!("Reservation missing")
    };
    let stage = workspace.join(format!(".ion-edit-{id}.pending"));
    fs::write(&stage, "Unrecognized user bytes").unwrap();
    fs::write(&path, new).unwrap();
    let blocked = f
        .store
        .report(
            &root.lease,
            AssignmentOutcome::Blocked("Lost acknowledgement".into()),
            now,
        )
        .unwrap();
    let c = TeamRecoveryConfirmation::FileApplied { id };
    assert!(
        f.store
            .confirm_recovery(
                f.run.id,
                root.assignment.id,
                root.lease.attempt_id(),
                blocked.revision,
                c.clone(),
                now
            )
            .unwrap_err()
            .to_string()
            .contains("Private recovery stage remains")
    );
    assert_eq!(
        fs::read_to_string(&stage).unwrap(),
        "Unrecognized user bytes"
    );
    // User preserves the exact private stage in the disposable fixture, then clears it.
    fs::rename(&stage, f.dir.join("preserved-backup.txt")).unwrap();
    fs::write(&path, "Newer user edit").unwrap();
    assert!(
        f.store
            .confirm_recovery(
                f.run.id,
                root.assignment.id,
                root.lease.attempt_id(),
                blocked.revision,
                c.clone(),
                now
            )
            .is_err()
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "Newer user edit");
    assert!(f.store.workspace_has_pending_effect(f.run.id).unwrap());
    fs::write(&path, new).unwrap();
    f.store
        .confirm_recovery(
            f.run.id,
            root.assignment.id,
            root.lease.attempt_id(),
            blocked.revision,
            c,
            now,
        )
        .unwrap();
    assert_eq!(
        fs::read_to_string(f.dir.join("preserved-backup.txt")).unwrap(),
        "Unrecognized user bytes"
    );
}

#[path = "inheritance_tests.rs"]
mod inheritance_tests;
