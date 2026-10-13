//! Owner-reviewed content cleanup. Publications and provenance remain immutable.
use crate::control::Artifact;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactRetentionQuery {
    pub project_id: Option<Uuid>,
    pub task_id: Option<Uuid>,
    pub after: Option<Uuid>,
    pub limit: u16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactRetentionState {
    Retained,
    Protected,
    CleanupPending,
    Cleaned,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactRetentionItem {
    pub artifact: Artifact,
    pub state: ArtifactRetentionState,
    pub reason: Option<String>,
    pub cleanup_request: Option<Uuid>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactRetentionPage {
    pub items: Vec<ArtifactRetentionItem>,
    pub next: Option<Uuid>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactCleanupRequest {
    pub request_id: Uuid,
    pub expected: Artifact,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactCleanupReceipt {
    pub request_id: Uuid,
    pub artifact_id: Uuid,
    pub bytes: u64,
    pub cleaned_at: u64,
}

/// Owner diagnostics for the original collector of a selected GPU request.
/// Remote receipt deletion is unavailable; these reservations remain held.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GpuCollectorScope {
    pub prefix: String,
    pub reserved_bytes: u64,
    pub reserved_receipts: u32,
    pub observed_bytes: u64,
    pub observed_receipts: u32,
    pub max_bytes: Option<u64>,
    pub max_receipts: Option<u32>,
    pub remaining_bytes: Option<u64>,
    pub remaining_receipts: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GpuCollectorSnapshot {
    pub request_id: Uuid,
    pub bucket_name: String,
    pub region: String,
    pub prefix: String,
    pub reserved_bytes: u64,
    pub reserved_receipts: u32,
    pub observed_bytes: u64,
    pub observed_receipts: u32,
    pub historical_unbounded_receipts: u32,
    pub bucket_reserved_bytes: u64,
    pub bucket_reserved_receipts: u32,
    pub admissible_receipts: Option<u32>,
    pub scopes: Vec<GpuCollectorScope>,
}
