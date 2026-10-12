//! Owner-local command allocation summaries. No credentials, commands, host paths or release authority.
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandResourceVector {
    pub jobs: u64,
    pub cpu_millis: u64,
    pub memory_bytes: u64,
    pub processes: u64,
    pub storage_bytes: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandResourceUsage {
    pub project_id: Option<Uuid>,
    pub used: CommandResourceVector,
    pub limit: Option<CommandResourceVector>,
    pub unknown_jobs: u32,
    pub retained_archive_bytes: u64,
    pub held_compute_jobs: u32,
}
