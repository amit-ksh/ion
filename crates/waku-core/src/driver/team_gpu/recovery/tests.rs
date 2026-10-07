use super::super::tests::{Fixture, bytes, client, job, launch};
use super::*;
use crate::driver::team_test_http::Server;
use crate::teams::gpu_jobs::{GpuComputeInventory, GpuDiskInventory};

fn reserve(f: &Fixture, l: &Launch) {
    f.coordinator
        .reserve_gpu_launch(
            &f.lease,
            &l.request,
            &l.body().unwrap(),
            &f.policy,
            crate::model::unix_time(),
        )
        .unwrap();
}
fn revision(f: &Fixture) -> u64 {
    f.coordinator.snapshot(f.lease.run_id()).unwrap().revision
}
fn unknown(f: &Fixture) {
    f.coordinator
        .reconcile_owner_restart(crate::model::unix_time())
        .unwrap();
}
fn bind(
    c: &Client,
    f: &Fixture,
    l: &Launch,
    rev: u64,
    purpose: Purpose,
    operation: &str,
) -> Result<()> {
    bind_with(
        c,
        &f.coordinator,
        f.lease.run_id(),
        f.lease.work_id(),
        f.lease.attempt_id(),
        rev,
        l.request.request_id,
        purpose,
        operation,
    )
}
fn create(l: &Launch) -> Value {
    let mut request = l.body().unwrap();
    request["@type"] = json!("type.googleapis.com/nebius.ai.v1.CreateJobRequest");
    json!({"id":"operation-recovered","resourceId":"aijob-fixture","request":request,
        "requestHeaders":{"x-idempotency-key":{"values":[l.request.request_id.to_string()]}}})
}
fn operation(purpose: Purpose, key: Uuid) -> Value {
    json!({"id":"operation-recovered","resourceId":"aijob-fixture",
        "request":{"@type":format!("type.googleapis.com/nebius.ai.v1.{}JobRequest",match purpose {Purpose::Creation=>"Create",Purpose::Cancellation=>"Cancel",Purpose::Deletion=>"Delete"}),"id":"aijob-fixture"},
        "requestHeaders":{"x-idempotency-key":{"values":[key.to_string()]}}})
}
fn held_money(f: &Fixture) -> i64 {
    rusqlite::Connection::open(f.dir.join("control.db"))
        .unwrap()
        .query_row(
            "SELECT SUM(reserved_micros) FROM ion_team_gpu_jobs",
            [],
            |r| r.get(0),
        )
        .unwrap()
}
fn bound(f: &Fixture, l: &Launch) {
    reserve(f, l);
    f.coordinator
        .bind_gpu_acknowledgement(
            l.request.request_id,
            "operation-create",
            "aijob-fixture",
            crate::model::unix_time(),
        )
        .unwrap();
}
#[test]
fn team_gpu_recovery_redacted_creation_preserves_identity_resources_money_and_exact_replay() {
    let f = Fixture::new(true, true);
    let l = launch();
    reserve(&f, &l);
    unknown(&f);
    let mut op = create(&l);
    for entry in op["request"]["spec"]["environmentVariables"]
        .as_array_mut()
        .unwrap()
    {
        entry.as_object_mut().unwrap().remove("value");
    }
    op["request"]["spec"]["injectedFiles"][0]["content"] = json!("***");
    for key in ["publicIp", "preemptible", "restartAttempts", "shmSizeBytes"] {
        op["request"]["spec"].as_object_mut().unwrap().remove(key);
    }
    op["request"]["dryRun"] = json!(false);
    let mut observed = job(&l, "RUNNING");
    observed["status"]["instances"] = json!([{"computeInstanceId":"compute-fixture"}]);
    let server = Server::new(vec![(200, bytes(op)), (200, bytes(observed))]);
    let c = client(&server);
    let rev = revision(&f);
    bind(&c, &f, &l, rev, Purpose::Creation, "operation-recovered").unwrap();
    let record = f
        .coordinator
        .gpu_jobs(f.lease.attempt_id())
        .unwrap()
        .remove(0);
    assert_eq!(record.job_id.as_deref(), Some("aijob-fixture"));
    assert_eq!(record.compute_ids, vec!["compute-fixture"]);
    assert_eq!(held_money(&f), 10000);
    assert!(f.coordinator.has_pending_effect(f.lease.run_id()).unwrap());
    let after = revision(&f);
    bind(&c, &f, &l, rev, Purpose::Creation, "operation-recovered").unwrap();
    assert_eq!(revision(&f), after);
    assert!(bind(&c, &f, &l, rev, Purpose::Creation, "different-operation").is_err());
    let (review, _) = f
        .coordinator
        .recovery_review(f.lease.run_id(), f.lease.work_id())
        .unwrap();
    let projected = serde_json::to_string(&review).unwrap();
    assert!(
        !projected.contains("injectedFiles")
            && !projected.contains("gpu fixture")
            && !projected.contains("ION_JOB_CHILD")
    );
    assert!(!review.gpu_jobs[0].can_bind_creation && !review.can_prepare_continuation);
    let requests = server.finish();
    assert_eq!(requests.len(), 2);
    assert!(requests.iter().all(|r| r.line.starts_with("GET ")));
    assert!(requests[1].line.contains("view=SECRET"));
}
#[test]
fn team_gpu_recovery_create_rejects_missing_key_wrong_type_and_changed_intent_before_secret_read() {
    for case in [
        "missing-key",
        "wrong-key",
        "ambiguous-key",
        "type",
        "dry-run",
        "extra",
        "path",
        "platform",
        "missing-input",
    ] {
        let f = Fixture::new(true, true);
        let l = launch();
        reserve(&f, &l);
        unknown(&f);
        let mut op = create(&l);
        match case {
            "missing-key" => {
                op.as_object_mut().unwrap().remove("requestHeaders");
            }
            "wrong-key" => {
                op["requestHeaders"]["x-idempotency-key"]["values"] =
                    json!([Uuid::new_v4().to_string()])
            }
            "ambiguous-key" => {
                op["requestHeaders"]["x-idempotency-key"]["values"] = json!([
                    l.request.request_id.to_string(),
                    l.request.request_id.to_string()
                ])
            }
            "type" => {
                op["request"]["@type"] = json!("type.googleapis.com/nebius.ai.v1.DeleteJobRequest")
            }
            "dry-run" => op["request"]["dryRun"] = json!(true),
            "extra" => op["request"]["spec"]["sshAuthorizedKeys"] = json!(["unapproved"]),
            "path" => {
                op["request"]["spec"]["injectedFiles"][0]["containerPath"] =
                    json!("/different/request.json")
            }
            "platform" => op["request"]["spec"]["platform"] = json!("different-gpu"),
            "missing-input" => {
                op["request"]["spec"]
                    .as_object_mut()
                    .unwrap()
                    .remove("injectedFiles");
            }
            _ => unreachable!(),
        }
        let server = Server::new(vec![(200, bytes(op))]);
        let c = client(&server);
        let rev = revision(&f);
        assert!(
            bind(&c, &f, &l, rev, Purpose::Creation, "operation-recovered").is_err(),
            "{case}"
        );
        assert_eq!(revision(&f), rev);
        assert_eq!(held_money(&f), 10000);
        assert!(
            f.coordinator.gpu_jobs(f.lease.attempt_id()).unwrap()[0]
                .job_id
                .is_none()
        );
        assert_eq!(server.finish().len(), 1);
    }
}
#[test]
fn team_gpu_recovery_requires_actual_immutable_secret_bytes_and_fixed_environment() {
    for case in ["input", "environment", "project", "capability", "masked"] {
        let f = Fixture::new(true, true);
        let l = launch();
        reserve(&f, &l);
        unknown(&f);
        let op = create(&l);
        let mut observed = job(&l, "RUNNING");
        match case {
            "input" => {
                observed["spec"]["injectedFiles"][0]["content"] =
                    json!(STANDARD.encode(b"different request"))
            }
            "environment" => observed["spec"]["environmentVariables"][1]["value"] = json!("0"),
            "project" => observed["metadata"]["parentId"] = json!("different-project"),
            "capability" => observed["spec"]["ports"] = json!([{"containerPort":8080}]),
            "masked" => observed["spec"]["injectedFiles"][0]["content"] = json!("***"),
            _ => unreachable!(),
        }
        let server = Server::new(vec![(200, bytes(op)), (200, bytes(observed))]);
        let c = client(&server);
        let rev = revision(&f);
        assert!(
            bind(&c, &f, &l, rev, Purpose::Creation, "operation-recovered").is_err(),
            "{case}"
        );
        assert_eq!(revision(&f), rev);
        assert_eq!(held_money(&f), 10000);
        assert_eq!(server.finish().len(), 2);
    }
}
#[test]
fn team_gpu_recovery_cancel_failure_binds_only_original_claim_without_stopping_or_resubmitting() {
    let f = Fixture::new(true, true);
    let l = launch();
    bound(&f, &l);
    let key = f
        .coordinator
        .claim_gpu_cancellation(l.request.request_id, "aijob-fixture")
        .unwrap()
        .unwrap();
    unknown(&f);
    let mut op = operation(Purpose::Cancellation, key);
    op["status"] = json!({"code":7});
    let server = Server::new(vec![(200, bytes(op))]);
    let c = client(&server);
    let rev = revision(&f);
    bind(
        &c,
        &f,
        &l,
        rev,
        Purpose::Cancellation,
        "operation-recovered",
    )
    .unwrap();
    let record = f
        .coordinator
        .gpu_jobs(f.lease.attempt_id())
        .unwrap()
        .remove(0);
    assert_eq!(record.state, "canceling");
    assert_eq!(
        record.cancellation_operation.as_deref(),
        Some("operation-recovered")
    );
    assert!(
        !f.coordinator
            .remote_attempt_stopped(f.lease.attempt_id())
            .unwrap()
    );
    assert_eq!(held_money(&f), 10000);
    assert!(
        f.coordinator
            .claim_gpu_cancellation(l.request.request_id, "aijob-fixture")
            .unwrap()
            .is_none()
    );
    bind(
        &c,
        &f,
        &l,
        rev,
        Purpose::Cancellation,
        "operation-recovered",
    )
    .unwrap();
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].line.starts_with("GET "));
}
#[test]
fn team_gpu_recovery_delete_waits_for_real_cleanup_after_lost_ack_binding() {
    let f = Fixture::new(true, true);
    let l = launch();
    bound(&f, &l);
    let now = crate::model::unix_time();
    f.coordinator
        .record_gpu_observation(
            l.request.request_id,
            "aijob-fixture",
            "CANCELLED",
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
    unknown(&f);
    let pending = operation(Purpose::Deletion, claim.key);
    let server = Server::new(vec![(200, bytes(pending))]);
    let c = client(&server);
    let rev = revision(&f);
    bind(&c, &f, &l, rev, Purpose::Deletion, "operation-recovered").unwrap();
    let record = f
        .coordinator
        .gpu_jobs(f.lease.attempt_id())
        .unwrap()
        .remove(0);
    assert_eq!(record.cleanup.as_ref().unwrap().state, "acknowledged");
    assert!(
        !f.coordinator
            .remote_attempt_stopped(f.lease.attempt_id())
            .unwrap()
    );
    assert_eq!(held_money(&f), 10000);
    assert!(
        f.coordinator
            .claim_gpu_deletion(l.request.request_id, "aijob-fixture", now)
            .unwrap()
            .is_none()
    );
    assert_eq!(server.finish().len(), 1);
    let mut done = operation(Purpose::Deletion, claim.key);
    done["status"] = json!({});
    let absent = || (404, bytes(json!({"code":5})));
    let server = Server::new(vec![(200, bytes(done)), absent(), absent(), absent()]);
    let c = client(&server);
    assert!(advance_record(&c, &f.coordinator, &record, false).unwrap());
    assert!(
        f.coordinator
            .remote_attempt_stopped(f.lease.attempt_id())
            .unwrap()
    );
    assert_eq!(held_money(&f), 10000);
    assert!(server.finish().iter().all(|r| r.line.starts_with("GET ")));
}
#[test]
fn team_gpu_recovery_revision_change_during_network_does_not_bind_or_release() {
    let f = Fixture::new(true, true);
    let l = launch();
    reserve(&f, &l);
    unknown(&f);
    let rev = revision(&f);
    let run = f.lease.run_id();
    let owner = TeamCoordinator::new(f.dir.join("control.db"));
    let server = Server::with_hook(
        vec![(200, bytes(create(&l))), (200, bytes(job(&l, "RUNNING")))],
        move |r| {
            if r.line.contains("operations/") {
                owner.stop(run, rev, crate::model::unix_time()).unwrap();
            }
        },
    );
    let c = client(&server);
    assert!(bind(&c, &f, &l, rev, Purpose::Creation, "operation-recovered").is_err());
    assert!(
        f.coordinator.gpu_jobs(f.lease.attempt_id()).unwrap()[0]
            .job_id
            .is_none()
    );
    assert_eq!(held_money(&f), 10000);
    assert_eq!(server.finish().len(), 2);
}
#[test]
fn team_gpu_recovery_scope_and_unknown_identifier_fail_before_network() {
    let f = Fixture::new(true, true);
    let l = launch();
    reserve(&f, &l);
    unknown(&f);
    let other = Fixture::new(true, true);
    let server = Server::new(vec![]);
    let c = client(&server);
    let rev = revision(&f);
    assert!(
        bind_with(
            &c,
            &f.coordinator,
            f.lease.run_id(),
            other.lease.work_id(),
            f.lease.attempt_id(),
            rev,
            l.request.request_id,
            Purpose::Creation,
            "operation-recovered"
        )
        .is_err()
    );
    assert!(
        bind_with(
            &c,
            &f.coordinator,
            f.lease.run_id(),
            f.lease.work_id(),
            other.lease.attempt_id(),
            rev,
            l.request.request_id,
            Purpose::Creation,
            "operation-recovered"
        )
        .is_err()
    );
    assert!(
        bind(
            &c,
            &f,
            &l,
            rev,
            Purpose::Creation,
            "https://different.example/op"
        )
        .is_err()
    );
    assert_eq!(held_money(&f), 10000);
    assert!(server.finish().is_empty());
}
#[test]
fn team_gpu_recovery_waiting_closed_jobs_do_not_show_stale_missing_cancel() {
    let f = Fixture::new(true, true);
    let l = launch();
    bound(&f, &l);
    f.coordinator
        .claim_gpu_cancellation(l.request.request_id, "aijob-fixture")
        .unwrap()
        .unwrap();
    let db = rusqlite::Connection::open(f.dir.join("control.db")).unwrap();
    db.execute("UPDATE ion_team_work SET status='waiting',body=json_set(body,'$.status','waiting') WHERE id=?",[f.lease.work_id().to_string()]).unwrap();
    assert!(f.coordinator.snapshot(f.lease.run_id()).unwrap().work[0].can_reconcile);
    for state in ["settled", "cleaned"] {
        db.execute(
            "UPDATE ion_team_gpu_jobs SET state=? WHERE request_id=?",
            rusqlite::params![state, l.request.request_id.to_string()],
        )
        .unwrap();
        assert!(
            !f.coordinator.snapshot(f.lease.run_id()).unwrap().work[0].can_reconcile,
            "{state}"
        );
    }
}
