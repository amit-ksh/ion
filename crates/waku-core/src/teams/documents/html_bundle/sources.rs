//! Retain the original resource documents for a future native document loader.
//! This is separate from the flattened portable export, never model context.
use super::*;

const MAX_TOTAL: u64 = 9 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HtmlBundleSource {
    pub path: String,
    pub media_type: String,
    pub sha256: String,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HtmlBundleSources {
    pub artifact_id: Uuid,
    pub artifact_sha256: String,
    pub entry_path: String,
    pub bytes_total: u64,
    /// Original HTML first; explicit resources retain publication argument order.
    pub sources: Vec<HtmlBundleSource>,
}
pub(super) fn seal(artifact: &Artifact, args: &Args) -> Result<String> {
    Ok(hash(&serde_json::to_vec(&(
        "ion-html-source-archive-v1",
        artifact,
        args,
    ))?))
}
pub(super) fn save(
    db: &Connection,
    artifact: &Artifact,
    args: &Args,
    source: &[u8],
    resources: &HashMap<String, (&'static str, Vec<u8>)>,
) -> Result<()> {
    let bytes_total = resources
        .values()
        .try_fold(source.len() as u64, |total, (_, bytes)| {
            total
                .checked_add(bytes.len() as u64)
                .context("HTML source size overflow")
        })?;
    ensure!(
        bytes_total <= MAX_TOTAL,
        "HTML sources exceed their publication bound"
    );
    db.execute(
        "INSERT INTO ion_team_html_source_archives VALUES(?,1,?,?)",
        params![
            artifact.id.to_string(),
            args.resources.len() + 1,
            bytes_total
        ],
    )?;
    let mut insert = db.prepare("INSERT INTO ion_team_html_sources VALUES(?,?,?,?,?)")?;
    insert.execute(params![
        artifact.id.to_string(),
        args.path,
        "text/html",
        args.sha256,
        source
    ])?;
    for resource in &args.resources {
        let (mime, bytes) = resources
            .get(&resource.path)
            .context("Pinned source disappeared during publication")?;
        insert.execute(params![
            artifact.id.to_string(),
            resource.path,
            mime,
            resource.sha256,
            bytes
        ])?;
    }
    Ok(())
}
impl TeamCoordinator {
    /// Worker-only read. Callers must authorize the expected artifact using their
    /// existing owner/project/task or workflow-evidence grant before calling.
    /// A new artifact with damaged/missing sources errors; legacy receipts return
    /// None. Neither path repairs or reopens workspace files.
    pub fn read_html_bundle_sources(
        &self,
        expected: &Artifact,
    ) -> Result<Option<HtmlBundleSources>> {
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let body: String = tx.query_row("SELECT body FROM ion_control_records WHERE id=? AND kind='artifact' AND length(body)<=1048576", [expected.id.to_string()], |r| r.get(0)).context("HTML artifact is unavailable")?;
        let ControlRecord::Artifact(artifact) = serde_json::from_str(&body)? else {
            anyhow::bail!("HTML artifact identity changed")
        };
        ensure!(
            serde_json::to_vec(&artifact)? == serde_json::to_vec(expected)?,
            "HTML artifact publication changed"
        );
        let bytes = crate::control::verified_document(&tx, &artifact)?;
        if artifact
            .evidence
            .as_ref()
            .and_then(|e| e.producer.activity_id.as_deref())
            != Some("ion-html-bundle")
        {
            return Ok(None);
        }
        let (arguments, signature): (String, String) = tx.query_row("SELECT arguments,signature FROM ion_team_html_bundles WHERE artifact_id=? AND length(arguments)<=65536", [artifact.id.to_string()], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let args: Args = serde_json::from_str(&arguments)?;
        validate(&args)?;
        if super::seal(&artifact, &args)? == signature {
            return Ok(None);
        }
        ensure!(
            seal(&artifact, &args)? == signature,
            "HTML source receipt changed"
        );
        let (version, count, total): (u32, u32, u64) = tx.query_row("SELECT version,source_count,byte_count FROM ion_team_html_source_archives WHERE artifact_id=?", [artifact.id.to_string()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).context("HTML source archive is unavailable")?;
        ensure!(
            version == 1 && count as usize == args.resources.len() + 1 && total <= MAX_TOTAL,
            "HTML source archive manifest changed"
        );
        let rows: u32 = tx.query_row(
            "SELECT count(*) FROM ion_team_html_sources WHERE artifact_id=?",
            [artifact.id.to_string()],
            |r| r.get(0),
        )?;
        ensure!(rows == count, "HTML source archive membership changed");
        let mut sources = Vec::with_capacity(count as usize);
        let mut observed = 0u64;
        let mut entries = vec![(&args.path, &args.sha256, "text/html", 1024 * 1024u64)];
        for resource in &args.resources {
            entries.push((
                &resource.path,
                &resource.sha256,
                mime(&resource.path)?,
                2 * 1024 * 1024,
            ));
        }
        for (path, sha, media_type, limit) in entries {
            let length: u64 = tx.query_row("SELECT length(content) FROM ion_team_html_sources WHERE artifact_id=? AND path=?", params![artifact.id.to_string(), path], |r| r.get(0)).context("HTML source is missing")?;
            ensure!(length <= limit, "HTML source exceeds its publication bound");
            observed = observed
                .checked_add(length)
                .context("HTML source size overflow")?;
            ensure!(observed <= total, "HTML source archive size changed");
            let source: Vec<u8> = tx.query_row("SELECT content FROM ion_team_html_sources WHERE artifact_id=? AND path=? AND mime=? AND sha256=? AND length(content)=?", params![artifact.id.to_string(), path, media_type, sha, length], |r| r.get(0)).context("HTML source identity changed")?;
            ensure!(hash(&source) == *sha, "HTML source content hash changed");
            sources.push(HtmlBundleSource {
                path: path.clone(),
                media_type: media_type.into(),
                sha256: sha.clone(),
                bytes: source,
            });
        }
        ensure!(
            observed == total
                && observed.saturating_sub(sources[0].bytes.len() as u64) <= 8 * 1024 * 1024,
            "HTML source archive total changed"
        );
        let result = HtmlBundleSources {
            artifact_id: artifact.id,
            artifact_sha256: hash(&bytes),
            entry_path: args.path,
            bytes_total: total,
            sources,
        };
        tx.commit()?;
        Ok(Some(result))
    }
}
