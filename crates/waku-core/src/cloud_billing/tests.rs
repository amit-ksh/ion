use super::*;
use chrono::{TimeZone, Utc};
use std::io::{Read, Write};
fn fixture_scope() -> CloudBillingScope {
    CloudBillingScope {
        id: Uuid::new_v4(),
        name: "Personal Nebius".into(),
        account_id: "account-test".into(),
        contract_id: "contract-test".into(),
        currency: "USD".into(),
        archive_origin: "https://storage.eu-north1.nebius.cloud".into(),
    }
}
fn store() -> ControlStore {
    ControlStore::new(
        std::env::temp_dir()
            .join(format!("ion-billing-{}", Uuid::new_v4()))
            .join("control.db"),
    )
}
const HEADER: &str = "BillingAccountId,BillingCurrency,BillingPeriodStart,BillingPeriodEnd,ChargePeriodStart,ChargePeriodEnd,BilledCost,EffectiveCost,ContractedCost,ChargeCategory,ChargeClass,x_ContainerId,ResourceId,ResourceType\n";
fn csv(billed: &str, category: &str, resource: &str) -> String {
    format!(
        "{HEADER}account-test,USD,2026-09-01T00:00:00Z,2026-10-01T00:00:00Z,2026-09-05T00:00:00Z,2026-09-05T01:00:00Z,{billed},{billed},{billed},{category},,project-test,{resource},computeinstance\n"
    )
}
fn archive(files: &[(&str, &str)]) -> Vec<u8> {
    let gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut tar = tar::Builder::new(gzip);
    for (path, body) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o600);
        header.set_cksum();
        tar.append_data(&mut header, *path, body.as_bytes())
            .unwrap();
    }
    tar.into_inner().unwrap().finish().unwrap()
}
fn export(id: &str, seconds: i64) -> transport::Export {
    transport::Export{id:id.into(),period:"2026-09".into(),created:Utc.timestamp_opt(seconds,0).unwrap(),url:url::Url::parse("https://storage.eu-north1.nebius.cloud/billing-report-exports-external/onetimeexport/fixture?test-only").unwrap()}
}
#[test]
fn cloud_billing_exact_decimal_preserves_submicro_charges_credits_and_exponents() {
    use decimal::Decimal as D;
    assert_eq!(
        D::parse("0.000000000000000001")
            .unwrap()
            .add(D::parse("1e-6").unwrap())
            .unwrap()
            .add(D::parse("-0.2").unwrap())
            .unwrap()
            .text(),
        "-0.199998999999999999"
    );
    for (text, normal) in [
        ("-0", "0"),
        ("001.2300", "1.23"),
        ("1.2300e2", "123"),
        ("1000e-21", "0.000000000000000001"),
    ] {
        assert_eq!(D::parse(text).unwrap().text(), normal);
    }
    for text in [
        "NaN",
        "inf",
        " 1",
        "1 ",
        "+1",
        "1e",
        "1e37",
        "0.0000000000000000001",
        "1.2.3",
        "1e2e3",
    ] {
        assert!(D::parse(text).is_err(), "{text}");
    }
}
#[test]
fn cloud_billing_focus_preserves_all_categories_and_original_attribution() {
    let scope = fixture_scope();
    let input = csv("1.0000001", "Usage", "vm-test");
    let correction = csv("-0.2", "Credit", "");
    let parsed = focus::parse(
        &scope,
        "2026-09",
        &archive(&[("usage.csv", &input), ("credits.csv", &correction)]),
    )
    .unwrap();
    assert_eq!(parsed.billed, "0.8000001");
    assert_eq!(parsed.charges.len(), 2);
    assert_eq!(parsed.missing_resources, 1);
    assert_eq!(parsed.charges[0].resource_id.as_deref(), Some("vm-test"));
    assert_eq!(parsed.charges[1].category, "Credit");
}
#[test]
fn cloud_billing_focus_rejects_mixed_identity_currency_period_missing_fields_and_precision() {
    let scope = fixture_scope();
    let original = csv("1", "Tax", "");
    for bad in [
        original.replace("account-test", "another-account"),
        original.replace(",USD,", ",EUR,"),
        original.replace("2026-10-01T00:00:00Z", "2026-09-20T00:00:00Z"),
        original.replace("BilledCost", "Other"),
        original.replace("Tax", ""),
        csv("NaN", "Usage", ""),
        csv("1e-19", "Usage", ""),
    ] {
        assert!(focus::parse(&scope, "2026-09", &archive(&[("report.csv", &bad)])).is_err());
    }
    assert!(focus::parse(&scope, "2026-09", &archive(&[("report.csv", HEADER)])).is_err());
}
#[test]
fn cloud_billing_archives_reject_duplicates_links_non_csv_and_trailing_payloads() {
    let scope = fixture_scope();
    let input = csv("1", "Usage", "vm-test");
    assert!(
        focus::parse(
            &scope,
            "2026-09",
            &archive(&[("same.csv", &input), ("same.csv", &input)])
        )
        .is_err()
    );
    assert!(focus::parse(&scope, "2026-09", &archive(&[("report.txt", &input)])).is_err());
    let gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut tar = tar::Builder::new(gzip);
    let mut link = tar::Header::new_gnu();
    link.set_entry_type(tar::EntryType::Symlink);
    link.set_size(0);
    link.set_mode(0o600);
    link.set_link_name("outside").unwrap();
    link.set_cksum();
    tar.append_data(&mut link, "report.csv", std::io::empty())
        .unwrap();
    assert!(
        focus::parse(
            &scope,
            "2026-09",
            &tar.into_inner().unwrap().finish().unwrap()
        )
        .is_err()
    );
    let mut gzip = flate2::read::GzDecoder::new(archive(&[("report.csv", &input)]).as_slice())
        .bytes()
        .collect::<std::io::Result<Vec<_>>>()
        .unwrap();
    gzip.extend(b"hidden-nonzero");
    let mut encoded = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoded.write_all(&gzip).unwrap();
    assert!(focus::parse(&scope, "2026-09", &encoded.finish().unwrap()).is_err());
}
#[test]
fn cloud_billing_scope_replay_conflicts_and_account_uniqueness_survive_restart() {
    let store = store();
    let scope = fixture_scope();
    let first = store.save_cloud_billing_scope(scope.clone(), None).unwrap();
    assert_eq!(first.revision, 1);
    assert_eq!(
        store
            .save_cloud_billing_scope(scope.clone(), None)
            .unwrap()
            .revision,
        1
    );
    let mut other = scope.clone();
    other.id = Uuid::new_v4();
    assert!(store.save_cloud_billing_scope(other, None).is_err());
    let mut edit = scope.clone();
    edit.name = "Renamed".into();
    assert!(
        store
            .save_cloud_billing_scope(edit.clone(), Some(0))
            .is_err()
    );
    assert_eq!(
        store
            .save_cloud_billing_scope(edit, Some(1))
            .unwrap()
            .revision,
        2
    );
    let restarted = ControlStore::new(store.path());
    let page = restarted.cloud_billing(None, 64).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.scopes[0].revision, 2);
}
#[test]
fn cloud_billing_snapshot_replacement_replay_and_out_of_order_reports_do_not_add_costs() {
    let store = store();
    let scope = fixture_scope();
    store.save_cloud_billing_scope(scope.clone(), None).unwrap();
    let data = archive(&[("report.csv", &csv("10", "Usage", "vm-test"))]);
    let first = store
        .commit_cloud_billing(&scope, 1, export("export-first", 100), data.clone(), 200)
        .unwrap();
    assert!(first.current);
    let replay = store
        .commit_cloud_billing(&scope, 1, export("export-first", 100), data, 300)
        .unwrap();
    assert_eq!(replay.fetched_at, 200);
    let newer = store
        .commit_cloud_billing(
            &scope,
            1,
            export("export-new", 150),
            archive(&[("report.csv", &csv("12", "Usage", "vm-test"))]),
            250,
        )
        .unwrap();
    assert!(newer.current);
    let older = store
        .commit_cloud_billing(
            &scope,
            1,
            export("export-old", 50),
            archive(&[("report.csv", &csv("99", "Usage", "vm-test"))]),
            300,
        )
        .unwrap();
    assert!(!older.current);
    let current = store
        .cloud_billing(None, 64)
        .unwrap()
        .scopes
        .remove(0)
        .reports;
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].billed_cost, "12");
    assert_eq!(current[0].export_id, "export-new");
    let historical = store
        .cloud_billing_charges(scope.id, "export-first", None, 1)
        .unwrap();
    assert!(!historical.report.current);
    assert_eq!(historical.charges[0].billed_cost, "10");
    let mut changed = scope.clone();
    changed.currency = "EUR".into();
    assert!(store.save_cloud_billing_scope(changed, Some(1)).is_err());
    assert!(
        store
            .commit_cloud_billing(
                &scope,
                1,
                export("export-first", 100),
                archive(&[("report.csv", &csv("11", "Usage", "vm-test"))]),
                300
            )
            .is_err()
    );
}
#[test]
fn cloud_billing_failed_and_stale_syncs_rollback_and_charge_pages_are_scoped() {
    let store = store();
    let scope = fixture_scope();
    store.save_cloud_billing_scope(scope.clone(), None).unwrap();
    assert!(
        store
            .commit_cloud_billing(
                &scope,
                0,
                export("export-stale", 100),
                archive(&[("report.csv", &csv("1", "Usage", "vm-test"))]),
                200
            )
            .is_err()
    );
    assert!(
        store
            .commit_cloud_billing(&scope, 1, export("export-bad", 100), b"bad".to_vec(), 200)
            .is_err()
    );
    assert!(
        store.cloud_billing(None, 64).unwrap().scopes[0]
            .reports
            .is_empty()
    );
    store
        .commit_cloud_billing(
            &scope,
            1,
            export("export-ok", 100),
            archive(&[
                ("a.csv", &csv("1", "Usage", "vm-a")),
                ("b.csv", &csv("2", "Tax", "")),
            ]),
            200,
        )
        .unwrap();
    let first = store
        .cloud_billing_charges(scope.id, "export-ok", None, 1)
        .unwrap();
    assert_eq!(first.next_cursor, Some(1));
    let next = store
        .cloud_billing_charges(scope.id, "export-ok", first.next_cursor, 1)
        .unwrap();
    assert_eq!(next.charges[0].billed_cost, "2");
    assert!(next.next_cursor.is_none());
    assert!(
        store
            .cloud_billing_charges(Uuid::new_v4(), "export-ok", None, 64)
            .is_err()
    );
    assert!(
        store
            .cloud_billing_charges(scope.id, "export-ok", None, 65)
            .is_err()
    );
}
#[test]
fn cloud_billing_native_export_checks_contract_state_format_period_expiry_and_origin() {
    let scope = fixture_scope();
    let value = serde_json::json!({"metadata":{"id":"export-ok","parentId":scope.contract_id,"createdAt":"2026-10-10T00:00:00Z"},"spec":{"startPeriod":"2026-09","endPeriod":"2026-09","format":"EXPORT_FORMAT_FOCUS_1_2_CSV"},"status":{"state":"ONE_TIME_EXPORT_STATE_SUCCESS","expiresAt":"2026-10-17T00:00:00Z","downloadUrl":"https://storage.eu-north1.nebius.cloud/billing-report-exports-external/onetimeexport/fixture?sensitive=test"}});
    let now = Utc
        .with_ymd_and_hms(2026, 10, 12, 0, 0, 0)
        .unwrap()
        .timestamp() as u64;
    assert!(transport::validate_export(&scope, "export-ok", &value, now).is_ok());
    for (pointer, replacement) in [
        ("/metadata/parentId", "other"),
        ("/status/state", "ONE_TIME_EXPORT_STATE_RUNNING"),
        ("/spec/endPeriod", "2026-10"),
        ("/spec/format", "PDF"),
        ("/status/expiresAt", "2026-10-01T00:00:00Z"),
        ("/status/downloadUrl", "https://other.test/secret"),
        (
            "/status/downloadUrl",
            "http://storage.eu-north1.nebius.cloud/secret",
        ),
    ] {
        let mut changed = value.clone();
        *changed.pointer_mut(pointer).unwrap() = replacement.into();
        assert!(transport::validate_export(&scope, "export-ok", &changed, now).is_err());
    }
    for origin in [
        "http://storage.eu-north1.nebius.cloud",
        "https://storage.eu-north1.nebius.cloud.evil.test",
        "https://storage.eu-north1.nebius.cloud/path",
        "https://storage.eu-north1.nebius.cloud?token=secret",
        "https://127.0.0.1",
        "https://user:secret@storage.eu-north1.nebius.cloud",
    ] {
        assert!(transport::origin(origin).is_err());
    }
}

