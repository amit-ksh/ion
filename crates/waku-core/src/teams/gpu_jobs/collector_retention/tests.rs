use super::*;
use rusqlite::Connection;

fn policy(receipts: u32) -> GpuCollectorRetentionPolicy {
    GpuCollectorRetentionPolicy {
        version: 1,
        mode: GpuCollectorRetentionMode::KeepUntilExplicitlyCleaned,
        max_bytes: RECEIPT_BYTES * u64::from(receipts),
        max_receipts: receipts,
    }
}
fn volume(prefix: &str, receipts: u32) -> GpuResultVolume {
    GpuResultVolume {
        version: 1,
        bucket_id: "storagebucket-fixture".into(),
        bucket_name: "ion-results".into(),
        region: "eu-north1".into(),
        prefix: prefix.into(),
        retention: Some(policy(receipts)),
    }
}
fn db() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE ion_team_gpu_jobs(request_id TEXT PRIMARY KEY,project TEXT NOT NULL,policy TEXT NOT NULL,signature TEXT NOT NULL,body TEXT NOT NULL,state TEXT NOT NULL);
        CREATE TABLE ion_team_gpu_results(request_id TEXT PRIMARY KEY,receipt TEXT NOT NULL,artifact_id TEXT NOT NULL);").unwrap();
    db
}
fn insert(db: &Connection, volume: &GpuResultVolume, state: &str, observed: bool) -> Uuid {
    let id = Uuid::new_v4();
    let body = json!({"metadata":{"parentId":"project-fixture","name":format!("ion-gpu-{id}"),"labels":{"ion-request":id}},
        "spec":{"args":"--request-file /ion/job-request.json --result-volume /ion/results","volumes":[volume.mount()]}});
    let policy = json!({"result_volume":volume});
    let signature = json!({"request":{"request_id":id},"body":body,"policy":policy});
    db.execute(
        "INSERT INTO ion_team_gpu_jobs VALUES(?,?,?,?,?,?)",
        params![
            id.to_string(),
            "project-fixture",
            policy.to_string(),
            signature.to_string(),
            body.to_string(),
            state
        ],
    )
    .unwrap();
    if observed {
        db.execute(
            "INSERT INTO ion_team_gpu_results VALUES(?,?,?)",
            params![
                id.to_string(),
                json!({"request_id":id,"note":"actual λ receipt"}).to_string(),
                Uuid::new_v4().to_string()
            ],
        )
        .unwrap();
    }
    id
}
fn review(db: &mut Connection, volume: &GpuResultVolume) -> GpuCollectorUsage {
    let tx = db.transaction().unwrap();
    usage(&tx, volume).unwrap()
}
fn admits(db: &mut Connection, volume: &GpuResultVolume) -> bool {
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    reserve(&tx, volume, Uuid::new_v4()).is_ok()
}
fn mutate(db: &Connection, id: Uuid, field: &str, edit: impl FnOnce(&mut Value)) {
    assert!(matches!(field, "body" | "signature" | "policy"));
    let old: String = db
        .query_row(
            &format!("SELECT {field} FROM ion_team_gpu_jobs WHERE request_id=?"),
            [id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    let mut value: Value = serde_json::from_str(&old).unwrap();
    edit(&mut value);
    db.execute(
        &format!("UPDATE ion_team_gpu_jobs SET {field}=? WHERE request_id=?"),
        params![value.to_string(), id.to_string()],
    )
    .unwrap();
}
#[test]
fn gpu_collector_historical_omission_keeps_exact_volume_and_manual_default() {
    let mut legacy = volume("ion/results", 4);
    legacy.retention = None;
    legacy.validate().unwrap();
    assert!(legacy.require_retention().is_err());
    let wire = serde_json::to_value(&legacy).unwrap();
    assert!(wire.get("retention").is_none());
    assert_eq!(
        serde_json::from_value::<GpuResultVolume>(wire).unwrap(),
        legacy
    );
    assert_eq!(
        GpuCollectorRetentionMode::default(),
        GpuCollectorRetentionMode::KeepUntilExplicitlyCleaned
    );
    let mut db = db();
    assert_eq!(review(&mut db, &legacy).admissible_receipts, None);
    assert!(!admits(&mut db, &legacy));
}
#[test]
fn gpu_collector_policy_is_strictly_bounded_and_has_no_automatic_cleanup_mode() {
    for changed in [
        json!({"version":2,"mode":"keepUntilExplicitlyCleaned","max_bytes":RECEIPT_BYTES,"max_receipts":1}),
        json!({"version":1,"mode":"keepUntilExplicitlyCleaned","max_bytes":RECEIPT_BYTES-1,"max_receipts":1}),
        json!({"version":1,"mode":"keepUntilExplicitlyCleaned","max_bytes":MAX_SCOPE_BYTES+1,"max_receipts":1}),
        json!({"version":1,"mode":"keepUntilExplicitlyCleaned","max_bytes":RECEIPT_BYTES,"max_receipts":0}),
        json!({"version":1,"mode":"keepUntilExplicitlyCleaned","max_bytes":RECEIPT_BYTES,"max_receipts":4097}),
    ] {
        assert!(
            serde_json::from_value::<GpuCollectorRetentionPolicy>(changed)
                .unwrap()
                .validate()
                .is_err()
        );
    }
    for changed in [
        json!({"version":1,"mode":"expire","max_bytes":RECEIPT_BYTES,"max_receipts":1}),
        json!({"version":1,"mode":"keepUntilExplicitlyCleaned","max_bytes":RECEIPT_BYTES,"max_receipts":1,"ttl_seconds":1}),
    ] {
        assert!(serde_json::from_value::<GpuCollectorRetentionPolicy>(changed).is_err());
    }
}
#[test]
fn gpu_collector_receipt_and_encoded_byte_ceilings_each_block_new_admission() {
    let mut db = db();
    let mut approved = volume("ion/results", 2);
    insert(&db, &approved, "submitting", false);
    assert!(admits(&mut db, &approved));
    approved.retention.as_mut().unwrap().max_bytes = RECEIPT_BYTES;
    assert!(!admits(&mut db, &approved));
    approved.retention = Some(policy(2));
    approved.retention.as_mut().unwrap().max_receipts = 1;
    assert!(!admits(&mut db, &approved));
}
#[test]
fn gpu_collector_uncertain_terminal_cleaned_and_settled_originals_all_keep_holds() {
    let mut db = db();
    let approved = volume("ion/results", 4);
    for state in ["submitting", "terminal", "cleaned", "settled"] {
        insert(&db, &approved, state, state == "settled");
    }
    let facts = review(&mut db, &approved);
    assert_eq!(facts.reserved_receipts, 4);
    assert_eq!(facts.reserved_bytes, 4 * RECEIPT_BYTES);
    assert_eq!(facts.observed_receipts, 1);
    assert!(facts.observed_bytes > 0 && facts.observed_bytes < RECEIPT_BYTES);
    assert_eq!(facts.admissible_receipts, Some(0));
    assert!(!admits(&mut db, &approved));
    assert_eq!(
        db.query_row::<u32, _, _>("SELECT count(*) FROM ion_team_gpu_results", [], |r| r
            .get(0))
            .unwrap(),
        1
    );
}
#[test]
fn gpu_collector_prior_ceiling_cannot_be_loosened_by_a_later_owner_policy() {
    let mut db = db();
    let original = volume("ion/results", 1);
    insert(&db, &original, "settled", true);
    let new = volume("ion/results", 4096);
    let facts = review(&mut db, &new);
    assert_eq!(facts.scopes[0].max_receipts, Some(1));
    assert_eq!(facts.scopes[0].max_bytes, Some(RECEIPT_BYTES));
    assert!(!admits(&mut db, &new));
}
#[test]
fn gpu_collector_ancestor_caps_include_sibling_descendants_outside_current_prefix() {
    let mut db = db();
    let parent = volume("ion", 2);
    insert(&db, &parent, "settled", true);
    insert(&db, &volume("ion/team-a", 100), "cleaned", true);
    let sibling = volume("ion/team-b", 100);
    let facts = review(&mut db, &sibling);
    assert_eq!(facts.reserved_receipts, 1); // The ancestor itself overlaps this child.
    assert_eq!(facts.bucket_reserved_receipts, 2);
    let enclosing = facts.scopes.iter().find(|s| s.prefix == "ion").unwrap();
    assert_eq!(enclosing.reserved_receipts, 2);
    assert_eq!(enclosing.remaining_receipts, Some(0));
    assert!(!admits(&mut db, &sibling));
}
#[test]
fn gpu_collector_child_caps_survive_launch_from_a_broader_parent_prefix() {
    let mut db = db();
    let child = volume("ion/results/team-a", 1);
    insert(&db, &child, "cleaned", false);
    let ancestor = volume("ion/results", 100);
    assert!(!admits(&mut db, &ancestor));
    assert_eq!(review(&mut db, &ancestor).scopes.len(), 2);
}
#[test]
fn gpu_collector_disjoint_component_prefixes_do_not_share_caps() {
    let mut db = db();
    insert(&db, &volume("ion/results", 1), "settled", true);
    assert!(admits(&mut db, &volume("ion/results-two", 1)));
    let facts = review(&mut db, &volume("ion/results-two", 1));
    assert_eq!(facts.reserved_receipts, 0);
    assert_eq!(facts.bucket_reserved_receipts, 1);
    assert_eq!(facts.admissible_receipts, Some(1));
}
#[test]
fn gpu_collector_historical_unknown_cap_retains_original_bytes_without_fabricating_deletion() {
    let mut db = db();
    let mut historical = volume("ion/results", 2);
    historical.retention = None;
    insert(&db, &historical, "cleaned", false);
    let approved = volume("ion/results", 1);
    let facts = review(&mut db, &approved);
    assert_eq!(facts.historical_unbounded_receipts, 1);
    assert_eq!(facts.reserved_bytes, RECEIPT_BYTES);
    assert_eq!(facts.observed_bytes, 0);
    assert!(!admits(&mut db, &approved));
}
#[test]
fn gpu_collector_other_bucket_is_scoped_without_releasing_retained_current_bucket() {
    let mut db = db();
    let approved = volume("ion/results", 1);
    let mut other = approved.clone();
    other.bucket_id = "storagebucket-other".into();
    other.bucket_name = "other-results".into();
    for _ in 0..4100 {
        insert(&db, &other, "settled", false);
    }
    assert!(admits(&mut db, &approved));
    assert_eq!(review(&mut db, &approved).bucket_reserved_receipts, 0);
}
#[test]
fn gpu_collector_same_named_bucket_in_other_region_has_a_distinct_safe_identity() {
    let mut db = db();
    let approved = volume("ion/results", 1);
    let mut other = approved.clone();
    other.bucket_id = "storagebucket-other".into();
    other.region = "eu-west1".into();
    insert(&db, &other, "settled", true);
    assert!(admits(&mut db, &approved));
}
#[test]
fn gpu_collector_same_origin_alias_or_changed_region_blocks_new_admission() {
    let approved = volume("ion/results", 10);
    for changed in 0..3 {
        let mut db = db();
        let mut old = approved.clone();
        if changed == 0 {
            old.bucket_id = "storagebucket-other".into();
        }
        if changed == 1 {
            old.bucket_name = "changed-results".into();
        }
        if changed == 2 {
            old.region = "eu-west1".into();
        }
        insert(&db, &old, "settled", true);
        assert!(!admits(&mut db, &approved));
    }
}
#[test]
fn gpu_collector_original_request_cannot_be_reserved_again_even_after_cleanup() {
    let mut db = db();
    let approved = volume("ion/results", 2);
    let request = insert(&db, &approved, "cleaned", true);
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    assert!(reserve(&tx, &approved, request).is_err());
    assert_eq!(usage(&tx, &approved).unwrap().reserved_receipts, 1);
}
#[test]
fn gpu_collector_changed_original_policy_mount_and_request_witnesses_are_rejected() {
    let approved = volume("ion/results", 10);
    for changed in 0..7 {
        let mut db = db();
        let id = insert(&db, &approved, "settled", true);
        match changed {
            0 => mutate(&db, id, "policy", |v| {
                v["result_volume"]["retention"]["max_receipts"] = json!(100)
            }),
            1 => mutate(&db, id, "signature", |v| {
                v["request"]["request_id"] = json!(Uuid::new_v4())
            }),
            2 => mutate(&db, id, "body", |v| {
                v["spec"]["volumes"][0]["sourcePath"] = json!("changed")
            }),
            3 => mutate(&db, id, "body", |v| {
                v["metadata"]["labels"]["ion-request"] = json!(Uuid::new_v4())
            }),
            4 => mutate(&db, id, "signature", |v| {
                v["body"]["spec"]["args"] = json!("changed")
            }),
            5 => mutate(&db, id, "signature", |v| {
                v["policy"]["result_volume"] = Value::Null
            }),
            _ => mutate(&db, id, "body", |v| {
                v["metadata"]["parentId"] = json!("project-changed")
            }),
        }
        assert!(!admits(&mut db, &approved), "changed witness {changed}");
    }
}
#[test]
fn gpu_collector_unknown_or_malformed_identity_is_not_excluded_as_another_bucket() {
    let approved = volume("ion/results", 2);
    for changed in 0..5 {
        let mut db = db();
        let id = insert(&db, &approved, "submitting", false);
        match changed {
            0 => {
                db.execute(
                    "UPDATE ion_team_gpu_jobs SET policy='invalid' WHERE request_id=?",
                    [id.to_string()],
                )
                .unwrap();
            }
            1 => mutate(&db, id, "policy", |v| v["result_volume"] = json!("unknown")),
            2 => mutate(&db, id, "policy", |v| {
                v["result_volume"]["bucket_id"] = Value::Null
            }),
            3 => mutate(&db, id, "policy", |v| v["result_volume"] = Value::Null),
            _ => mutate(&db, id, "policy", |v| {
                v["result_volume"]["bucket_id"] = json!("another-bucket")
            }),
        }
        assert!(!admits(&mut db, &approved), "unknown identity {changed}");
    }
}
#[test]
fn gpu_collector_changed_retained_receipt_provenance_and_oversize_are_rejected() {
    let approved = volume("ion/results", 10);
    for changed in 0..4 {
        let mut db = db();
        let id = insert(&db, &approved, "settled", true);
        match changed {
            0 => {
                db.execute(
                    "UPDATE ion_team_gpu_results SET receipt=?",
                    [json!({"request_id":Uuid::new_v4()}).to_string()],
                )
                .unwrap();
            }
            1 => {
                db.execute(
                    "UPDATE ion_team_gpu_results SET artifact_id=?",
                    [Uuid::nil().to_string()],
                )
                .unwrap();
            }
            2 => {
                db.execute("UPDATE ion_team_gpu_results SET receipt=?", ["invalid"])
                    .unwrap();
            }
            _ => {
                db.execute(
                    "UPDATE ion_team_gpu_results SET receipt=?",
                    [
                        json!({"request_id":id,"padding":"x".repeat(RECEIPT_BYTES as usize)})
                            .to_string(),
                    ],
                )
                .unwrap();
            }
        }
        assert!(!admits(&mut db, &approved));
    }
}
#[test]
fn gpu_collector_review_reports_utf8_bytes_and_never_payloads() {
    let mut db = db();
    let approved = volume("ion/results", 10);
    insert(&db, &approved, "settled", true);
    let actual: u64 = db
        .query_row(
            "SELECT length(CAST(receipt AS BLOB)) FROM ion_team_gpu_results",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let facts = review(&mut db, &approved);
    assert_eq!(facts.observed_bytes, actual);
    let wire = serde_json::to_string(&facts).unwrap();
    assert!(
        !wire.contains("actual λ receipt")
            && !wire.contains("content_base64")
            && !wire.contains("http")
            && !wire.contains("credential")
    );
}
#[test]
fn gpu_collector_exhaustion_is_read_only_and_preserves_exact_original_result_identity() {
    let mut db = db();
    let approved = volume("ion/results", 1);
    let id = insert(&db, &approved, "settled", true);
    let before: (String, String) = db
        .query_row(
            "SELECT receipt,artifact_id FROM ion_team_gpu_results WHERE request_id=?",
            [id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    for _ in 0..3 {
        assert!(!admits(&mut db, &approved));
    }
    let after: (String, String) = db
        .query_row(
            "SELECT receipt,artifact_id FROM ion_team_gpu_results WHERE request_id=?",
            [id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(before, after);
}
#[test]
fn gpu_collector_oversized_original_mount_facts_fail_before_unbounded_allocation() {
    let mut db = db();
    let approved = volume("ion/results", 10);
    let id = insert(&db, &approved, "settled", false);
    mutate(&db, id, "body", |v| {
        v["spec"]["volumes"][0]["padding"] = json!("x".repeat(10000))
    });
    assert!(!admits(&mut db, &approved));
}
#[test]
fn gpu_collector_known_noncollector_historical_job_does_not_invent_a_storage_hold() {
    let mut db = db();
    let id = Uuid::new_v4();
    let body = json!({"spec":{"args":"--request-file /ion/job-request.json"}});
    let signature = json!({"body":body,"policy":{},"request":{"request_id":id}});
    db.execute(
        "INSERT INTO ion_team_gpu_jobs VALUES(?,?,?,?,?,'cleaned')",
        params![
            id.to_string(),
            "project-fixture",
            "{}",
            signature.to_string(),
            body.to_string()
        ],
    )
    .unwrap();
    assert!(admits(&mut db, &volume("ion/results", 1)));
}

#[test]
fn gpu_collector_matching_corrupt_originals_with_unknown_bucket_identity_fail_closed() {
    let approved = volume("ion/results", 2);
    for unknown in ["", "unknown/identity", &"x".repeat(161)] {
        let mut db = db();
        let mut malformed = approved.clone();
        malformed.bucket_id = unknown.into();
        malformed.bucket_name = "other-results".into();
        insert(&db, &malformed, "settled", false);
        assert!(!admits(&mut db, &approved));
    }
}
#[test]
fn gpu_collector_nil_new_request_never_uses_retention_headroom() {
    let mut db = db();
    let approved = volume("ion/results", 2);
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    assert!(reserve(&tx, &approved, Uuid::nil()).is_err());
    assert_eq!(usage(&tx, &approved).unwrap().reserved_receipts, 0);
}

#[test]
fn gpu_collector_owner_request_lookup_refuses_nil_before_any_storage_open() {
    let path = std::env::temp_dir()
        .join(format!("ion-collector-lookup-{}", Uuid::new_v4()))
        .join("control.db");
    let coordinator = TeamCoordinator::new(path.clone());
    assert!(coordinator.gpu_collector_for_request(Uuid::nil()).is_err());
    assert!(!path.exists());
}

#[test]
fn gpu_collector_original_collection_bound_fails_closed_for_one_retained_bucket() {
    let mut db = db();
    let approved = volume("ion/results", 4096);
    for _ in 0..4097 {
        insert(&db, &approved, "settled", false);
    }
    let tx = db.transaction().unwrap();
    assert!(usage(&tx, &approved).is_err());
    assert!(reserve(&tx, &approved, Uuid::new_v4()).is_err());
    assert_eq!(
        tx.query_row::<u32, _, _>("SELECT count(*) FROM ion_team_gpu_jobs", [], |r| r.get(0))
            .unwrap(),
        4097
    );
}

#[test]
fn gpu_collector_new_disjoint_prefix_cannot_exceed_retained_bucket_review_bound() {
    let mut db = db();
    let previous = volume("ion/results/old", 4096);
    for _ in 0..4096 {
        insert(&db, &previous, "settled", false);
    }
    let new = volume("ion/results/new", 4096);
    let facts = review(&mut db, &new);
    assert_eq!(facts.reserved_receipts, 0);
    assert_eq!(facts.bucket_reserved_receipts, 4096);
    assert!(!admits(&mut db, &new));
}
