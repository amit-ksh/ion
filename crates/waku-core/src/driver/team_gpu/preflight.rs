//! Fresh, fixed-origin provider reads for the exact approved GPU resource shape.
//! Availability is advisory. This receipt neither locks a price nor proves egress denial.
use super::*;
use crate::teams::gpu_jobs::{GpuQuotaKind, GpuResourcePolicy};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, time::Instant};
#[derive(Clone, Serialize)]
pub(crate) struct GpuPreflight {
    version: u16,
    request_id: uuid::Uuid,
    body_sha256: String,
    checked_at: u64,
    policy: GpuResourcePolicy,
    reads: Vec<ReadWitness>,
    quotas: Vec<QuotaWitness>,
    capacity: CapacityWitness,
}
#[derive(Clone, Serialize)]
struct ReadWitness {
    path: String,
    query: Vec<(String, String)>,
    sha256: String,
}
#[derive(Clone, Serialize)]
struct QuotaWitness {
    id: String,
    remaining: u64,
}
#[derive(Clone, Serialize)]
struct CapacityWitness {
    id: String,
    effective_at: u64,
    available: u64,
    limit: u64,
}
fn hash(value: &Value) -> Result<String> {
    Ok(Sha256::digest(serde_json::to_vec(value)?)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
fn number(value: &Value) -> Result<u64> {
    // Protobuf emits uint64 as decimal strings; fixtures/numeric JSON must also be integral.
    let n = if let Some(s) = value.as_str() {
        ensure!(
            !s.is_empty() && s.len() <= 20 && s.bytes().all(|b| b.is_ascii_digit()),
            "GPU quantity is not an unsigned integer"
        );
        s.parse().context("GPU quantity overflow")?
    } else {
        value
            .as_u64()
            .context("GPU quantity is missing or not an unsigned integer")?
    };
    Ok(n)
}
impl GpuPreflight {
    pub(crate) fn validate(
        &self,
        body: &Value,
        request: &JobRequest,
        policy: &GpuResourcePolicy,
        now: u64,
    ) -> Result<()> {
        ensure!(
            self.version == 1
                && self.request_id == request.request_id
                && body["spec"]["injectedFiles"]
                    == json!([{"containerPath":"/ion/job-request.json","content":STANDARD.encode(serde_json::to_vec(request)?)}])
                && self.policy == *policy
                && self.body_sha256 == hash(body)?
                && body["metadata"]["labels"]["ion-request"] == self.request_id.to_string()
                && self.checked_at <= now
                && now - self.checked_at <= 30
                && self.capacity.effective_at <= now.saturating_add(5)
                && now.saturating_sub(self.capacity.effective_at) <= 120
                && self.quotas.len() == policy.quotas.len(),
            "GPU preflight expired or differs from the original approved launch"
        );
        policy.matches_body(body)
    }
    pub(crate) fn quota_remaining(&self, index: usize) -> Result<u64> {
        self.quotas
            .get(index)
            .map(|q| q.remaining)
            .context("GPU preflight quota witness missing")
    }
    pub(crate) fn capacity_remaining(&self) -> u64 {
        self.capacity.available.min(self.capacity.limit)
    }
    pub(crate) fn persisted(&self) -> Result<String> {
        let body = serde_json::to_string(self)?;
        ensure!(
            body.len() <= 32 * 1024,
            "GPU preflight witness exceeds its bound"
        );
        Ok(body)
    }
}
impl Client {
    fn preflight_read(
        &self,
        path: &str,
        query: &[(&str, &str)],
        reads: &mut Vec<ReadWitness>,
    ) -> Result<Value> {
        let value = Self::response(
            self.request(reqwest::Method::GET, path)
                .query(query)
                .send()?,
        )?;
        reads.push(ReadWitness {
            path: path.into(),
            query: query
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
            sha256: hash(&value)?,
        });
        Ok(value)
    }
    pub(super) fn verify_preflight(
        &self,
        launch: &Launch,
        policy: &GpuBudgetPolicy,
    ) -> Result<GpuPreflight> {
        let approved = policy
            .resources
            .as_ref()
            .context("Approve an exact GPU project/network/resource policy before a new launch")?;
        approved.require_child_network()?;
        let body = launch.body()?;
        approved.matches_body(&body)?;
        ensure!(
            approved.project == self.project,
            "GPU preflight owner project changed"
        );
        let began = Instant::now();
        let mut reads = Vec::new();
        let project = self.preflight_read(
            &format!("/iam/v2/projects/{}", approved.project),
            &[],
            &mut reads,
        )?;
        ensure!(
            project["metadata"]["id"] == approved.project
                && project["metadata"]["parentId"] == approved.tenant
                && project["spec"]["region"] == approved.region
                && project["status"]["projectState"] == "ACTIVE",
            "GPU project identity, tenant, region or active state changed"
        );
        let platform = self.preflight_read(
            "/compute/v1/platforms/get-by-name",
            &[
                ("parentId", &approved.project),
                ("name", &approved.platform),
            ],
            &mut reads,
        )?;
        ensure!(
            platform["metadata"]["parentId"] == approved.project
                && platform["metadata"]["name"] == approved.platform,
            "GPU platform identity/project changed"
        );
        identifier(
            platform["metadata"]["id"]
                .as_str()
                .context("GPU platform identity missing")?,
        )?;
        ensure!(
            number(&platform["spec"]["gpuMemoryGigabytes"])? == approved.gpu_memory_gb,
            "GPU memory GB differs from its approval"
        );
        let presets = platform["spec"]["presets"]
            .as_array()
            .context("GPU platform presets missing")?;
        ensure!(presets.len() <= 128, "GPU preset catalog exceeds its bound");
        let matching = presets
            .iter()
            .filter(|p| p["name"] == approved.preset)
            .collect::<Vec<_>>();
        ensure!(matching.len() == 1, "GPU preset is missing or ambiguous");
        check_shape(&matching[0]["resources"], approved)?;
        let subnet = self.preflight_read(
            &format!("/vpc/v1/subnets/{}", approved.subnet),
            &[],
            &mut reads,
        )?;
        ensure!(
            subnet["metadata"]["id"] == approved.subnet
                && subnet["metadata"]["parentId"] == approved.project
                && subnet["spec"]["networkId"] == approved.network
                && subnet["status"]["state"] == "READY",
            "GPU subnet identity/project/network/readiness changed"
        );
        let network = self.preflight_read(
            &format!("/vpc/v1/networks/{}", approved.network),
            &[],
            &mut reads,
        )?;
        ensure!(
            network["metadata"]["id"] == approved.network
                && network["metadata"]["parentId"] == approved.project
                && network["status"]["state"] == "READY",
            "GPU network identity/project/readiness changed"
        );
        let mut quotas = Vec::new();
        let mut quota_ids = HashSet::new();
        for rule in &approved.quotas {
            if rule.kind == GpuQuotaKind::Gpu {
                ensure!(
                    platform["spec"]["gpuCountQuotaType"] == rule.name,
                    "GPU quota differs from the native platform quota type"
                );
            }
            let parent = approved.parent(rule.scope);
            let quota = self.preflight_read(
                "/quotas/v1/quota-allowances/get-by-name",
                &[
                    ("parentId", parent),
                    ("name", &rule.name),
                    ("region", &approved.region),
                ],
                &mut reads,
            )?;
            ensure!(
                quota["metadata"]["parentId"] == parent
                    && quota["metadata"]["name"] == rule.name
                    && quota["spec"]["region"] == approved.region
                    && quota["status"]["state"] == "STATE_ACTIVE"
                    && quota["status"]["unit"] == rule.unit
                    && matches!(
                        quota["status"]["usageState"].as_str(),
                        Some("USAGE_STATE_USED" | "USAGE_STATE_NOT_USED")
                    ),
                "GPU quota identity/scope/region/unit/state is unknown or changed"
            );
            let id = quota["metadata"]["id"]
                .as_str()
                .context("GPU quota identity missing")?;
            identifier(id)?;
            ensure!(
                quota_ids.insert(id.to_owned()),
                "GPU quota identity is duplicated"
            );
            let used = number(&quota["status"]["usage"])?;
            if quota["status"]["usageState"] == "USAGE_STATE_NOT_USED" {
                ensure!(used == 0, "GPU unused quota has nonzero usage");
            }
            let remaining = number(&quota["spec"]["limit"])?
                .checked_sub(used)
                .context("GPU quota is already exceeded")?;
            ensure!(
                remaining >= approved.quantity(rule.kind),
                "GPU quota lacks approved resource headroom"
            );
            quotas.push(QuotaWitness {
                id: id.into(),
                remaining,
            });
        }
        let now = crate::model::unix_time();
        let capacity = self.capacity_preflight(approved, now, &mut reads)?;
        ensure!(
            began.elapsed() <= Duration::from_secs(120),
            "GPU preflight observations took too long; refresh before launch"
        );
        let checked_at = crate::model::unix_time();
        ensure!(
            capacity.effective_at <= checked_at + 5
                && checked_at.saturating_sub(capacity.effective_at) <= 120,
            "GPU capacity measurement expired during preflight"
        );
        Ok(GpuPreflight {
            version: 1,
            request_id: launch.request.request_id,
            body_sha256: hash(&body)?,
            checked_at,
            policy: approved.clone(),
            reads,
            quotas,
            capacity,
        })
    }
    fn capacity_preflight(
        &self,
        policy: &GpuResourcePolicy,
        now: u64,
        reads: &mut Vec<ReadWitness>,
    ) -> Result<CapacityWitness> {
        let mut token = String::new();
        let mut seen = HashSet::new();
        let mut matched = None;
        for _ in 0..4 {
            let mut query = vec![("parentId", policy.tenant.as_str()), ("pageSize", "64")];
            if !token.is_empty() {
                query.push(("pageToken", &token));
            }
            let page = self.preflight_read("/capacity/v1/resource-advice", &query, reads)?;
            let items = page["items"]
                .as_array()
                .context("GPU capacity items missing")?;
            ensure!(items.len() <= 64, "GPU capacity page exceeds its bound");
            for item in items {
                if item["spec"]["region"] != policy.region
                    || item["spec"]["computeInstance"]["platform"] != policy.platform
                    || item["spec"]["computeInstance"]["preset"]["name"] != policy.preset
                {
                    continue;
                }
                // JobSpec cannot pin a fabric; fabric-specific advice is not generic readiness.
                ensure!(
                    item["spec"]
                        .get("fabric")
                        .is_none_or(|f| f.is_null() || f == ""),
                    "GPU capacity requires an unsupported pinned fabric"
                );
                ensure!(matched.is_none(), "GPU capacity shape is ambiguous");
                ensure!(
                    item["metadata"]["parentId"] == policy.tenant,
                    "GPU capacity tenant changed"
                );
                let id = item["metadata"]["id"]
                    .as_str()
                    .context("GPU capacity identity missing")?;
                identifier(id)?;
                let compute = &item["spec"]["computeInstance"];
                check_shape(&compute["preset"]["resources"], policy)?;
                ensure!(
                    number(&compute["gpuMemoryGigabytes"])? == policy.gpu_memory_gb,
                    "GPU capacity memory shape changed"
                );
                let state = &item["status"]["onDemand"];
                ensure!(
                    state["dataState"] == "DATA_STATE_FRESH"
                        && matches!(
                            state["availabilityLevel"].as_str(),
                            Some("AVAILABILITY_LEVEL_HIGH" | "AVAILABILITY_LEVEL_MEDIUM")
                        ),
                    "GPU capacity is stale, unknown or unavailable"
                );
                let effective = chrono::DateTime::parse_from_rfc3339(
                    state["effectiveAt"]
                        .as_str()
                        .context("GPU capacity measurement time missing")?,
                )?
                .timestamp();
                let effective =
                    u64::try_from(effective).context("GPU capacity measurement time invalid")?;
                ensure!(
                    effective <= now + 5 && now.saturating_sub(effective) <= 120,
                    "GPU capacity measurement is stale or in the future"
                );
                let available = number(&state["available"])?;
                let limit = number(&state["limit"])?;
                ensure!(
                    available >= policy.shape.gpus && limit >= policy.shape.gpus,
                    "GPU on-demand capacity is insufficient"
                );
                matched = Some(CapacityWitness {
                    id: id.into(),
                    effective_at: effective,
                    available,
                    limit,
                });
            }
            token = page
                .get("nextPageToken")
                .map(|v| v.as_str().context("GPU capacity page token invalid"))
                .transpose()?
                .unwrap_or("")
                .to_owned();
            if token.is_empty() {
                return matched.context("GPU approved capacity shape not found");
            }
            ensure!(
                token.len() <= 1024
                    && !token.chars().any(char::is_control)
                    && seen.insert(token.clone()),
                "GPU capacity page token invalid or repeated"
            );
        }
        anyhow::bail!("GPU capacity pagination exceeds its bound")
    }
}
fn check_shape(value: &Value, policy: &GpuResourcePolicy) -> Result<()> {
    ensure!(
        number(&value["gpuCount"])? == policy.shape.gpus
            && number(&value["vcpuCount"])? == policy.shape.vcpus
            && number(&value["memoryGibibytes"])? == policy.shape.memory_gib,
        "GPU preset quantities differ from their owner approval"
    );
    Ok(())
}
#[cfg(test)]
pub(super) mod tests;
