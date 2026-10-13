//! Retained collectors are admitted separately from compute and workspace cleanup.
//! Every original request holds its full encoded receipt allowance indefinitely.
use super::*;
use std::collections::BTreeMap;

pub(super) const RECEIPT_BYTES: u64 = 512 * 1024;
const MAX_ORIGINALS: usize = 4096;
const MAX_SCOPE_BYTES: u64 = RECEIPT_BYTES * MAX_ORIGINALS as u64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum GpuCollectorRetentionMode {
    #[default]
    KeepUntilExplicitlyCleaned,
}
/// An owner allowance for this bucket/prefix. This does not assert remote deletion,
/// a physical bucket quota, version-storage overhead or a settled invoice amount.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GpuCollectorRetentionPolicy {
    pub version: u16,
    pub mode: GpuCollectorRetentionMode,
    pub max_bytes: u64,
    pub max_receipts: u32,
}
impl GpuCollectorRetentionPolicy {
    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(
            self.version == 1
                && (RECEIPT_BYTES..=MAX_SCOPE_BYTES).contains(&self.max_bytes)
                && (1..=MAX_ORIGINALS as u32).contains(&self.max_receipts),
            "GPU collector retention needs version1 and bounded encoded bytes/receipt count"
        );
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct GpuCollectorScopeUsage {
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
/// A consistent owner/background snapshot. It contains no payload, credential or URL.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct GpuCollectorUsage {
    pub version: u16,
    pub bucket_id: String,
    pub bucket_name: String,
    pub region: String,
    pub prefix: String,
    pub mode: GpuCollectorRetentionMode,
    pub reserved_bytes: u64,
    pub reserved_receipts: u32,
    pub observed_bytes: u64,
    pub observed_receipts: u32,
    pub historical_unbounded_receipts: u32,
    pub bucket_reserved_bytes: u64,
    pub bucket_reserved_receipts: u32,
    /// Maximum further full receipts allowed by every overlapping original cap.
    /// None means no cap was approved; it never authorizes a new launch.
    pub admissible_receipts: Option<u32>,
    pub scopes: Vec<GpuCollectorScopeUsage>,
}
struct Original {
    request: Uuid,
    volume: GpuResultVolume,
    observed_bytes: Option<u64>,
}
fn overlaps(a: &str, b: &str) -> bool {
    a == b
        || a.strip_prefix(b).is_some_and(|tail| tail.starts_with('/'))
        || b.strip_prefix(a).is_some_and(|tail| tail.starts_with('/'))
}
/// SQL scopes known identities, but routes unknown/malformed identities to the
/// verifier instead of excluding them. Compact bounded facts avoid loading every
/// saved command payload or collector output into this background operation.
fn originals(tx: &Transaction<'_>, volume: &GpuResultVolume) -> Result<Vec<Original>> {
    volume.validate()?;
    let sql = r#"
        WITH candidates AS (
            SELECT j.request_id,j.project,g.artifact_id,
                json_valid(j.policy) AND json_valid(j.signature) AND json_valid(j.body)
                    AND (g.receipt IS NULL OR json_valid(g.receipt)) AS valid,
                CASE WHEN json_valid(j.policy) THEN json_type(j.policy,'$.result_volume') END AS kind,
                CASE WHEN json_valid(j.signature) THEN json_type(j.signature,'$.policy.result_volume') END AS signature_kind,
                CASE WHEN json_valid(j.policy) THEN json_extract(j.policy,'$.result_volume') END AS volume,
                CASE WHEN json_valid(j.signature) THEN json_extract(j.signature,'$.policy.result_volume') END AS signature_volume,
                CASE WHEN json_valid(j.policy) THEN json_extract(j.policy,'$.result_volume.bucket_id') END AS bucket_id,
                CASE WHEN json_valid(j.policy) THEN json_extract(j.policy,'$.result_volume.bucket_name') END AS bucket_name,
                CASE WHEN json_valid(j.policy) THEN json_extract(j.policy,'$.result_volume.region') END AS region,
                CASE WHEN json_valid(j.policy) THEN json_extract(j.policy,'$.result_volume.prefix') END AS prefix,
                CASE WHEN json_valid(j.signature) THEN json_extract(j.signature,'$.policy.result_volume.bucket_id') END AS original_bucket_id,
                CASE WHEN json_valid(j.signature) THEN json_extract(j.signature,'$.policy.result_volume.bucket_name') END AS original_bucket_name,
                CASE WHEN json_valid(j.signature) THEN json_extract(j.signature,'$.policy.result_volume.region') END AS original_region,
                CASE WHEN json_valid(j.signature) THEN json_extract(j.signature,'$.policy.result_volume.prefix') END AS original_prefix,
                CASE WHEN json_valid(j.signature) THEN json_extract(j.signature,'$.request.request_id') END AS original_request,
                CASE WHEN json_valid(j.body) THEN json_object(
                    'project',json_extract(j.body,'$.metadata.parentId'),
                    'name',json_extract(j.body,'$.metadata.name'),
                    'request',json_extract(j.body,'$.metadata.labels.ion-request'),
                    'volumes',json_extract(j.body,'$.spec.volumes'),
                    'args',json_extract(j.body,'$.spec.args')) END AS body,
                CASE WHEN json_valid(j.signature) THEN json_object(
                    'project',json_extract(j.signature,'$.body.metadata.parentId'),
                    'name',json_extract(j.signature,'$.body.metadata.name'),
                    'request',json_extract(j.signature,'$.body.metadata.labels.ion-request'),
                    'volumes',json_extract(j.signature,'$.body.spec.volumes'),
                    'args',json_extract(j.signature,'$.body.spec.args')) END AS original_body,
                CASE WHEN json_valid(j.body) THEN json_extract(j.body,'$.spec.volumes') END AS mounts,
                CASE WHEN json_valid(j.signature) THEN json_extract(j.signature,'$.body.spec.volumes') END AS original_mounts,
                CASE WHEN json_valid(j.body) THEN json_extract(j.body,'$.spec.volumes[0].source') END AS mounted_bucket,
                CASE WHEN json_valid(j.body) THEN json_extract(j.body,'$.spec.volumes[0].sourcePath') END AS mounted_prefix,
                CASE WHEN json_valid(j.body) THEN json_extract(j.body,'$.spec.args') END AS args,
                length(CAST(g.receipt AS BLOB)) AS observed_bytes,
                CASE WHEN json_valid(g.receipt) THEN json_extract(g.receipt,'$.request_id') END AS observed_request
            FROM ion_team_gpu_jobs j LEFT JOIN ion_team_gpu_results g ON g.request_id=j.request_id
        )
        SELECT request_id,project,valid,
            CASE WHEN length(CAST(volume AS BLOB))<=2048 THEN volume END,
            CASE WHEN length(CAST(signature_volume AS BLOB))<=2048 THEN signature_volume END,
            original_request,
            CASE WHEN length(CAST(body AS BLOB))<=4096 THEN body END,
            CASE WHEN length(CAST(original_body AS BLOB))<=4096 THEN original_body END,
            observed_bytes,observed_request,artifact_id
        FROM candidates WHERE valid=0
            OR (kind IS NULL OR kind='null') AND (
                signature_kind IS NOT NULL AND signature_kind!='null'
                OR mounts IS NOT NULL AND mounts!='[]'
                OR original_mounts IS NOT NULL AND original_mounts!='[]'
                OR instr(coalesce(args,''),'--result-volume')>0)
            OR kind IS NOT NULL AND kind!='null' AND (
                kind!='object' OR signature_kind IS NOT 'object'
                OR typeof(bucket_id)!='text' OR typeof(bucket_name)!='text'
                OR typeof(region)!='text' OR typeof(prefix)!='text'
                OR length(bucket_id) NOT BETWEEN 1 AND 160 OR bucket_id GLOB '*[^A-Za-z0-9_-]*'
                OR length(bucket_name) NOT BETWEEN 3 AND 63 OR bucket_name GLOB '*[^a-z0-9.-]*'
                OR substr(bucket_name,1,1) NOT GLOB '[a-z0-9]' OR substr(bucket_name,-1) NOT GLOB '[a-z0-9]'
                OR instr(bucket_name,'..')>0
                OR length(region) NOT BETWEEN 1 AND 40 OR region GLOB '*[^a-z0-9-]*'
                OR length(prefix) NOT BETWEEN 1 AND 256 OR prefix GLOB '*[^A-Za-z0-9/_.-]*'
                OR substr(prefix,1,1)='/' OR substr(prefix,-1)='/' OR instr(prefix,'//')>0
                OR bucket_id IS NOT original_bucket_id OR bucket_name IS NOT original_bucket_name
                OR region IS NOT original_region OR prefix IS NOT original_prefix
                OR bucket_id IS NOT mounted_bucket OR prefix IS NOT mounted_prefix
                OR bucket_id=?1 OR (bucket_name=?2 AND region=?3))
        ORDER BY request_id LIMIT 4097
    "#;
    let rows = tx
        .prepare(sql)?
        .query_map(
            params![volume.bucket_id, volume.bucket_name, volume.region],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, bool>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                    r.get::<_, Option<String>>(7)?,
                    r.get::<_, Option<u64>>(8)?,
                    r.get::<_, Option<String>>(9)?,
                    r.get::<_, Option<String>>(10)?,
                ))
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ensure!(
        rows.len() <= MAX_ORIGINALS,
        "GPU retained collector originals exceed the bounded owner review; no storage hold was released"
    );
    let mut kept = Vec::new();
    for (
        id,
        project,
        valid,
        saved,
        signature,
        original_request,
        body,
        original_body,
        observed_bytes,
        observed_request,
        artifact,
    ) in rows
    {
        ensure!(
            valid,
            "GPU collector has an unknown original identity; no storage hold was released"
        );
        let request = Uuid::parse_str(&id)?;
        ensure!(
            !request.is_nil() && original_request.as_deref() == Some(id.as_str()),
            "GPU collector original request identity changed"
        );
        resource_id(&project)?;
        let saved: GpuResultVolume = serde_json::from_str(
            saved
                .as_deref()
                .context("GPU original collector policy is missing or exceeds its bound")?,
        )?;
        let signature: GpuResultVolume = serde_json::from_str(
            signature
                .as_deref()
                .context("GPU immutable collector policy is missing or exceeds its bound")?,
        )?;
        saved.validate()?;
        signature.validate()?;
        ensure!(
            saved == signature,
            "GPU collector policy differs from its original signature"
        );
        let body: Value = serde_json::from_str(
            body.as_deref()
                .context("GPU original collector mount exceeds its bound")?,
        )?;
        let original_body: Value = serde_json::from_str(
            original_body
                .as_deref()
                .context("GPU immutable collector mount exceeds its bound")?,
        )?;
        ensure!(
            body == original_body
                && body["project"] == project
                && body["name"] == format!("ion-gpu-{request}")
                && body["request"] == id
                && body["volumes"] == json!([saved.mount()])
                && body["args"]
                    == "--request-file /ion/job-request.json --result-volume /ion/results",
            "GPU collector mount or original owner identity changed"
        );
        if let Some(bytes) = observed_bytes {
            ensure!(
                (1..=RECEIPT_BYTES).contains(&bytes)
                    && observed_request.as_deref() == Some(id.as_str())
                    && artifact
                        .as_deref()
                        .and_then(|a| Uuid::parse_str(a).ok())
                        .is_some_and(|a| !a.is_nil()),
                "GPU retained collector receipt identity or byte allowance changed"
            );
        } else {
            ensure!(
                observed_request.is_none() && artifact.is_none(),
                "GPU collector receipt provenance is incomplete"
            );
        }
        if saved.bucket_id == volume.bucket_id {
            ensure!(
                saved.bucket_name == volume.bucket_name && saved.region == volume.region,
                "GPU retained bucket identity/region changed"
            );
            kept.push(Original {
                request,
                volume: saved,
                observed_bytes,
            });
        } else {
            ensure!(
                saved.bucket_name != volume.bucket_name || saved.region != volume.region,
                "GPU retained bucket alias changed its original resource identity"
            );
        }
    }
    Ok(kept)
}
fn scope_usage(
    prefix: String,
    policy: Option<GpuCollectorRetentionPolicy>,
    rows: &[Original],
) -> Result<GpuCollectorScopeUsage> {
    let mut usage = GpuCollectorScopeUsage {
        prefix,
        reserved_bytes: 0,
        reserved_receipts: 0,
        observed_bytes: 0,
        observed_receipts: 0,
        max_bytes: policy.as_ref().map(|p| p.max_bytes),
        max_receipts: policy.as_ref().map(|p| p.max_receipts),
        remaining_bytes: None,
        remaining_receipts: None,
    };
    for row in rows
        .iter()
        .filter(|r| overlaps(&usage.prefix, &r.volume.prefix))
    {
        usage.reserved_bytes = usage
            .reserved_bytes
            .checked_add(RECEIPT_BYTES)
            .context("GPU collector byte reservation overflow")?;
        usage.reserved_receipts = usage
            .reserved_receipts
            .checked_add(1)
            .context("GPU collector receipt reservation overflow")?;
        if let Some(bytes) = row.observed_bytes {
            usage.observed_bytes = usage
                .observed_bytes
                .checked_add(bytes)
                .context("GPU collector observed byte overflow")?;
            usage.observed_receipts += 1;
        }
    }
    usage.remaining_bytes = usage
        .max_bytes
        .map(|cap| cap.saturating_sub(usage.reserved_bytes));
    usage.remaining_receipts = usage
        .max_receipts
        .map(|cap| cap.saturating_sub(usage.reserved_receipts));
    Ok(usage)
}
fn snapshot(volume: &GpuResultVolume, rows: &[Original]) -> Result<GpuCollectorUsage> {
    let mut policies = BTreeMap::new();
    policies.insert(volume.prefix.clone(), volume.retention.clone());
    for row in rows {
        if overlaps(&volume.prefix, &row.volume.prefix)
            && let Some(approved) = &row.volume.retention
        {
            let current = policies
                .entry(row.volume.prefix.clone())
                .or_insert_with(|| Some(approved.clone()));
            if let Some(current) = current {
                current.max_bytes = current.max_bytes.min(approved.max_bytes);
                current.max_receipts = current.max_receipts.min(approved.max_receipts);
            } else {
                *current = Some(approved.clone());
            }
        }
    }
    let mut scopes = Vec::new();
    let mut admissible_receipts = None;
    for (prefix, policy) in policies {
        let scope = scope_usage(prefix, policy, rows)?;
        if let (Some(bytes), Some(count)) = (scope.remaining_bytes, scope.remaining_receipts) {
            let remaining = count.min(u32::try_from(bytes / RECEIPT_BYTES)?);
            admissible_receipts = Some(
                admissible_receipts.map_or(remaining, |previous: u32| previous.min(remaining)),
            );
        }
        scopes.push(scope);
    }
    admissible_receipts =
        admissible_receipts.map(|count| count.min((MAX_ORIGINALS - rows.len()) as u32));
    let current = scopes
        .iter()
        .find(|s| s.prefix == volume.prefix)
        .context("GPU collector scope missing")?;
    Ok(GpuCollectorUsage {
        version: 1,
        bucket_id: volume.bucket_id.clone(),
        bucket_name: volume.bucket_name.clone(),
        region: volume.region.clone(),
        prefix: volume.prefix.clone(),
        mode: GpuCollectorRetentionMode::KeepUntilExplicitlyCleaned,
        reserved_bytes: current.reserved_bytes,
        reserved_receipts: current.reserved_receipts,
        observed_bytes: current.observed_bytes,
        observed_receipts: current.observed_receipts,
        historical_unbounded_receipts: u32::try_from(
            rows.iter()
                .filter(|r| {
                    overlaps(&volume.prefix, &r.volume.prefix) && r.volume.retention.is_none()
                })
                .count(),
        )?,
        bucket_reserved_bytes: RECEIPT_BYTES
            .checked_mul(rows.len() as u64)
            .context("GPU collector byte reservation overflow")?,
        bucket_reserved_receipts: u32::try_from(rows.len())?,
        admissible_receipts,
        scopes,
    })
}
pub(super) fn usage(tx: &Transaction<'_>, volume: &GpuResultVolume) -> Result<GpuCollectorUsage> {
    snapshot(volume, &originals(tx, volume)?)
}
/// Admission does not mutate an existing request. Original receipt accounting is
/// part of the launch transaction; cleanup/cost state is intentionally irrelevant.
pub(super) fn reserve(tx: &Transaction<'_>, volume: &GpuResultVolume, request: Uuid) -> Result<()> {
    volume.validate()?;
    volume.require_retention()?;
    ensure!(!request.is_nil(), "GPU collector request identity missing");
    let rows = originals(tx, volume)?;
    ensure!(
        rows.len() < MAX_ORIGINALS,
        "GPU retained bucket collection bound reached; no storage hold was released"
    );
    ensure!(
        !rows.iter().any(|r| r.request == request),
        "GPU collector request already has an original hold; validate its exact saved replay"
    );
    let usage = snapshot(volume, &rows)?;
    ensure!(
        usage
            .admissible_receipts
            .is_some_and(|available| available > 0),
        "GPU retained collector ceiling reached; explicit reviewed cleanup has not released any hold"
    );
    Ok(())
}

#[cfg(test)]
#[path = "collector_retention/tests.rs"]
mod tests;
