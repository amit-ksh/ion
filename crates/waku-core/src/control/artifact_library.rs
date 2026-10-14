//! An append-index bounds paging independently of unrelated workspace revisions.
//! Cursor filters and exact immutable anchor metadata are rechecked on every page.
use super::*;
use rusqlite::{params_from_iter, types::Value};
use serde::{Deserialize, Serialize};
use waku_protocol::artifact_library::*;

pub(super) fn schema(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS ion_artifact_library (seq INTEGER PRIMARY KEY AUTOINCREMENT,id TEXT NOT NULL UNIQUE,project TEXT,task TEXT,format TEXT,created INTEGER NOT NULL,body TEXT NOT NULL);
      CREATE INDEX IF NOT EXISTS ion_artifact_order ON ion_artifact_library(created DESC,id DESC);
      CREATE INDEX IF NOT EXISTS ion_artifact_project_order ON ion_artifact_library(project,created DESC,id DESC);
      CREATE INDEX IF NOT EXISTS ion_artifact_task_order ON ion_artifact_library(task,created DESC,id DESC);
      CREATE INDEX IF NOT EXISTS ion_artifact_format_order ON ion_artifact_library(format,created DESC,id DESC);
      CREATE TABLE IF NOT EXISTS ion_artifact_library_install (id INTEGER PRIMARY KEY CHECK(id=1));
      CREATE TRIGGER IF NOT EXISTS ion_artifact_library_insert AFTER INSERT ON ion_control_records WHEN NEW.kind='artifact' BEGIN INSERT INTO ion_artifact_library(id,project,task,format,created,body) VALUES(NEW.id,json_extract(NEW.body,'$.projectId'),json_extract(NEW.body,'$.taskId'),json_extract(NEW.body,'$.format'),COALESCE(json_extract(NEW.body,'$.createdAt'),0),NEW.body); END;
      CREATE TRIGGER IF NOT EXISTS ion_artifact_library_update AFTER UPDATE ON ion_control_records WHEN NEW.kind='artifact' BEGIN UPDATE ion_artifact_library SET project=json_extract(NEW.body,'$.projectId'),task=json_extract(NEW.body,'$.taskId'),format=json_extract(NEW.body,'$.format'),created=COALESCE(json_extract(NEW.body,'$.createdAt'),0),body=NEW.body WHERE id=NEW.id; END;
      CREATE TRIGGER IF NOT EXISTS ion_artifact_library_delete AFTER DELETE ON ion_control_records WHEN OLD.kind='artifact' BEGIN DELETE FROM ion_artifact_library WHERE id=OLD.id; END;
      SAVEPOINT artifact_library_install;
      INSERT INTO ion_artifact_library(id,project,task,format,created,body) SELECT id,json_extract(body,'$.projectId'),json_extract(body,'$.taskId'),json_extract(body,'$.format'),COALESCE(json_extract(body,'$.createdAt'),0),body FROM ion_control_records WHERE kind='artifact' AND NOT EXISTS(SELECT 1 FROM ion_artifact_library_install) ORDER BY id;
      INSERT OR IGNORE INTO ion_artifact_library_install VALUES(1);
      RELEASE artifact_library_install;")?;
    Ok(())
}
// Explicit publisher provenance. Default-only historical imports are not AI receipts.
const ELIGIBLE: &str = "json_extract(body,'$.evidence.version')=1 AND json_extract(body,'$.evidence.bytes') BETWEEN 1 AND 16777216 AND project IS NOT NULL AND task IS NOT NULL AND (
 (json_extract(body,'$.evidence.producer.origin')='agentGenerated' AND length(json_extract(body,'$.evidence.producer.activityId')) BETWEEN 1 AND 512) OR
 (json_extract(body,'$.evidence.producer.origin')='agentEdit' AND json_extract(body,'$.evidence.producer.turnId') IS NOT NULL AND length(json_extract(body,'$.evidence.producer.activityId')) BETWEEN 1 AND 512 AND length(json_extract(body,'$.evidence.producer.sourcePath')) BETWEEN 1 AND 8192) OR
 (json_extract(body,'$.evidence.producer.origin')='teamAssignment' AND json_extract(body,'$.evidence.producer.runId') IS NOT NULL AND json_extract(body,'$.evidence.producer.assignmentId') IS NOT NULL AND json_extract(body,'$.evidence.producer.agentId') IS NOT NULL AND json_extract(body,'$.evidence.producer.requestId') IS NOT NULL))";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    version: u8,
    scope: String,
    ceiling: i64,
    anchor: ArtifactLibraryItem,
    record_sha256: String,
}
fn record_sha256(artifact: &Artifact) -> Result<String> {
    use sha2::{Digest, Sha256};
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(artifact)?)
    ))
}
fn scope(query: &ArtifactLibraryQuery) -> Result<String> {
    use sha2::{Digest, Sha256};
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(
            query.project_id,
            query.task_id,
            &query.format,
            query.limit
        ))?)
    ))
}
fn parse_cursor(text: &str) -> Result<Cursor> {
    ensure!(
        !text.is_empty()
            && text.len() <= 4096
            && text.len() % 2 == 0
            && text
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "Artifact cursor is invalid; refresh the library"
    );
    let bytes: Vec<u8> = text
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16))
        .collect::<std::result::Result<_, _>>()?;
    Ok(
        serde_json::from_slice(&bytes)
            .context("Artifact cursor is invalid; refresh the library")?,
    )
}
fn encode_cursor(cursor: &Cursor) -> Result<String> {
    Ok(serde_json::to_vec(cursor)?
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}
fn valid_format(format: &str) -> bool {
    matches!(
        format,
        "md" | "txt"
            | "json"
            | "csv"
            | "html"
            | "htm"
            | "pdf"
            | "docx"
            | "xlsx"
            | "png"
            | "svg"
            | "xml"
            | "yaml"
            | "yml"
    )
}
fn item(artifact: &Artifact) -> Result<ArtifactLibraryItem> {
    let evidence = artifact
        .evidence
        .as_ref()
        .context("Document lacks verified publisher evidence")?;
    let producer = &evidence.producer;
    let known = match producer.origin {
        ArtifactOrigin::AgentGenerated => producer
            .activity_id
            .as_ref()
            .is_some_and(|s| !s.is_empty() && s.len() <= 512),
        ArtifactOrigin::AgentEdit => {
            producer.turn_id.is_some_and(|id| !id.is_nil())
                && producer
                    .activity_id
                    .as_ref()
                    .is_some_and(|s| !s.is_empty() && s.len() <= 512)
                && producer
                    .source_path
                    .as_ref()
                    .is_some_and(|s| !s.is_empty() && s.len() <= 8192)
        }
        ArtifactOrigin::TeamAssignment => [
            producer.run_id,
            producer.assignment_id,
            producer.agent_id,
            producer.request_id,
        ]
        .iter()
        .all(|id| id.is_some_and(|id| !id.is_nil())),
    };
    let project = artifact.project_id.context("Artifact project is missing")?;
    let task = artifact.task_id.context("Artifact task is missing")?;
    ensure!(
        known
            && !artifact.id.is_nil()
            && !project.is_nil()
            && !task.is_nil()
            && artifact.created_at > 0
            && artifact.created_at <= i64::MAX as u64
            && !artifact.name.trim().is_empty()
            && artifact.name.len() <= 160
            && !artifact
                .name
                .chars()
                .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
            && artifact.name != "."
            && artifact.name != ".."
            && valid_format(&artifact.format)
            && artifact.reference == format!("ion-document:{}", artifact.id)
            && evidence.version == 1
            && evidence.bytes > 0
            && evidence.bytes <= 16 * 1024 * 1024
            && evidence.sha256.len() == 64
            && evidence
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "Artifact metadata or publisher evidence changed"
    );
    Ok(ArtifactLibraryItem {
        id: artifact.id,
        name: artifact.name.clone(),
        format: artifact.format.clone(),
        project_id: project,
        task_id: task,
        created_at: artifact.created_at,
        bytes: evidence.bytes,
        sha256: evidence.sha256.clone(),
        origin: producer.origin,
    })
}
fn stored(db: &Connection, id: Uuid) -> Result<Artifact> {
    let body: String = db
        .query_row(
            "SELECT body FROM ion_control_records WHERE id=? AND kind='artifact'",
            [id.to_string()],
            |row| row.get(0),
        )
        .context("Artifact no longer exists; refresh its library")?;
    let ControlRecord::Artifact(artifact) = serde_json::from_str(&body)? else {
        anyhow::bail!("Artifact identity changed")
    };
    ensure!(artifact.id == id, "Artifact identity changed");
    Ok(artifact)
}
fn matches_query(item: &ArtifactLibraryItem, query: &ArtifactLibraryQuery) -> bool {
    query.project_id.is_none_or(|id| id == item.project_id)
        && query.task_id.is_none_or(|id| id == item.task_id)
        && query
            .format
            .as_ref()
            .is_none_or(|format| format == &item.format)
}
impl ControlStore {
    pub fn list_artifacts(&self, query: &ArtifactLibraryQuery) -> Result<ArtifactLibraryPage> {
        ensure!(
            (1..=32).contains(&query.limit)
                && query.project_id.is_none_or(|id| !id.is_nil())
                && query.task_id.is_none_or(|id| !id.is_nil())
                && query.format.as_ref().is_none_or(|s| valid_format(s)),
            "Use valid artifact filters and a page size from 1 to 32"
        );
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let maximum:i64=tx.query_row("SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name='ion_artifact_library'),0)",[],|r|r.get(0))?;
        let cursor = query
            .after
            .as_ref()
            .map(|value| parse_cursor(value))
            .transpose()?;
        let fingerprint = scope(query)?;
        let ceiling = if let Some(cursor) = &cursor {
            ensure!(
                cursor.version == 1
                    && cursor.scope == fingerprint
                    && cursor.ceiling > 0
                    && cursor.ceiling <= maximum
                    && matches_query(&cursor.anchor, query),
                "Artifact cursor belongs to different filters; refresh the library"
            );
            let original = stored(&tx, cursor.anchor.id)?;
            let anchor = item(&original)?;
            let sequence: i64 = tx.query_row(
                "SELECT seq FROM ion_artifact_library WHERE id=?",
                [anchor.id.to_string()],
                |r| r.get(0),
            )?;
            ensure!(
                anchor == cursor.anchor
                    && sequence <= cursor.ceiling
                    && record_sha256(&original)? == cursor.record_sha256,
                "Artifact cursor document changed; refresh the library"
            );
            cursor.ceiling
        } else {
            maximum
        };
        let mut filter = format!("seq<=? AND ({ELIGIBLE})");
        let mut values = vec![Value::Integer(ceiling)];
        for (column, value) in [
            ("project", query.project_id.map(|id| id.to_string())),
            ("task", query.task_id.map(|id| id.to_string())),
            ("format", query.format.clone()),
        ] {
            if let Some(value) = value {
                filter.push_str(&format!(" AND {column}=?"));
                values.push(Value::Text(value));
            }
        }
        let total: u32 = tx.query_row(
            &format!("SELECT count(*) FROM ion_artifact_library WHERE {filter}"),
            params_from_iter(&values),
            |r| r.get(0),
        )?;
        if let Some(cursor) = &cursor {
            filter.push_str(" AND (created<? OR (created=? AND id<?))");
            values.extend([
                Value::Integer(cursor.anchor.created_at as i64),
                Value::Integer(cursor.anchor.created_at as i64),
                Value::Text(cursor.anchor.id.to_string()),
            ]);
        }
        values.push(Value::Integer(i64::from(query.limit) + 1));
        let mut statement=tx.prepare(&format!("SELECT id,body FROM ion_artifact_library WHERE {filter} ORDER BY created DESC,id DESC LIMIT ?"))?;
        let bodies = statement
            .query_map(params_from_iter(&values), |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut items = Vec::new();
        for (id, body) in bodies {
            let ControlRecord::Artifact(artifact) = serde_json::from_str(&body)? else {
                anyhow::bail!("Artifact index identity changed")
            };
            ensure!(
                id == artifact.id.to_string(),
                "Artifact index identity changed"
            );
            items.push(item(&artifact)?);
        }
        let more = items.len() > usize::from(query.limit);
        items.truncate(usize::from(query.limit));
        let next_cursor = if more {
            Some(encode_cursor(&Cursor {
                version: 1,
                scope: fingerprint,
                ceiling,
                anchor: items.last().unwrap().clone(),
                record_sha256: record_sha256(&stored(&tx, items.last().unwrap().id)?)?,
            })?)
        } else {
            None
        };
        drop(statement);
        tx.commit()?;
        Ok(ArtifactLibraryPage {
            items,
            total,
            next_cursor,
        })
    }
    /// One process-wide raster worker admission, never a blocking lock or UI operation.
    pub fn render_artifact_pdf(&self, query: &ArtifactPdfQuery) -> Result<ArtifactPdfPage> {
        use sha2::{Digest, Sha256};
        ensure!(
            query.expected.format == "pdf"
                && query.expected.bytes > 0
                && query.expected.bytes <= waku_document::MAX_DOCUMENT_BYTES as u64
                && query.page < waku_document::MAX_PAGES
                && [800, 1200, 1600].contains(&query.width),
            "Select a supported PDF page/zoom; larger originals remain exportable"
        );
        let (original, source_hash) = self.verified_artifact_document(&ArtifactDocumentQuery {
            expected: query.expected.clone(),
        })?;
        let raster = waku_document::try_render_pdf_png(&original.bytes, query.page, query.width)?;
        let png = raster.png;
        // Keep replacement/removal during rendering from returning stale publication bytes.
        let db = self.connect()?;
        let artifact = stored(&db, query.expected.id)?;
        ensure!(
            item(&artifact)? == query.expected && record_sha256(&artifact)? == source_hash,
            "PDF publication changed during rendering. Refresh the library."
        );
        let verified = verified_document(&db, &artifact)?;
        ensure!(
            verified == original.bytes,
            "PDF content changed during rendering. Refresh the library."
        );
        Ok(ArtifactPdfPage {
            document: original.document,
            page: raster.page,
            pages: raster.pages,
            requested_width: query.width,
            width: raster.width,
            height: raster.height,
            sha256: format!("{:x}", Sha256::digest(&png)),
            png,
        })
    }
    pub fn read_artifact_document(
        &self,
        query: &ArtifactDocumentQuery,
    ) -> Result<ArtifactDocument> {
        self.verified_artifact_document(query)
            .map(|(document, _)| document)
    }
    fn verified_artifact_document(
        &self,
        query: &ArtifactDocumentQuery,
    ) -> Result<(ArtifactDocument, String)> {
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let artifact = stored(&tx, query.expected.id)?;
        let document = item(&artifact)?;
        ensure!(
            document == query.expected,
            "Artifact changed or belongs to a different selection; refresh its library"
        );
        let bytes = verified_document(&tx, &artifact)?;
        ensure!(
            artifact
                .evidence
                .as_ref()
                .is_some_and(|e| e.matches(&bytes)),
            "Artifact content digest changed"
        );
        let hash = record_sha256(&artifact)?;
        tx.commit()?;
        Ok((ArtifactDocument { document, bytes }, hash))
    }
}
#[cfg(test)]
mod tests;
