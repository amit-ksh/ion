//! Exact owner resource approval and aggregate capacity reservations.
use super::*;
const GIB: u64 = 1 << 30;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GpuResourcePolicy {
    pub version: u16,
    pub project: String,
    pub tenant: String,
    pub region: String,
    pub network: String,
    pub subnet: String,
    pub image: String,
    pub platform: String,
    pub preset: String,
    pub gpu_memory_gb: u64,
    pub shape: GpuResourceVector,
    pub shm_bytes: u64,
    pub project_limit: GpuResourceVector,
    pub tenant_limit: GpuResourceVector,
    pub quotas: Vec<GpuQuotaPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_network: Option<GpuChildNetworkPolicy>,
}
/// Image approval names the tested mechanism; it is not a provider egress attestation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum GpuChildNetworkPolicy {
    SeccompSocketDenyV1,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GpuResourceVector {
    pub jobs: u64,
    pub gpus: u64,
    pub vcpus: u64,
    pub memory_gib: u64,
    pub disk_bytes: u64,
}
impl GpuResourceVector {
    fn bounded(&self) -> bool {
        (1..=128).contains(&self.jobs)
            && (1..=1024).contains(&self.gpus)
            && (1..=65536).contains(&self.vcpus)
            && (1..=262144).contains(&self.memory_gib)
            && (GIB..=128 * 1024 * GIB).contains(&self.disk_bytes)
    }
    fn add(&mut self, other: &Self) -> Result<()> {
        for (a, b) in [
            (&mut self.jobs, other.jobs),
            (&mut self.gpus, other.gpus),
            (&mut self.vcpus, other.vcpus),
            (&mut self.memory_gib, other.memory_gib),
            (&mut self.disk_bytes, other.disk_bytes),
        ] {
            *a = a
                .checked_add(b)
                .context("GPU resource reservation overflow")?;
        }
        Ok(())
    }
    fn within(&self, cap: &Self) -> bool {
        self.jobs <= cap.jobs
            && self.gpus <= cap.gpus
            && self.vcpus <= cap.vcpus
            && self.memory_gib <= cap.memory_gib
            && self.disk_bytes <= cap.disk_bytes
    }
    fn restrict(&mut self, cap: &Self) {
        self.jobs = self.jobs.min(cap.jobs);
        self.gpus = self.gpus.min(cap.gpus);
        self.vcpus = self.vcpus.min(cap.vcpus);
        self.memory_gib = self.memory_gib.min(cap.memory_gib);
        self.disk_bytes = self.disk_bytes.min(cap.disk_bytes);
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum GpuQuotaScope {
    Project,
    Tenant,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum GpuQuotaKind {
    Gpu,
    Vm,
    Disk,
    DiskBytes,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GpuQuotaPolicy {
    pub scope: GpuQuotaScope,
    pub kind: GpuQuotaKind,
    pub name: String,
    pub unit: String,
}
impl GpuResourcePolicy {
    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(self.version == 1, "Unsupported GPU resource policy");
        for id in [
            &self.project,
            &self.tenant,
            &self.region,
            &self.network,
            &self.subnet,
            &self.platform,
            &self.preset,
        ] {
            resource_id(id)?;
        }
        ensure!(
            self.project != self.tenant
                && self.shape.bounded()
                && self.shape.jobs == 1
                && self.shape.gpus <= 8
                && self.shape.disk_bytes <= 1024 * GIB
                && (1..=1024).contains(&self.gpu_memory_gb)
                && self.shm_bytes
                    <= self
                        .shape
                        .memory_gib
                        .checked_mul(GIB)
                        .context("GPU memory overflow")?
                && self.project_limit.bounded()
                && self.tenant_limit.bounded()
                && self.shape.within(&self.project_limit)
                && self.project_limit.within(&self.tenant_limit),
            "GPU approved quantities or aggregate limits are invalid"
        );
        let (repository, digest) = self
            .image
            .rsplit_once("@sha256:")
            .context("GPU policy requires an immutable image digest")?;
        ensure!(
            !repository.is_empty()
                && repository.len() <= 512
                && repository
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"/._-:".contains(&b))
                && digest.len() == 64
                && digest.bytes().all(|b| b.is_ascii_hexdigit()),
            "GPU policy image invalid"
        );
        ensure!(
            self.quotas.len() == 8,
            "Approve GPU/VM/disk-count/disk-byte quotas for both project and tenant"
        );
        let mut kinds = std::collections::HashSet::new();
        let mut names = std::collections::HashSet::new();
        for quota in &self.quotas {
            ensure!(
                !quota.name.is_empty()
                    && quota.name.len() <= 160
                    && quota
                        .name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                    && !quota.unit.is_empty()
                    && quota.unit.len() <= 32
                    && quota
                        .unit
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
                    && kinds.insert((quota.scope, quota.kind))
                    && names.insert((quota.scope, &quota.name))
                    && (quota.kind != GpuQuotaKind::DiskBytes || quota.unit == "byte"),
                "GPU quota identity/unit approval invalid"
            );
        }
        Ok(())
    }
    pub(crate) fn require_child_network(&self) -> Result<()> {
        ensure!(
            self.child_network == Some(GpuChildNetworkPolicy::SeccompSocketDenyV1),
            "Approve the pinned image's tested Linux child socket boundary before a new GPU launch"
        );
        Ok(())
    }
    pub(crate) fn matches_body(&self, body: &Value) -> Result<()> {
        self.validate()?;
        ensure!(
            body["metadata"]["parentId"] == self.project
                && body["spec"]["image"] == self.image
                && body["spec"]["platform"] == self.platform
                && body["spec"]["preset"] == self.preset
                && body["spec"]["subnetId"] == self.subnet
                && body["spec"]["disk"]["type"] == "NETWORK_SSD"
                && body["spec"]["disk"]["sizeBytes"] == self.shape.disk_bytes.to_string()
                && body["spec"]["shmSizeBytes"] == self.shm_bytes.to_string()
                && body["spec"]["publicIp"] == false
                && body["spec"]["preemptible"] == false
                && body["spec"]["restartAttempts"] == "0"
                && body["spec"]["pricingModel"] == json!({"onDemand":{}}),
            "GPU launch differs from the exact owner-approved resource policy"
        );
        Ok(())
    }
    pub(crate) fn parent(&self, scope: GpuQuotaScope) -> &str {
        match scope {
            GpuQuotaScope::Project => &self.project,
            GpuQuotaScope::Tenant => &self.tenant,
        }
    }
    pub(crate) fn quantity(&self, kind: GpuQuotaKind) -> u64 {
        match kind {
            GpuQuotaKind::Gpu => self.shape.gpus,
            GpuQuotaKind::Vm | GpuQuotaKind::Disk => 1,
            GpuQuotaKind::DiskBytes => self.shape.disk_bytes,
        }
    }
}
/// Called inside the one launch reservation transaction. Unknown legacy jobs block
/// new checked capacity; terminal or cancel acknowledgements are never release proof.
pub(super) fn reserve_resources(
    tx: &Transaction<'_>,
    policy: &GpuResourcePolicy,
    receipt: &crate::driver::team_gpu::GpuPreflight,
) -> Result<()> {
    let rows:Vec<(String,String)>=tx.prepare("SELECT j.project,j.policy FROM ion_team_gpu_jobs j WHERE j.state!='cleaned' AND NOT EXISTS(SELECT 1 FROM ion_team_gpu_cleanup c WHERE c.request_id=j.request_id AND c.state='verified') LIMIT 257")?
        .query_map([],|r|Ok((r.get(0)?,r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    ensure!(
        rows.len() <= 256,
        "GPU active resource collection exceeds its bound"
    );
    let mut capacity_usage = policy.shape.gpus;
    let mut project = policy.shape.clone();
    let mut tenant = policy.shape.clone();
    let mut project_cap = policy.project_limit.clone();
    let mut tenant_cap = policy.tenant_limit.clone();
    let mut quota_usage = policy
        .quotas
        .iter()
        .map(|q| policy.quantity(q.kind))
        .collect::<Vec<_>>();
    for (saved_project, body) in rows {
        let old: GpuBudgetPolicy = serde_json::from_str(&body)?;
        let approved = old.resources.context(
            "GPU job has unknown historical quantities; verify its cleanup before new allocation",
        )?;
        approved.validate()?;
        ensure!(
            saved_project == approved.project,
            "GPU saved project/resource policy changed"
        );
        if policy.project == approved.project {
            ensure!(
                policy.tenant == approved.tenant && policy.region == approved.region,
                "GPU project tenant/region changed"
            );
        }
        if policy.region != approved.region {
            continue;
        }
        if policy.project == approved.project {
            project.add(&approved.shape)?;
            project_cap.restrict(&approved.project_limit);
        }
        if policy.tenant == approved.tenant {
            tenant.add(&approved.shape)?;
            tenant_cap.restrict(&approved.tenant_limit);
        }
        if policy.tenant == approved.tenant
            && policy.platform == approved.platform
            && policy.preset == approved.preset
        {
            capacity_usage = capacity_usage
                .checked_add(approved.shape.gpus)
                .context("GPU capacity reservation overflow")?;
        }
        for (index, quota) in policy.quotas.iter().enumerate() {
            if policy.parent(quota.scope) == approved.parent(quota.scope) {
                for previous in &approved.quotas {
                    if previous.scope == quota.scope && previous.name == quota.name {
                        ensure!(
                            previous.kind == quota.kind && previous.unit == quota.unit,
                            "GPU saved quota quantity/unit changed"
                        );
                        quota_usage[index] = quota_usage[index]
                            .checked_add(approved.quantity(quota.kind))
                            .context("GPU quota reservation overflow")?;
                    }
                }
            }
        }
    }
    ensure!(
        project.within(&project_cap),
        "GPU project resource reservation limit reached"
    );
    ensure!(
        tenant.within(&tenant_cap),
        "GPU tenant resource reservation limit reached"
    );
    ensure!(
        capacity_usage <= receipt.capacity_remaining(),
        "GPU capacity advice headroom is held by outstanding owner reservations"
    );
    for (index, quantity) in quota_usage.into_iter().enumerate() {
        ensure!(
            quantity <= receipt.quota_remaining(index)?,
            "GPU quota headroom is held by outstanding owner reservations"
        );
    }
    Ok(())
}
