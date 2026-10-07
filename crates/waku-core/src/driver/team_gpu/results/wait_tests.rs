use super::super::tests::{bytes, client, job};
use super::tests::{receipt, setup, volume};
use super::*;
use crate::driver::team_test_http::Server;
use crate::teams::gpu_jobs::{GpuComputeInventory, GpuDiskInventory};
use waku_protocol::teams::{AssignmentStatus, TeamRunStatus};
fn running() -> (super::super::tests::Fixture, Launch) {
    let (f, l) = setup(false);
    f.coordinator
        .record_gpu_observation(
            l.request.request_id,
            "aijob-fixture",
            "RUNNING",
            &[],
            crate::model::unix_time(),
        )
        .unwrap();
    (f, l)
}
fn finish_with_proofs(f: &super::super::tests::Fixture, l: &Launch) {
    finish_cleanup(f, l);
    f.coordinator
        .record_gpu_result(
            l.request.request_id,
            "aijob-fixture",
            &receipt(&l.request),
            crate::model::unix_time(),
        )
        .unwrap();
}
pub(super) fn finish_cleanup(f: &super::super::tests::Fixture, l: &Launch) {
    let now = crate::model::unix_time();
    f.coordinator
        .record_gpu_observation(
            l.request.request_id,
            "aijob-fixture",
            "COMPLETED",
            &["compute-fixture".into()],
            now,
        )
        .unwrap();
    f.coordinator
        .record_gpu_inventory(
            l.request.request_id,
            "aijob-fixture",
            &GpuComputeInventory {
                compute_id: "compute-fixture".into(),
                disks: vec![GpuDiskInventory {
                    id: "disk-fixture".into(),
                    managed: true,
                }],
                complete: true,
            },
            now,
        )
        .unwrap();
    let claim = f
        .coordinator
        .claim_gpu_deletion(l.request.request_id, "aijob-fixture", now)
        .unwrap()
        .unwrap();
    f.coordinator
        .bind_gpu_deletion(
            l.request.request_id,
            "aijob-fixture",
            "operation-delete",
            now,
        )
        .unwrap();
    let ack = Acknowledged {
        operation: "operation-create".into(),
        job: "aijob-fixture".into(),
    };
    let deletion = json!({"id":"operation-delete","resourceId":"aijob-fixture","status":{},
        "request":{"@type":"type.googleapis.com/nebius.ai.v1.DeleteJobRequest","id":"aijob-fixture"},
        "requestHeaders":{"x-idempotency-key":{"values":[claim.key.to_string()]}}});
    let absent = (404, bytes(json!({"code":5})));
    let server = Server::new(vec![
        (200, bytes(deletion)),
        absent.clone(),
        absent.clone(),
        absent,
    ]);
    let mut bound = claim;
    bound.operation = Some("operation-delete".into());
    assert!(client(&server).cleanup_proven(&ack, &bound).unwrap());
    f.coordinator
        .verify_gpu_cleanup(l.request.request_id, "operation-delete", &bound, now)
        .unwrap();
    assert_eq!(server.finish().len(), 4);
}
fn checkpoint() -> &'static str {
    "{\"version\":1,\"purpose\":\"GPU wait fixture\"}"
}
#[test]
fn team_gpu_wait_releases_agent_slot_fences_old_generation_and_replays_exact_checkpoint_after_restart()
 {
    let (f, l) = running();
    let now = crate::model::unix_time();
    let waiting = f
        .coordinator
        .suspend_for_gpu(&f.lease, l.request.request_id, checkpoint(), now)
        .unwrap();
    assert_eq!(waiting.work[0].status, AssignmentStatus::Waiting);
    assert!(!waiting.work[0].status.holds_capacity());
    let db = rusqlite::Connection::open(f.dir.join("control.db")).unwrap();
    let live: i32 = db
        .query_row(
            "SELECT live FROM ion_team_attempts WHERE id=?",
            [f.lease.attempt_id().to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(live, 0);
    assert!(f.coordinator.renew(&f.lease, now, 180).is_err());
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    let same = reopened
        .suspend_for_gpu(&f.lease, l.request.request_id, checkpoint(), now)
        .unwrap();
    assert_eq!(same.revision, waiting.revision);
    assert!(
        reopened
            .suspend_for_gpu(&f.lease, Uuid::new_v4(), checkpoint(), now)
            .is_err()
    );
    assert!(
        reopened
            .suspend_for_gpu(&f.lease, l.request.request_id, "{}", now)
            .is_err()
    );
    assert!(
        reopened
            .resume_after_children(f.lease.run_id(), f.lease.work_id(), now, 180)
            .is_err()
    );
    assert!(reopened.has_pending_effect(f.lease.run_id()).unwrap());
    drop(db);
}
#[test]
fn team_gpu_wait_cannot_park_unknown_unapproved_cross_attempt_or_other_unfinished_effect() {
    let (f, l) = setup(false);
    let now = crate::model::unix_time();
    assert!(
        f.coordinator
            .suspend_for_gpu(&f.lease, l.request.request_id, checkpoint(), now)
            .is_err()
    );
    let (f, l) = running();
    let other = super::super::tests::Fixture::new(true, true);
    assert!(
        f.coordinator
            .authorize_gpu_wait(&other.lease, l.request.request_id, now)
            .is_err()
    );
    assert!(
        f.coordinator
            .authorize_gpu_wait(&f.lease, Uuid::new_v4(), now)
            .is_err()
    );
    // An interrupted workspace mutation in this run must remain a global fence.
    let db = rusqlite::Connection::open(f.dir.join("control.db")).unwrap();
    db.execute("INSERT INTO ion_team_workspace_edits(id,run_id,work_id,attempt_id,request_id,signature,state) VALUES(?,?,?,?,?,?,'pending')",
        rusqlite::params![Uuid::new_v4().to_string(),f.lease.run_id().to_string(),f.lease.work_id().to_string(),f.lease.attempt_id().to_string(),Uuid::new_v4().to_string(),"fixture" ]).unwrap();
    assert!(
        f.coordinator
            .authorize_gpu_wait(&f.lease, l.request.request_id, now)
            .is_err()
    );
    assert!(
        f.coordinator
            .suspend_for_gpu(&f.lease, l.request.request_id, checkpoint(), now)
            .is_err()
    );
    drop(db);
}
#[test]
fn team_gpu_wait_stop_and_deadline_keep_resources_uncertain_until_proven_stopped() {
    for stop in [true, false] {
        let (f, l) = running();
        let now = crate::model::unix_time();
        let waiting = f
            .coordinator
            .suspend_for_gpu(&f.lease, l.request.request_id, checkpoint(), now)
            .unwrap();
        let stopped = if stop {
            f.coordinator
                .stop(waiting.id, waiting.revision, now)
                .unwrap()
        } else {
            f.coordinator
                .reconcile_expired(waiting.id, waiting.deadline)
                .unwrap()
        };
        assert_eq!(stopped.work[0].status, AssignmentStatus::Uncertain);
        if stop {
            assert_eq!(stopped.status, TeamRunStatus::Stopping);
        }
        assert!(
            !f.coordinator
                .remote_attempt_stopped(f.lease.attempt_id())
                .unwrap()
        );
        assert!(
            f.coordinator
                .resume_after_children(waiting.id, f.lease.work_id(), now, 180)
                .is_err()
        );
        assert!(f.coordinator.has_pending_effect(waiting.id).unwrap());
    }
}
#[test]
fn team_gpu_wait_active_observation_does_not_cancel_but_deadline_mode_claims_cancel_once() {
    let (f, l) = running();
    let api = Server::new(vec![
        (200, bytes(job(&l, "RUNNING"))),
        (200, bytes(job(&l, "RUNNING"))),
        (
            200,
            bytes(json!({"id":"operation-cancel","resourceId":"aijob-fixture"})),
        ),
        (200, bytes(job(&l, "RUNNING"))),
    ]);
    let c = client(&api);
    let record = f
        .coordinator
        .gpu_jobs(f.lease.attempt_id())
        .unwrap()
        .remove(0);
    assert!(!advance_record(&c, &f.coordinator, &record, false).unwrap());
    assert!(!f.coordinator.gpu_jobs(f.lease.attempt_id()).unwrap()[0].cancel_requested);
    assert!(!advance_record(&c, &f.coordinator, &record, true).unwrap());
    let saved = f
        .coordinator
        .gpu_jobs(f.lease.attempt_id())
        .unwrap()
        .remove(0);
    assert!(saved.cancel_requested);
    assert!(!advance_record(&c, &f.coordinator, &saved, true).unwrap());
    let requests = api.finish();
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.line.starts_with("POST "))
            .count(),
        1
    );
    assert!(requests[2].line.starts_with("POST /ai/v1/jobs/cancel "));
}
#[test]
fn team_gpu_wait_real_receipt_and_cleanup_resume_once_with_original_host_model_and_spending() {
    use crate::driver::{NativeTeamExit, NativeTeamStart, nebius::team::run_with_remote_harness};
    use crate::teams::{ClaimedAssignment, ModelCallAllowance, ModelTokenUsage};
    use std::sync::atomic::AtomicBool;
    let mut f = super::super::tests::Fixture::new(true, true);
    let mut l = super::super::tests::launch();
    l.request.output_paths = vec!["result.bin".into()];
    l.result_volume = Some(volume());
    f.policy.result_volume = l.result_volume.clone();
    let now = crate::model::unix_time();
    let original = f.coordinator.snapshot(f.lease.run_id()).unwrap();
    let start = ClaimedAssignment {
        lease: f.lease.clone(),
        assignment: original.work[0].clone(),
        profile: original.profiles[0].clone(),
    };
    let (events, _) = crate::driver::test_event_channel();
    let native=run_with_remote_harness(&f.coordinator,NativeTeamStart::Fresh(start),&f.dir,&events,&AtomicBool::new(false),original.profiles[0].provider,
        |model|Ok(ModelCallAllowance{model:model.to_owned(),max_input_tokens:32768,max_output_tokens:8192}),
        |model,_,_,tools| {
            assert_eq!(model,"fixture");
            // The job is launched after this completion is reserved. An already
            // pending GPU effect must still prevent a fresh model reservation.
            f.coordinator.reserve_gpu_launch(&f.lease,&l.request,&l.body().unwrap(),&f.policy,now).unwrap();
            f.coordinator.bind_gpu_acknowledgement(l.request.request_id,"operation-create","aijob-fixture",now).unwrap();
            f.coordinator.record_gpu_observation(l.request.request_id,"aijob-fixture","RUNNING",&[],now).unwrap();
            crate::driver::team_completion::actions(&json!({"text":"","calls":[{"name":"await_gpu_job","arguments":{"request_id":l.request.request_id}}]}).to_string(),
                &tools,ModelTokenUsage{input_tokens:100,output_tokens:5})
        }).unwrap();
    assert_eq!(native, NativeTeamExit::Yielded);
    assert!(
        !f.coordinator
            .settle_gpu_result(l.request.request_id, now)
            .unwrap()
    );
    let waiting = f.coordinator.snapshot(original.id).unwrap();
    assert_eq!(waiting.work[0].attempts.len(), 1);
    finish_with_proofs(&f, &l);
    // Cleanup alone keeps the runtime fence until the explicit verified settlement.
    assert!(f.coordinator.has_pending_effect(original.id).unwrap());
    assert!(
        f.coordinator
            .settle_gpu_result(l.request.request_id, now)
            .unwrap()
    );
    assert!(!f.coordinator.has_pending_effect(original.id).unwrap());
    let revision = f.coordinator.snapshot(original.id).unwrap().revision;
    assert!(progress_wait(&f.coordinator, f.lease.work_id()).unwrap()); // Saved proofs, no environment credentials or network.
    assert_eq!(
        f.coordinator.snapshot(original.id).unwrap().revision,
        revision
    );
    let resumed = TeamCoordinator::new(f.dir.join("control.db"))
        .resume_after_children(original.id, f.lease.work_id(), now, 180)
        .unwrap();
    assert_eq!(resumed.claimed.lease.attempt_id(), f.lease.attempt_id());
    assert_eq!(resumed.claimed.profile.model, Some("fixture".into()));
    assert_eq!(
        resumed.claimed.assignment.attempts[0].host,
        original.work[0].attempts[0].host
    );
    assert!(
        resumed
            .gpu_result
            .as_ref()
            .unwrap()
            .contains("\"exit_code\":7")
    );
    assert!(
        f.coordinator
            .resume_after_children(original.id, f.lease.work_id(), now, 180)
            .is_err()
    );
    let original_lease = f.lease.clone();
    assert!(f.coordinator.renew(&original_lease, now, 180).is_err());
    let result=run_with_remote_harness(&f.coordinator,NativeTeamStart::Resumed(resumed),&f.dir,&events,&AtomicBool::new(false),original.profiles[0].provider,
        |model|Ok(ModelCallAllowance{model:model.to_owned(),max_input_tokens:32768,max_output_tokens:8192}),
        |model,_,messages,tools| {
            assert_eq!(model,"fixture");
            assert!(messages.iter().any(|m|m["content"].as_str().is_some_and(|s|s.contains("\"exit_code\":7") && s.contains("GPU command"))));
            let artifact=f.coordinator.gpu_jobs(f.lease.attempt_id()).unwrap()[0].result_artifact.unwrap();
            crate::driver::team_completion::actions(&json!({"text":"","calls":[{"name":"report_result","arguments":{"summary":"Training failed with its authentic command receipt; output retained for review","artifact_ids":[artifact]}}]}).to_string(),
                &tools,ModelTokenUsage{input_tokens:100,output_tokens:5})
        }).unwrap();
    assert_eq!(result, NativeTeamExit::Settled);
    assert_eq!(
        f.coordinator.snapshot(original.id).unwrap().status,
        TeamRunStatus::Completed
    );
    let db = rusqlite::Connection::open(f.dir.join("control.db")).unwrap();
    let (money, state): (u64, String) = db
        .query_row(
            "SELECT reserved_micros,state FROM ion_team_gpu_jobs WHERE request_id=?",
            [l.request.request_id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(money, f.policy.reservation_micros);
    assert_eq!(state, "settled");
    drop(db);
}
