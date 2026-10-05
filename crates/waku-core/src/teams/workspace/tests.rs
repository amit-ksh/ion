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
                    id: Uuid::new_v4(),
                    root_task_id: Uuid::new_v4(),
                    project_id: Uuid::new_v4(),
                    team_id: team.id,
                    expected_definition_revision: definitions.load().unwrap().revision,
                    objective: "Change value to 2".into(),
                    limits: Default::default(),
                    access,
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
            3=>("delete_file",json!({"path":"main.rs","old_content":old})),
            4=>("publish_change",json!({"path":"main.rs"})),
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
    assert!(!workspace.join("main.rs").exists());
    assert_eq!(fs::read_to_string(f.source.join("main.rs")).unwrap(), old);
}
