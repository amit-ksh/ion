use super::super::tests::{Fixture, bytes, client, job, launch};
use super::*;
use crate::driver::team_test_http::Server;
use uuid::Uuid;
fn absent() -> (u16, Vec<u8>) {
    (404, bytes(json!({"code":5,"message":"Resource not found"})))
}
fn vm(l: &Launch) -> Value {
    json!({"metadata":{"id":"compute-fixture","parentId":"project-fixture"},
        "spec":{"resources":{"platform":l.platform,"preset":l.preset},"bootDisk":{"managedDisk":{"name":"gpu-main","spec":{}}}},
        "status":{"diskAttachments":[{"id":"disk-fixture","name":"gpu-main","isManaged":true}]}})
}
fn disk() -> Value {
    json!({"metadata":{"id":"disk-fixture","parentId":"project-fixture"}})
}
fn terminal(l: &Launch) -> Value {
    let mut v = job(l, "CANCELLED");
    v["status"]["instances"] = json!([{"computeInstanceId":"compute-fixture"}]);
    v
}
fn deletion(key: Uuid, status: Option<Value>) -> Value {
    let mut v = json!({"id":"operation-delete","resourceId":"aijob-fixture",
        "request":{"@type":"type.googleapis.com/nebius.ai.v1.DeleteJobRequest","id":"aijob-fixture"},
        "requestHeaders":{"x-idempotency-key":{"values":[key.to_string()]}}});
    if let Some(status) = status {
        v["status"] = status;
    }
    v
}
fn setup() -> (Fixture, Launch, GpuCleanupRecord) {
    let f = Fixture::new(true, true);
    let l = launch();
    let now = crate::model::unix_time();
    f.coordinator
        .reserve_gpu_launch(&f.lease, &l.request, &l.body().unwrap(), &f.policy, now)
        .unwrap();
    f.coordinator
        .bind_gpu_acknowledgement(
            l.request.request_id,
            "operation-create",
            "aijob-fixture",
            now,
        )
        .unwrap();
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
    (f, l, claim)
}
#[test]
fn team_gpu_cleanup_waits_for_typed_delete_and_vm_disk_absence_without_repeating_mutation() {
    let (f, l, claim) = setup();
    let server = Server::new(vec![
        (
            200,
            bytes(json!({"id":"operation-delete","resourceId":"aijob-fixture"})),
        ),
        (200, bytes(deletion(claim.key, None))),
        (200, bytes(deletion(claim.key, Some(json!({}))))),
        absent(),
        absent(),
        (200, bytes(disk())),
        (200, bytes(deletion(claim.key, Some(json!({"code":0}))))),
        absent(),
        absent(),
        absent(),
    ]);
    let c = client(&server);
    let ack = Acknowledged {
        operation: "operation-create".into(),
        job: "aijob-fixture".into(),
    };
    let operation = c.delete_job(&ack, claim.key).unwrap();
    f.coordinator
        .bind_gpu_deletion(
            l.request.request_id,
            &ack.job,
            &operation,
            crate::model::unix_time(),
        )
        .unwrap();
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    assert!(
        !reconcile_with(
            &c,
            &reopened,
            reopened.gpu_jobs(f.lease.attempt_id()).unwrap()
        )
        .unwrap()
    );
    assert!(
        !reconcile_with(
            &c,
            &reopened,
            reopened.gpu_jobs(f.lease.attempt_id()).unwrap()
        )
        .unwrap()
    );
    assert!(
        !reopened
            .remote_attempt_stopped(f.lease.attempt_id())
            .unwrap()
    );
    assert!(
        reconcile_with(
            &c,
            &reopened,
            reopened.gpu_jobs(f.lease.attempt_id()).unwrap()
        )
        .unwrap()
    );
    assert!(
        reopened
            .remote_attempt_stopped(f.lease.attempt_id())
            .unwrap()
    );
    // Resource cleanup does not manufacture a command receipt or release billing.
    assert!(reopened.has_pending_effect(f.lease.run_id()).unwrap());
    let mut next = launch();
    next.request.request_id = Uuid::new_v4();
    assert!(
        reopened
            .reserve_gpu_launch(
                &f.lease,
                &next.request,
                &next.body().unwrap(),
                &f.policy,
                crate::model::unix_time()
            )
            .unwrap_err()
            .to_string()
            .contains("reservation budget reached")
    );
    assert!(
        reconcile_with(
            &c,
            &reopened,
            reopened.gpu_jobs(f.lease.attempt_id()).unwrap()
        )
        .unwrap()
    );
    let requests = server.finish();
    assert_eq!(requests.len(), 10);
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.line.starts_with("DELETE "))
            .count(),
        1
    );
    assert!(
        requests[0]
            .line
            .starts_with("DELETE /ai/v1/jobs/aijob-fixture ")
    );
}
#[test]
fn team_gpu_inventory_captures_managed_and_existing_disks_and_rejects_foreign_resources() {
    let l = launch();
    let mut v = vm(&l);
    v["spec"]["secondaryDisks"] = json!([{"existingDisk":{"id":"disk-extra"}}]);
    v["status"]["diskAttachments"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"disk-extra"}));
    let server = Server::new(vec![
        (200, bytes(v)),
        (200, bytes(disk())),
        (
            200,
            bytes(json!({"metadata":{"id":"disk-extra","parentId":"project-fixture"}})),
        ),
    ]);
    let c = client(&server);
    let inventory = c
        .inventory(&l.body().unwrap(), "compute-fixture")
        .unwrap()
        .unwrap();
    assert!(inventory.complete);
    assert_eq!(inventory.disks.len(), 2);
    assert_eq!(inventory.disks.iter().filter(|d| d.managed).count(), 1);
    assert_eq!(server.finish().len(), 3);
    let mut wrong = disk();
    wrong["metadata"]["parentId"] = json!("foreign-project");
    let server = Server::new(vec![(200, bytes(vm(&l))), (200, bytes(wrong))]);
    assert!(
        client(&server)
            .inventory(&l.body().unwrap(), "compute-fixture")
            .is_err()
    );
    assert_eq!(server.finish().len(), 2);
}
#[test]
fn team_gpu_missing_inventory_or_incomplete_attachment_cannot_authorize_delete() {
    let f = Fixture::new(true, true);
    let l = launch();
    let now = crate::model::unix_time();
    f.coordinator
        .reserve_gpu_launch(&f.lease, &l.request, &l.body().unwrap(), &f.policy, now)
        .unwrap();
    f.coordinator
        .bind_gpu_acknowledgement(
            l.request.request_id,
            "operation-create",
            "aijob-fixture",
            now,
        )
        .unwrap();
    let server = Server::new(vec![(200, bytes(terminal(&l))), absent()]);
    let c = client(&server);
    assert!(
        !reconcile_with(
            &c,
            &f.coordinator,
            f.coordinator.gpu_jobs(f.lease.attempt_id()).unwrap()
        )
        .unwrap()
    );
    assert!(
        f.coordinator
            .claim_gpu_deletion(l.request.request_id, "aijob-fixture", now)
            .is_err()
    );
    assert_eq!(server.finish().len(), 2);
    let mut pending = vm(&l);
    pending["status"]["diskAttachments"][0]["id"] = json!("");
    let server = Server::new(vec![(200, bytes(pending))]);
    let inventory = client(&server)
        .inventory(&l.body().unwrap(), "compute-fixture")
        .unwrap()
        .unwrap();
    assert!(!inventory.complete);
    f.coordinator
        .record_gpu_inventory(l.request.request_id, "aijob-fixture", &inventory, now)
        .unwrap();
    assert!(
        f.coordinator
            .claim_gpu_deletion(l.request.request_id, "aijob-fixture", now)
            .is_err()
    );
    assert_eq!(server.finish().len(), 1);
}
#[test]
fn team_gpu_lost_delete_ack_keeps_claim_and_never_repeats_delete_after_restart() {
    let (f, l, claim) = setup();
    let server = Server::new(vec![(500, bytes(json!({})))]);
    let c = client(&server);
    let ack = Acknowledged {
        operation: "operation-create".into(),
        job: "aijob-fixture".into(),
    };
    assert!(c.delete_job(&ack, claim.key).is_err());
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    assert!(
        !reconcile_with(
            &c,
            &reopened,
            reopened.gpu_jobs(f.lease.attempt_id()).unwrap()
        )
        .unwrap()
    );
    assert!(
        reopened
            .claim_gpu_deletion(l.request.request_id, &ack.job, crate::model::unix_time())
            .unwrap()
            .is_none()
    );
    assert!(
        !reopened
            .remote_attempt_stopped(f.lease.attempt_id())
            .unwrap()
    );
    assert_eq!(server.finish().len(), 1);
}
#[test]
fn team_gpu_cleanup_rejects_wrong_operations_weak_404_and_changed_resource_inventory() {
    let (f, l, mut claim) = setup();
    claim.operation = Some("operation-delete".into());
    f.coordinator
        .bind_gpu_deletion(
            l.request.request_id,
            "aijob-fixture",
            "operation-delete",
            crate::model::unix_time(),
        )
        .unwrap();
    let ack = Acknowledged {
        operation: "operation-create".into(),
        job: "aijob-fixture".into(),
    };
    for field in ["type", "job", "key", "failed"] {
        let mut value = deletion(claim.key, Some(json!({})));
        match field {
            "type" => {
                value["request"]["@type"] =
                    json!("type.googleapis.com/nebius.ai.v1.CancelJobRequest")
            }
            "job" => value["request"]["id"] = json!("another-job"),
            "key" => {
                value["requestHeaders"]["x-idempotency-key"]["values"] =
                    json!([Uuid::new_v4().to_string()])
            }
            _ => value["status"] = json!({"code":7}),
        }
        let server = Server::new(vec![(200, bytes(value))]);
        assert!(client(&server).cleanup_proven(&ack, &claim).is_err());
        assert_eq!(server.finish().len(), 1);
    }
    for (code, body) in [
        (404, b"<html>missing route</html>".to_vec()),
        (404, bytes(json!({"code":7}))),
        (403, bytes(json!({"code":5}))),
        (500, bytes(json!({"code":5}))),
    ] {
        let server = Server::new(vec![
            (200, bytes(deletion(claim.key, Some(json!({}))))),
            (code, body),
        ]);
        assert!(client(&server).cleanup_proven(&ack, &claim).is_err());
        assert_eq!(server.finish().len(), 2);
    }
    let mut changed = claim.clone();
    changed.inventory[0].disks.clear();
    assert!(
        f.coordinator
            .verify_gpu_cleanup(
                l.request.request_id,
                "operation-delete",
                &changed,
                crate::model::unix_time()
            )
            .is_err()
    );
    assert!(
        !f.coordinator
            .remote_attempt_stopped(f.lease.attempt_id())
            .unwrap()
    );
}
#[test]
fn team_gpu_inventory_permission_failure_still_cancels_original_job() {
    let f = Fixture::new(true, true);
    let l = launch();
    let now = crate::model::unix_time();
    f.coordinator
        .reserve_gpu_launch(&f.lease, &l.request, &l.body().unwrap(), &f.policy, now)
        .unwrap();
    f.coordinator
        .bind_gpu_acknowledgement(
            l.request.request_id,
            "operation-create",
            "aijob-fixture",
            now,
        )
        .unwrap();
    let mut running = terminal(&l);
    running["status"]["state"] = json!("RUNNING");
    let server = Server::new(vec![
        (200, bytes(running)),
        (403, bytes(json!({"code":7}))),
        (
            200,
            bytes(json!({"id":"operation-cancel","resourceId":"aijob-fixture"})),
        ),
    ]);
    assert!(
        reconcile_with(
            &client(&server),
            &f.coordinator,
            f.coordinator.gpu_jobs(f.lease.attempt_id()).unwrap()
        )
        .is_err()
    );
    let record = f
        .coordinator
        .gpu_jobs(f.lease.attempt_id())
        .unwrap()
        .remove(0);
    assert!(record.cancel_requested);
    assert_eq!(
        record.cancellation_operation.as_deref(),
        Some("operation-cancel")
    );
    let requests = server.finish();
    assert_eq!(requests.len(), 3);
    assert!(requests[2].line.starts_with("POST /ai/v1/jobs/cancel "));
}

