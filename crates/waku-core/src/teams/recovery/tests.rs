use super::*;
use serde_json::{Value, json};
fn journal(pending: Value) -> String {
    let mut messages = vec![json!({"role":"user","content":"Pinned objective"})];
    if !pending.as_array().unwrap().is_empty() {
        messages.push(json!({"role":"assistant","content":null,"tool_calls":pending.as_array().unwrap().iter().map(|p|json!({"id":p["id"],"type":"function","function":{"name":p["name"],"arguments":p["arguments"].to_string()}})).collect::<Vec<_>>()}));
    }
    json!({"version":1,"steps":1,"messages":messages,"pending":pending}).to_string()
}
fn call(request: Uuid, name: &str, args: Value) -> Value {
    json!({"id":"call-recovery","name":name,"arguments":args,"request_id":request,"activity_id":"recovery-tool"})
}
fn confirm(
    f: &Fixture,
    run: Uuid,
    work: Uuid,
    attempt: Uuid,
    c: TeamRecoveryConfirmation,
) -> (TeamRecoveryReview, TeamRunSnapshot) {
    let revision = f.store.snapshot(run).unwrap().revision;
    f.store
        .confirm_recovery(run, work, attempt, revision, c, 104)
        .unwrap()
}
#[test]
fn team_recovery_unknown_host_requires_explicit_confirmation_and_retains_allowances() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    f.store
        .save_runtime_checkpoint(&root.lease, &journal(json!([])), 102)
        .unwrap();
    f.store
        .reserve_model_call(
            &root.lease,
            1,
            ModelCallAllowance {
                model: "model-0".into(),
                max_input_tokens: 500,
                max_output_tokens: 100,
            },
            102,
        )
        .unwrap();
    f.store.reconcile_owner_restart(103).unwrap();
    let (review, unknown) = f.store.recovery_review(run.id, root.assignment.id).unwrap();
    assert!(
        !review.host_stopped && !review.can_prepare_continuation && unknown.work[0].can_reconcile
    );
    assert!(
        f.store
            .confirm_recovery(
                run.id,
                root.assignment.id,
                root.lease.attempt_id(),
                unknown.revision,
                TeamRecoveryConfirmation::PrepareContinuation,
                104
            )
            .is_err()
    );
    assert!(
        f.store
            .confirm_recovery(
                run.id,
                root.assignment.id,
                Uuid::new_v4(),
                unknown.revision,
                TeamRecoveryConfirmation::HostStopped,
                104
            )
            .is_err()
    );
    let (review, blocked) = confirm(
        &f,
        run.id,
        root.assignment.id,
        root.lease.attempt_id(),
        TeamRecoveryConfirmation::HostStopped,
    );
    assert!(review.host_stopped && review.can_prepare_continuation && !blocked.work[0].can_resume);
    let repeated = f
        .store
        .confirm_recovery(
            run.id,
            root.assignment.id,
            root.lease.attempt_id(),
            unknown.revision,
            TeamRecoveryConfirmation::HostStopped,
            105,
        )
        .unwrap()
        .1;
    assert_eq!(repeated.revision, blocked.revision);
    assert!(
        f.store
            .confirm_recovery(
                run.id,
                root.assignment.id,
                root.lease.attempt_id(),
                unknown.revision,
                TeamRecoveryConfirmation::PrepareContinuation,
                105
            )
            .is_err()
    );
    let (_, prepared) = confirm(
        &f,
        run.id,
        root.assignment.id,
        root.lease.attempt_id(),
        TeamRecoveryConfirmation::PrepareContinuation,
    );
    assert!(prepared.work[0].can_resume);
    assert_eq!(
        prepared.usage.reserved_tokens,
        unknown.usage.reserved_tokens
    );
    assert_eq!(prepared.usage.unknown_calls, 1);
    assert_eq!(prepared.work[0].attempts.len(), 1);
    assert!(f.store.renew(&root.lease, 104, 100).is_err());
}
#[test]
fn team_recovery_recovers_committed_documents_and_delegation_without_duplicate_effects() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let doc_request = Uuid::new_v4();
    let delegate_request = Uuid::new_v4();
    let doc = f
        .store
        .publish_document(
            &root.lease,
            doc_request,
            "Evidence.md",
            "md",
            "Durable evidence",
            102,
        )
        .unwrap();
    let mut delegation = f.delegation(1);
    delegation.request_id = delegate_request;
    let args = json!({"agent_id":delegation.agent_id,"objective":delegation.objective,"expected_output":delegation.expected_output});
    let child = f
        .store
        .delegate_with_contract(
            &root.lease,
            delegation,
            OutputContract {
                required_commands: vec![],
                minimum_documents: 1,
                ..Default::default()
            },
            102,
        )
        .unwrap();
    let mut calls = json!([
        call(
            doc_request,
            "save_document",
            json!({"name":"Evidence.md","format":"md","content":"Durable evidence"})
        ),
        call(delegate_request, "delegate", args)
    ]);
    calls[1]["id"] = json!("call-delegation");
    f.store
        .save_runtime_checkpoint(&root.lease, &journal(calls), 102)
        .unwrap();
    f.store
        .report(
            &root.lease,
            AssignmentOutcome::Blocked("Acknowledgement interrupted".into()),
            103,
        )
        .unwrap();
    let (_, ready) = confirm(
        &f,
        run.id,
        root.assignment.id,
        root.lease.attempt_id(),
        TeamRecoveryConfirmation::PrepareContinuation,
    );
    assert!(ready.work[0].can_resume);
    assert_eq!(ready.work.len(), 2);
    let db = f.store.connect().unwrap();
    let checkpoint: String = db
        .query_row(
            "SELECT checkpoint FROM ion_team_continuations WHERE work_id=?",
            [root.assignment.id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    let value: Value = serde_json::from_str(&checkpoint).unwrap();
    assert_eq!(value["pending"], json!([]));
    let replies: Vec<Value> = value["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "tool")
        .map(|m| serde_json::from_str(m["content"].as_str().unwrap()).unwrap())
        .collect();
    assert_eq!(replies[0]["artifact_id"], doc.id.to_string());
    assert_eq!(replies[1]["assignment_id"], child.id.to_string());
    let count: u32 = db
        .query_row("SELECT count(*) FROM ion_team_document_receipts", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(count, 1);
}
#[test]
fn team_recovery_refuses_mismatched_tool_receipts_and_exhausted_native_journals() {
    for exhausted in [false, true] {
        let f = Fixture::new();
        let run = f.run();
        let root = f.claim(run.id, run.work[0].id);
        let request = Uuid::new_v4();
        f.store
            .publish_document(&root.lease, request, "Evidence.md", "md", "Actual", 102)
            .unwrap();
        let mut value: Value = serde_json::from_str(&journal(json!([call(
            request,
            "save_document",
            json!({"name":"Evidence.md","format":"md","content":"Changed"})
        )])))
        .unwrap();
        if exhausted {
            value["steps"] = json!(16);
        }
        f.store
            .save_runtime_checkpoint(&root.lease, &value.to_string(), 102)
            .unwrap();
        let blocked = f
            .store
            .report(
                &root.lease,
                AssignmentOutcome::Blocked("Interrupted".into()),
                103,
            )
            .unwrap();
        assert!(
            f.store
                .confirm_recovery(
                    run.id,
                    root.assignment.id,
                    root.lease.attempt_id(),
                    blocked.revision,
                    TeamRecoveryConfirmation::PrepareContinuation,
                    104
                )
                .is_err()
        );
        assert_eq!(f.store.snapshot(run.id).unwrap().revision, blocked.revision);
        assert!(!f.store.snapshot(run.id).unwrap().work[0].can_resume);
    }
}
#[test]
fn team_recovery_app_witness_continues_actual_native_worker_without_sending_app_again() {
    use crate::driver::{self, NativeTeamExit, NativeTeamStart};
    use std::sync::atomic::AtomicBool;
    for successful in [true, false] {
        let f = Fixture::new();
        let grant = AppActionGrant {
            account_id: "fixture-account".into(),
            action: "GITHUB_CREATE_ISSUE".into(),
            version: "20250905_00".into(),
        };
        let catalog = f.definitions.load().unwrap();
        let mut profile = catalog
            .records
            .iter()
            .find_map(|r| match r {
                ControlRecord::Agent(a) if a.id == f.agents[0] => Some(a.clone()),
                _ => None,
            })
            .unwrap();
        profile.access.app_actions = vec![grant.clone()];
        f.definitions
            .save(catalog.revision, ControlRecord::Agent(profile))
            .unwrap();
        let now = crate::model::unix_time();
        let mut request = f.request();
        request.expected_definition_revision = f.definitions.load().unwrap().revision;
        request.access.workspace_write = false;
        request.access.app_actions = vec![grant.clone()];
        let run = f.store.prepare(request, now).unwrap();
        let root = f
            .store
            .claim(run.id, run.work[0].id, DispatchHost::Local, now, 180)
            .unwrap();
        let decision = f
            .store
            .suspend_for_app_action(
                &root.lease,
                Uuid::new_v4(),
                grant,
                json!({"title":"Exact approved action"}),
                &journal(json!([])),
                now,
            )
            .unwrap();
        f.store.resolve_decision(decision.id, 1, true, now).unwrap();
        let started = f
            .store
            .resume_after_children(run.id, root.assignment.id, now, 180)
            .unwrap();
        // Native worker saves its inherited checkpoint in the new generation
        // before starting the approved app effect.
        f.store
            .save_runtime_checkpoint(&started.claimed.lease, &started.checkpoint, now)
            .unwrap();
        let ticket = match f
            .store
            .begin_app_action(&started.claimed.lease, decision.id, now)
            .unwrap()
        {
            AppActionStart::Execute(t) => t,
            _ => panic!("Expected reservation"),
        };
        let blocked = f
            .store
            .report(
                &started.claimed.lease,
                AssignmentOutcome::Blocked("Lost provider acknowledgement".into()),
                now,
            )
            .unwrap();
        assert!(!blocked.work[0].can_resume);
        let confirmation = if successful {
            TeamRecoveryConfirmation::AppSucceeded {
                decision_id: decision.id,
                evidence: "Provider item 123 verified by user".into(),
            }
        } else {
            TeamRecoveryConfirmation::AppFailed {
                decision_id: decision.id,
                evidence: "Provider confirmed rejection 123".into(),
            }
        };
        let (_, settled) = f
            .store
            .confirm_recovery(
                run.id,
                root.assignment.id,
                started.claimed.lease.attempt_id(),
                blocked.revision,
                confirmation.clone(),
                now,
            )
            .unwrap();
        assert!(!f.store.has_pending_effect(run.id).unwrap());
        let repeat = f
            .store
            .confirm_recovery(
                run.id,
                root.assignment.id,
                started.claimed.lease.attempt_id(),
                blocked.revision,
                confirmation,
                now,
            )
            .unwrap();
        assert_eq!(repeat.1.revision, settled.revision);
        assert!(
            f.store
                .finish_app_action(&ticket, &json!({"successful":!successful}).to_string(), now)
                .is_err()
        );
        let (_, prepared) = f
            .store
            .confirm_recovery(
                run.id,
                root.assignment.id,
                started.claimed.lease.attempt_id(),
                settled.revision,
                TeamRecoveryConfirmation::PrepareContinuation,
                now,
            )
            .unwrap();
        f.store
            .queue_checkpoint_resume(run.id, root.assignment.id, prepared.revision, now)
            .unwrap();
        let resumed = f
            .store
            .resume_after_children(run.id, root.assignment.id, now, 180)
            .unwrap();
        assert!(resumed.decision.is_none());
        let (events, _) = driver::test_event_channel();
        let exit=driver::run_scripted_team_assignment(&f.store,NativeTeamStart::Resumed(resumed),&f.dir,&events,&AtomicBool::new(false),|model,_,messages,_|{
            assert_eq!(model,"model-0");let receipt=messages.iter().filter_map(|m|serde_json::from_str::<Value>(m["content"].as_str()?).ok()).find(|v|v["app_action_receipt"]==decision.id.to_string()).expect("Recovered app outcome returned to model");
            assert_eq!(receipt["successful"],successful);assert_eq!(receipt["confirmed_by"],"user");assert_eq!(receipt["replayed"],false);
            let call=json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"recovery-result","function":{"name":"report_result","arguments":json!({"summary":"App outcome reviewed","artifact_ids":[receipt["artifact_id"]]}).to_string()}}]}}]});
            Ok(format!("data: {call}\n\ndata: [DONE]\n\n"))
        }).unwrap();
        assert_eq!(exit, NativeTeamExit::Settled);
        let done = f.store.snapshot(run.id).unwrap();
        assert_eq!(done.status, TeamRunStatus::Completed);
        assert_eq!(done.work[0].attempts.len(), 1);
        let db = f.store.connect().unwrap();
        let count: u32 = db
            .query_row("SELECT count(*) FROM ion_team_app_effects", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
        let docs: u32 = db
            .query_row("SELECT count(*) FROM ion_team_document_receipts", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(docs, 1);
    }
}

