use super::super::tests::{Fixture, bytes, client, launch};
use super::*;
use crate::driver::team_test_http::Server;
use crate::teams::jobs::{JobOutput, JobStream};
pub(super) fn volume() -> GpuResultVolume {
    GpuResultVolume {
        version: 1,
        bucket_id: "storagebucket-fixture".into(),
        bucket_name: "ion-results".into(),
        region: "eu-north1".into(),
        prefix: "ion/results".into(),
    }
}
fn bucket() -> Value {
    json!({"metadata":{"id":"storagebucket-fixture","parentId":"project-fixture","name":"ion-results"},
        "status":{"state":"ACTIVE","suspensionState":"NOT_SUSPENDED","bucketType":"REGULAR","region":"eu-north1","domainName":"storage.eu-north1.nebius.cloud","anonymousAccessEnabled":false}})
}
fn reader(server: &Server) -> ResultReader {
    ResultReader {
        http: reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap(),
        access_key: "AKIDEXAMPLE".into(),
        secret_key: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY".into(),
        session_token: None,
        base: Some(server.url.clone()),
    }
}
pub(super) fn receipt(request: &JobRequest) -> JobResponse {
    JobResponse {
        project_sha256: None,
        version: 1,
        request_id: request.request_id,
        exit_code: 7,
        timed_out: false,
        canceled: false,
        elapsed_milliseconds: 500,
        stdout: JobStream {
            content_base64: STANDARD.encode(b"actual training output"),
            bytes: 22,
            truncated: false,
        },
        stderr: JobStream {
            content_base64: STANDARD.encode(b"failed check"),
            bytes: 12,
            truncated: false,
        },
        outputs: vec![JobOutput {
            path: "result.bin".into(),
            content_base64: STANDARD.encode(b"\0\xffcheckpoint"),
            sha256: hex(&Sha256::digest(b"\0\xffcheckpoint")),
            bytes: 12,
        }],
        output_errors: vec![],
    }
}
pub(super) fn setup(terminal: bool) -> (Fixture, Launch) {
    let mut f = Fixture::new(true, true);
    let mut l = launch();
    l.request.output_paths = vec!["result.bin".into()];
    l.result_volume = Some(volume());
    f.policy.result_volume = l.result_volume.clone();
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
    if terminal {
        f.coordinator
            .record_gpu_observation(
                l.request.request_id,
                "aijob-fixture",
                "COMPLETED",
                &["compute-fixture".into()],
                now,
            )
            .unwrap();
    }
    (f, l)
}
#[test]
fn team_gpu_result_signatures_match_independent_aws_botocore_vectors_with_session_token() {
    // Golden vectors generated independently with AWS botocore 1.40.0 S3SigV4Auth.
    // https://github.com/boto/botocore/blob/1.40.0/botocore/auth.py
    let server = Server::new(vec![]);
    let mut r = reader(&server);
    let at = chrono::DateTime::parse_from_rfc3339("2026-10-07T01:02:03Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let uri = "/ion-results/ion/results/12345678-1234-4234-8234-123456789abc/receipt.json";
    for (token, signed, signature) in [
        (
            None,
            "host;x-amz-content-sha256;x-amz-date",
            "941f3a56fe7fe4847c6ab48eca7b95b36b53a751c7d21895343069ebf31e696c",
        ),
        (
            Some("fixture-session-token"),
            "host;x-amz-content-sha256;x-amz-date;x-amz-security-token",
            "24a0809899ec2dbd7581f94a9f1fc21bc4ac76ec2ed9039b1a97b78adf0b956a",
        ),
    ] {
        r.session_token = token.map(str::to_owned);
        let headers = r
            .headers("storage.eu-north1.nebius.cloud", uri, "eu-north1", at)
            .unwrap();
        assert_eq!(
            headers["authorization"],
            format!(
                "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20261007/eu-north1/s3/aws4_request, SignedHeaders={signed}, Signature={signature}"
            )
        );
        assert!(headers["authorization"].is_sensitive());
        assert_eq!(
            headers["x-amz-content-sha256"],
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        if token.is_some() {
            assert!(headers["x-amz-security-token"].is_sensitive());
        }
    }
    assert!(server.finish().is_empty());
}
#[test]
fn team_gpu_result_volume_launch_preflight_and_exact_replay_keep_approved_bucket_without_credentials()
 {
    let mut f = Fixture::new(true, true);
    let mut l = launch();
    l.result_volume = Some(volume());
    f.policy.result_volume = l.result_volume.clone();
    f.policy.resources = Some(super::super::preflight::tests::policy(&l));
    let mut replies =
        super::super::preflight::tests::launch_responses(&l, f.policy.resources.as_ref().unwrap());
    replies.extend(vec![
        (200, bytes(bucket())),
        (
            200,
            bytes(json!({"id":"operation-create","resourceId":"aijob-fixture"})),
        ),
    ]);
    let server = Server::new(replies);
    let c = client(&server);
    launch_once_with(&c, &f.coordinator, &f.lease, &l, &f.policy).unwrap();
    launch_once_with(&c, &f.coordinator, &f.lease, &l, &f.policy).unwrap();
    let requests = server.finish();
    assert_eq!(requests.len(), 16);
    assert!(
        requests[14]
            .line
            .starts_with("GET /storage/v1/buckets/storagebucket-fixture ")
    );
    assert_eq!(
        requests[15].body["spec"]["volumes"],
        json!([volume().mount()])
    );
    assert_eq!(
        requests[15].body["spec"]["args"],
        "--request-file /ion/job-request.json --result-volume /ion/results"
    );
    assert!(!requests[15].body.to_string().contains("AKIDEXAMPLE"));
    let mut changed = l;
    changed.result_volume.as_mut().unwrap().prefix = "other".into();
    assert!(launch_once_with(&c, &f.coordinator, &f.lease, &changed, &f.policy).is_err());
}
#[test]
fn team_gpu_result_bucket_and_grant_errors_fail_before_create() {
    let mut denied = Fixture::new(true, false);
    let mut l = launch();
    l.result_volume = Some(volume());
    denied.policy.result_volume = l.result_volume.clone();
    let server = Server::new(vec![]);
    assert!(
        launch_once_with(
            &client(&server),
            &denied.coordinator,
            &denied.lease,
            &l,
            &denied.policy
        )
        .is_err()
    );
    assert!(server.finish().is_empty());
    assert!(
        denied
            .coordinator
            .gpu_jobs(denied.lease.attempt_id())
            .unwrap()
            .is_empty()
    );
    let edits = [
        ("/metadata/parentId", json!("other")),
        ("/metadata/id", json!("storagebucket-other")),
        ("/metadata/name", json!("other")),
        ("/status/region", json!("eu-west1")),
        ("/status/domainName", json!("attacker.example")),
        ("/status/anonymousAccessEnabled", json!(true)),
        ("/status/anonymousAccessEnabled", json!("false")),
        ("/status/bucketType", json!("FILESYSTEM")),
        ("/status/state", json!("UPDATING")),
        ("/status/suspensionState", json!("SUSPENDED")),
    ];
    let bad: Vec<_> = edits
        .into_iter()
        .map(|(key, value)| {
            let mut b = bucket();
            *b.pointer_mut(key).unwrap() = value;
            (200, bytes(b))
        })
        .collect();
    let server = Server::new(bad);
    let c = client(&server);
    for _ in 0..10 {
        assert!(c.verify_result_bucket(&volume()).is_err());
    }
    assert!(server.finish().iter().all(|r| r.line.starts_with("GET ")));
    for prefix in [
        "../outside",
        "x//y",
        "x/.env",
        "https://example",
        "x?redirect=evil",
        "é",
        "x/",
    ] {
        let mut v = volume();
        v.prefix = prefix.into();
        assert!(v.validate().is_err());
    }
    let mut v = volume();
    v.region = "eu-north1.attacker".into();
    assert!(v.validate().is_err());
}
#[test]
fn team_gpu_result_receipt_artifact_binary_pages_and_exact_replay_survive_restart_with_reservations()
 {
    let (f, l) = setup(true);
    let result = receipt(&l.request);
    let api = Server::new(vec![(200, bytes(bucket()))]);
    let storage = Server::new(vec![(200, serde_json::to_vec(&result).unwrap())]);
    let record = f
        .coordinator
        .gpu_jobs(f.lease.attempt_id())
        .unwrap()
        .remove(0);
    assert!(collect_record(&reader(&storage), &client(&api), &f.coordinator, &record).unwrap());
    let http = storage.finish();
    assert_eq!(http.len(), 1);
    assert!(http[0].line.starts_with(&format!(
        "GET /ion-results/{}/{}",
        volume().prefix,
        l.request.request_id
    )));
    assert!(http[0].headers.contains("AWS4-HMAC-SHA256"));
    assert!(!http[0].headers.contains("wJalrXUtn"));
    assert_eq!(api.finish().len(), 1);
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    let saved = reopened.gpu_jobs(f.lease.attempt_id()).unwrap().remove(0);
    let artifact = saved.result_artifact.unwrap();
    assert_eq!(saved.result_receipt.as_ref().unwrap().exit_code, 7);
    assert_eq!(saved.state, "terminal");
    assert!(reopened.has_pending_effect(f.lease.run_id()).unwrap());
    assert!(
        !reopened
            .remote_attempt_stopped(f.lease.attempt_id())
            .unwrap()
    );
    let revision = reopened.snapshot(f.lease.run_id()).unwrap().revision;
    assert_eq!(
        reopened
            .record_gpu_result(
                l.request.request_id,
                "aijob-fixture",
                &result,
                crate::model::unix_time()
            )
            .unwrap(),
        artifact
    );
    assert_eq!(
        reopened.snapshot(f.lease.run_id()).unwrap().revision,
        revision
    );
    let page: Value = serde_json::from_str(
        &read_output(&reopened, &f.lease, l.request.request_id, "result.bin", 2).unwrap(),
    )
    .unwrap();
    assert_eq!(
        STANDARD
            .decode(page["content_base64"].as_str().unwrap())
            .unwrap(),
        b"checkpoint"
    );
    assert_eq!(page["artifact_id"], artifact.to_string());
    assert!(page["next_offset"].is_null());
    assert!(read_output(&reopened, &f.lease, l.request.request_id, "result.bin", 13).is_err());
    let other = Fixture::new(true, true);
    assert!(
        read_output(
            &reopened,
            &other.lease,
            l.request.request_id,
            "result.bin",
            0
        )
        .is_err()
    );
    let mut changed = result.clone();
    changed.exit_code = 0;
    assert!(
        reopened
            .record_gpu_result(
                l.request.request_id,
                "aijob-fixture",
                &changed,
                crate::model::unix_time()
            )
            .is_err()
    );
    // Saved results need neither current credentials nor fresh network requests.
    let empty = Server::new(vec![]);
    assert!(collect_record(&reader(&empty), &client(&empty), &reopened, &saved).unwrap());
    assert!(empty.finish().is_empty());
    let documents = crate::control::ControlStore::new(f.dir.join("control.db"));
    let summary: Value =
        serde_json::from_slice(&documents.read_document(artifact).unwrap()).unwrap();
    assert_eq!(summary["exit_code"], 7);
    assert_eq!(summary["stderr"]["preview"], "failed check");
}
#[test]
fn team_gpu_result_invalid_hash_contract_identity_and_early_receipts_cannot_publish() {
    let (f, l) = setup(false);
    let r = receipt(&l.request);
    assert!(
        f.coordinator
            .record_gpu_result(
                l.request.request_id,
                "aijob-fixture",
                &r,
                crate::model::unix_time()
            )
            .is_err()
    );
    f.coordinator
        .record_gpu_observation(
            l.request.request_id,
            "aijob-fixture",
            "COMPLETED",
            &[],
            crate::model::unix_time(),
        )
        .unwrap();
    let cases = [
        serde_json::json!({"version":2}),
        serde_json::json!({"request_id":Uuid::new_v4()}),
        serde_json::json!({"outputs":[]}),
        serde_json::json!({"output_errors":[{"path":"unexpected.txt","reason":"absent"}]}),
        serde_json::json!({"stdout":{"content_base64":"YQ==","bytes":0,"truncated":false}}),
    ];
    for edit in cases {
        let mut value = serde_json::to_value(&r).unwrap();
        for (k, v) in edit.as_object().unwrap() {
            value[k] = v.clone();
        }
        let bad: JobResponse = serde_json::from_value(value).unwrap();
        assert!(
            f.coordinator
                .record_gpu_result(
                    l.request.request_id,
                    "aijob-fixture",
                    &bad,
                    crate::model::unix_time()
                )
                .is_err()
        );
    }
    let mut bad = r.clone();
    bad.outputs[0].sha256 = "0".repeat(64);
    let api = Server::new(vec![(200, bytes(bucket()))]);
    let storage = Server::new(vec![(200, serde_json::to_vec(&bad).unwrap())]);
    let record = f
        .coordinator
        .gpu_jobs(f.lease.attempt_id())
        .unwrap()
        .remove(0);
    assert!(collect_record(&reader(&storage), &client(&api), &f.coordinator, &record).is_err());
    api.finish();
    storage.finish();
    assert!(
        f.coordinator.gpu_jobs(f.lease.attempt_id()).unwrap()[0]
            .result_artifact
            .is_none()
    );
    assert!(
        f.coordinator
            .record_gpu_result(
                l.request.request_id,
                "aijob-other",
                &r,
                crate::model::unix_time()
            )
            .is_err()
    );
}
#[test]
fn team_gpu_result_storage_missing_auth_malformed_partial_and_oversize_remain_unresolved() {
    let request = Uuid::new_v4();
    let server = Server::new(vec![
        (404, b"not found".to_vec()),
        (403, b"secret-bearing upstream error".to_vec()),
        (503, vec![]),
        (302, vec![]),
        (206, vec![]),
        (200, b"{}".to_vec()),
        (200, vec![b'x'; MAX_RECEIPT as usize + 1]),
    ]);
    let r = reader(&server);
    assert!(r.read(&volume(), request).unwrap().is_none());
    for _ in 0..6 {
        let message = r.read(&volume(), request).err().unwrap().to_string();
        assert!(!message.contains("secret-bearing"));
    }
    assert_eq!(server.finish().len(), 7);
}
#[test]
fn team_gpu_result_without_terminal_observation_or_approved_volume_does_no_storage_read() {
    let (f, _) = setup(false);
    let record = f
        .coordinator
        .gpu_jobs(f.lease.attempt_id())
        .unwrap()
        .remove(0);
    let server = Server::new(vec![]);
    assert!(!collect_record(&reader(&server), &client(&server), &f.coordinator, &record).unwrap());
    assert!(server.finish().is_empty());
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
        .record_gpu_observation(l.request.request_id, "aijob-fixture", "COMPLETED", &[], now)
        .unwrap();
    let mut r = receipt(&l.request);
    r.outputs.clear();
    assert!(
        f.coordinator
            .record_gpu_result(l.request.request_id, "aijob-fixture", &r, now)
            .is_err()
    );
}

#[test]
fn team_gpu_maintenance_collects_late_canceled_receipt_after_restart_without_rerunning_or_releasing_money()
 {
    use crate::teams::AssignmentOutcome;
    use waku_protocol::teams::TeamRunStatus;
    let (f, l) = setup(true);
    super::wait_tests::finish_cleanup(&f, &l);
    let now = crate::model::unix_time();
    let run = f.coordinator.snapshot(f.lease.run_id()).unwrap();
    f.coordinator.stop(run.id, run.revision, now).unwrap();
    let stopped = f
        .coordinator
        .report(&f.lease, AssignmentOutcome::Canceled, now)
        .unwrap();
    assert_eq!(stopped.status, TeamRunStatus::Canceled);
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    assert!(reopened.dispatch_runs(None, 32).unwrap().is_empty());
    let api = Server::new(vec![(200, bytes(bucket())), (200, bytes(bucket()))]);
    let objects = Server::new(vec![
        (404, vec![]),
        (200, serde_json::to_vec(&receipt(&l.request)).unwrap()),
    ]);
    let c = client(&api);
    let claim = reopened.claim_gpu_maintenance(now, &[]).unwrap().unwrap();
    let record = reopened.gpu_jobs(claim.attempt_id).unwrap().remove(0);
    assert!(
        !super::super::progress_saved_with(
            &c,
            &reopened,
            &record,
            claim.attempt_id,
            |c, coordinator, record, attempt| collect_saved_with(
                c,
                coordinator,
                record,
                attempt,
                || Ok(reader(&objects))
            )
        )
        .unwrap()
    );
    reopened.finish_gpu_maintenance(&claim, false, now).unwrap();
    assert!(reopened.has_pending_effect(run.id).unwrap());
    assert!(
        reopened.gpu_jobs(claim.attempt_id).unwrap()[0]
            .result_receipt
            .is_none()
    );
    let restarted = TeamCoordinator::new(f.dir.join("control.db"));
    let next = restarted
        .claim_gpu_maintenance(now + 5, &[])
        .unwrap()
        .unwrap();
    assert_eq!(next.request_id, claim.request_id);
    assert!(
        super::super::progress_saved_with(
            &c,
            &restarted,
            &record,
            next.attempt_id,
            |c, coordinator, record, attempt| collect_saved_with(
                c,
                coordinator,
                record,
                attempt,
                || Ok(reader(&objects))
            )
        )
        .unwrap()
    );
    restarted
        .finish_gpu_maintenance(&next, true, now + 5)
        .unwrap();
    let saved = restarted.gpu_jobs(next.attempt_id).unwrap().remove(0);
    assert_eq!(saved.state, "settled");
    assert_eq!(saved.result_receipt.as_ref().unwrap().exit_code, 7);
    assert_eq!(
        saved.result_receipt.as_ref().unwrap().outputs[0].sha256,
        receipt(&l.request).outputs[0].sha256
    );
    assert!(saved.result_artifact.is_some());
    assert_eq!(
        restarted.snapshot(run.id).unwrap().status,
        TeamRunStatus::Canceled
    );
    assert!(!restarted.has_pending_effect(run.id).unwrap());
    assert!(
        restarted
            .claim_gpu_maintenance(now + 1000, &[])
            .unwrap()
            .is_none()
    );
    let revision = restarted.snapshot(run.id).unwrap().revision;
    // The actual production entry point replays saved proofs without any credentials.
    assert!(
        super::super::progress_saved_job(&restarted, next.attempt_id, next.request_id).unwrap()
    );
    assert!(super::super::progress_saved_job(&restarted, Uuid::new_v4(), next.request_id).is_err());
    assert_eq!(restarted.snapshot(run.id).unwrap().revision, revision);
    let db = rusqlite::Connection::open(f.dir.join("control.db")).unwrap();
    let (money,count):(u64,u64)=db.query_row("SELECT reserved_micros,(SELECT count(*) FROM ion_team_gpu_results) FROM ion_team_gpu_jobs WHERE request_id=?",[next.request_id.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert_eq!(money, f.policy.reservation_micros);
    assert_eq!(count, 1);
    assert!(
        api.finish()
            .iter()
            .all(|r| r.line.starts_with("GET /storage/v1/buckets/"))
    );
    let reads = objects.finish();
    assert_eq!(reads.len(), 2);
    assert_eq!(reads[0].line, reads[1].line);
}
#[test]
fn team_gpu_maintenance_retains_real_result_even_when_cleanup_observation_fails() {
    let (f, l) = setup(true);
    let api = Server::new(vec![(500, bytes(json!({}))), (200, bytes(bucket()))]);
    let objects = Server::new(vec![(
        200,
        serde_json::to_vec(&receipt(&l.request)).unwrap(),
    )]);
    let record = f
        .coordinator
        .gpu_jobs(f.lease.attempt_id())
        .unwrap()
        .remove(0);
    assert!(
        super::super::progress_saved_with(
            &client(&api),
            &f.coordinator,
            &record,
            f.lease.attempt_id(),
            |c, coordinator, record, attempt| collect_saved_with(
                c,
                coordinator,
                record,
                attempt,
                || Ok(reader(&objects))
            )
        )
        .is_err()
    );
    let saved = f
        .coordinator
        .gpu_jobs(f.lease.attempt_id())
        .unwrap()
        .remove(0);
    assert_eq!(saved.result_receipt.as_ref().unwrap().exit_code, 7);
    assert!(saved.result_artifact.is_some());
    assert_eq!(saved.state, "terminal");
    assert!(f.coordinator.has_pending_effect(f.lease.run_id()).unwrap());
    assert!(api.finish().iter().all(|r| r.line.starts_with("GET ")));
    assert_eq!(objects.finish().len(), 1);
}
#[test]
fn team_gpu_maintenance_missing_result_credentials_do_not_undo_cleanup_or_claim_success() {
    let (f, l) = setup(true);
    super::wait_tests::finish_cleanup(&f, &l);
    let api = Server::new(vec![]);
    let record = f
        .coordinator
        .gpu_jobs(f.lease.attempt_id())
        .unwrap()
        .remove(0);
    let error = super::super::progress_saved_with(
        &client(&api),
        &f.coordinator,
        &record,
        f.lease.attempt_id(),
        |c, coordinator, record, attempt| {
            collect_saved_with(c, coordinator, record, attempt, || {
                anyhow::bail!("S3 credentials unavailable")
            })
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("S3 credentials unavailable"));
    assert!(
        record
            .cleanup
            .as_ref()
            .is_some_and(|c| c.state == "verified")
    );
    assert!(
        f.coordinator.gpu_jobs(f.lease.attempt_id()).unwrap()[0]
            .result_receipt
            .is_none()
    );
    assert!(f.coordinator.has_pending_effect(f.lease.run_id()).unwrap());
    assert!(api.finish().is_empty());
}

#[test]
fn global_usage_gpu_allowance_and_exact_result_stay_separate_from_costs_after_stop() {
    let (f, l) = setup(true);
    let control = crate::control::ControlStore::new(f.dir.join("control.db"));
    let page = control
        .usage_ledger(crate::usage_ledger::all_query())
        .unwrap();
    assert_eq!(page.summary.records, 1);
    let e = &page.entries[0];
    assert_eq!(e.kind, waku_protocol::usage_ledger::UsageLedgerKind::GpuJob);
    assert_eq!(
        e.environment,
        Some(waku_protocol::teams::ExecutionEnvironment::NebiusSandbox)
    );
    assert_eq!(e.host, "project-fixture");
    assert!(e.cost.is_none() && e.elapsed_milliseconds.is_none());
    let allowance = e.spending_allowance.clone().unwrap();
    assert_eq!(allowance.currency, f.policy.currency);
    assert_eq!(allowance.micros, f.policy.reservation_micros);
    let run = f.coordinator.snapshot(f.lease.run_id()).unwrap();
    f.coordinator
        .stop(run.id, run.revision, crate::model::unix_time())
        .unwrap();
    let result = receipt(&l.request);
    f.coordinator
        .record_gpu_result(
            l.request.request_id,
            "aijob-fixture",
            &result,
            crate::model::unix_time(),
        )
        .unwrap();
    let page = control
        .usage_ledger(crate::usage_ledger::all_query())
        .unwrap();
    assert_eq!(page.summary.elapsed_milliseconds, 500);
    assert_eq!(page.summary.unknown_cost_records, 1);
    assert!(page.summary.costs.is_empty());
    assert_eq!(
        page.entries[0].spending_allowance.as_ref().unwrap().micros,
        allowance.micros
    );
    let revision = page.revision;
    f.coordinator
        .record_gpu_result(
            l.request.request_id,
            "aijob-fixture",
            &result,
            crate::model::unix_time(),
        )
        .unwrap();
    assert_eq!(
        control
            .usage_ledger(crate::usage_ledger::all_query())
            .unwrap()
            .revision,
        revision
    );
    let wire = serde_json::to_string(&page).unwrap();
    assert!(
        !wire.contains("actual training output")
            && !wire.contains("checkpoint")
            && !wire.contains("content_base64")
    );
}
