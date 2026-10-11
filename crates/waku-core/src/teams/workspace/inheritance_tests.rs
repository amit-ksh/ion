use super::*;
const ORIGINAL: &str = "fn value() -> i32 { 1 }\n";
const FIRST: &str = "fn value() -> i32 { 2 }\n";
const FINAL: &str = "fn value() -> i32 { 3 }\n";
fn definition(f: &Fixture, count: usize) -> Workflow {
    Workflow {
        id: Uuid::new_v4(),
        name: "Code handoff".into(),
        project_id: f.run.project_id,
        prompt: "Implement, review and report".into(),
        limits: None,
        stages: (0..count)
            .map(|i| WorkflowStage {
                inherit_workspace: i > 0,
                retry: None,
                title: format!("Stage {i}"),
                instructions: "Review the actual isolated code and publish evidence".into(),
                requires_decision: false,
                team_id: Some(f.run.team.id),
                required_formats: vec![],
                transition: Default::default(),
            })
            .collect(),
    }
}
fn start(f: &Fixture, workflow: Workflow) -> AutomationRun {
    let id = workflow.id;
    let defs = crate::control::ControlStore::new(f.dir.join("control.db"));
    let rev = defs
        .save(
            defs.load().unwrap().revision,
            ControlRecord::Workflow(workflow),
        )
        .unwrap()
        .revision;
    let run = f
        .store
        .start_automation(Uuid::new_v4(), id, rev, crate::model::unix_time())
        .unwrap();
    tick(f);
    f.store.automation_run(run.id).unwrap()
}
fn tick(f: &Fixture) {
    f.store
        .tick_automations(
            &HashSet::from([f.run.project_id]),
            crate::model::unix_time(),
        )
        .unwrap();
}
fn claim(f: &Fixture, run: Uuid) -> ClaimedAssignment {
    for pending in f.store.pending_materializations(128).unwrap() {
        f.store
            .acknowledge_materialized(
                pending.run_id,
                pending.assignment.id,
                pending.assignment.task_id,
                crate::model::unix_time(),
            )
            .unwrap();
    }
    let team = f.store.snapshot(run).unwrap();
    f.store
        .claim(
            run,
            team.work[0].id,
            DispatchHost::Local,
            crate::model::unix_time(),
            180,
        )
        .unwrap()
}
fn workspace(f: &Fixture, run: Uuid) -> PathBuf {
    f.store
        .prepare_local_workspace(run, &f.source, crate::model::unix_time())
        .unwrap()
}
fn finish(f: &Fixture, root: &ClaimedAssignment, json: bool) {
    let doc = f
        .store
        .publish_document(
            &root.lease,
            Uuid::new_v4(),
            if json { "route.json" } else { "Review.md" },
            if json { "json" } else { "md" },
            if json {
                "{\"ready\":true}"
            } else {
                "Reviewed the exact isolated code; no tests are claimed"
            },
            crate::model::unix_time(),
        )
        .unwrap();
    f.store
        .report(
            &root.lease,
            AssignmentOutcome::Completed(AssignmentResult {
                executed_checks: vec![],
                summary: "Stage complete".into(),
                artifact_ids: vec![doc.id],
                checks: vec![],
            }),
            crate::model::unix_time(),
        )
        .unwrap();
}
#[test]
fn workflow_code_handoff_accumulates_receipts_and_final_source_review() {
    let f = Fixture::new();
    fs::write(f.source.join("old.txt"), "obsolete").unwrap();
    git(&f.source, &["add", "old.txt"]).unwrap();
    git(
        &f.source,
        &[
            "-c",
            "user.name=Ion Tests",
            "-c",
            "user.email=ion@example.test",
            "commit",
            "-m",
            "extra baseline",
        ],
    )
    .unwrap();
    let a = start(&f, definition(&f, 3));
    let first = a.stages[0].team_run_id;
    let one = workspace(&f, first);
    let root = claim(&f, first);
    let now = crate::model::unix_time();
    f.store
        .edit_workspace_file(
            &root.lease,
            Uuid::new_v4(),
            "main.rs",
            Some(ORIGINAL),
            FIRST,
            now,
        )
        .unwrap();
    f.store
        .create_workspace_directory(&root.lease, Uuid::new_v4(), "src", now)
        .unwrap();
    f.store
        .edit_workspace_file(
            &root.lease,
            Uuid::new_v4(),
            "src/lib.rs",
            None,
            "pub fn lib() {}\n",
            now,
        )
        .unwrap();
    f.store
        .delete_workspace_file(&root.lease, Uuid::new_v4(), "old.txt", "obsolete", now)
        .unwrap();
    finish(&f, &root, false);
    tick(&f);
    let a = f.store.automation_run(a.id).unwrap();
    let second = a.stages[1].team_run_id;
    assert_eq!(a.stages[1].workspace_from, Some(first));
    assert!(f.store.workspace_has_pending_effect(second).unwrap());
    let team = f.store.snapshot(second).unwrap();
    assert!(
        f.store
            .claim(second, team.work[0].id, DispatchHost::Local, now, 180)
            .is_err()
    );
    let two = workspace(&f, second);
    assert_ne!(one, two);
    assert_eq!(fs::read_to_string(two.join("main.rs")).unwrap(), FIRST);
    assert_eq!(
        fs::read_to_string(two.join("src/lib.rs")).unwrap(),
        "pub fn lib() {}\n"
    );
    assert!(!two.join("old.txt").exists());
    let root = claim(&f, second);
    f.store
        .edit_workspace_file(
            &root.lease,
            Uuid::new_v4(),
            "main.rs",
            Some(FIRST),
            FINAL,
            now,
        )
        .unwrap();
    f.store
        .delete_workspace_file(
            &root.lease,
            Uuid::new_v4(),
            "src/lib.rs",
            "pub fn lib() {}\n",
            now,
        )
        .unwrap();
    finish(&f, &root, false);
    tick(&f);
    let third = f.store.automation_run(a.id).unwrap().stages[2].team_run_id;
    let three = workspace(&f, third);
    assert_eq!(fs::read_to_string(three.join("main.rs")).unwrap(), FINAL);
    assert!(!three.join("src/lib.rs").exists());
    assert!(!three.join("old.txt").exists());
    let root = claim(&f, third);
    assert_eq!(
        f.store.prepare_local_workspace(third, &three, now).unwrap(),
        three
    );
    let doc = f
        .store
        .publish_workspace_change(&root.lease, Uuid::new_v4(), "main.rs", now)
        .unwrap();
    assert!(doc.evidence.is_some());
    finish(&f, &root, false);
    tick(&f);
    assert_eq!(
        f.store.automation_run(a.id).unwrap().status,
        AutomationStatus::Completed
    );
    assert_eq!(
        fs::read_to_string(f.source.join("main.rs")).unwrap(),
        ORIGINAL
    );
    assert!(f.source.join("old.txt").exists());
    assert!(!f.source.join("src").exists());
    let team = f.store.snapshot(third).unwrap();
    let plan = f
        .store
        .prepare_source_integration(third, team.revision, &f.source, now)
        .unwrap();
    let code = plan.changes.iter().find(|c| c.path == "main.rs").unwrap();
    assert_eq!(code.before.as_deref(), Some(ORIGINAL));
    assert_eq!(code.after.as_deref(), Some(FINAL));
    assert_eq!(code.edit_ids.len(), 2);
    assert!(plan.conflicts.is_empty());
    f.store
        .apply_source_integration(plan.id, plan.revision, &f.source, now)
        .unwrap();
    assert_eq!(fs::read_to_string(f.source.join("main.rs")).unwrap(), FINAL);
    assert!(!f.source.join("old.txt").exists());
}
#[test]
fn workflow_code_handoff_follows_selected_predecessor_and_exact_gate() {
    let f = Fixture::new();
    let mut w = definition(&f, 3);
    w.stages[0].transition = WorkflowTransition::DocumentCondition {
        document: "route.json".into(),
        pointer: "/ready".into(),
        equals: serde_json::json!(true),
        on_match: WorkflowTarget::Stage { index: 2 },
        on_mismatch: WorkflowTarget::Stage { index: 1 },
    };
    w.stages[2].requires_decision = true;
    let a = start(&f, w);
    let first = a.stages[0].team_run_id;
    workspace(&f, first);
    let root = claim(&f, first);
    f.store
        .edit_workspace_file(
            &root.lease,
            Uuid::new_v4(),
            "main.rs",
            Some(ORIGINAL),
            FIRST,
            crate::model::unix_time(),
        )
        .unwrap();
    finish(&f, &root, true);
    tick(&f);
    let a = f.store.automation_run(a.id).unwrap();
    assert_eq!(a.status, AutomationStatus::WaitingDecision);
    assert!(a.stages[1].skipped);
    assert!(f.store.snapshot(a.stages[1].team_run_id).is_err());
    assert!(f.store.snapshot(a.stages[2].team_run_id).is_err());
    assert!(a.stages[2].workspace_from.is_none());
    f.store
        .resolve_automation_gate(a.id, a.revision, 2, true, crate::model::unix_time())
        .unwrap();
    tick(&f);
    let a = f.store.automation_run(a.id).unwrap();
    assert_eq!(a.stages[2].workspace_from, Some(first));
    assert_eq!(
        fs::read_to_string(workspace(&f, a.stages[2].team_run_id).join("main.rs")).unwrap(),
        FIRST
    );
}
#[test]
fn workflow_code_handoff_rejects_unreceipted_input_and_keeps_original_base() {
    let f = Fixture::new();
    let a = start(&f, definition(&f, 2));
    let first = a.stages[0].team_run_id;
    let one = workspace(&f, first);
    let root = claim(&f, first);
    f.store
        .edit_workspace_file(
            &root.lease,
            Uuid::new_v4(),
            "main.rs",
            Some(ORIGINAL),
            FIRST,
            crate::model::unix_time(),
        )
        .unwrap();
    finish(&f, &root, false);
    tick(&f);
    let second = f.store.automation_run(a.id).unwrap().stages[1].team_run_id;
    fs::write(one.join("rogue.txt"), "not a receipt").unwrap();
    assert!(
        f.store
            .prepare_local_workspace(second, &f.source, crate::model::unix_time())
            .is_err()
    );
    assert!(f.store.snapshot(second).unwrap().workspace.is_none());
    fs::remove_file(one.join("rogue.txt")).unwrap();
    inheritance::pin(
        &f.store,
        &f.store.connect().unwrap(),
        second,
        &fs::canonicalize(&f.source).unwrap(),
        crate::model::unix_time(),
    )
    .unwrap();
    let db = f.store.connect().unwrap();
    let body: String = db
        .query_row(
            "SELECT body FROM ion_team_workspace_handoffs WHERE run_id=?",
            [second.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    let mut altered: serde_json::Value = serde_json::from_str(&body).unwrap();
    altered["record"]["tree"] = serde_json::json!("C:/unexpected-code-tree");
    db.execute(
        "UPDATE ion_team_workspace_handoffs SET body=? WHERE run_id=?",
        params![altered.to_string(), second.to_string()],
    )
    .unwrap();
    let error = f
        .store
        .prepare_local_workspace(second, &f.source, crate::model::unix_time())
        .unwrap_err();
    assert!(error.to_string().contains("Inherited code receipt changed"));
    assert!(f.store.snapshot(second).unwrap().workspace.is_none());
    db.execute(
        "UPDATE ion_team_workspace_handoffs SET body=? WHERE run_id=?",
        params![body, second.to_string()],
    )
    .unwrap();
    fs::write(f.source.join("main.rs"), FINAL).unwrap();
    git(&f.source, &["add", "main.rs"]).unwrap();
    git(
        &f.source,
        &[
            "-c",
            "user.name=Ion Tests",
            "-c",
            "user.email=ion@example.test",
            "commit",
            "-m",
            "source advanced",
        ],
    )
    .unwrap();
    let two = workspace(&f, second);
    assert_eq!(fs::read_to_string(two.join("main.rs")).unwrap(), FIRST);
    assert_eq!(fs::read_to_string(f.source.join("main.rs")).unwrap(), FINAL);
    let root = claim(&f, second);
    finish(&f, &root, false);
    let plan = f
        .store
        .prepare_source_integration(
            second,
            f.store.snapshot(second).unwrap().revision,
            &f.source,
            crate::model::unix_time(),
        )
        .unwrap();
    assert!(!plan.conflicts.is_empty());
    assert!(
        f.store
            .apply_source_integration(plan.id, plan.revision, &f.source, crate::model::unix_time())
            .is_err()
    );
}
#[test]
fn workflow_code_handoff_restart_adopts_exact_result_without_replaying_partial_copy() {
    let f = Fixture::new();
    let a = start(&f, definition(&f, 2));
    let first = a.stages[0].team_run_id;
    workspace(&f, first);
    let root = claim(&f, first);
    let now = crate::model::unix_time();
    f.store
        .edit_workspace_file(
            &root.lease,
            Uuid::new_v4(),
            "main.rs",
            Some(ORIGINAL),
            FIRST,
            now,
        )
        .unwrap();
    finish(&f, &root, false);
    tick(&f);
    let second = f.store.automation_run(a.id).unwrap().stages[1].team_run_id;
    let two = workspace(&f, second);
    let db = f.store.connect().unwrap();
    db.execute(
        "UPDATE ion_team_workspaces SET state='preparing' WHERE run_id=?",
        [second.to_string()],
    )
    .unwrap();
    for p in f.store.pending_materializations(128).unwrap() {
        f.store
            .acknowledge_materialized(p.run_id, p.assignment.id, p.assignment.task_id, now)
            .unwrap();
    }
    let team = f.store.snapshot(second).unwrap();
    let error = f
        .store
        .claim(second, team.work[0].id, DispatchHost::Local, now, 180)
        .err()
        .unwrap();
    assert!(
        error
            .to_string()
            .contains("Inherited code workspace must be sealed")
    );
    db.execute(
        "UPDATE ion_team_workspace_handoffs SET state='applying' WHERE run_id=?",
        [second.to_string()],
    )
    .unwrap();
    db.execute(
        "UPDATE ion_team_workspaces SET state='preparing' WHERE run_id=?",
        [second.to_string()],
    )
    .unwrap();
    fs::write(two.join("main.rs"), ORIGINAL).unwrap();
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    reopened.reconcile_owner_restart(now).unwrap();
    assert!(
        reopened
            .prepare_local_workspace(second, &f.source, now)
            .is_err()
    );
    assert_eq!(fs::read_to_string(two.join("main.rs")).unwrap(), ORIGINAL);
    assert!(reopened.snapshot(second).unwrap().workspace.is_none());
    fs::write(two.join("main.rs"), FIRST).unwrap();
    let modified = fs::metadata(two.join("main.rs"))
        .unwrap()
        .modified()
        .unwrap();
    let stage = two.join(format!(".ion-edit-{second}-0.integration"));
    fs::write(&stage, FIRST).unwrap();
    assert!(
        reopened
            .prepare_local_workspace(second, &f.source, now)
            .is_err()
    );
    fs::remove_file(stage).unwrap();
    fs::write(two.join("rogue.txt"), "unreceipted recovery file").unwrap();
    assert!(
        reopened
            .prepare_local_workspace(second, &f.source, now)
            .is_err()
    );
    fs::remove_file(two.join("rogue.txt")).unwrap();
    assert_eq!(
        reopened
            .prepare_local_workspace(second, &f.source, now)
            .unwrap(),
        two
    );
    assert_eq!(
        fs::metadata(two.join("main.rs"))
            .unwrap()
            .modified()
            .unwrap(),
        modified
    );
    assert!(!reopened.workspace_has_pending_effect(second).unwrap());
}
#[test]
fn workflow_code_handoff_readonly_stage_keeps_write_denied_and_legacy_opt_out() {
    let f = Fixture::new();
    let defs = crate::control::ControlStore::new(f.dir.join("control.db"));
    let mut profile = f.run.profiles[0].clone();
    profile.id = Uuid::new_v4();
    profile.access.workspace_write = false;
    defs.save(
        defs.load().unwrap().revision,
        ControlRecord::Agent(profile.clone()),
    )
    .unwrap();
    let mut team = f.run.team.clone();
    team.id = Uuid::new_v4();
    team.members = vec![TeamMember {
        agent_id: profile.id,
        parent_id: None,
    }];
    defs.save(
        defs.load().unwrap().revision,
        ControlRecord::Team(team.clone()),
    )
    .unwrap();
    let mut w = definition(&f, 2);
    w.stages[1].team_id = Some(team.id);
    let a = start(&f, w);
    let first = a.stages[0].team_run_id;
    workspace(&f, first);
    let root = claim(&f, first);
    let now = crate::model::unix_time();
    f.store
        .edit_workspace_file(
            &root.lease,
            Uuid::new_v4(),
            "main.rs",
            Some(ORIGINAL),
            FIRST,
            now,
        )
        .unwrap();
    finish(&f, &root, false);
    tick(&f);
    let second = f.store.automation_run(a.id).unwrap().stages[1].team_run_id;
    let two = workspace(&f, second);
    let root = claim(&f, second);
    assert!(!root.assignment.access.workspace_write);
    assert_eq!(
        f.store
            .assignment_workspace(&root.lease, &f.source, now)
            .unwrap(),
        two
    );
    assert_eq!(fs::read_to_string(two.join("main.rs")).unwrap(), FIRST);
    assert!(
        f.store
            .edit_workspace_file(
                &root.lease,
                Uuid::new_v4(),
                "main.rs",
                Some(FIRST),
                FINAL,
                now
            )
            .is_err()
    );
    let wire = serde_json::json!({"title":"Legacy","instructions":"Independent","requiresDecision":false,"teamId":team.id});
    let legacy: WorkflowStage = serde_json::from_value(wire).unwrap();
    assert!(!legacy.inherit_workspace);
    assert!(
        serde_json::to_value(legacy)
            .unwrap()
            .get("inheritWorkspace")
            .is_none()
    );
    let mut invalid = definition(&f, 2);
    invalid.stages[0].inherit_workspace = true;
    assert!(invalid.validate_graph().is_err());
}
