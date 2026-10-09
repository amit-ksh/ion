use super::*;
fn retry_workflow(f: &Fixture) -> Workflow {
    let mut w = workflow(f, &[false, false]);
    w.stages[0].retry = Some(WorkflowRetryPolicy {
        max_attempts: 2,
        delay_seconds: 2,
    });
    w
}
fn stage_claim(f: &Fixture, a: &AutomationRun, now: u64) -> ClaimedAssignment {
    tick(f, now);
    f.materialize();
    let team = f.store.snapshot(a.stages[0].team_run_id).unwrap();
    f.store
        .claim(team.id, team.work[0].id, DispatchHost::Local, now, 100)
        .unwrap()
}
#[test]
fn workflow_retry_retains_task_policy_usage_and_delay_across_restart() {
    let f = Fixture::new();
    let w = retry_workflow(&f);
    let mut edited = w.clone();
    let a = start(&f, w);
    let first = stage_claim(&f, &a, 101);
    let receipt = f
        .store
        .reserve_model_call(&first.lease, 1, allowance("model-0", 100, 10), 101)
        .unwrap();
    f.store
        .record_model_usage(
            &receipt,
            ModelTokenUsage {
                input_tokens: 10,
                output_tokens: 1,
            },
            102,
        )
        .unwrap();
    f.store
        .report(
            &first.lease,
            AssignmentOutcome::FailedBeforeEffects("Confirmed startup failure".into()),
            102,
        )
        .unwrap();
    edited.stages[0].retry = None;
    save(&f, ControlRecord::Workflow(edited));
    tick(&f, 103);
    let waiting = f.store.automation_run(a.id).unwrap();
    assert_eq!(waiting.status, AutomationStatus::Running);
    assert!(waiting.reason.is_some());
    assert_eq!(waiting.stages[0].retry.unwrap().max_attempts, 2);
    let reopen = TeamCoordinator::new(f.definitions.path());
    reopen
        .tick_automations(&HashSet::from([f.project]), 104)
        .unwrap();
    assert_eq!(
        reopen.automation_run(a.id).unwrap().revision,
        waiting.revision
    );
    reopen
        .tick_automations(&HashSet::from([f.project]), 105)
        .unwrap();
    let queued = reopen.snapshot(first.lease.run_id()).unwrap();
    assert_eq!(queued.work[0].status, AssignmentStatus::Queued);
    assert_eq!(queued.work[0].task_id, first.assignment.task_id);
    assert_eq!(queued.work[0].attempts.len(), 1);
    assert_eq!(queued.usage.consumed_tokens, 11);
    assert_eq!(queued.work[0].access, first.assignment.access);
    assert_eq!(queued.profiles[0].model, Some("model-0".into()));
    assert!(reopen.renew(&first.lease, 106, 100).is_err());
    let second = reopen
        .claim(queued.id, queued.work[0].id, DispatchHost::Local, 106, 100)
        .unwrap();
    assert_ne!(second.lease.attempt_id(), first.lease.attempt_id());
    reopen
        .report(&second.lease, AssignmentOutcome::Completed(result()), 107)
        .unwrap();
    tick(&f, 108);
    let advanced = f.store.automation_run(a.id).unwrap();
    assert_eq!(advanced.current_stage, 1);
    assert!(advanced.stages[1].started);
    let next = f.store.snapshot(advanced.stages[1].team_run_id).unwrap();
    assert_eq!(next.limits.max_model_calls, 63);
    assert_eq!(next.limits.max_tokens, Some(2_000_000 - 11));
}
#[test]
fn workflow_retry_exhaustion_and_original_call_budget_block_further_work() {
    let f = Fixture::new();
    let a = start(&f, retry_workflow(&f));
    let first = stage_claim(&f, &a, 101);
    f.store
        .report(
            &first.lease,
            AssignmentOutcome::FailedBeforeEffects("first".into()),
            102,
        )
        .unwrap();
    tick(&f, 103);
    tick(&f, 105);
    let second = f
        .store
        .claim(
            first.lease.run_id(),
            first.assignment.id,
            DispatchHost::Local,
            106,
            100,
        )
        .unwrap();
    f.store
        .report(
            &second.lease,
            AssignmentOutcome::FailedBeforeEffects("second".into()),
            107,
        )
        .unwrap();
    tick(&f, 108);
    let blocked = f.store.automation_run(a.id).unwrap();
    assert_eq!(blocked.status, AutomationStatus::Blocked);
    assert!(!blocked.stages[1].started);
    tick(&f, 110);
    assert_eq!(
        f.store.automation_run(a.id).unwrap().revision,
        blocked.revision
    );
    let f = Fixture::new();
    let mut w = retry_workflow(&f);
    w.limits = Some(TeamRunLimits {
        max_model_calls: 1,
        ..Default::default()
    });
    let a = start(&f, w);
    let first = stage_claim(&f, &a, 101);
    let receipt = f
        .store
        .reserve_model_call(&first.lease, 1, allowance("model-0", 100, 10), 101)
        .unwrap();
    f.store
        .record_model_usage(
            &receipt,
            ModelTokenUsage {
                input_tokens: 1,
                output_tokens: 1,
            },
            102,
        )
        .unwrap();
    f.store
        .report(
            &first.lease,
            AssignmentOutcome::FailedBeforeEffects("failed".into()),
            102,
        )
        .unwrap();
    tick(&f, 103);
    let blocked = f.store.automation_run(a.id).unwrap();
    assert_eq!(blocked.status, AutomationStatus::Blocked);
    assert!(blocked.reason.unwrap().contains("budget"));
    assert_eq!(
        f.store.snapshot(first.lease.run_id()).unwrap().work[0]
            .attempts
            .len(),
        1
    );
}
#[test]
fn workflow_retry_never_replays_child_work_unknown_outcomes_or_stopped_runs() {
    let f = Fixture::new();
    let a = start(&f, retry_workflow(&f));
    let first = stage_claim(&f, &a, 101);
    f.store
        .delegate(&first.lease, f.delegation(1), 102)
        .unwrap();
    f.store
        .report(
            &first.lease,
            AssignmentOutcome::FailedBeforeEffects("misclassified failure".into()),
            103,
        )
        .unwrap();
    tick(&f, 104);
    let blocked = f.store.automation_run(a.id).unwrap();
    assert_eq!(blocked.status, AutomationStatus::Blocked);
    assert!(blocked.reason.unwrap().contains("prior effects"));
    assert_eq!(
        f.store.snapshot(first.lease.run_id()).unwrap().work[0].status,
        AssignmentStatus::Failed
    );
    let f = Fixture::new();
    let a = start(&f, retry_workflow(&f));
    let first = stage_claim(&f, &a, 101);
    f.store
        .report(
            &first.lease,
            AssignmentOutcome::Uncertain("Lost host acknowledgement".into()),
            102,
        )
        .unwrap();
    tick(&f, 103);
    assert_eq!(
        f.store.automation_run(a.id).unwrap().status,
        AutomationStatus::Blocked
    );
    let f = Fixture::new();
    let a = start(&f, retry_workflow(&f));
    let first = stage_claim(&f, &a, 101);
    f.store
        .report(
            &first.lease,
            AssignmentOutcome::FailedBeforeEffects("failed".into()),
            102,
        )
        .unwrap();
    tick(&f, 103);
    let waiting = f.store.automation_run(a.id).unwrap();
    f.store
        .stop_automation(waiting.id, waiting.revision, 104)
        .unwrap();
    tick(&f, 105);
    assert_eq!(
        f.store.automation_run(a.id).unwrap().status,
        AutomationStatus::Canceled
    );
    assert_ne!(
        f.store.snapshot(first.lease.run_id()).unwrap().work[0].status,
        AssignmentStatus::Queued
    );
}
#[test]
fn workflow_retry_is_opt_in_and_invalid_policy_cannot_change_definition_revision() {
    let f = Fixture::new();
    let w = workflow(&f, &[false]);
    let wire = serde_json::to_value(&w.stages[0]).unwrap();
    assert!(wire.get("retry").is_none());
    assert!(
        serde_json::from_value::<WorkflowStage>(wire)
            .unwrap()
            .retry
            .is_none()
    );
    let a = start(&f, w);
    let first = stage_claim(&f, &a, 101);
    f.store
        .report(
            &first.lease,
            AssignmentOutcome::FailedBeforeEffects("failed".into()),
            102,
        )
        .unwrap();
    tick(&f, 103);
    assert_eq!(
        f.store.automation_run(a.id).unwrap().status,
        AutomationStatus::Blocked
    );
    for policy in [
        WorkflowRetryPolicy {
            max_attempts: 1,
            delay_seconds: 2,
        },
        WorkflowRetryPolicy {
            max_attempts: 4,
            delay_seconds: 2,
        },
        WorkflowRetryPolicy {
            max_attempts: 2,
            delay_seconds: 0,
        },
        WorkflowRetryPolicy {
            max_attempts: 2,
            delay_seconds: 301,
        },
    ] {
        let mut w = workflow(&f, &[false]);
        w.stages[0].retry = Some(policy);
        let before = f.definitions.load().unwrap().revision;
        assert!(
            f.definitions
                .save(before, ControlRecord::Workflow(w))
                .is_err()
        );
        assert_eq!(f.definitions.load().unwrap().revision, before);
    }
}
