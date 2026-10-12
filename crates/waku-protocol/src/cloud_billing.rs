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
