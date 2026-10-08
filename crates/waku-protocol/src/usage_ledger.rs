//! Owner usage facts. Spending allowances are not charges; absent costs are unknown.
use crate::{model::ProviderKind, teams::ExecutionEnvironment};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum UsageLedgerKind {
    ModelCall,
    RemoteCommand,
    GpuJob,
    SoloModelCall,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum UsageCostQuality {
    Estimated,
    Reported,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum UsageCostFinality {
    Provisional,
    Final,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsageLedgerQuery {
    pub project_id: Option<Uuid>,
    pub task_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub provider: Option<ProviderKind>,
    pub environment: Option<ExecutionEnvironment>,
    pub kind: Option<UsageLedgerKind>,
    pub model: Option<String>,
    pub since: Option<u64>,
    pub after: Option<u64>,
    pub limit: u16,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UsageMoney {
    pub currency: String,
    pub micros: u64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UsageCost {
    pub amount: UsageMoney,
    pub quality: UsageCostQuality,
    pub finality: UsageCostFinality,
    pub origin: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UsageLedgerEntry {
    pub sequence: u64,
    pub kind: UsageLedgerKind,
    pub source_id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub run_id: Option<Uuid>,
    pub assignment_id: Option<Uuid>,
    pub attempt_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub agent_name: Option<String>,
    pub provider: Option<ProviderKind>,
    pub model: Option<String>,
    pub environment: Option<ExecutionEnvironment>,
    pub host: String,
    pub observed_at: u64,
    pub started_at: Option<u64>,
    pub updated_at: u64,
    pub status: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub reserved_tokens: u64,
    pub elapsed_milliseconds: Option<u64>,
    pub cost: Option<UsageCost>,
    /// Approved maximum for this resource request, never a measured cost.
    pub spending_allowance: Option<UsageMoney>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UsageCostTotal {
    pub amount: UsageMoney,
    pub quality: UsageCostQuality,
    pub finality: UsageCostFinality,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UsageLedgerSummary {
    pub records: u64,
    pub model_calls: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub reserved_tokens: u64,
    pub unknown_calls: u64,
    pub unknown_cost_records: u64,
    pub elapsed_milliseconds: u64,
    pub costs: Vec<UsageCostTotal>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UsageLedgerPage {
    pub revision: u64,
    pub entries: Vec<UsageLedgerEntry>,
    pub next_cursor: Option<u64>,
    pub summary: UsageLedgerSummary,
}
