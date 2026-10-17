//! Metadata-only bounded resource chunks. The client verifies each full source
//! against the manifest committed inside its already verified HTML original.
use super::*;
use waku_protocol::{artifact_library::*, html_resources::*};
fn selected(db: &Connection, expected: &Artifact) -> Result<()> {
    let body:String=db.query_row("SELECT body FROM ion_control_records WHERE id=? AND kind='artifact' AND length(body)<=1048576",[expected.id.to_string()],|r|r.get(0))?;
    let ControlRecord::Artifact(actual) = serde_json::from_str(&body)? else {
        anyhow::bail!("HTML publication changed")
    };
    ensure!(
        serde_json::to_vec(&actual)? == serde_json::to_vec(expected)?,
        "HTML publication changed"
    );
    crate::control::artifact_retention::ensure_available(db, expected.id)?;
    Ok(())
}
fn manifest(db: &Connection, a: &Artifact) -> Result<Option<HtmlResourceManifest>> {
    selected(db, a)?;
    ensure!(a.format == "html", "Select an HTML publication");
    let receipt:Option<(String,String)>=db.query_row("SELECT arguments,signature FROM ion_team_html_bundles WHERE artifact_id=? AND length(arguments)<=65536",[a.id.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    let Some((arguments, signature)) = receipt else {
        ensure!(
            a.evidence
                .as_ref()
                .and_then(|e| e.producer.activity_id.as_deref())
                != Some("ion-html-bundle"),
            "HTML publication receipt is unavailable"
        );
        return Ok(None);
    };
    let args: Args = serde_json::from_str(&arguments)?;
    validate(&args)?;
    if sources::seal(a, &args)? != signature {
        ensure!(
            seal(a, &args)? == signature || sources::v1_seal(a, &args)? == signature,
            "HTML source receipt changed"
        );
        return Ok(None);
    }
    let evidence = a
        .evidence
        .as_ref()
        .context("HTML publication evidence missing")?;
    ensure!(
        evidence.version == 1
            && valid_hash(&evidence.sha256)
            && evidence.bytes > 0
            && evidence.bytes <= 16 * 1024 * 1024
            && !a.id.is_nil(),
        "HTML publication evidence changed"
    );
    ensure!(
        evidence.producer.activity_id.as_deref() == Some("ion-html-bundle")
            && evidence.producer.origin == ArtifactOrigin::TeamAssignment,
        "HTML source producer changed"
    );
    let (version,count,total):(u32,u32,u32)=db.query_row("SELECT version,source_count,byte_count FROM ion_team_html_source_archives WHERE artifact_id=?",[a.id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
    ensure!(
        version == 1 && count as usize == args.resources.len() + 1,
        "HTML source manifest changed"
    );
    let rows: u32 = db.query_row(
        "SELECT count(*) FROM (SELECT 1 FROM ion_team_html_sources WHERE artifact_id=? LIMIT 34)",
        [a.id.to_string()],
        |r| r.get(0),
    )?;
    ensure!(rows == count, "HTML source membership changed");
    let mut entries = vec![(&args.path, &args.sha256, "text/html")];
    for r in &args.resources {
        entries.push((&r.path, &r.sha256, mime(&r.path)?));
    }
    let mut sources = Vec::with_capacity(entries.len());
    for (i, (path, sha, media)) in entries.into_iter().enumerate() {
        let length:u32=db.query_row("SELECT length(content) FROM ion_team_html_sources WHERE artifact_id=? AND path=? AND mime=? AND sha256=?",params![a.id.to_string(),path,media,sha],|r|r.get(0))?;
        sources.push(HtmlSourceDescriptor {
            index: i.try_into()?,
            path: path.clone(),
            media_type: media.into(),
            bytes: length,
            sha256: sha.clone(),
        });
    }
    let (sha256, observed) = commitment(&args.path, &sources)?;
    ensure!(total == observed, "HTML source total changed");
    let anchor = anchor(&sha256);
    let (length,prefix):(u64,Vec<u8>)=db.query_row("SELECT length(content),substr(content,1,?) FROM ion_team_document_receipts WHERE artifact_id=?",params![anchor.len(),a.id.to_string()],|r|Ok((r.get(0)?,r.get(1)?)))?;
    ensure!(
        length == evidence.bytes && prefix == anchor.as_bytes(),
        "HTML source commitment changed"
    );
    Ok(Some(HtmlResourceManifest {
        version: 1,
        artifact_id: a.id,
        artifact_sha256: evidence.sha256.clone(),
        entry_path: args.path,
        total_bytes: total,
        sha256,
        sources,
    }))
}
impl TeamCoordinator {
    pub fn html_resource_manifest(
        &self,
        expected: &Artifact,
    ) -> Result<Option<HtmlResourceManifest>> {
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let result = manifest(&tx, expected)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn html_resource_chunk(
        &self,
        expected: &Artifact,
        query: &HtmlSourceQuery,
    ) -> Result<HtmlSourceChunk> {
        query.validate()?;
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let manifest =
            manifest(&tx, expected)?.context("HTML originals have no publication commitment")?;
        ensure!(
            manifest.sha256 == query.manifest_sha256,
            "HTML source manifest changed"
        );
        let source = manifest
            .sources
            .get(query.source_index as usize)
            .context("HTML source index exceeds manifest")?;
        ensure!(
            source.sha256 == query.source_sha256 && query.offset < source.bytes,
            "HTML source selection changed"
        );
        let count = u32::from(query.limit).min(source.bytes - query.offset);
        let data:Vec<u8>=tx.query_row("SELECT substr(content,?,?) FROM ion_team_html_sources WHERE artifact_id=? AND path=? AND length(content)=?",params![query.offset+1,count,expected.id.to_string(),source.path,source.bytes],|r|r.get(0))?;
        ensure!(
            data.len() == count as usize,
            "HTML source chunk length changed"
        );
        let end = query.offset + count;
        let result = HtmlSourceChunk {
            artifact_id: expected.id,
            artifact_sha256: manifest.artifact_sha256,
            manifest_sha256: manifest.sha256,
            source_index: source.index,
            source_sha256: source.sha256.clone(),
            offset: query.offset,
            next_offset: (end < source.bytes).then_some(end),
            data,
        };
        tx.commit()?;
        Ok(result)
    }
    pub fn read_artifact_html_manifest(
        &self,
        query: &ArtifactDocumentQuery,
    ) -> Result<ArtifactHtmlManifest> {
        let artifact = self.control.selected_html_artifact(&query.expected)?;
        Ok(ArtifactHtmlManifest {
            document: query.expected.clone(),
            manifest: self.html_resource_manifest(&artifact)?,
        })
    }
    pub fn read_artifact_html_source(
        &self,
        query: &ArtifactHtmlSourceQuery,
    ) -> Result<ArtifactHtmlSource> {
        let artifact = self.control.selected_html_artifact(&query.expected)?;
        Ok(ArtifactHtmlSource {
            document: query.expected.clone(),
            chunk: self.html_resource_chunk(&artifact, &query.source)?,
        })
    }
    pub fn read_workflow_html(&self, query: &WorkflowHtmlQuery) -> Result<serde_json::Value> {
        query.validate()?;
        let (run_id, id, sha, source) = match query {
            WorkflowHtmlQuery::WorkflowHtmlManifest { query } => {
                (query.run_id, query.artifact_id, &query.sha256, None)
            }
            WorkflowHtmlQuery::WorkflowHtmlSource { query } => (
                query.run_id,
                query.artifact_id,
                &query.sha256,
                Some(&query.source),
            ),
        };
        let run = self.automation_run(run_id)?;
        let teams = run
            .stages
            .iter()
            .filter(|s| s.started && !s.skipped)
            .map(|s| self.snapshot(s.team_run_id))
            .collect::<Result<Vec<_>>>()?;
        let records = self.control.load()?;
        let row =
            waku_protocol::gateway_portfolio::history::accepted_artifacts(&run, &teams, &records)?
                .into_iter()
                .find(|r| r["id"] == id.to_string())
                .context("HTML document is not accepted workflow evidence")?;
        ensure!(
            row["sha256"] == *sha && row["format"] == "html",
            "HTML workflow selection changed"
        );
        let artifact = records
            .records
            .iter()
            .find_map(|r| match r {
                ControlRecord::Artifact(a) if a.id == id => Some(a),
                _ => None,
            })
            .context("HTML evidence is unavailable")?;
        let now: u64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis()
            .try_into()?;
        Ok(if let Some(source) = source {
            serde_json::json!({"kind":"workflowHtmlSource","page":{"runId":run_id,"document":row,"chunk":self.html_resource_chunk(artifact,source)?},"pageTruncated":false,"servedAt":now})
        } else {
            serde_json::json!({"kind":"workflowHtmlManifest","page":{"runId":run_id,"document":row,"manifest":self.html_resource_manifest(artifact)?},"pageTruncated":false,"servedAt":now})
        })
    }
}
