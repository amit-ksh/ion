use super::*;
use chrono::{TimeZone, Utc};
use serde_json::{Value, json};
fn setup() -> (
    ControlStore,
    CloudBillingScope,
    CloudBillingExportQuery,
    u64,
) {
    let store = ControlStore::new(
        std::env::temp_dir()
            .join(format!("ion-billing-discovery-{}", Uuid::new_v4()))
            .join("control.db"),
    );
    let scope = CloudBillingScope {
        id: Uuid::new_v4(),
        name: "Owner Nebius".into(),
        account_id: "account-fixture".into(),
        contract_id: "contract-fixture".into(),
        currency: "USD".into(),
        archive_origin: "https://storage.eu-north1.nebius.cloud".into(),
    };
    store.save_cloud_billing_scope(scope.clone(), None).unwrap();
    let query = CloudBillingExportQuery {
        scope_id: scope.id,
        expected_revision: 1,
        after: None,
        limit: 32,
    };
    let now = Utc
        .with_ymd_and_hms(2026, 10, 12, 0, 0, 0)
        .unwrap()
        .timestamp() as u64;
    (store, scope, query, now)
}
fn ready(scope: &CloudBillingScope, id: &str) -> Value {
    json!({"metadata":{"id":id,"parentId":scope.contract_id,"createdAt":"2026-10-10T00:00:00.123456789Z"},"spec":{"startPeriod":"2026-09","endPeriod":"2026-09","format":"EXPORT_FORMAT_FOCUS_1_2_CSV"},"status":{"state":"ONE_TIME_EXPORT_STATE_SUCCESS","expiresAt":"2026-10-17T00:00:00Z","downloadUrl":"https://storage.eu-north1.nebius.cloud/private?credential=fixture-signed-url","stateDetails":{"error":{"message":"fixture-private-provider-body"}}}})
}
fn page(scope: &CloudBillingScope, id: &str, next: &str, now: u64) -> transport::ExportDiscovery {
    transport::discovery_page(
        scope,
        &json!({"items":[ready(scope,id)],"nextPageToken":next}),
        32,
        "",
        now,
    )
    .unwrap()
}
fn accept(
    store: &ControlStore,
    scope: &CloudBillingScope,
    query: &CloudBillingExportQuery,
    page: transport::ExportDiscovery,
    now: u64,
) -> Result<CloudBillingExportPage> {
    let (_, prior) = prepare(&store.connect()?, query, now)?;
    store.commit_discovery(scope, query, prior, page, now)
}
#[test]
fn discovery_offers_only_ready_single_month_focus_without_transport_metadata() {
    let (store, scope, query, now) = setup();
    let mut rows = vec![ready(&scope, "ready")];
    for (id, pointer, replacement) in [
        (
            "running",
            "/status/state",
            json!("ONE_TIME_EXPORT_STATE_RUNNING"),
        ),
        ("archived", "/status/state", json!(5)),
        (
            "failed",
            "/status/state",
            json!("ONE_TIME_EXPORT_STATE_FAILED"),
        ),
        (
            "expired",
            "/status/expiresAt",
            json!("2026-10-12T00:00:00Z"),
        ),
        (
            "unsupported",
            "/spec/format",
            json!("EXPORT_FORMAT_UNSPECIFIED"),
        ),
        ("multiple-months", "/spec/endPeriod", json!("2026-10")),
    ] {
        let mut row = ready(&scope, id);
        *row.pointer_mut(pointer).unwrap() = replacement;
        rows.push(row);
    }
    let mut no_url = ready(&scope, "no-url");
    no_url["status"]
        .as_object_mut()
        .unwrap()
        .remove("downloadUrl");
    no_url["status"]["state"] = json!(3);
    no_url["spec"]["format"] = json!(1);
    rows.push(no_url);
    let decoded = transport::discovery_page(
        &scope,
        &json!({"items":rows,"nextPageToken":"private-provider-continuation"}),
        32,
        "",
        now,
    )
    .unwrap();
    assert_eq!(decoded.examined, 8);
    assert_eq!(decoded.items.len(), 2);
    let reply = accept(&store, &scope, &query, decoded, now).unwrap();
    let wire = serde_json::to_string(&reply).unwrap();
    for private in [
        "downloadUrl",
        "https://",
        "credential",
        "private-provider-continuation",
        "fixture-private-provider-body",
        "contract-fixture",
        "account-fixture",
    ] {
        assert!(!wire.contains(private), "{private}");
    }
    assert!(reply.next_cursor.is_some());
    assert_eq!(
        store.cloud_billing(None, 64).unwrap().scopes[0]
            .reports
            .len(),
        0
    );
    assert_eq!(
        reply.items[0].created_at,
        "2026-10-10T00:00:00.123456789+00:00"
    );
}
#[test]
fn discovery_rejects_foreign_duplicate_oversized_and_malformed_pages() {
    let (_, scope, _, now) = setup();
    let row = ready(&scope, "good");
    for input in [
        json!({"items":[row.clone(),row.clone()]}),
        json!({"items":vec![row.clone();33]}),
        json!({"items":"not-a-list"}),
        json!({"error":"fixture-private-error"}),
        json!([]),
        json!({"items":[row.clone()],"nextPageToken":42}),
        json!({"items":[row.clone()],"nextPageToken":"private\ncontinuation"}),
        json!({"items":[row.clone()],"nextPageToken":"same"}),
    ] {
        assert!(transport::discovery_page(&scope, &input, 32, "same", now).is_err());
    }
    for (pointer, replacement) in [
        ("/metadata/parentId", json!("other-contract")),
        ("/metadata/id", json!("https://secret")),
        ("/metadata/createdAt", json!("2026-12-01T00:00:00Z")),
        ("/status/state", json!("FUTURE_STATE")),
        ("/status/state", json!(99)),
        ("/spec/startPeriod", json!("2026-99")),
        ("/spec/endPeriod", json!("missing-month")),
        ("/status/expiresAt", json!("not-time")),
    ] {
        let mut changed = row.clone();
        *changed.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            transport::discovery_page(&scope, &json!({"items":[changed]}), 32, "", now).is_err(),
            "{pointer}"
        );
    }
    assert!(transport::page_token(&"t".repeat(4097)).is_err());
    let empty = transport::discovery_page(&scope, &json!({}), 32, "", now).unwrap();
    assert_eq!(empty.examined, 0);
    assert!(empty.next_token.is_empty());
}
#[test]
fn discovery_cursor_survives_restart_but_refuses_scope_revision_limit_and_expiry_changes() {
    let (store, scope, query, now) = setup();
    let first = accept(
        &store,
        &scope,
        &query,
        page(&scope, "first", "private-next", now),
        now,
    )
    .unwrap();
    let mut next = query.clone();
    next.after = first.next_cursor;
    let restarted = ControlStore::new(store.path());
    assert_eq!(
        prepare(&restarted.connect().unwrap(), &next, now + 1)
            .unwrap()
            .1
            .token,
        "private-next"
    );
    for at in [now - 1, now + TTL] {
        assert!(prepare(&restarted.connect().unwrap(), &next, at).is_err());
    }
    let mut changed = next.clone();
    changed.limit = 16;
    assert!(prepare(&store.connect().unwrap(), &changed, now).is_err());
    let mut other = scope.clone();
    other.id = Uuid::new_v4();
    other.contract_id = "another-contract".into();
    store.save_cloud_billing_scope(other.clone(), None).unwrap();
    changed = next.clone();
    changed.scope_id = other.id;
    assert!(prepare(&store.connect().unwrap(), &changed, now).is_err());
    let mut renamed = scope.clone();
    renamed.name = "Renamed".into();
    store.save_cloud_billing_scope(renamed, Some(1)).unwrap();
    assert!(prepare(&store.connect().unwrap(), &next, now).is_err());
    next.expected_revision = 2;
    assert!(prepare(&store.connect().unwrap(), &next, now).is_err());
}
#[test]
fn discovery_scope_change_during_provider_request_accepts_no_rows_or_cursor() {
    let (store, scope, query, now) = setup();
    let (_, prior) = prepare(&store.connect().unwrap(), &query, now).unwrap();
    let mut changed = scope.clone();
    changed.contract_id = "replaced-contract".into();
    store.save_cloud_billing_scope(changed, Some(1)).unwrap();
    assert!(
        store
            .commit_discovery(
                &scope,
                &query,
                prior,
                page(&scope, "ready", "next", now),
                now
            )
            .is_err()
    );
    let count: u64 = store
        .connect()
        .unwrap()
        .query_row(
            "SELECT count(*) FROM ion_cloud_billing_discovery_cursors",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}
#[test]
fn discovery_detects_token_cycles_and_cross_page_identity_repetition() {
    let (store, scope, query, now) = setup();
    let first = accept(
        &store,
        &scope,
        &query,
        page(&scope, "first", "token-a", now),
        now,
    )
    .unwrap();
    let mut next = query.clone();
    next.after = first.next_cursor;
    assert!(
        accept(
            &store,
            &scope,
            &next,
            page(&scope, "first", "token-b", now),
            now
        )
        .is_err()
    );
    assert!(
        accept(
            &store,
            &scope,
            &next,
            page(&scope, "second", "token-a", now),
            now
        )
        .is_err()
    );
    let second = accept(
        &store,
        &scope,
        &next,
        page(&scope, "second", "token-b", now),
        now,
    )
    .unwrap();
    next.after = second.next_cursor;
    assert!(
        accept(
            &store,
            &scope,
            &next,
            page(&scope, "third", "token-a", now),
            now
        )
        .is_err()
    );
    let final_page = accept(&store, &scope, &next, page(&scope, "third", "", now), now).unwrap();
    assert!(final_page.next_cursor.is_none());
}
#[test]
fn discovery_invalid_or_foreign_requests_fail_before_native_credentials_and_network() {
    let (store, _, query, now) = setup();
    for changed in [
        CloudBillingExportQuery {
            limit: 0,
            ..query.clone()
        },
        CloudBillingExportQuery {
            limit: 33,
            ..query.clone()
        },
        CloudBillingExportQuery {
            scope_id: Uuid::nil(),
            ..query.clone()
        },
        CloudBillingExportQuery {
            expected_revision: 0,
            ..query.clone()
        },
        CloudBillingExportQuery {
            expected_revision: 2,
            ..query.clone()
        },
        CloudBillingExportQuery {
            scope_id: Uuid::new_v4(),
            ..query.clone()
        },
        CloudBillingExportQuery {
            after: Some(Uuid::nil()),
            ..query.clone()
        },
        CloudBillingExportQuery {
            after: Some(Uuid::new_v4()),
            ..query.clone()
        },
    ] {
        let error = store
            .discover_cloud_billing_exports(changed, now)
            .unwrap_err()
            .to_string();
        assert!(
            !error.contains("IAM")
                && !error.contains("native")
                && !error.contains("Could not discover"),
            "{error}"
        );
    }
}
#[test]
fn discovery_expiry_at_receipt_filters_rows_without_importing_them() {
    let (store, scope, query, now) = setup();
    let mut row = ready(&scope, "ready");
    row["status"]["expiresAt"] = json!("2026-10-12T00:00:01Z");
    let decoded = transport::discovery_page(&scope, &json!({"items":[row]}), 32, "", now).unwrap();
    let reply = accept(&store, &scope, &query, decoded, now + 1).unwrap();
    assert_eq!(reply.examined, 1);
    assert!(reply.items.is_empty());
    assert!(
        store.cloud_billing(None, 64).unwrap().scopes[0]
            .reports
            .is_empty()
    );
}
#[test]
fn discovery_cursor_cache_is_bounded_and_expired_entries_are_pruned() {
    let (store, scope, query, now) = setup();
    let db = store.connect().unwrap();
    for _ in 0..CURSORS {
        db.execute(
            "INSERT INTO ion_cloud_billing_discovery_cursors(id,created_at,body) VALUES(?,?,?)",
            params![Uuid::new_v4().to_string(), now, "{}"],
        )
        .unwrap();
    }
    assert!(
        accept(
            &store,
            &scope,
            &query,
            page(&scope, "ready", "next", now),
            now
        )
        .is_err()
    );
    let later = now + TTL;
    assert!(
        accept(
            &store,
            &scope,
            &query,
            page(&scope, "ready", "next", later),
            later
        )
        .is_ok()
    );
    let count: u64 = db
        .query_row(
            "SELECT count(*) FROM ion_cloud_billing_discovery_cursors",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}
