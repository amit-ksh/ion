//! Hash-bound originals for isolated HTML resource documents. No host URLs.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;
use uuid::Uuid;

pub const PREFIX: &str = "<!--ion-html-sources-v1:";
pub const CHUNK_BYTES: u16 = 16_384;
pub fn valid_hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HtmlSourceDescriptor {
    pub index: u8,
    pub path: String,
    pub media_type: String,
    pub bytes: u32,
    pub sha256: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HtmlResourceManifest {
    pub version: u8,
    pub artifact_id: Uuid,
    pub artifact_sha256: String,
    pub entry_path: String,
    pub total_bytes: u32,
    pub sha256: String,
    pub sources: Vec<HtmlSourceDescriptor>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HtmlSourceQuery {
    pub manifest_sha256: String,
    pub source_index: u8,
    pub source_sha256: String,
    pub offset: u32,
    pub limit: u16,
}
impl HtmlSourceQuery {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            valid_hash(&self.manifest_sha256)
                && valid_hash(&self.source_sha256)
                && self.source_index <= 32
                && self.offset < 2 * 1024 * 1024
                && self.offset % u32::from(CHUNK_BYTES) == 0
                && self.limit == CHUNK_BYTES,
            "Invalid HTML source chunk"
        );
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HtmlSourceChunk {
    pub artifact_id: Uuid,
    pub artifact_sha256: String,
    pub manifest_sha256: String,
    pub source_index: u8,
    pub source_sha256: String,
    pub offset: u32,
    pub next_offset: Option<u32>,
    #[serde(with = "crate::protocol::base64_bytes")]
    #[ts(type = "string")]
    pub data: Vec<u8>,
}
pub fn commitment(entry: &str, sources: &[HtmlSourceDescriptor]) -> Result<(String, u32)> {
    ensure!(
        !sources.is_empty() && sources.len() <= 33 && sources[0].path == entry,
        "Invalid HTML source membership"
    );
    let mut total = 0u32;
    let mut paths = std::collections::HashSet::new();
    for (i, s) in sources.iter().enumerate() {
        ensure!(
            s.index as usize == i
                && !s.path.is_empty()
                && s.path.len() <= 512
                && !s.path.chars().any(char::is_control)
                && !s.path.contains(['\\', ':', '?', '#'])
                && s.path
                    .split('/')
                    .all(|p| !p.is_empty() && p != "." && p != "..")
                && paths.insert(&s.path)
                && valid_hash(&s.sha256)
                && s.bytes <= if i == 0 { 1024 * 1024 } else { 2 * 1024 * 1024 },
            "Invalid HTML source descriptor"
        );
        ensure!(
            if i == 0 {
                s.media_type == "text/html" && s.bytes > 0
            } else {
                matches!(
                    s.media_type.as_str(),
                    "text/css"
                        | "image/svg+xml"
                        | "image/png"
                        | "image/jpeg"
                        | "image/gif"
                        | "image/webp"
                        | "font/woff"
                        | "font/woff2"
                        | "font/ttf"
                        | "font/otf"
                )
            },
            "Invalid HTML source media type"
        );
        total = total
            .checked_add(s.bytes)
            .ok_or_else(|| anyhow::anyhow!("HTML sources overflow"))?;
    }
    ensure!(
        total <= 9 * 1024 * 1024 && total - sources[0].bytes <= 8 * 1024 * 1024,
        "HTML source total exceeds bound"
    );
    let rows: Vec<_> = sources
        .iter()
        .map(|s| (s.index, &s.path, &s.media_type, s.bytes, &s.sha256))
        .collect();
    let bytes = serde_json::to_vec(&(1, entry, total, rows))?;
    Ok((format!("{:x}", Sha256::digest(bytes)), total))
}
pub fn anchor(hash: &str) -> String {
    format!("{PREFIX}{hash}-->\n")
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowHtmlManifestQuery {
    pub run_id: Uuid,
    pub artifact_id: Uuid,
    pub sha256: String,
}
impl WorkflowHtmlManifestQuery {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.run_id.is_nil() && !self.artifact_id.is_nil() && valid_hash(&self.sha256),
            "Invalid HTML evidence selection"
        );
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowHtmlSourceQuery {
    pub run_id: Uuid,
    pub artifact_id: Uuid,
    pub sha256: String,
    pub source: HtmlSourceQuery,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum WorkflowHtmlQuery {
    WorkflowHtmlManifest { query: WorkflowHtmlManifestQuery },
    WorkflowHtmlSource { query: WorkflowHtmlSourceQuery },
}
impl WorkflowHtmlQuery {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::WorkflowHtmlManifest { query } => query.validate(),
            Self::WorkflowHtmlSource { query } => {
                WorkflowHtmlManifestQuery {
                    run_id: query.run_id,
                    artifact_id: query.artifact_id,
                    sha256: query.sha256.clone(),
                }
                .validate()?;
                query.source.validate()
            }
        }
    }
}
