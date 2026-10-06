use super::*;
use waku_protocol::model::QueuedMessage;

fn checkpoint() -> String {
    serde_json::json!({"version":1,"steps":1,"messages":[{"role":"user","content":"Pinned objective"}],"pending":[]}).to_string()
}
fn queue(f: &Fixture, run: &TeamRunSnapshot, message: &QueuedMessage) -> TeamRunSnapshot {
    f.store
        .queue_followup(run.id, run.root_task_id, message.clone(), 103)
        .unwrap()
}

#[test]
fn team_followup_queue_is_exact_durable_and_never_expands_run_authority() {
    let f = Fixture::new();
    let run = f.run();
    let message = QueuedMessage::new("Include the API evidence");
    let queued = queue(&f, &run, &message);
    assert_eq!(queued.follow_ups.len(), 1);
    assert_eq!(queued.follow_ups[0].message.id, message.id);
    assert_eq!(queued.follow_ups[0].message.created_at, 103);
    assert_eq!(queued.follow_ups[0].status, TeamFollowUpStatus::Queued);
    assert_eq!(queued.access, run.access);
    assert_eq!(
        serde_json::to_value(&queued.limits).unwrap(),
        serde_json::to_value(&run.limits).unwrap()
    );
    assert_eq!(queued.usage, run.usage);
    assert_eq!(
        serde_json::to_value(&queued.profiles).unwrap(),
        serde_json::to_value(&run.profiles).unwrap()
    );
    assert_eq!(queued.deadline, run.deadline);
    assert!(queued.work[0].attempts.is_empty());
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    assert_eq!(
        reopened
            .queue_followup(run.id, run.root_task_id, message.clone(), 104)
            .unwrap()
            .revision,
        queued.revision
    );
    let mut changed = message.clone();
    changed.content = "Changed text".into();
    assert!(
        reopened
            .queue_followup(run.id, run.root_task_id, changed, 104)
            .is_err()
    );
    assert!(
        reopened
            .queue_followup(run.id, Uuid::new_v4(), message, 104)
            .is_err()
    );
}

#[test]
fn team_followup_queue_has_text_count_harness_budget_and_terminal_bounds() {
    let f = Fixture::new();
    let run = f.run();
    for invalid in [
        QueuedMessage::new(" "),
        QueuedMessage::new("x".repeat(4097)),
        QueuedMessage::new("\u{0}".repeat(2000)),
    ] {
        assert!(
            f.store
                .queue_followup(run.id, run.root_task_id, invalid, 103)
                .is_err()
        );
    }
    let mut invalid = QueuedMessage::new("Text");
    invalid.id = Uuid::nil();
    assert!(
        f.store
            .queue_followup(run.id, run.root_task_id, invalid, 103)
            .is_err()
    );
    let mut invalid = QueuedMessage::new("Text");
    invalid.display_content = Some("Different display".into());
    assert!(
        f.store
            .queue_followup(run.id, run.root_task_id, invalid, 103)
            .is_err()
    );
    for _ in 0..8 {
        queue(&f, &run, &QueuedMessage::new("More evidence"));
    }
    assert!(
        f.store
            .queue_followup(run.id, run.root_task_id, QueuedMessage::new("Ninth"), 104)
            .is_err()
    );
    let latest = f.store.snapshot(run.id).unwrap();
    let stopped = f.store.stop(run.id, latest.revision, 104).unwrap();
    assert!(
        stopped
            .follow_ups
            .iter()
            .all(|f| f.status == TeamFollowUpStatus::Canceled)
    );
    assert!(
        f.store
            .queue_followup(
                run.id,
                run.root_task_id,
                QueuedMessage::new("After stop"),
                105
            )
            .is_err()
    );

    let f = Fixture::new();
    let mut request = f.request();
    request.limits.max_model_calls = 1;
    let run = f.store.prepare(request, 100).unwrap();
    let root = f.claim(run.id, run.work[0].id);
    f.store
        .reserve_model_call(
            &root.lease,
            1,
            ModelCallAllowance {
                model: "model-0".into(),
                max_input_tokens: 100,
                max_output_tokens: 10,
            },
            102,
        )
        .unwrap();
    assert!(
        f.store
            .queue_followup(
                run.id,
                run.root_task_id,
                QueuedMessage::new("Budget reset"),
                103
            )
            .is_err()
    );

    let f = Fixture::new();
    let mut catalog = f.definitions.load().unwrap();
    let index = catalog
        .records
        .iter()
        .position(|r| matches!(r, ControlRecord::Agent(p) if p.id == f.agents[0]))
        .unwrap();
    let ControlRecord::Agent(mut profile) = catalog.records.remove(index) else {
        panic!("profile")
    };
    profile.provider = ProviderKind::Codex;
    f.definitions
        .save(catalog.revision, ControlRecord::Agent(profile))
        .unwrap();
    let run = f.run();
    assert!(
        f.store
            .queue_followup(
                run.id,
                run.root_task_id,
                QueuedMessage::new("No adapter"),
                103
            )
            .is_err()
    );
}

