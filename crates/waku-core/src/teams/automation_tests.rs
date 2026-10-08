use super::*;
fn workflow(f: &Fixture, gates: &[bool]) -> Workflow {
    Workflow {
        id: Uuid::new_v4(),
        name: "Release".into(),
        project_id: f.project,
        prompt: "Prepare the requested release documents".into(),
        limits: None,
        stages: gates
            .iter()
            .enumerate()
            .map(|(i, gate)| WorkflowStage {
                title: format!("Stage {i}"),
                instructions: "Create a report and review its evidence".into(),
                requires_decision: *gate,
                team_id: Some(f.team),
                required_formats: vec![],
            })
            .collect(),
    }
}
fn save(f: &Fixture, r: ControlRecord) -> u64 {
    let rev = f.definitions.load().unwrap().revision;
    f.definitions.save(rev, r).unwrap().revision
}
fn tick(f: &Fixture, now: u64) {
    f.store
        .tick_automations(&HashSet::from([f.project]), now)
        .unwrap();
}
fn start(f: &Fixture, w: Workflow) -> AutomationRun {
    let id = w.id;
    let rev = save(f, ControlRecord::Workflow(w));
    f.store
        .start_automation(Uuid::new_v4(), id, rev, 100)
        .unwrap()
}
fn complete_stage(f: &Fixture, a: Uuid, now: u64) -> TeamRunSnapshot {
    let a = f.store.automation_run(a).unwrap();
    let stage = &a.stages[usize::from(a.current_stage)];
    f.materialize();
    let team = f.store.snapshot(stage.team_run_id).unwrap();
    let claimed = f
        .store
        .claim(team.id, team.work[0].id, DispatchHost::Local, now, 100)
        .unwrap();
    f.store
        .report(
            &claimed.lease,
            AssignmentOutcome::Completed(result()),
            now + 1,
        )
        .unwrap()
}
fn routine(f: &Fixture, w: Option<Uuid>) -> Routine {
    Routine {
        id: Uuid::new_v4(),
        name: "Daily review".into(),
        project_id: f.project,
        instructions: "Review recent changes".into(),
        interval_minutes: 1,
        calendar: None,
        missed_run_policy: MissedRunPolicy::default(),
        overlap_policy: OverlapPolicy::default(),
        paused: true,
        next_run: 0,
        provider: ProviderKind::Nebius,
        team_id: if w.is_none() { Some(f.team) } else { None },
        workflow_id: w,
        last_error: None,
    }
}
#[test]
fn automation_gates_are_enforced_and_exact_answers_do_not_approve_later_stages() {
    let f = Fixture::new();
    let a = start(&f, workflow(&f, &[true, true]));
    tick(&f, 100);
    let a = f.store.automation_run(a.id).unwrap();
    assert_eq!(a.status, AutomationStatus::WaitingDecision);
    assert!(f.store.snapshot(a.stages[0].team_run_id).is_err());
    assert!(
        f.store
            .resolve_automation_gate(a.id, a.revision - 1, 0, true, 100)
            .is_err()
    );
    f.store
        .resolve_automation_gate(a.id, a.revision, 0, true, 100)
        .unwrap();
    tick(&f, 100);
    complete_stage(&f, a.id, 101);
    tick(&f, 103);
    let next = f.store.automation_run(a.id).unwrap();
    assert_eq!(next.current_stage, 1);
    assert_eq!(next.status, AutomationStatus::WaitingDecision);
    assert!(f.store.snapshot(next.stages[1].team_run_id).is_err());
    f.store
        .resolve_automation_gate(a.id, a.revision, 0, true, 103)
        .unwrap();
    assert!(!f.store.automation_run(a.id).unwrap().stages[1].approved);
    f.store
        .resolve_automation_gate(a.id, next.revision, 1, false, 103)
        .unwrap();
    tick(&f, 104);
    assert_eq!(
        f.store.automation_run(a.id).unwrap().status,
        AutomationStatus::Canceled
    );
}
#[test]
fn automation_pins_future_stages_and_replays_ids_after_definition_edits() {
    let f = Fixture::new();
    let w = workflow(&f, &[false, false]);
    let w_id = w.id;
    let rev = save(&f, ControlRecord::Workflow(w));
    let id = Uuid::new_v4();
    let a = f.store.start_automation(id, w_id, rev, 100).unwrap();
    tick(&f, 100);
    let mut agent = f
        .definitions
        .load()
        .unwrap()
        .records
        .into_iter()
        .find_map(|r| match r {
            ControlRecord::Agent(a) if a.id == f.agents[0] => Some(a),
            _ => None,
        })
        .unwrap();
    agent.model = Some("changed-model".into());
    save(&f, ControlRecord::Agent(agent));
    let same = f.store.start_automation(id, w_id, rev, 101).unwrap();
    assert_eq!(same.stages[0].task_id, a.stages[0].task_id);
    assert!(f.store.start_automation(id, w_id, rev + 1, 101).is_err());
    complete_stage(&f, a.id, 101);
    tick(&f, 103);
    let next = f.store.snapshot(a.stages[1].team_run_id).unwrap();
    assert_eq!(
        next.profiles
            .iter()
            .find(|p| p.id == f.agents[0])
            .unwrap()
            .model
            .as_deref(),
        Some("model-0")
    );
    complete_stage(&f, a.id, 104);
    tick(&f, 106);
    assert_eq!(
        f.store.automation_run(a.id).unwrap().status,
        AutomationStatus::Completed
    );
    assert_eq!(f.store.automation_run(a.id).unwrap().current_stage, 2);
    assert!(
        f.store
            .resolve_automation_gate(a.id, 1, 2, true, 106)
            .is_err()
    );
}
#[test]
fn automation_handoff_uses_durable_accepted_documents_not_model_file_paths() {
    let f = Fixture::new();
    let a = start(&f, workflow(&f, &[false, false]));
    tick(&f, 100);
    f.materialize();
    let team = f.store.snapshot(a.stages[0].team_run_id).unwrap();
    let c = f.claim(team.id, team.work[0].id);
    let doc = f
        .store
        .publish_document(
            &c.lease,
            Uuid::new_v4(),
            "release.md",
            "md",
            "Pinned release evidence",
            102,
        )
        .unwrap();
    let mut out = result();
    out.artifact_ids = vec![doc.id];
    f.store
        .report(&c.lease, AssignmentOutcome::Completed(out), 103)
        .unwrap();
    tick(&f, 104);
    let next = f.store.snapshot(a.stages[1].team_run_id).unwrap();
    assert_eq!(next.inputs.len(), 2);
    assert_eq!(next.inputs[1].name, "release.md");
    let db = f.store.connect().unwrap();
    let bytes: String = db
        .query_row(
            "SELECT content FROM ion_team_input_files WHERE run_id=? AND key=?",
            params![next.id.to_string(), doc.id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(bytes, "Pinned release evidence");
    assert!(
        next.source_prompt
            .as_ref()
            .unwrap()
            .contains("Complete this stage only")
    );
}
#[test]
fn artifact_integrity_workflow_cannot_start_a_stage_with_changed_prior_document() {
    let f = Fixture::new();
    let run = start(&f, workflow(&f, &[false, false]));
    tick(&f, 100);
    f.materialize();
    let team = f.store.snapshot(run.stages[0].team_run_id).unwrap();
    let claim = f.claim(team.id, team.work[0].id);
    let document = f
        .store
        .publish_document(
            &claim.lease,
            Uuid::new_v4(),
            "release.md",
            "md",
            "Original accepted release",
            102,
        )
        .unwrap();
    let mut output = result();
    output.artifact_ids = vec![document.id];
    f.store
        .report(&claim.lease, AssignmentOutcome::Completed(output), 103)
        .unwrap();
    let db = f.store.connect().unwrap();
    db.execute(
        "UPDATE ion_team_document_receipts SET content=? WHERE artifact_id=?",
        params![
            b"Changed after completion".as_slice(),
            document.id.to_string()
        ],
    )
    .unwrap();
    tick(&f, 104);
    let blocked = f.store.automation_run(run.id).unwrap();
    assert_eq!(blocked.status, AutomationStatus::Blocked);
    assert!(blocked.reason.as_ref().unwrap().contains("original hash"));
    assert!(f.store.snapshot(run.stages[1].team_run_id).is_err());
    db.execute(
        "UPDATE ion_team_document_receipts SET content=? WHERE artifact_id=?",
        params![
            b"Original accepted release".as_slice(),
            document.id.to_string()
        ],
    )
    .unwrap();
    tick(&f, 105);
    assert_eq!(f.store.automation_run(run.id).unwrap().current_stage, 1);
    assert_eq!(
        f.store
            .snapshot(run.stages[1].team_run_id)
            .unwrap()
            .inputs
            .len(),
        2
    );
}

#[test]
fn automation_aggregate_model_budget_and_deadline_do_not_reset_per_stage() {
    let f = Fixture::new();
    let mut w = workflow(&f, &[false, false]);
    w.limits = Some(TeamRunLimits {
        max_model_calls: 1,
        timeout_seconds: 60,
        ..Default::default()
    });
    let a = start(&f, w);
    tick(&f, 100);
    f.materialize();
    let team = f.store.snapshot(a.stages[0].team_run_id).unwrap();
    let c = f.claim(team.id, team.work[0].id);
    f.store
        .reserve_model_call(
            &c.lease,
            1,
            ModelCallAllowance {
                model: "model-0".into(),
                max_input_tokens: 100,
                max_output_tokens: 10,
            },
            102,
        )
        .unwrap();
    f.complete(&c.lease);
    tick(&f, 105);
    let blocked = f.store.automation_run(a.id).unwrap();
    assert_eq!(blocked.status, AutomationStatus::Blocked);
    assert!(blocked.reason.unwrap().contains("model-call budget"));
    assert!(f.store.snapshot(a.stages[1].team_run_id).is_err());
    let a = start(&f, workflow(&f, &[false]));
    tick(&f, 100);
    let deadline = f.store.automation_run(a.id).unwrap().deadline;
    tick(&f, deadline);
    assert_eq!(
        f.store.automation_run(a.id).unwrap().status,
        AutomationStatus::Blocked
    );
    assert_eq!(
        f.store.snapshot(a.stages[0].team_run_id).unwrap().status,
        TeamRunStatus::Canceled
    );
}
#[test]
fn automation_stop_fence_survives_restart_and_never_launches_next_stage() {
    let f = Fixture::new();
    let a = start(&f, workflow(&f, &[false, false]));
    tick(&f, 100);
    let a = f.store.automation_run(a.id).unwrap();
    f.store.stop_automation(a.id, a.revision, 101).unwrap();
    let owner = TeamCoordinator::new(f.definitions.path());
    owner
        .tick_automations(&HashSet::from([f.project]), 102)
        .unwrap();
    assert_eq!(
        owner.snapshot(a.stages[0].team_run_id).unwrap().status,
        TeamRunStatus::Canceled
    );
    assert!(owner.snapshot(a.stages[1].team_run_id).is_err());
    assert_eq!(
        owner.stop_automation(a.id, 1, 103).unwrap().status,
        AutomationStatus::Canceled
    );
}
#[test]
fn automation_missing_project_and_denied_handoff_read_are_setup_blockers() {
    let f = Fixture::new();
    let a = start(&f, workflow(&f, &[false]));
    f.store.tick_automations(&HashSet::new(), 100).unwrap();
    assert_eq!(
        f.store.automation_run(a.id).unwrap().status,
        AutomationStatus::Blocked
    );
    assert!(f.store.snapshot(a.stages[0].team_run_id).is_err());
    tick(&f, 101);
    assert!(f.store.snapshot(a.stages[0].team_run_id).is_ok());
    let mut agent = f
        .definitions
        .load()
        .unwrap()
        .records
        .into_iter()
        .find_map(|r| match r {
            ControlRecord::Agent(a) if a.id == f.agents[0] => Some(a),
            _ => None,
        })
        .unwrap();
    agent.access.workspace_read = false;
    agent.access.workspace_write = false;
    save(&f, ControlRecord::Agent(agent));
    let a = start(&f, workflow(&f, &[false, false]));
    tick(&f, 100);
    complete_stage(&f, a.id, 101);
    tick(&f, 103);
    assert_eq!(
        f.store.automation_run(a.id).unwrap().status,
        AutomationStatus::Blocked
    );
    assert!(f.store.snapshot(a.stages[1].team_run_id).is_err());
}
#[test]
fn automation_routine_is_paused_until_activated_coalesces_missed_slots_and_skips_overlap() {
    let f = Fixture::new();
    let r = routine(&f, None);
    let r_id = r.id;
    let rev = save(&f, ControlRecord::Routine(r));
    tick(&f, 1000);
    assert!(
        f.store
            .automation_runs(false, None, Some(r_id), None, 64)
            .unwrap()
            .runs
            .is_empty()
    );
    f.store.set_routine_paused(r_id, rev, false, 100).unwrap();
    tick(&f, 1000);
    tick(&f, 1000);
    let first = f
        .store
        .automation_runs(false, None, Some(r_id), None, 64)
        .unwrap()
        .runs;
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].scheduled_at, Some(1000));
    let owner = TeamCoordinator::new(f.definitions.path());
    owner
        .tick_automations(&HashSet::from([f.project]), 1060)
        .unwrap();
    let runs = owner
        .automation_runs(false, None, Some(r_id), None, 64)
        .unwrap()
        .runs;
    assert_eq!(runs.len(), 2);
    assert_eq!(
        runs.iter()
            .filter(|r| r.status == AutomationStatus::Skipped)
            .count(),
        1
    );
    let defs = f.definitions.load().unwrap();
    f.store
        .set_routine_paused(r_id, defs.revision, true, 1061)
        .unwrap();
    tick(&f, 5000);
    assert_eq!(
        f.store
            .automation_runs(false, None, Some(r_id), None, 64)
            .unwrap()
            .runs
            .len(),
        2
    );
}
#[test]
fn automation_routine_workflow_pins_at_fire_and_stale_schedule_changes_reject() {
    let f = Fixture::new();
    let w = workflow(&f, &[true]);
    let wid = w.id;
    save(&f, ControlRecord::Workflow(w));
    let r = routine(&f, Some(wid));
    let rid = r.id;
    let rev = save(&f, ControlRecord::Routine(r));
    assert!(
        f.store
            .set_routine_paused(rid, rev - 1, false, 100)
            .is_err()
    );
    f.store.set_routine_paused(rid, rev, false, 100).unwrap();
    tick(&f, 160);
    let runs = f
        .store
        .automation_runs(false, None, Some(rid), None, 64)
        .unwrap()
        .runs;
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].workflow_id, Some(wid));
    assert_eq!(runs[0].status, AutomationStatus::WaitingDecision);
}
#[test]
fn automation_drafts_remain_compatible_and_invalid_targets_do_not_activate() {
    let f = Fixture::new();
    let mut w = workflow(&f, &[false]);
    w.stages[0].team_id = None;
    let wid = w.id;
    let rev = save(&f, ControlRecord::Workflow(w));
    assert!(
        f.store
            .start_automation(Uuid::new_v4(), wid, rev, 100)
            .is_err()
    );
    let mut r = routine(&f, None);
    r.team_id = None;
    let rid = r.id;
    let rev = save(&f, ControlRecord::Routine(r));
    assert!(f.store.set_routine_paused(rid, rev, false, 100).is_err());
    assert!(f.store.automation_runs(false, None, None, None, 0).is_err());
    assert!(
        f.store
            .automation_runs(false, None, None, None, 65)
            .is_err()
    );
}
#[test]
fn automation_scheduler_rotates_bounded_batches_past_waiting_gates() {
    let f = Fixture::new();
    let w = workflow(&f, &[true]);
    let wid = w.id;
    let rev = save(&f, ControlRecord::Workflow(w));
    for _ in 0..35 {
        f.store
            .start_automation(Uuid::new_v4(), wid, rev, 100)
            .unwrap();
    }
    tick(&f, 100);
    tick(&f, 100);
    let runs = f
        .store
        .automation_runs(false, None, None, None, 64)
        .unwrap()
        .runs;
    assert_eq!(runs.len(), 35);
    assert!(
        runs.iter()
            .all(|r| r.status == AutomationStatus::WaitingDecision)
    );
}

