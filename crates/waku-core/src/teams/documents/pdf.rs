//! Opaque generated PDF bytes plus an explicitly agent-authored summary.
//! No parser, renderer, external process or user-selected file is reached here.
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};

const MAX_PDF_BYTES: usize = 24 * 1024;
const MAX_SUMMARY_BYTES: usize = 4096;

fn decode(encoded: &str, summary: &str) -> Result<Vec<u8>> {
    ensure!(
        encoded.len() <= MAX_PDF_BYTES.div_ceil(3) * 4,
        "Generated PDF exceeds 24 KiB"
    );
    text(summary, MAX_SUMMARY_BYTES, "PDF summary")?;
    ensure!(
        !summary
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')),
        "PDF summary must be readable text"
    );
    let bytes = STANDARD
        .decode(encoded)
        .context("PDF requires strict standard base64")?;
    ensure!(
        STANDARD.encode(&bytes) == encoded,
        "PDF requires canonical base64"
    );
    ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_PDF_BYTES,
        "Generated PDF exceeds 24 KiB"
    );
    // Envelope checks are not PDF interpretation or a rendering acceptance claim.
    let version = bytes.get(..8).unwrap_or_default();
    ensure!(
        matches!(
            version,
            b"%PDF-1.0"
                | b"%PDF-1.1"
                | b"%PDF-1.2"
                | b"%PDF-1.3"
                | b"%PDF-1.4"
                | b"%PDF-1.5"
                | b"%PDF-1.6"
                | b"%PDF-1.7"
                | b"%PDF-2.0"
        ),
        "Generated file has no supported PDF header"
    );
    let end = bytes
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .map_or(0, |i| i + 1);
    ensure!(
        bytes[..end].ends_with(b"%%EOF"),
        "Generated PDF has no final EOF marker"
    );
    Ok(bytes)
}

fn signature(artifact: &Artifact, summary: &str) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(artifact, summary))?)
    ))
}

pub(in crate::teams) fn verify(
    db: &Connection,
    artifact: &Artifact,
    bytes: &[u8],
) -> Result<Option<String>> {
    if artifact.format != "pdf" {
        return Ok(None);
    }
    let (summary, seal): (String, String) = db.query_row(
        "SELECT summary,signature FROM ion_team_pdf_summaries WHERE artifact_id=? AND length(summary) BETWEEN 1 AND 4096",
        [artifact.id.to_string()], |r| Ok((r.get(0)?, r.get(1)?)),
    ).context("Generated PDF summary receipt is unavailable")?;
    ensure!(
        signature(artifact, &summary)? == seal
            && artifact.evidence.as_ref().is_some_and(
                |e| e.matches(bytes) && e.producer.origin == ArtifactOrigin::TeamAssignment
            ),
        "Generated PDF evidence changed"
    );
    Ok(Some(summary))
}

