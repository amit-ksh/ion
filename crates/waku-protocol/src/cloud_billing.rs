//! Native Nebius account report snapshots; never invoice finality or monetary hold release.
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloudBillingScope {
    pub id: Uuid,
    pub name: String,
    pub account_id: String,
    pub contract_id: String,
    pub currency: String,
    pub archive_origin: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CloudBillingScopeStatus {
    pub scope: CloudBillingScope,
    pub revision: u64,
    pub reports: Vec<CloudBillingReport>,
}
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CloudBillingPage {
    pub scopes: Vec<CloudBillingScopeStatus>,
    pub next_cursor: Option<Uuid>,
    pub total: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CloudBillingReport {
    pub scope_id: Uuid,
    pub export_id: String,
    pub period: String,
    pub currency: String,
    /// Exact decimals, including negative credits/corrections. Never binary floating point.
    pub billed_cost: String,
    pub effective_cost: String,
    pub contracted_cost: String,
    pub rows: u64,
    pub unattributed_resource_rows: u64,
    /// Provider export creation timestamp, preserving fractional precision.
    pub created_at: String,
    pub fetched_at: u64,
    pub archive_sha256: String,
    pub csv_sha256: String,
    /// Latest selected snapshot for this scope/month. Older exports are retained, never summed.
    pub current: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CloudBillingCharge {
    pub ordinal: u64,
    pub container_id: String,
    pub resource_id: Option<String>,
    pub resource_type: Option<String>,
    pub category: String,
    pub class: Option<String>,
    pub charge_period_start: String,
    pub charge_period_end: String,
    pub billed_cost: String,
    pub effective_cost: String,
    pub contracted_cost: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CloudBillingChargePage {
    pub report: CloudBillingReport,
    pub charges: Vec<CloudBillingCharge>,
    pub next_cursor: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CloudBillingReportPage {
    pub scope_id: Uuid,
    pub reports: Vec<CloudBillingReport>,
    pub next_cursor: Option<String>,
    pub total: u64,
}

/// Owner-requested discovery of existing provider exports. No export creation or import.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloudBillingExportQuery {
    pub scope_id: Uuid,
    pub expected_revision: u64,
    /// Opaque owner-local continuation. Provider tokens and signed URLs never cross the wire.
    pub after: Option<Uuid>,
    pub limit: u16,
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloudBillingReadyExport {
    pub export_id: String,
    pub period: String,
    pub created_at: String,
    pub expires_at: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloudBillingExportPage {
    pub scope_id: Uuid,
    pub revision: u64,
    pub items: Vec<CloudBillingReadyExport>,
    /// Existing exports examined on this provider page, including ineligible exports.
    pub examined: u16,
    pub next_cursor: Option<Uuid>,
    pub served_at: u64,
}

#[cfg(test)]
mod discovery_contract_tests {
    use super::*;
    #[test]
    fn discovery_query_cannot_carry_credentials_provider_tokens_or_creation_fields() {
        let query = serde_json::json!({"scopeId":Uuid::new_v4(),"expectedRevision":1,"after":null,"limit":32});
        assert!(serde_json::from_value::<CloudBillingExportQuery>(query.clone()).is_ok());
        for field in [
            "token",
            "pageToken",
            "archiveOrigin",
            "downloadUrl",
            "createExport",
            "sync",
            "contractId",
        ] {
            let mut changed = query.clone();
            changed[field] = serde_json::json!("fixture-private-input");
            assert!(
                serde_json::from_value::<CloudBillingExportQuery>(changed).is_err(),
                "{field}"
            );
        }
    }
}
