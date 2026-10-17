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

/// The complete immutable publication plus an explicitly selected bounded page/zoom.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactPdfQuery {
    pub expected: ArtifactLibraryItem,
    pub page: u32,
    pub width: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactPdfPage {
    pub document: ArtifactLibraryItem,
    pub page: u32,
    pub pages: u32,
    pub requested_width: u32,
    pub width: u32,
    pub height: u32,
    pub sha256: String,
    #[serde(with = "crate::protocol::base64_bytes")]
    #[ts(type = "string")]
    pub png: Vec<u8>,
}

#[cfg(test)]
mod pdf_tests {
    use super::*;
    #[test]
    fn pdf_query_and_derived_page_keep_source_identity_and_refuse_private_fields() {
        let item = ArtifactLibraryItem {
            id: Uuid::new_v4(),
            name: "doc.pdf".into(),
            format: "pdf".into(),
            project_id: Uuid::new_v4(),
            task_id: Uuid::new_v4(),
            created_at: 1,
            bytes: 100,
            sha256: "0".repeat(64),
            origin: ArtifactOrigin::AgentEdit,
        };
        let query = ArtifactPdfQuery {
            expected: item.clone(),
            page: 0,
            width: 800,
        };
        let mut value = serde_json::to_value(&query).unwrap();
        assert_eq!(
            serde_json::from_value::<ArtifactPdfQuery>(value.clone())
                .unwrap()
                .expected,
            item
        );
        value["path"] = "private.pdf".into();
        assert!(serde_json::from_value::<ArtifactPdfQuery>(value).is_err());
        let page = ArtifactPdfPage {
            document: item,
            page: 0,
            pages: 1,
            requested_width: 800,
            width: 800,
            height: 1200,
            sha256: "1".repeat(64),
            png: vec![1, 2, 3],
        };
        let mut value = serde_json::to_value(page).unwrap();
        assert_eq!(value["png"], "AQID");
        value["sourcePath"] = "private".into();
        assert!(serde_json::from_value::<ArtifactPdfPage>(value).is_err());
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactHtmlSourceQuery {
    pub expected: ArtifactLibraryItem,
    pub source: crate::html_resources::HtmlSourceQuery,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactHtmlManifest {
    pub document: ArtifactLibraryItem,
    pub manifest: Option<crate::html_resources::HtmlResourceManifest>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactHtmlSource {
    pub document: ArtifactLibraryItem,
    pub chunk: crate::html_resources::HtmlSourceChunk,
}