#[test]
fn automation_pending_counts_and_pages_reconcile_without_loading_private_profiles() {
    let f = Fixture::new();
    let w = workflow(&f, &[true]);
    let wid = w.id;
    let rev = save(&f, ControlRecord::Workflow(w));
    for _ in 0..3 {
        f.store
            .start_automation(Uuid::new_v4(), wid, rev, 100)
            .unwrap();
    }
    tick(&f, 100);
    let page = f
        .store
        .automation_runs(true, Some(f.project), Some(wid), None, 2)
        .unwrap();
    assert_eq!(page.pending_count, 3);
    assert_eq!(page.runs.len(), 2);
    let next = f
        .store
        .automation_runs(true, Some(f.project), Some(wid), page.next_cursor, 2)
        .unwrap();
    assert_eq!(next.runs.len(), 1);
    assert_eq!(next.pending_count, 3);
    assert!(next.next_cursor.is_none());
    let json = serde_json::to_string(&page).unwrap();
    assert!(!json.contains("private instructions"));
    assert!(!json.contains("profiles"));
    assert!(json.contains("instructions"));
}

#[test]
fn automation_late_owner_tick_retains_completed_final_stage_but_cannot_start_another() {
    for final_stage in [true, false] {
        let f = Fixture::new();
        let a = start(
            &f,
            workflow(
                &f,
                if final_stage {
                    &[false]
                } else {
                    &[false, false]
                },
            ),
        );
        tick(&f, 100);
        complete_stage(&f, a.id, 105);
        tick(&f, a.deadline + 10);
        let after = f.store.automation_run(a.id).unwrap();
        assert!(after.stages[0].completed);
        assert_eq!(
            after.status,
            if final_stage {
                AutomationStatus::Completed
            } else {
                AutomationStatus::Blocked
            }
        );
        if !final_stage {
            assert!(f.store.snapshot(after.stages[1].team_run_id).is_err());
        }
    }
}