#[test]
fn cloud_billing_corrections_tax_and_non_ion_resources_are_complete_account_charges() {
    let scope = fixture_scope();
    let usage = csv("1e-7", "Usage", "vm-original");
    let correction =
        csv("-0.00000001", "Adjustment", "").replace("Adjustment,", "Adjustment,Correction");
    let correction = correction.replace("2026-09-05", "2026-08-05");
    let tax = csv("0.2", "Tax", "").replace("project-test", "outside-ion-project");
    let parsed = focus::parse(
        &scope,
        "2026-09",
        &archive(&[
            ("usage.csv", &usage),
            ("correction.csv", &correction),
            ("tax.csv", &tax),
        ]),
    )
    .unwrap();
    assert_eq!(parsed.billed, "0.20000009");
    assert_eq!(parsed.charges[1].class.as_deref(), Some("Correction"));
    assert_eq!(parsed.charges[2].container_id, "outside-ion-project");
    assert_eq!(parsed.missing_resources, 2);
}
#[test]
fn cloud_billing_csv_preserves_quoted_fields_unicode_and_rejects_duplicate_headers() {
    let scope = fixture_scope();
    let raw = csv("1", "Usage", "vm-original");
    let quoted = raw.replace("computeinstance\n", "\"compute,instance\"\n");
    let parsed = focus::parse(&scope, "2026-09", &archive(&[("report.csv", &quoted)])).unwrap();
    assert_eq!(
        parsed.charges[0].resource_type.as_deref(),
        Some("compute,instance")
    );
    let duplicate = raw.replace("ResourceType", "ResourceId");
    assert!(focus::parse(&scope, "2026-09", &archive(&[("report.csv", &duplicate)])).is_err());
}
#[test]
fn cloud_billing_ambiguous_snapshot_times_reject_and_native_urls_never_persist() {
    let store = store();
    let scope = fixture_scope();
    store.save_cloud_billing_scope(scope.clone(), None).unwrap();
    let report = store
        .commit_cloud_billing(
            &scope,
            1,
            export("export-original", 100),
            archive(&[("report.csv", &csv("1", "Usage", "vm-original"))]),
            200,
        )
        .unwrap();
    assert!(
        !serde_json::to_string(&report)
            .unwrap()
            .contains("test-only")
    );
    assert!(
        store
            .commit_cloud_billing(
                &scope,
                1,
                export("export-ambiguous", 100),
                archive(&[("report.csv", &csv("2", "Usage", "vm-original"))]),
                200
            )
            .is_err()
    );
    assert_eq!(
        store.cloud_billing(None, 64).unwrap().scopes[0].reports[0].billed_cost,
        "1"
    );
    let db = store.connect().unwrap();
    let count: u64 = db
        .query_row("SELECT count(*) FROM ion_cloud_billing_reports", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn cloud_billing_export_history_is_bounded_scoped_and_available_after_restart() {
    let store = store();
    let scope = fixture_scope();
    store.save_cloud_billing_scope(scope.clone(), None).unwrap();
    for i in 0..3 {
        store
            .commit_cloud_billing(
                &scope,
                1,
                export(&format!("export-{i}"), 100 + i),
                archive(&[(
                    "report.csv",
                    &csv(&format!("{}", i + 1), "Usage", "vm-original"),
                )]),
                200,
            )
            .unwrap();
    }
    let restarted = ControlStore::new(store.path());
    let first = restarted.cloud_billing_reports(scope.id, None, 2).unwrap();
    assert_eq!(first.total, 3);
    assert_eq!(first.next_cursor.as_deref(), Some("export-1"));
    assert!(first.reports.iter().all(|r| !r.current));
    let next = restarted
        .cloud_billing_reports(scope.id, first.next_cursor.as_deref(), 2)
        .unwrap();
    assert_eq!(next.reports.len(), 1);
    assert!(next.reports[0].current);
    assert!(next.next_cursor.is_none());
    assert!(
        restarted
            .cloud_billing_reports(Uuid::new_v4(), None, 2)
            .is_err()
    );
    assert!(restarted.cloud_billing_reports(scope.id, None, 33).is_err());
}

#[test]
fn cloud_billing_identity_partitions_contracts_and_currencies_without_rewriting_history() {
    let store = store();
    let original = fixture_scope();
    store
        .save_cloud_billing_scope(original.clone(), None)
        .unwrap();
    let mut eur = original.clone();
    eur.id = Uuid::new_v4();
    eur.currency = "EUR".into();
    store.save_cloud_billing_scope(eur, None).unwrap();
    let mut contract = original.clone();
    contract.id = Uuid::new_v4();
    contract.contract_id = "contract-new".into();
    store.save_cloud_billing_scope(contract, None).unwrap();
    assert_eq!(store.cloud_billing(None, 64).unwrap().total, 3);
}
#[test]
fn cloud_billing_pre_release_schema_migration_preserves_original_identity_and_revision() {
    let db = Connection::open_in_memory().unwrap();
    let scope = fixture_scope();
    db.execute_batch("CREATE TABLE ion_cloud_billing_scopes(id TEXT PRIMARY KEY,account_id TEXT UNIQUE NOT NULL,revision INTEGER NOT NULL,body TEXT NOT NULL);").unwrap();
    db.execute(
        "INSERT INTO ion_cloud_billing_scopes VALUES(?,?,?,?)",
        params![
            scope.id.to_string(),
            scope.account_id,
            7,
            serde_json::to_string(&scope).unwrap()
        ],
    )
    .unwrap();
    schema(&db).unwrap();
    let (saved, revision) = super::scope(&db, scope.id).unwrap();
    assert_eq!(saved, scope);
    assert_eq!(revision, 7);
    schema(&db).unwrap();
}
#[test]
fn cloud_billing_import_does_not_release_or_modify_gpu_spending_holds() {
    let store = store();
    let scope = fixture_scope();
    store.save_cloud_billing_scope(scope.clone(), None).unwrap();
    let db = store.connect().unwrap();
    db.execute("INSERT INTO ion_cloud_spending_holds(request_id,budget_id,run_id,cloud_project,target,currency,held_micros,budget_snapshot) VALUES('request-original','budget-original','run-original','project-test','sandbox-test','USD',10000000,'{}')",[]).unwrap();
    drop(db);
    store
        .commit_cloud_billing(
            &scope,
            1,
            export("export-cost", 100),
            archive(&[("report.csv", &csv("1", "Usage", "vm-original"))]),
            200,
        )
        .unwrap();
    let held: u64 = store
        .connect()
        .unwrap()
        .query_row(
            "SELECT held_micros FROM ion_cloud_spending_holds WHERE request_id='request-original'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(held, 10000000);
}
