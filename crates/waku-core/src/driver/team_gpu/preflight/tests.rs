use super::*;
use crate::driver::team_gpu::tests::{Fixture, bytes, client, launch};
use crate::driver::team_test_http::Server;
use crate::teams::gpu_jobs::{
    GpuChildNetworkPolicy, GpuComputeInventory, GpuDiskInventory, GpuQuotaPolicy, GpuQuotaScope,
    GpuResourceVector,
};
pub(in crate::driver::team_gpu) fn policy(l: &Launch) -> GpuResourcePolicy {
    let cap = GpuResourceVector {
        jobs: 8,
        gpus: 8,
        vcpus: 64,
        memory_gib: 256,
        disk_bytes: 8 * l.disk_bytes,
    };
    GpuResourcePolicy {
        version: 1,
        project: l.project.clone(),
        tenant: "tenant-fixture".into(),
        region: "eu-north1".into(),
        network: "vpcnetwork-fixture".into(),
        subnet: l.subnet.clone(),
        image: l.image.clone(),
        platform: l.platform.clone(),
        preset: l.preset.clone(),
        gpu_memory_gb: 48,
        shape: GpuResourceVector {
            jobs: 1,
            gpus: 1,
            vcpus: 8,
            memory_gib: 32,
            disk_bytes: l.disk_bytes,
        },
        shm_bytes: l.shm_bytes,
        project_limit: cap.clone(),
        tenant_limit: cap,
        child_network: Some(GpuChildNetworkPolicy::SeccompSocketDenyV1),
        quotas: [GpuQuotaScope::Project, GpuQuotaScope::Tenant]
            .into_iter()
            .flat_map(|scope| {
                [
                    (GpuQuotaKind::Gpu, "compute.gpu.fixture.count", "count"),
                    (GpuQuotaKind::Vm, "compute.vm.fixture.count", "count"),
                    (GpuQuotaKind::Disk, "compute.disk.fixture.count", "count"),
                    (
                        GpuQuotaKind::DiskBytes,
                        "compute.disk.fixture.bytes",
                        "byte",
                    ),
                ]
                .into_iter()
                .map(move |(kind, name, unit)| GpuQuotaPolicy {
                    scope,
                    kind,
                    name: name.into(),
                    unit: unit.into(),
                })
            })
            .collect(),
    }
}
pub(in crate::driver::team_gpu) fn values(l: &Launch, p: &GpuResourcePolicy) -> Vec<Value> {
    let shape = json!({"gpuCount":p.shape.gpus,"vcpuCount":p.shape.vcpus,"memoryGibibytes":p.shape.memory_gib});
    let mut reads = vec![
        json!({"metadata":{"id":p.project,"parentId":p.tenant},"spec":{"region":p.region},"status":{"projectState":"ACTIVE"}}),
        json!({"metadata":{"id":"platform-fixture","parentId":p.project,"name":p.platform},"spec":{"gpuMemoryGigabytes":p.gpu_memory_gb,"gpuCountQuotaType":"compute.gpu.fixture.count","presets":[{"name":p.preset,"resources":shape}]}}),
        json!({"metadata":{"id":l.subnet,"parentId":p.project},"spec":{"networkId":p.network},"status":{"state":"READY"}}),
        json!({"metadata":{"id":p.network,"parentId":p.project},"status":{"state":"READY"}}),
    ];
    for (i, q) in p.quotas.iter().enumerate() {
        reads.push(json!({"metadata":{"id":format!("quota-{i}"),"parentId":p.parent(q.scope),"name":q.name},"spec":{"region":p.region,"limit":p.quantity(q.kind).saturating_mul(100).to_string()},"status":{"state":"STATE_ACTIVE","unit":q.unit,"usage":"0","usageState":"USAGE_STATE_NOT_USED"}}));
    }
    reads.push(json!({"items":[{"metadata":{"id":"advice-fixture","parentId":p.tenant},"spec":{"region":p.region,"computeInstance":{"platform":p.platform,"gpuMemoryGigabytes":p.gpu_memory_gb,"preset":{"name":p.preset,"resources":shape}}},"status":{"onDemand":{"available":8,"limit":8,"dataState":"DATA_STATE_FRESH","availabilityLevel":"AVAILABILITY_LEVEL_HIGH","effectiveAt":chrono::Utc::now().to_rfc3339()}}}],"nextPageToken":""}));
    reads
}
pub(in crate::driver::team_gpu) fn responses(
    l: &Launch,
    p: &GpuResourcePolicy,
) -> Vec<(u16, Vec<u8>)> {
    values(l, p).into_iter().map(|v| (200, bytes(v))).collect()
}
#[test]
fn team_gpu_preflight_exact_resource_reads_are_persisted_before_single_create_and_replay_needs_no_read()
 {
    let mut f = Fixture::new(true, true);
    let l = launch();
    let p = policy(&l);
    f.policy.resources = Some(p.clone());
    let mut reply = responses(&l, &p);
    reply.push((
        200,
        bytes(json!({"id":"operation-create","resourceId":"aijob-fixture"})),
    ));
    let dbpath = f.dir.join("control.db");
    let request = l.request.request_id;
    let server = Server::with_hook(reply, move |r| {
        if r.line.starts_with("POST ") {
            let db = rusqlite::Connection::open(&dbpath).unwrap();
            let body: String = db
                .query_row(
                    "SELECT body FROM ion_team_gpu_preflight WHERE request_id=?",
                    [request.to_string()],
                    |r| r.get(0),
                )
                .unwrap();
            let witness: Value = serde_json::from_str(&body).unwrap();
            assert_eq!(witness["request_id"], request.to_string());
            assert_eq!(witness["policy"]["shape"]["gpus"], 1);
            assert_eq!(witness["reads"].as_array().unwrap().len(), 13);
            assert!(!body.contains("fixture-only"));
        }
    });
    let c = client(&server);
    let saved = launch_once_with(&c, &f.coordinator, &f.lease, &l, &f.policy).unwrap();
    assert_eq!(saved.job_id.as_deref(), Some("aijob-fixture"));
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    assert_eq!(
        launch_once_with(&c, &reopened, &f.lease, &l, &f.policy)
            .unwrap()
            .creation_operation,
        saved.creation_operation
    );
    let requests = server.finish();
    assert_eq!(requests.len(), 14);
    assert!(requests[..13].iter().all(|r| r.line.starts_with("GET ")));
    assert!(
        requests[1]
            .line
            .contains("parentId=project-fixture&name=gpu-l40s-a")
    );
    assert!(
        requests[4]
            .line
            .contains("parentId=project-fixture&name=compute.gpu.fixture.count&region=eu-north1")
    );
    assert!(requests[8].line.contains("parentId=tenant-fixture"));
    assert!(
        requests[12]
            .line
            .contains("parentId=tenant-fixture&pageSize=64")
    );
    assert!(requests[13].line.starts_with("POST /ai/v1/jobs "));
}
#[test]
fn team_gpu_preflight_missing_or_changed_owner_approval_cannot_read_or_reserve() {
    let f = Fixture::new(true, true);
    let l = launch();
    let server = Server::new(vec![]);
    let c = client(&server);
    assert!(
        launch_once_with(&c, &f.coordinator, &f.lease, &l, &f.policy)
            .err()
            .unwrap()
            .to_string()
            .contains("exact GPU")
    );
    for field in 0..12 {
        let mut budget = f.policy.clone();
        let mut p = policy(&l);
        match field {
            0 => p.project = "project-other".into(),
            1 => p.image = format!("other/image@sha256:{}", "b".repeat(64)),
            2 => p.platform = "gpu-other".into(),
            3 => p.preset = "other".into(),
            4 => p.subnet = "subnet-other".into(),
            5 => p.shape.disk_bytes *= 2,
            6 => p.shm_bytes = 1,
            7 => p.quotas.pop().map(|_| ()).unwrap(),
            8 => p.shape.jobs = 2,
            9 => p.quotas[3].unit = "GiB".into(),
            10 => p.quotas[0].name = "../quota".into(),
            _ => p.child_network = None,
        };
        budget.resources = Some(p);
        assert!(launch_once_with(&c, &f.coordinator, &f.lease, &l, &budget).is_err());
    }
    assert!(
        f.coordinator
            .gpu_jobs(f.lease.attempt_id())
            .unwrap()
            .is_empty()
    );
    assert!(server.finish().is_empty());
}
#[test]
fn team_gpu_preflight_wrong_project_shape_network_and_quota_fail_without_reservation_or_post() {
    let mut f = Fixture::new(true, true);
    let l = launch();
    let p = policy(&l);
    f.policy.resources = Some(p.clone());
    let cases = vec![
        (0, "/metadata/id", json!("project-other")),
        (0, "/metadata/parentId", json!("tenant-other")),
        (0, "/spec/region", json!("eu-west1")),
        (0, "/status/projectState", json!("PARKED")),
        (1, "/metadata/parentId", json!("project-other")),
        (1, "/spec/gpuMemoryGigabytes", json!(80)),
        (1, "/spec/presets/0/resources/vcpuCount", json!(16)),
        (1, "/spec/presets/0/resources/gpuCount", json!(0)),
        (1, "/spec/presets/0/resources/memoryGibibytes", json!(null)),
        (2, "/spec/networkId", json!("vpcnetwork-other")),
        (2, "/status/state", json!("CREATING")),
        (3, "/metadata/parentId", json!("project-other")),
        (4, "/metadata/name", json!("quota-other")),
        (4, "/spec/region", json!("eu-west1")),
        (4, "/spec/limit", json!(null)),
        (4, "/spec/limit", json!("-1")),
        (4, "/spec/limit", json!("1.5")),
        (4, "/status/usageState", json!("USAGE_STATE_UNKNOWN")),
        (4, "/status/usage", json!("1")),
        (4, "/status/unit", json!("byte")),
        (4, "/status/state", json!("STATE_FROZEN")),
        (5, "/metadata/id", json!("quota-0")),
        (8, "/metadata/parentId", json!("tenant-other")),
        (11, "/spec/limit", json!("1")),
    ];
    for (index, path, value) in cases {
        let mut replies = values(&l, &p);
        *replies[index].pointer_mut(path).unwrap() = value;
        let server = Server::new(
            replies
                .into_iter()
                .take(index + 1)
                .map(|v| (200, bytes(v)))
                .collect(),
        );
        assert!(
            launch_once_with(&client(&server), &f.coordinator, &f.lease, &l, &f.policy).is_err(),
            "accepted invalid field {path}"
        );
        assert!(
            f.coordinator
                .gpu_jobs(f.lease.attempt_id())
                .unwrap()
                .is_empty()
        );
        assert!(server.finish().iter().all(|r| r.line.starts_with("GET ")));
    }
}
#[test]
fn team_gpu_preflight_capacity_requires_fresh_exact_generic_shape_and_bounded_pagination() {
    let mut f = Fixture::new(true, true);
    let l = launch();
    let p = policy(&l);
    f.policy.resources = Some(p.clone());
    let changes = [
        ("/metadata/parentId", json!("tenant-other")),
        ("/spec/computeInstance/gpuMemoryGigabytes", json!(80)),
        (
            "/spec/computeInstance/preset/resources/memoryGibibytes",
            json!(64),
        ),
        ("/status/onDemand/dataState", json!("DATA_STATE_STALE")),
        (
            "/status/onDemand/availabilityLevel",
            json!("AVAILABILITY_LEVEL_UNKNOWN"),
        ),
        ("/status/onDemand/available", json!(0)),
        ("/status/onDemand/limit", json!(0)),
        (
            "/status/onDemand/effectiveAt",
            json!((chrono::Utc::now() - chrono::Duration::seconds(121)).to_rfc3339()),
        ),
        (
            "/status/onDemand/effectiveAt",
            json!((chrono::Utc::now() + chrono::Duration::minutes(5)).to_rfc3339()),
        ),
    ];
    for (path, value) in changes {
        let mut replies = values(&l, &p);
        *replies[12]["items"][0].pointer_mut(path).unwrap() = value;
        let server = Server::new(replies.into_iter().map(|v| (200, bytes(v))).collect());
        assert!(
            client(&server).verify_preflight(&l, &f.policy).is_err(),
            "accepted {path}"
        );
        assert_eq!(server.finish().len(), 13);
    }
    for mode in 0..4 {
        let mut replies = values(&l, &p);
        match mode {
            0 => replies[12]["items"] = json!([]),
            1 => {
                let item = replies[12]["items"][0].clone();
                replies[12]["items"].as_array_mut().unwrap().push(item);
            }
            2 => replies[12]["items"][0]["spec"]["fabric"] = json!("fabric-unpinned"),
            _ => {
                replies[12]["items"] = json!([]);
                replies[12]["nextPageToken"] = json!("same-token");
                let page = replies[12].clone();
                replies.push(page);
            }
        }
        let count = replies.len();
        let server = Server::new(replies.into_iter().map(|v| (200, bytes(v))).collect());
        assert!(client(&server).verify_preflight(&l, &f.policy).is_err());
        assert_eq!(server.finish().len(), count);
    }
    // A valid match on the second page retains both read hashes, without early acceptance.
    let mut replies = values(&l, &p);
    let page = replies[12].clone();
    replies[12] = json!({"items":[],"nextPageToken":"next+page/=token"});
    replies.push(page);
    let server = Server::new(replies.into_iter().map(|v| (200, bytes(v))).collect());
    assert!(client(&server).verify_preflight(&l, &f.policy).is_ok());
    let requests = server.finish();
    assert_eq!(requests.len(), 14);
    assert!(
        requests[13]
            .line
            .contains("pageToken=next%2Bpage%2F%3Dtoken")
    );
}
#[test]
fn team_gpu_preflight_receipt_cannot_be_reused_for_another_body_policy_or_stale_reservation() {
    let mut f = Fixture::new(true, true);
    let l = launch();
    let p = policy(&l);
    f.policy.resources = Some(p.clone());
    let server = Server::new(responses(&l, &p));
    let receipt = client(&server).verify_preflight(&l, &f.policy).unwrap();
    server.finish();
    let now = crate::model::unix_time();
    let body = l.body().unwrap();
    let mut changed = body.clone();
    changed["spec"]["disk"]["sizeBytes"] = json!("1");
    assert!(
        f.coordinator
            .reserve_verified_gpu_launch(&f.lease, &l.request, &changed, &f.policy, &receipt, now)
            .is_err()
    );
    let mut expired_capacity = receipt.clone();
    expired_capacity.capacity.effective_at = now.saturating_sub(121);
    assert!(
        f.coordinator
            .reserve_verified_gpu_launch(
                &f.lease,
                &l.request,
                &body,
                &f.policy,
                &expired_capacity,
                now
            )
            .is_err()
    );
    let mut changed_request = l.request.clone();
    changed_request.argv.push("--changed-command".into());
    assert!(
        f.coordinator
            .reserve_verified_gpu_launch(
                &f.lease,
                &changed_request,
                &body,
                &f.policy,
                &receipt,
                now
            )
            .is_err()
    );
    let mut changed_policy = f.policy.clone();
    changed_policy
        .resources
        .as_mut()
        .unwrap()
        .project_limit
        .jobs -= 1;
    assert!(
        f.coordinator
            .reserve_verified_gpu_launch(
                &f.lease,
                &l.request,
                &body,
                &changed_policy,
                &receipt,
                now
            )
            .is_err()
    );
    assert!(
        f.coordinator
            .reserve_verified_gpu_launch(
                &f.lease,
                &l.request,
                &body,
                &f.policy,
                &receipt,
                receipt.checked_at + 31
            )
            .is_err()
    );
    assert!(
        f.coordinator
            .gpu_jobs(f.lease.attempt_id())
            .unwrap()
            .is_empty()
    );
    assert!(
        f.coordinator
            .reserve_verified_gpu_launch(&f.lease, &l.request, &body, &f.policy, &receipt, now)
            .unwrap()
    );
    assert!(
        !f.coordinator
            .reserve_verified_gpu_launch(
                &f.lease,
                &l.request,
                &body,
                &f.policy,
                &receipt,
                receipt.checked_at + 31
            )
            .unwrap()
    );
}
#[test]
fn team_gpu_preflight_atomic_project_resource_caps_hold_across_runs_terminal_states_and_restart() {
    let mut first = Fixture::new(true, true);
    let mut second = Fixture::new_in(first.dir.clone(), true, true);
    let a = launch();
    let b = launch();
    let mut p = policy(&a);
    p.project_limit = p.shape.clone();
    first.policy.resources = Some(p.clone());
    second.policy.resources = Some(p.clone());
    let server = Server::new(responses(&a, &p));
    let checked_a = client(&server).verify_preflight(&a, &first.policy).unwrap();
    server.finish();
    let server = Server::new(responses(&b, &p));
    let checked_b = client(&server)
        .verify_preflight(&b, &second.policy)
        .unwrap();
    server.finish();
    let now = crate::model::unix_time();
    let barrier = std::sync::Barrier::new(2);
    let results = std::thread::scope(|scope| {
        let aa = scope.spawn(|| {
            barrier.wait();
            first.coordinator.reserve_verified_gpu_launch(
                &first.lease,
                &a.request,
                &a.body().unwrap(),
                &first.policy,
                &checked_a,
                now,
            )
        });
        let bb = scope.spawn(|| {
            barrier.wait();
            second.coordinator.reserve_verified_gpu_launch(
                &second.lease,
                &b.request,
                &b.body().unwrap(),
                &second.policy,
                &checked_b,
                now,
            )
        });
        vec![aa.join().unwrap(), bb.join().unwrap()]
    });
    assert_eq!(results.iter().filter(|r| matches!(r, Ok(true))).count(), 1);
    assert!(results.iter().any(|r| {
        r.as_ref()
            .is_err_and(|e| e.to_string().contains("resource reservation limit"))
    }));
    let (winner, l, loser, other, checked) = if results[0].is_ok() {
        (&first, &a, &second, &b, &checked_b)
    } else {
        (&second, &b, &first, &a, &checked_a)
    };
    winner
        .coordinator
        .bind_gpu_acknowledgement(
            l.request.request_id,
            "operation-create",
            "aijob-fixture",
            now,
        )
        .unwrap();
    winner
        .coordinator
        .record_gpu_observation(l.request.request_id, "aijob-fixture", "CANCELLED", &[], now)
        .unwrap();
    let reopened = TeamCoordinator::new(first.dir.join("control.db"));
    assert!(
        reopened
            .reserve_verified_gpu_launch(
                &loser.lease,
                &other.request,
                &other.body().unwrap(),
                &loser.policy,
                checked,
                now
            )
            .unwrap_err()
            .to_string()
            .contains("resource reservation limit")
    );
    assert!(reopened.has_pending_effect(winner.lease.run_id()).unwrap());
    // A terminal state is not a capacity release. The original typed deletion and
    // complete VM/disk absence proof release quantities, without settling money/output.
    reopened
        .record_gpu_observation(
            l.request.request_id,
            "aijob-fixture",
            "CANCELLED",
            &["compute-fixture".into()],
            now,
        )
        .unwrap();
    reopened
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
    let mut claim = reopened
        .claim_gpu_deletion(l.request.request_id, "aijob-fixture", now)
        .unwrap()
        .unwrap();
    reopened
        .bind_gpu_deletion(
            l.request.request_id,
            "aijob-fixture",
            "operation-delete",
            now,
        )
        .unwrap();
    claim.operation = Some("operation-delete".into());
    let absent = (404, bytes(json!({"code":5})));
    let server = Server::new(vec![
        (
            200,
            bytes(json!({
                "id":"operation-delete","resourceId":"aijob-fixture","status":{},
                "request":{"@type":"type.googleapis.com/nebius.ai.v1.DeleteJobRequest","id":"aijob-fixture"},
                "requestHeaders":{"x-idempotency-key":{"values":[claim.key.to_string()]}}
            })),
        ),
        absent.clone(),
        absent.clone(),
        absent,
    ]);
    assert!(
        client(&server)
            .cleanup_proven(
                &Acknowledged {
                    operation: "operation-create".into(),
                    job: "aijob-fixture".into(),
                },
                &claim
            )
            .unwrap()
    );
    reopened
        .verify_gpu_cleanup(l.request.request_id, "operation-delete", &claim, now)
        .unwrap();
    assert_eq!(server.finish().len(), 4);
    let server = Server::new(responses(other, &p));
    let refreshed = client(&server)
        .verify_preflight(other, &loser.policy)
        .unwrap();
    server.finish();
    assert!(
        reopened
            .reserve_verified_gpu_launch(
                &loser.lease,
                &other.request,
                &other.body().unwrap(),
                &loser.policy,
                &refreshed,
                crate::model::unix_time(),
            )
            .unwrap()
    );
    let db = rusqlite::Connection::open(first.dir.join("control.db")).unwrap();
    let held: u64 = db
        .query_row(
            "SELECT reserved_micros FROM ion_team_gpu_jobs WHERE request_id=?",
            [l.request.request_id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(held, winner.policy.reservation_micros);
    assert!(reopened.has_pending_effect(winner.lease.run_id()).unwrap());
}
#[test]
fn team_gpu_preflight_tenant_caps_quota_headroom_and_unknown_legacy_resources_block_new_allocation()
{
    for mode in 0..3 {
        let mut first = Fixture::new(true, true);
        let mut second = Fixture::new_in(first.dir.clone(), true, true);
        let a = launch();
        let mut b = launch();
        let mut p = policy(&a);
        let mut q = p.clone();
        if mode == 0 {
            p.project_limit = p.shape.clone();
            p.tenant_limit = p.shape.clone();
            q.project = "project-second".into();
            q.project_limit = q.shape.clone();
            q.tenant_limit = q.shape.clone();
            b.project = q.project.clone();
        }
        first.policy.resources = if mode == 2 { None } else { Some(p.clone()) };
        second.policy.resources = Some(q.clone());
        let now = crate::model::unix_time();
        if mode == 2 {
            first
                .coordinator
                .reserve_gpu_launch(
                    &first.lease,
                    &a.request,
                    &a.body().unwrap(),
                    &first.policy,
                    now,
                )
                .unwrap();
        } else {
            let server = Server::new(responses(&a, &p));
            let r = client(&server).verify_preflight(&a, &first.policy).unwrap();
            server.finish();
            first
                .coordinator
                .reserve_verified_gpu_launch(
                    &first.lease,
                    &a.request,
                    &a.body().unwrap(),
                    &first.policy,
                    &r,
                    crate::model::unix_time(),
                )
                .unwrap();
        }
        let mut reply = values(&b, &q);
        if mode == 1 {
            reply[4]["spec"]["limit"] = json!("1");
        }
        let server = Server::new(reply.into_iter().map(|v| (200, bytes(v))).collect());
        let mut c = client(&server);
        c.project = b.project.clone();
        let r = c.verify_preflight(&b, &second.policy).unwrap();
        server.finish();
        let error = second
            .coordinator
            .reserve_verified_gpu_launch(
                &second.lease,
                &b.request,
                &b.body().unwrap(),
                &second.policy,
                &r,
                crate::model::unix_time(),
            )
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(match mode {
                0 => "tenant resource reservation",
                1 => "quota headroom",
                _ => "unknown historical quantities",
            }),
            "unexpected error: {error}"
        );
        assert!(
            second
                .coordinator
                .gpu_jobs(second.lease.attempt_id())
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn team_gpu_preflight_capacity_headroom_is_reserved_atomically_across_independent_runs() {
    let mut first = Fixture::new(true, true);
    let mut second = Fixture::new_in(first.dir.clone(), true, true);
    let a = launch();
    let b = launch();
    let p = policy(&a);
    first.policy.resources = Some(p.clone());
    second.policy.resources = Some(p.clone());
    let preflight = |l: &Launch, budget: &GpuBudgetPolicy| {
        let mut replies = values(l, &p);
        replies[12]["items"][0]["status"]["onDemand"]["available"] = json!(1);
        replies[12]["items"][0]["status"]["onDemand"]["limit"] = json!(1);
        let server = Server::new(replies.into_iter().map(|v| (200, bytes(v))).collect());
        let checked = client(&server).verify_preflight(l, budget).unwrap();
        server.finish();
        checked
    };
    let checked_a = preflight(&a, &first.policy);
    let checked_b = preflight(&b, &second.policy);
    let now = crate::model::unix_time();
    let barrier = std::sync::Barrier::new(2);
    let results = std::thread::scope(|scope| {
        let aa = scope.spawn(|| {
            barrier.wait();
            first.coordinator.reserve_verified_gpu_launch(
                &first.lease,
                &a.request,
                &a.body().unwrap(),
                &first.policy,
                &checked_a,
                now,
            )
        });
        let bb = scope.spawn(|| {
            barrier.wait();
            second.coordinator.reserve_verified_gpu_launch(
                &second.lease,
                &b.request,
                &b.body().unwrap(),
                &second.policy,
                &checked_b,
                now,
            )
        });
        vec![aa.join().unwrap(), bb.join().unwrap()]
    });
    assert_eq!(results.iter().filter(|r| matches!(r, Ok(true))).count(), 1);
    assert!(results.iter().any(|r| {
        r.as_ref()
            .is_err_and(|e| e.to_string().contains("capacity advice headroom"))
    }));
    assert_eq!(
        first
            .coordinator
            .gpu_jobs(first.lease.attempt_id())
            .unwrap()
            .len()
            + second
                .coordinator
                .gpu_jobs(second.lease.attempt_id())
                .unwrap()
                .len(),
        1
    );
}

#[test]
fn team_gpu_preflight_legacy_socket_policy_replays_original_without_reads_but_cannot_launch_new_work()
 {
    let mut f = Fixture::new(true, true);
    let l = launch();
    let p = policy(&l);
    f.policy.resources = Some(p.clone());
    let server = Server::new(responses(&l, &p));
    let receipt = client(&server).verify_preflight(&l, &f.policy).unwrap();
    server.finish();
    f.coordinator
        .reserve_verified_gpu_launch(
            &f.lease,
            &l.request,
            &l.body().unwrap(),
            &f.policy,
            &receipt,
            crate::model::unix_time(),
        )
        .unwrap();
    // Simulate a persisted resource-policy reservation from before the optional
    // socket contract was introduced. Missing extension fields must remain omitted.
    f.policy.resources.as_mut().unwrap().child_network = None;
    let policy_value = serde_json::to_value(&f.policy).unwrap();
    assert!(policy_value["resources"].get("child_network").is_none());
    let decoded: GpuBudgetPolicy = serde_json::from_value(policy_value.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), policy_value);
    let signature =
        json!({"request":l.request,"body":l.body().unwrap(),"policy":f.policy}).to_string();
    let db = rusqlite::Connection::open(f.dir.join("control.db")).unwrap();
    db.execute(
        "UPDATE ion_team_gpu_jobs SET policy=?,signature=? WHERE request_id=?",
        rusqlite::params![
            policy_value.to_string(),
            signature,
            l.request.request_id.to_string()
        ],
    )
    .unwrap();
    db.execute(
        "DELETE FROM ion_team_gpu_preflight WHERE request_id=?",
        [l.request.request_id.to_string()],
    )
    .unwrap();
    drop(db);
    let restarted = TeamCoordinator::new(f.dir.join("control.db"));
    let server = Server::new(vec![]);
    let c = client(&server);
    let original = launch_once_with(&c, &restarted, &f.lease, &l, &f.policy).unwrap();
    assert_eq!(original.request_id, l.request.request_id);
    assert_eq!(original.state, "submitting");
    let new = launch();
    assert!(
        launch_once_with(&c, &restarted, &f.lease, &new, &f.policy)
            .err()
            .unwrap()
            .to_string()
            .contains("child socket boundary")
    );
    assert!(server.finish().is_empty());
    assert_eq!(restarted.gpu_jobs(f.lease.attempt_id()).unwrap().len(), 1);
    assert!(restarted.has_pending_effect(f.lease.run_id()).unwrap());
}
