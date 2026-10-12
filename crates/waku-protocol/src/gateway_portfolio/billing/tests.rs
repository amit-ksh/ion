use super::*;
use crate::cloud_billing::*;
fn report(scope: Uuid) -> CloudBillingReport {
    serde_json::from_value(json!({"scopeId":scope,"exportId":"export-1","period":"2026-09","currency":"USD","billedCost":"-0.000000000000000001","effectiveCost":"12345678901234567890.123456789012345678","contractedCost":"0","rows":3,"unattributedResourceRows":1,"createdAt":"2026-10-01T00:00:00.123456789Z","fetchedAt":1,"archiveSha256":"a".repeat(64),"csvSha256":"b".repeat(64),"current":true})).unwrap()
}
#[test]
fn billing_queries_are_scoped_bounded_reads_with_no_transport_or_sync_fields() {
    let scope = Uuid::new_v4();
    for value in [
        json!({"kind":"cloudBilling","query":{"after":null,"limit":32}}),
        json!({"kind":"cloudBillingReports","query":{"scopeId":scope,"after":"export-1","limit":16}}),
        json!({"kind":"cloudBillingCharges","query":{"scopeId":scope,"exportId":"export-1","after":1,"limit":32}}),
    ] {
        assert!(
            serde_json::from_value::<PortfolioQuery>(value)
                .unwrap()
                .validate()
                .is_ok()
        );
    }
    for value in [
        json!({"kind":"cloudBillingReports","query":{"scopeId":scope,"after":"https://archive.invalid","limit":16}}),
        json!({"kind":"cloudBillingCharges","query":{"scopeId":scope,"exportId":"export-1","after":0,"limit":32}}),
        json!({"kind":"cloudBilling","query":{"after":null,"limit":33}}),
    ] {
        assert!(
            serde_json::from_value::<PortfolioQuery>(value)
                .unwrap()
                .validate()
                .is_err()
        );
    }
    assert!(
        serde_json::from_value::<PortfolioQuery>(
            json!({"kind":"cloudBilling","query":{"after":null,"limit":32,"token":"private"}})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<PortfolioQuery>(
            json!({"kind":"syncCloudBilling","scopeId":scope})
        )
        .is_err()
    );
}
#[test]
fn billing_accounts_redact_connection_identity_and_transfer_complete_monthly_evidence() {
    let id = Uuid::new_v4();
    let page = CloudBillingPage {
        scopes: vec![CloudBillingScopeStatus {
            scope: CloudBillingScope {
                id,
                name: "My costs".into(),
                account_id: "private-account".into(),
                contract_id: "private-contract".into(),
                currency: "USD".into(),
                archive_origin: "https://private-origin.invalid".into(),
            },
            revision: 1,
            reports: vec![report(id)],
        }],
        next_cursor: None,
        total: 1,
    };
    let result = accounts_result(page, 1000).unwrap();
    assert!(!result.to_string().contains("private-"));
    assert_eq!(result["page"]["scopes"][0].as_object().unwrap().len(), 5);
    assert_eq!(
        result["page"]["scopes"][0]["reports"][0]["billedCost"],
        "-0.000000000000000001"
    );
    assert_eq!(
        result["page"]["scopes"][0]["reports"][0]["csvSha256"],
        "b".repeat(64)
    );
}
#[test]
fn billing_accounts_transfer_bound_keeps_whole_rows_counts_and_a_recoverable_cursor() {
    let mut scopes = Vec::new();
    for value in 1..=32 {
        let id = Uuid::from_u128(value);
        let mut reports = Vec::new();
        for month in (1..=12).rev() {
            let mut r = report(id);
            r.period = format!("2026-{month:02}");
            reports.push(r);
        }
        scopes.push(CloudBillingScopeStatus {
            scope: CloudBillingScope {
                id,
                name: "🟢".repeat(100),
                account_id: "private-account".into(),
                contract_id: "private-contract".into(),
                currency: "USD".into(),
                archive_origin: "https://private-origin.invalid".into(),
            },
            revision: 1,
            reports,
        });
    }
    let result = accounts_result(
        CloudBillingPage {
            scopes,
            next_cursor: None,
            total: 32,
        },
        1000,
    )
    .unwrap();
    assert!(result.to_string().len() <= 60_000);
    assert_eq!(result["pageTruncated"], true);
    assert_eq!(result["page"]["total"], 32);
    let scopes = result["page"]["scopes"].as_array().unwrap();
    assert!(scopes.len() > 0 && scopes.len() < 32);
    assert!(
        scopes
            .iter()
            .all(|r| r["reports"].as_array().unwrap().len() == 12)
    );
    assert_eq!(result["page"]["nextCursor"], scopes.last().unwrap()["id"]);
}
#[test]
fn charge_redaction_preserves_original_decimal_and_report_provenance() {
    let id = Uuid::new_v4();
    let charge:CloudBillingCharge=serde_json::from_value(json!({"ordinal":1,"containerId":"private-container","resourceId":"private-resource","resourceType":"gpu","category":"Usage","class":null,"chargePeriodStart":"2026-09-01T00:00:00Z","chargePeriodEnd":"2026-10-01T00:00:00Z","billedCost":"-0.000000000000000001","effectiveCost":"1.2","contractedCost":"0"})).unwrap();
    let result = charges_result(
        CloudBillingChargePage {
            report: report(id),
            charges: vec![charge],
            next_cursor: Some(1),
        },
        1000,
    )
    .unwrap();
    assert!(!result.to_string().contains("private-"));
    assert_eq!(result["page"]["charges"][0].as_object().unwrap().len(), 9);
    assert_eq!(
        result["page"]["charges"][0]["billedCost"],
        "-0.000000000000000001"
    );
    assert_eq!(result["page"]["report"]["archiveSha256"], "a".repeat(64));
    assert_eq!(result["page"]["nextCursor"], 1);
}