#[test]
fn workflow_documents_pin_each_stage_and_reject_missing_formats_before_completion() {
    let f = Fixture::new();
    let mut definition = workflow(&f, &[false, false]);
    definition.stages[0].required_formats = vec!["html".into(), "md".into()];
    definition.stages[1].required_formats = vec!["json".into()];
    let run = start(&f, definition.clone());
    assert_eq!(run.stages[0].required_formats, vec!["html", "md"]);
    for stage in &mut definition.stages {
        stage.required_formats.clear();
    }
    save(&f, ControlRecord::Workflow(definition));
    tick(&f, 100);
    f.materialize();
    let team = f.store.snapshot(run.stages[0].team_run_id).unwrap();
    assert_eq!(team.work[0].output_contract.minimum_documents, 2);
    assert_eq!(
        team.work[0].output_contract.required_formats,
        vec!["html", "md"]
    );
    let claim = f.claim(team.id, team.work[0].id);
    assert!(
        f.store
            .report(&claim.lease, AssignmentOutcome::Completed(result()), 102)
            .is_err()
    );
    let md = f
        .store
        .publish_document(
            &claim.lease,
            Uuid::new_v4(),
            "report.md",
            "md",
            "Reviewed report",
            102,
        )
        .unwrap();
    let txt = f
        .store
        .publish_document(
            &claim.lease,
            Uuid::new_v4(),
            "report.txt",
            "txt",
            "Text alone cannot satisfy HTML",
            102,
        )
        .unwrap();
    let mut output = result();
    output.artifact_ids = vec![md.id, txt.id];
    assert!(
        f.store
            .report(
                &claim.lease,
                AssignmentOutcome::Completed(output.clone()),
                103
            )
            .unwrap_err()
            .to_string()
            .contains("formats")
    );
    assert!(f.store.snapshot(run.stages[1].team_run_id).is_err());
    let html = f
        .store
        .publish_document(
            &claim.lease,
            Uuid::new_v4(),
            "report.html",
            "html",
            "<h1>Reviewed report</h1>",
            103,
        )
        .unwrap();
    output.artifact_ids = vec![md.id, html.id];
    f.store
        .report(&claim.lease, AssignmentOutcome::Completed(output), 103)
        .unwrap();
    tick(&f, 104);
    f.materialize();
    let team = f.store.snapshot(run.stages[1].team_run_id).unwrap();
    assert_eq!(team.work[0].output_contract.required_formats, vec!["json"]);
    let claim = f
        .store
        .claim(team.id, team.work[0].id, DispatchHost::Local, 105, 100)
        .unwrap();
    assert!(
        f.store
            .report(&claim.lease, AssignmentOutcome::Completed(result()), 106)
            .is_err()
    );
    assert_eq!(
        f.store.automation_run(run.id).unwrap().status,
        AutomationStatus::Running
    );
    let json = f
        .store
        .publish_document(
            &claim.lease,
            Uuid::new_v4(),
            "result.json",
            "json",
            "{\"ready\":true}",
            106,
        )
        .unwrap();
    let mut output = result();
    output.artifact_ids = vec![json.id];
    f.store
        .report(&claim.lease, AssignmentOutcome::Completed(output), 107)
        .unwrap();
    tick(&f, 108);
    assert_eq!(
        f.store.automation_run(run.id).unwrap().status,
        AutomationStatus::Completed
    );
}
#[test]
fn workflow_documents_validate_before_save_and_legacy_empty_requirements_are_compatible() {
    let f = Fixture::new();
    let mut definition = workflow(&f, &[false]);
    let legacy = serde_json::to_value(&definition).unwrap();
    assert!(legacy["stages"][0].get("requiredFormats").is_none());
    let restored: Workflow = serde_json::from_value(legacy.clone()).unwrap();
    assert!(restored.stages[0].required_formats.is_empty());
    assert_eq!(serde_json::to_value(restored).unwrap(), legacy);
    let revision = f.definitions.load().unwrap().revision;
    for formats in [
        vec!["html", "html"],
        vec!["pdf"],
        vec!["exe"],
        vec!["HTML"],
        vec!["md", "txt", "html", "csv", "json", "md"],
    ] {
        definition.stages[0].required_formats = formats.into_iter().map(str::to_owned).collect();
        assert!(
            f.definitions
                .save(revision, ControlRecord::Workflow(definition.clone()))
                .is_err()
        );
        assert_eq!(f.definitions.load().unwrap().revision, revision);
    }
    definition.stages[0].required_formats = vec!["html".into()];
    let run = start(&f, definition);
    tick(&f, 100);
    f.materialize();
    let team = f.store.snapshot(run.stages[0].team_run_id).unwrap();
    let claim = f.claim(team.id, team.work[0].id);
    let doc = f
        .store
        .publish_document(
            &claim.lease,
            Uuid::new_v4(),
            "report.html",
            "html",
            "<h1>Original report</h1>",
            102,
        )
        .unwrap();
    let mut output = result();
    output.artifact_ids = vec![doc.id];
    f.store
        .report(&claim.lease, AssignmentOutcome::Completed(output), 103)
        .unwrap();
    let db = f.store.connect().unwrap();
    db.execute(
        "UPDATE ion_team_document_receipts SET content=? WHERE artifact_id=?",
        params![b"Changed report".as_slice(), doc.id.to_string()],
    )
    .unwrap();
    tick(&f, 104);
    assert_eq!(
        f.store.automation_run(run.id).unwrap().status,
        AutomationStatus::Blocked
    );
    assert!(!f.store.automation_run(run.id).unwrap().stages[0].completed);
    db.execute(
        "UPDATE ion_team_document_receipts SET content=? WHERE artifact_id=?",
        params![b"<h1>Original report</h1>".as_slice(), doc.id.to_string()],
    )
    .unwrap();
    tick(&f, 105);
    assert_eq!(
        f.store.automation_run(run.id).unwrap().status,
        AutomationStatus::Completed
    );
}