#[test]
fn team_followup_delivery_is_atomic_lease_bound_and_blocks_premature_completion() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let original = checkpoint();
    f.store
        .save_recoverable_runtime_checkpoint(&root.lease, &original, true, 102)
        .unwrap();
    let queued = queue(&f, &run, &QueuedMessage::new("Include the API evidence"));
    assert!(
        f.store
            .report(&root.lease, AssignmentOutcome::Completed(result()), 104)
            .is_err()
    );
    assert!(f.store.deliver_followups(&root.lease, "{}", 104).is_err());
    assert_eq!(
        f.store.snapshot(run.id).unwrap().follow_ups[0].status,
        TeamFollowUpStatus::Queued
    );
    let (updated, delivered) = f
        .store
        .deliver_followups(&root.lease, &original, 104)
        .unwrap()
        .unwrap();
    assert!(updated.contains("Include the API evidence"));
    assert_eq!(delivered[0].attempt_id, Some(root.lease.attempt_id()));
    assert_eq!(delivered[0].status, TeamFollowUpStatus::Delivered);
    assert_eq!(f.store.snapshot(run.id).unwrap().usage, queued.usage);
    assert_eq!(
        continuation::checkpoint_row(&f.store.connect().unwrap(), root.assignment.id)
            .unwrap()
            .unwrap()
            .3,
        updated
    );
    assert!(
        f.store
            .deliver_followups(&root.lease, &updated, 104)
            .unwrap()
            .is_none()
    );
    let done = f
        .store
        .report(&root.lease, AssignmentOutcome::Completed(result()), 105)
        .unwrap();
    assert_eq!(done.status, TeamRunStatus::Completed);
    assert_eq!(done.work[0].attempts.len(), 1);
    assert!(
        f.store
            .queue_followup(
                run.id,
                run.root_task_id,
                QueuedMessage::new("New objective"),
                106
            )
            .is_err()
    );
}

#[test]
fn team_followup_preflight_does_not_deliver_between_tool_call_and_receipt() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let pending=serde_json::json!({"version":1,"steps":1,"messages":[{"role":"assistant","tool_calls":[{"id":"call"}]}],"pending":[{"id":"call","name":"read_file","arguments":{"path":"README.md"},"request_id":Uuid::new_v4(),"activity_id":"activity"}]}).to_string();
    f.store
        .save_runtime_checkpoint(&root.lease, &pending, 102)
        .unwrap();
    queue(&f, &run, &QueuedMessage::new("More evidence"));
    assert!(
        f.store
            .deliver_followups(&root.lease, &pending, 104)
            .is_err()
    );
    assert_eq!(
        continuation::checkpoint_row(&f.store.connect().unwrap(), root.assignment.id)
            .unwrap()
            .unwrap()
            .3,
        pending
    );
    assert_eq!(
        f.store.snapshot(run.id).unwrap().follow_ups[0].status,
        TeamFollowUpStatus::Queued
    );
    let exhausted =
        serde_json::json!({"version":1,"steps":16,"messages":[],"pending":[]}).to_string();
    f.store
        .save_runtime_checkpoint(&root.lease, &exhausted, 104)
        .unwrap();
    assert!(
        f.store
            .queue_followup(run.id, run.root_task_id, QueuedMessage::new("More"), 105)
            .is_err()
    );
}

#[test]
fn team_followup_on_blocked_lead_waits_for_user_resume_and_keeps_the_attempt() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let original = checkpoint();
    f.store
        .save_recoverable_runtime_checkpoint(&root.lease, &original, true, 102)
        .unwrap();
    f.store
        .report(
            &root.lease,
            AssignmentOutcome::Blocked("Temporary connection failure".into()),
            103,
        )
        .unwrap();
    let queued = queue(&f, &run, &QueuedMessage::new("Include a source link"));
    assert_eq!(queued.work[0].status, AssignmentStatus::Blocked);
    assert!(queued.work[0].can_resume);
    assert!(
        f.store
            .resume_after_children(run.id, root.assignment.id, 104, 100)
            .is_err()
    );
    f.store
        .queue_checkpoint_resume(run.id, root.assignment.id, queued.revision, 104)
        .unwrap();
    let resumed = f
        .store
        .resume_after_children(run.id, root.assignment.id, 105, 100)
        .unwrap();
    assert!(
        f.store
            .deliver_followups(&root.lease, &original, 105)
            .is_err()
    );
    // The native adapter persists its resumed lease before entering its model loop.
    f.store
        .save_runtime_checkpoint(&resumed.claimed.lease, &resumed.checkpoint, 105)
        .unwrap();
    let (updated, _) = f
        .store
        .deliver_followups(&resumed.claimed.lease, &resumed.checkpoint, 105)
        .unwrap()
        .unwrap();
    assert!(updated.contains("Include a source link"));
    assert_eq!(resumed.claimed.lease.attempt_id(), root.lease.attempt_id());
    assert_eq!(resumed.claimed.assignment.access, root.assignment.access);
    assert_eq!(resumed.claimed.profile.model, root.profile.model);
}