#[test]
fn team_recovery_can_confirm_a_stopped_owner_but_cannot_restart_a_canceled_run() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    f.store
        .save_runtime_checkpoint(&root.lease, &journal(json!([])), 102)
        .unwrap();
    f.store.reconcile_owner_restart(103).unwrap();
    let snapshot = f.store.snapshot(run.id).unwrap();
    let stopping = f.store.stop(run.id, snapshot.revision, 104).unwrap();
    let (_, canceled) = f
        .store
        .confirm_recovery(
            run.id,
            root.assignment.id,
            root.lease.attempt_id(),
            stopping.revision,
            TeamRecoveryConfirmation::HostStopped,
            104,
        )
        .unwrap();
    assert_eq!(canceled.status, TeamRunStatus::Canceled);
    assert!(!canceled.work[0].can_resume);
    assert!(
        f.store
            .confirm_recovery(
                run.id,
                root.assignment.id,
                root.lease.attempt_id(),
                canceled.revision,
                TeamRecoveryConfirmation::PrepareContinuation,
                105
            )
            .is_err()
    );
}
#[test]
fn team_recovery_racing_app_assertions_commit_one_labeled_outcome() {
    let (f, grant, run, root) = app_fixture();
    let decision = f
        .store
        .suspend_for_app_action(
            &root.lease,
            Uuid::new_v4(),
            grant,
            json!({"title":"Exact"}),
            "{}",
            102,
        )
        .unwrap();
    f.store.resolve_decision(decision.id, 1, true, 103).unwrap();
    let resumed = f
        .store
        .resume_after_children(run.id, root.assignment.id, 104, 100)
        .unwrap();
    f.store
        .begin_app_action(&resumed.claimed.lease, decision.id, 104)
        .unwrap();
    let blocked = f
        .store
        .report(
            &resumed.claimed.lease,
            AssignmentOutcome::Blocked("Lost acknowledgement".into()),
            104,
        )
        .unwrap();
    let gate = std::sync::Arc::new(std::sync::Barrier::new(2));
    let jobs: Vec<_> = [true, false]
        .into_iter()
        .map(|success| {
            let gate = gate.clone();
            let path = f.dir.join("control.db");
            let attempt = resumed.claimed.lease.attempt_id();
            let work = root.assignment.id;
            std::thread::spawn(move || {
                gate.wait();
                let c = if success {
                    TeamRecoveryConfirmation::AppSucceeded {
                        decision_id: decision.id,
                        evidence: "Provider item verified".into(),
                    }
                } else {
                    TeamRecoveryConfirmation::AppFailed {
                        decision_id: decision.id,
                        evidence: "Provider rejection verified".into(),
                    }
                };
                TeamCoordinator::new(path).confirm_recovery(
                    run.id,
                    work,
                    attempt,
                    blocked.revision,
                    c,
                    105,
                )
            })
        })
        .collect();
    let results: Vec<_> = jobs.into_iter().map(|j| j.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    let db = f.store.connect().unwrap();
    let count: u32 = db
        .query_row("SELECT count(*) FROM ion_team_document_receipts", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(count, 1);
    let confirmations: u32 = db
        .query_row(
            "SELECT count(*) FROM ion_team_recovery_confirmations",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(confirmations, 1);
    let receipt: String = db
        .query_row("SELECT receipt FROM ion_team_app_effects", [], |r| r.get(0))
        .unwrap();
    let receipt: Value = serde_json::from_str(&receipt).unwrap();
    assert_eq!(receipt["confirmed_by"], "user");
}

#[test]
fn team_recovery_pdf_acknowledges_original_bytes_once_and_rejects_changed_summary() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    for changed in [false, true] {
        let f = Fixture::new();
        let run = f.run();
        let root = f.claim(run.id, run.work[0].id);
        let request = Uuid::new_v4();
        let bytes = crate::teams::test_pdf_bytes();
        let encoded = STANDARD.encode(&bytes);
        let artifact = f
            .store
            .publish_pdf(
                &root.lease,
                request,
                "Report.pdf",
                &encoded,
                "Reviewed report",
                102,
            )
            .unwrap();
        let pending = call(
            request,
            "save_pdf",
            json!({"name":"Report.pdf","content_base64":encoded,"summary":if changed {"Changed summary"} else {"Reviewed report"}}),
        );
        f.store
            .save_runtime_checkpoint(&root.lease, &journal(json!([pending])), 102)
            .unwrap();
        let blocked = f
            .store
            .report(
                &root.lease,
                AssignmentOutcome::Blocked("Acknowledgement interrupted".into()),
                103,
            )
            .unwrap();
        let confirmation = f.store.confirm_recovery(
            run.id,
            root.assignment.id,
            root.lease.attempt_id(),
            blocked.revision,
            TeamRecoveryConfirmation::PrepareContinuation,
            104,
        );
        if changed {
            assert!(confirmation.is_err());
        } else {
            assert!(confirmation.unwrap().1.work[0].can_resume);
            let db = f.store.connect().unwrap();
            let checkpoint: String = db
                .query_row(
                    "SELECT checkpoint FROM ion_team_continuations WHERE work_id=?",
                    [root.assignment.id.to_string()],
                    |r| r.get(0),
                )
                .unwrap();
            let value: Value = serde_json::from_str(&checkpoint).unwrap();
            assert_eq!(value["pending"], json!([]));
            let reply: Value = serde_json::from_str(
                value["messages"].as_array().unwrap().last().unwrap()["content"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(reply["artifact_id"], artifact.id.to_string());
            assert_eq!(
                db.query_row("SELECT count(*) FROM ion_team_document_receipts", [], |r| r
                    .get::<_, u32>(0))
                    .unwrap(),
                1
            );
        }
        assert_eq!(f.definitions.read_document(artifact.id).unwrap(), bytes);
    }
}