fn calendar_at(time: &str) -> u64 {
    time.parse::<jiff::Timestamp>()
        .unwrap()
        .as_second()
        .try_into()
        .unwrap()
}
fn daily_routine(f: &Fixture) -> Routine {
    let mut r = routine(f, None);
    r.calendar = Some(CalendarSchedule {
        time_zone: "UTC".into(),
        hour: 9,
        minute: 0,
        recurrence: CalendarRecurrence::Daily,
    });
    r
}
fn saved_routine(f: &Fixture, id: Uuid) -> Routine {
    f.definitions
        .load()
        .unwrap()
        .records
        .into_iter()
        .find_map(|r| match r {
            ControlRecord::Routine(r) if r.id == id => Some(r),
            _ => None,
        })
        .unwrap()
}
#[test]
fn calendar_owner_coalesces_once_pins_trigger_and_survives_concurrent_restart() {
    let f = Fixture::new();
    let r = daily_routine(&f);
    let id = r.id;
    let rev = save(&f, ControlRecord::Routine(r));
    let enabled = calendar_at("2026-10-08T08:00:00Z");
    tick(&f, enabled);
    assert!(
        f.store
            .automation_runs(false, None, Some(id), None, 64)
            .unwrap()
            .runs
            .is_empty()
    );
    f.store.set_routine_paused(id, rev, false, enabled).unwrap();
    assert_eq!(
        saved_routine(&f, id).next_run,
        calendar_at("2026-10-08T09:00:00Z")
    );
    tick(&f, calendar_at("2026-10-08T08:59:59Z"));
    assert!(
        f.store
            .automation_runs(false, None, Some(id), None, 64)
            .unwrap()
            .runs
            .is_empty()
    );
    let now = calendar_at("2026-10-10T10:00:00Z");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    std::thread::scope(|scope| {
        for _ in 0..2 {
            let barrier = barrier.clone();
            let path = f.definitions.path().to_owned();
            let project = f.project;
            scope.spawn(move || {
                let owner = TeamCoordinator::new(path);
                barrier.wait();
                owner
                    .tick_automations(&HashSet::from([project]), now)
                    .unwrap();
            });
        }
    });
    let runs = f
        .store
        .automation_runs(false, None, Some(id), None, 64)
        .unwrap()
        .runs;
    assert_eq!(runs.len(), 1);
    let run = &runs[0];
    assert_eq!(run.scheduled_at, Some(calendar_at("2026-10-10T09:00:00Z")));
    assert_eq!(
        saved_routine(&f, id).next_run,
        calendar_at("2026-10-11T09:00:00Z")
    );
    assert_eq!(
        run.trigger
            .as_ref()
            .unwrap()
            .calendar
            .as_ref()
            .unwrap()
            .time_zone,
        "UTC"
    );
    assert!(f.store.snapshot(run.stages[0].team_run_id).is_ok());
    let mut edited = saved_routine(&f, id);
    edited.calendar.as_mut().unwrap().time_zone = "Asia/Kolkata".into();
    edited.missed_run_policy = MissedRunPolicy::Skip;
    edited.overlap_policy = OverlapPolicy::Allow;
    edited.paused = true;
    edited.next_run = 0;
    save(&f, ControlRecord::Routine(edited));
    let owner = TeamCoordinator::new(f.definitions.path());
    let pinned = owner.automation_run(run.id).unwrap().trigger.unwrap();
    assert_eq!(pinned.calendar.unwrap().time_zone, "UTC");
    assert_eq!(pinned.missed_run_policy, MissedRunPolicy::RunLatest);
    assert_eq!(pinned.overlap_policy, OverlapPolicy::Skip);
    owner
        .tick_automations(&HashSet::from([f.project]), now)
        .unwrap();
    assert_eq!(
        owner
            .automation_runs(false, None, Some(id), None, 64)
            .unwrap()
            .runs
            .len(),
        1
    );
}
#[test]
fn calendar_owner_skip_missed_records_no_effect_and_grace_fires_normally() {
    let f = Fixture::new();
    let mut r = daily_routine(&f);
    r.missed_run_policy = MissedRunPolicy::Skip;
    let id = r.id;
    let rev = save(&f, ControlRecord::Routine(r));
    f.store
        .set_routine_paused(id, rev, false, calendar_at("2026-10-08T08:00:00Z"))
        .unwrap();
    tick(&f, calendar_at("2026-10-08T09:01:01Z"));
    tick(&f, calendar_at("2026-10-08T09:01:02Z"));
    let runs = f
        .store
        .automation_runs(false, None, Some(id), None, 64)
        .unwrap()
        .runs;
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, AutomationStatus::Skipped);
    assert!(runs[0].reason.as_ref().unwrap().contains("missed"));
    assert!(f.store.snapshot(runs[0].stages[0].team_run_id).is_err());
    assert!(f.store.pending_materializations(128).unwrap().is_empty());
    tick(&f, calendar_at("2026-10-09T09:01:00Z"));
    let runs = f
        .store
        .automation_runs(false, None, Some(id), None, 64)
        .unwrap()
        .runs;
    assert_eq!(runs.len(), 2);
    let live = runs
        .iter()
        .find(|r| r.status == AutomationStatus::Running)
        .unwrap();
    assert!(f.store.snapshot(live.stages[0].team_run_id).is_ok());
}
#[test]
fn calendar_owner_manual_overlap_still_enforces_shared_agent_capacity() {
    let f = Fixture::new();
    let mut r = daily_routine(&f);
    r.overlap_policy = OverlapPolicy::Allow;
    r.missed_run_policy = MissedRunPolicy::Skip;
    let id = r.id;
    let rev = save(&f, ControlRecord::Routine(r));
    // Manual invocation uses the saved policies without imposing missed-time logic.
    let first = f
        .store
        .start_automation(Uuid::new_v4(), id, rev, 100)
        .unwrap();
    tick(&f, 100);
    f.materialize();
    let first_team = f.store.snapshot(first.stages[0].team_run_id).unwrap();
    let first_claim = f.claim(first_team.id, first_team.work[0].id);
    let second = f
        .store
        .start_automation(Uuid::new_v4(), id, rev, 102)
        .unwrap();
    assert_eq!(second.status, AutomationStatus::Running);
    assert!(second.scheduled_at.is_none());
    tick(&f, 102);
    f.materialize();
    let second_team = f.store.snapshot(second.stages[0].team_run_id).unwrap();
    assert!(
        f.store
            .claim(
                second_team.id,
                second_team.work[0].id,
                DispatchHost::Local,
                103,
                100
            )
            .is_err()
    );
    f.complete(&first_claim.lease);
    assert!(
        f.store
            .claim(
                second_team.id,
                second_team.work[0].id,
                DispatchHost::Local,
                105,
                100
            )
            .is_ok()
    );
    let mut r = saved_routine(&f, id);
    r.overlap_policy = OverlapPolicy::Skip;
    let rev = save(&f, ControlRecord::Routine(r));
    let skipped = f
        .store
        .start_automation(Uuid::new_v4(), id, rev, 105)
        .unwrap();
    assert_eq!(skipped.status, AutomationStatus::Skipped);
    assert!(skipped.reason.unwrap().contains("overlap"));
}
#[test]
fn calendar_owner_legacy_defaults_and_invalid_schedule_isolation() {
    let f = Fixture::new();
    let legacy = routine(&f, None);
    let bytes = serde_json::to_value(&legacy).unwrap();
    assert!(bytes.get("calendar").is_none());
    assert!(bytes.get("missedRunPolicy").is_none());
    assert!(bytes.get("overlapPolicy").is_none());
    let decoded: Routine = serde_json::from_value(bytes).unwrap();
    assert_eq!(decoded.missed_run_policy, MissedRunPolicy::RunLatest);
    assert_eq!(decoded.overlap_policy, OverlapPolicy::Skip);
    let mut bad = daily_routine(&f);
    let bad_id = bad.id;
    bad.calendar.as_mut().unwrap().time_zone = "No/Such_Zone".into();
    let rev = f.definitions.load().unwrap().revision;
    assert!(
        f.definitions
            .save(rev, ControlRecord::Routine(bad.clone()))
            .is_err()
    );
    assert_eq!(f.definitions.load().unwrap().revision, rev);
    bad.calendar.as_mut().unwrap().time_zone = "UTC".into();
    let rev = save(&f, ControlRecord::Routine(bad));
    let now = calendar_at("2026-10-08T08:00:00Z");
    f.store.set_routine_paused(bad_id, rev, false, now).unwrap();
    let good = daily_routine(&f);
    let good_id = good.id;
    let rev = save(&f, ControlRecord::Routine(good));
    f.store
        .set_routine_paused(good_id, rev, false, now)
        .unwrap();
    let mut bad = saved_routine(&f, bad_id);
    bad.calendar.as_mut().unwrap().time_zone = "No/Such_Zone".into();
    f.store
        .connect()
        .unwrap()
        .execute(
            "UPDATE ion_control_records SET body=? WHERE id=?",
            params![
                serde_json::to_string(&ControlRecord::Routine(bad)).unwrap(),
                bad_id.to_string()
            ],
        )
        .unwrap();
    tick(&f, calendar_at("2026-10-08T09:00:00Z"));
    let bad = saved_routine(&f, bad_id);
    assert!(bad.paused);
    assert_eq!(bad.next_run, 0);
    assert!(bad.last_error.is_some());
    assert!(
        f.store
            .automation_runs(false, None, Some(bad_id), None, 64)
            .unwrap()
            .runs
            .is_empty()
    );
    assert_eq!(
        f.store
            .automation_runs(false, None, Some(good_id), None, 64)
            .unwrap()
            .runs
            .len(),
        1
    );
}