impl TeamCoordinator {
    /// Selected harnesses publish bytes through the owner; callers cannot choose provenance.
    pub fn publish_pdf(
        &self,
        lease: &AssignmentLease,
        request_id: Uuid,
        name: &str,
        content_base64: &str,
        summary: &str,
        now: u64,
    ) -> Result<Artifact> {
        let bytes = decode(content_base64, summary)?;
        ensure!(
            name.to_ascii_lowercase().ends_with(".pdf"),
            "Use a .pdf document filename"
        );
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, lease.run_id)?;
        active(&run, now)?;
        let work = checked_lease(&run, lease, now, false)?;
        ensure!(
            !name.chars().any(char::is_control),
            "Use a readable PDF filename"
        );
        let already_published: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM ion_team_document_receipts WHERE work_id=? AND request_id=?)",
            params![work.id.to_string(), request_id.to_string()], |r| r.get(0),
        )?;
        let artifact = publish_bytes(&tx, &run, work, request_id, name, "pdf", &bytes, now)?;
        let seal = signature(&artifact, summary)?;
        let existing: Option<(String, String)> = tx
            .query_row(
                "SELECT summary,signature FROM ion_team_pdf_summaries WHERE artifact_id=?",
                [artifact.id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((old_summary, old_seal)) = existing {
            ensure!(
                old_summary == summary && old_seal == seal,
                "PDF replay summary changed"
            );
            verify(&tx, &artifact, &bytes)?;
        } else {
            // A missing summary on an existing publication must not be silently repaired.
            ensure!(
                !already_published,
                "Published PDF summary receipt was removed"
            );
            tx.execute(
                "INSERT INTO ion_team_pdf_summaries VALUES(?,?,?)",
                params![artifact.id.to_string(), summary, seal],
            )?;
        }
        tx.commit()?;
        Ok(artifact)
    }

    /// Only call after a scoped assignment/source read. No PDF bytes enter model context.
    pub(crate) fn document_model_reply(&self, id: Uuid, bytes: &[u8]) -> Result<serde_json::Value> {
        let mut connection = self.connect()?;
        let db = connection.transaction()?;
        let body: String = db.query_row(
            "SELECT body FROM ion_control_records WHERE id=? AND kind='artifact'",
            [id.to_string()],
            |r| r.get(0),
        )?;
        let ControlRecord::Artifact(artifact) = serde_json::from_str(&body)? else {
            anyhow::bail!("Document identity changed");
        };
        ensure!(
            crate::control::verified_document(&db, &artifact)? == bytes,
            "Document changed during model read"
        );
        let reply = model_reply(&db, &artifact, bytes)?;
        db.commit()?;
        Ok(reply)
    }
}

pub(super) fn model_reply(
    db: &Connection,
    artifact: &Artifact,
    bytes: &[u8],
) -> Result<serde_json::Value> {
    if artifact.format == "pdf" {
        let summary = verify(db, artifact, bytes)?.context("PDF summary receipt missing")?;
        Ok(
            serde_json::json!({"artifact_id":artifact.id,"name":artifact.name,"format":"pdf",
            "binary":true,"bytes":bytes.len(),"sha256":artifact.evidence.as_ref().context("PDF hash missing")?.sha256,
            "summary":summary,"summary_source":"agent","content_interpreted":false,
            "note":"Summary is agent-authored, not extracted PDF text. Exact PDF bytes are available through Artifacts export."}),
        )
    } else {
        let content = std::str::from_utf8(bytes).context("Document is not UTF-8 text")?;
        let excerpt = crate::control::json_excerpt(content, 8192);
        Ok(
            serde_json::json!({"artifact_id":artifact.id,"content":excerpt,"truncated":excerpt.len()!=content.len()}),
        )
    }
}

pub(in crate::teams) fn handoff(
    db: &Connection,
    artifact: &Artifact,
    bytes: Vec<u8>,
) -> Result<PreparedInput> {
    if artifact.format == "pdf" {
        Ok(PreparedInput {
            key: artifact.id.to_string(),
            name: format!("{}.artifact.json", artifact.id),
            content: serde_json::to_string(&model_reply(db, artifact, &bytes)?)?,
        })
    } else {
        Ok(PreparedInput {
            key: artifact.id.to_string(),
            name: artifact.name.clone(),
            content: String::from_utf8(bytes).context("Stage input is not UTF-8")?,
        })
    }
}

pub(in crate::teams) fn recover(
    db: &Connection,
    artifact: &Artifact,
    args: &serde_json::Value,
    bytes: &[u8],
) -> Result<()> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Args {
        name: String,
        content_base64: String,
        summary: String,
    }
    let args: Args = serde_json::from_value(args.clone())?;
    ensure!(
        artifact.name == args.name
            && artifact.format == "pdf"
            && decode(&args.content_base64, &args.summary)? == bytes,
        "Pending PDF differs from its durable receipt"
    );
    let summary = verify(db, artifact, bytes)?.context("PDF summary receipt missing")?;
    ensure!(
        summary == args.summary,
        "Pending PDF summary differs from its durable receipt"
    );
    Ok(())
}
