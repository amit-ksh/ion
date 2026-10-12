//! Direct authenticated phone artifact discovery. No publication or mutable client metadata.
use crate::control::ArtifactOrigin;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactLibraryQuery {
    pub project_id: Option<Uuid>,
    pub task_id: Option<Uuid>,
    pub format: Option<String>,
    pub after: Option<String>,
    pub limit: u16,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactLibraryItem {
    pub id: Uuid,
    pub name: String,
    pub format: String,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub created_at: u64,
    pub bytes: u64,
    pub sha256: String,
    pub origin: ArtifactOrigin,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactLibraryPage {
    pub items: Vec<ArtifactLibraryItem>,
    pub total: u32,
    pub next_cursor: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactDocumentQuery {
    pub expected: ArtifactLibraryItem,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactDocument {
    pub document: ArtifactLibraryItem,
    #[serde(with = "crate::protocol::base64_bytes")]
    #[ts(type = "string")]
    pub bytes: Vec<u8>,
}