#[test]
fn team_followup_does_not_approve_a_decision_and_deadline_cancels_queue() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    f.store
        .suspend_for_decision(
            &root.lease,
            Uuid::new_v4(),
            "Choose output",
            "Choose report format",
            &checkpoint(),
            102,
        )
        .unwrap();
    assert!(
        f.store
            .queue_followup(run.id, run.root_task_id, QueuedMessage::new("Approve"), 103)
            .is_err()
    );
    assert!(f.store.snapshot(run.id).unwrap().follow_ups.is_empty());
    let f = Fixture::new();
    let run = f.run();
    let message = QueuedMessage::new("Queued evidence");
    queue(&f, &run, &message);
    let expired = f.store.reconcile_expired(run.id, run.deadline).unwrap();
    assert_eq!(expired.follow_ups[0].status, TeamFollowUpStatus::Canceled);
    assert!(
        f.store
            .queue_followup(
                run.id,
                run.root_task_id,
                QueuedMessage::new("Too late"),
                run.deadline
            )
            .is_err()
    );
    assert_eq!(
        f.store
            .queue_followup(run.id, run.root_task_id, message, run.deadline)
            .unwrap()
            .follow_ups
            .len(),
        1
    );
}

#[test]
fn team_followup_transcript_outbox_recovers_without_duplicate_messages_or_turns() {
    use crate::{
        model::Project,
        persistence::{PersistedState, StateStore},
    };
    let f = Fixture::new();
    let mut request = f.request();
    request.source_prompt = Some(format!("@team:{} {}", f.team, request.objective));
    let run = f.store.queue_task(request, 100).unwrap();
    let store = StateStore::daemon(f.dir.join("tasks.db"));
    let mut state = PersistedState::empty();
    state.projects.push(Project {
        id: f.project,
        name: "Project".into(),
        path: f.dir.clone(),
        created_at: 100,
    });
    f.store
        .materialize_run(run.id, &store, &mut state, 102)
        .unwrap();
    let task = state.session_mut(run.root_task_id).unwrap();
    let turn = task.begin_provider_turn();
    task.messages[0].turn_id = Some(turn);
    let message = QueuedMessage::new("Include the API evidence");
    queue(&f, &run, &message);
    assert_eq!(
        f.store
            .materialize_followups(run.id, &store, &mut state)
            .unwrap(),
        1
    );
    // Simulate a crash after task save, before outbox acknowledgement.
    f.store
        .connect()
        .unwrap()
        .execute(
            "UPDATE ion_team_followups SET materialized=0 WHERE run_id=?",
            [run.id.to_string()],
        )
        .unwrap();
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    let mut loaded = store.load().unwrap();
    assert_eq!(
        reopened
            .materialize_followups(run.id, &store, &mut loaded)
            .unwrap(),
        0
    );
    let task = loaded.session_mut(run.root_task_id).unwrap();
    store.hydrate(task).unwrap();
    assert_eq!(task.messages.len(), 2);
    assert_eq!(task.messages[1].id, message.id);
    assert_eq!(task.messages[1].turn_id, Some(turn));
    assert_eq!(task.turns.len(), 1);
    assert_eq!(
        reopened
            .materialize_followups(run.id, &store, &mut loaded)
            .unwrap(),
        0
    );
}

#[test]
fn team_followup_initial_context_bound_is_enforced_before_the_worker_starts() {
    let f = Fixture::new();
    let run = f.run();
    let mut accepted = 0;
    for _ in 0..8 {
        let message = QueuedMessage::new("\n".repeat(4090) + "x");
        match f
            .store
            .queue_followup(run.id, run.root_task_id, message, 103)
        {
            Ok(_) => accepted += 1,
            Err(error) => {
                assert!(error.to_string().contains("context limit"));
                break;
            }
        }
    }
    assert!(accepted > 0 && accepted < 8);
    assert_eq!(f.store.snapshot(run.id).unwrap().follow_ups.len(), accepted);
}