#[test]
fn team_gpu_cleanup_releases_project_capacity_but_retains_spending_and_results() {
    let mut f = Fixture::new(true, true);
    let l = launch();
    f.policy.max_parallel = 1;
    let now = crate::model::unix_time();
    f.coordinator
        .reserve_gpu_launch(&f.lease, &l.request, &l.body().unwrap(), &f.policy, now)
        .unwrap();
    f.coordinator
        .bind_gpu_acknowledgement(
            l.request.request_id,
            "operation-create",
            "aijob-fixture",
            now,
        )
        .unwrap();
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
    let mut claim = f
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
    claim.operation = Some("operation-delete".into());
    let mut other = Fixture::new_in(f.dir.clone(), true, true);
    other.policy.max_parallel = 1;
    let next = launch();
    assert!(
        other
            .coordinator
            .reserve_gpu_launch(
                &other.lease,
                &next.request,
                &next.body().unwrap(),
                &other.policy,
                now
            )
            .unwrap_err()
            .to_string()
            .contains("concurrency")
    );
    let server = Server::new(vec![
        (200, bytes(deletion(claim.key, Some(json!({}))))),
        absent(),
        absent(),
        absent(),
    ]);
    assert!(
        reconcile_with(
            &client(&server),
            &f.coordinator,
            f.coordinator.gpu_jobs(f.lease.attempt_id()).unwrap()
        )
        .unwrap()
    );
    assert_eq!(server.finish().len(), 4);
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    assert!(reopened.has_pending_effect(f.lease.run_id()).unwrap());
    assert!(
        reopened
            .reserve_gpu_launch(
                &other.lease,
                &next.request,
                &next.body().unwrap(),
                &other.policy,
                now
            )
            .unwrap()
    );
    let third = launch();
    assert!(
        reopened
            .reserve_gpu_launch(
                &f.lease,
                &third.request,
                &third.body().unwrap(),
                &f.policy,
                now
            )
            .unwrap_err()
            .to_string()
            .contains("reservation budget reached")
    );
}
#[test]
fn team_gpu_stop_attempts_other_jobs_when_one_observation_fails() {
    let f = Fixture::new(true, true);
    let other = Fixture::new_in(f.dir.clone(), true, true);
    let first = launch();
    let second = launch();
    let now = crate::model::unix_time();
    for (fixture, l, op, job_id) in [
        (&f, &first, "operation-create", "aijob-fixture"),
        (&other, &second, "operation-second", "aijob-second"),
    ] {
        fixture
            .coordinator
            .reserve_gpu_launch(
                &fixture.lease,
                &l.request,
                &l.body().unwrap(),
                &fixture.policy,
                now,
            )
            .unwrap();
        fixture
            .coordinator
            .bind_gpu_acknowledgement(l.request.request_id, op, job_id, now)
            .unwrap();
    }
    let mut running = job(&second, "RUNNING");
    running["metadata"]["id"] = json!("aijob-second");
    let server = Server::new(vec![
        (403, bytes(json!({"code":7}))),
        (200, bytes(running)),
        (
            200,
            bytes(json!({"id":"operation-cancel-second","resourceId":"aijob-second"})),
        ),
    ]);
    let mut records = f.coordinator.gpu_jobs(f.lease.attempt_id()).unwrap();
    records.extend(
        other
            .coordinator
            .gpu_jobs(other.lease.attempt_id())
            .unwrap(),
    );
    assert!(reconcile_with(&client(&server), &f.coordinator, records).is_err());
    let second_record = other
        .coordinator
        .gpu_jobs(other.lease.attempt_id())
        .unwrap()
        .remove(0);
    assert_eq!(
        second_record.cancellation_operation.as_deref(),
        Some("operation-cancel-second")
    );
    let requests = server.finish();
    assert_eq!(requests.len(), 3);
    assert!(requests[2].line.starts_with("POST /ai/v1/jobs/cancel "));
    assert_eq!(requests[2].body["id"], "aijob-second");
}
