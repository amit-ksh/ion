//! Owner database / Ion project admission. Logical reservations are not host quotas.
//! Holds are written in the same immediate transaction as the original job/archive.
use super::super::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const MIB: u64 = 1024 * 1024;
const MAX_ROWS: usize = 2048;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CommandResourceVector {
    pub jobs: u64,
    pub cpu_millis: u64,
    pub memory_bytes: u64,
    pub processes: u64,
    pub storage_bytes: u64,
}
impl CommandResourceVector {
    fn bounded(&self) -> bool {
        (1..=128).contains(&self.jobs)
            && (100..=4_096_000).contains(&self.cpu_millis)
            && (64 * MIB..=1024 * 1024 * MIB).contains(&self.memory_bytes)
            && (2..=32768).contains(&self.processes)
            && (1..=1024 * 1024 * MIB).contains(&self.storage_bytes)
    }
    fn add(&mut self, other: &Self) -> Result<()> {
        for (a, b) in [
            (&mut self.jobs, other.jobs),
            (&mut self.cpu_millis, other.cpu_millis),
            (&mut self.memory_bytes, other.memory_bytes),
            (&mut self.processes, other.processes),
            (&mut self.storage_bytes, other.storage_bytes),
        ] {
            *a = a
                .checked_add(b)
                .context("Command resource reservation overflow")?;
        }
        Ok(())
    }
    fn restrict(&mut self, other: &Self) {
        self.jobs = self.jobs.min(other.jobs);
        self.cpu_millis = self.cpu_millis.min(other.cpu_millis);
        self.memory_bytes = self.memory_bytes.min(other.memory_bytes);
        self.processes = self.processes.min(other.processes);
        self.storage_bytes = self.storage_bytes.min(other.storage_bytes);
    }
    fn within(&self, cap: &Self) -> bool {
        self.jobs <= cap.jobs
            && self.cpu_millis <= cap.cpu_millis
            && self.memory_bytes <= cap.memory_bytes
            && self.processes <= cap.processes
            && self.storage_bytes <= cap.storage_bytes
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AdmissionPolicy {
    pub version: u16,
    pub owner_limit: CommandResourceVector,
    pub project_limit: CommandResourceVector,
}
impl AdmissionPolicy {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.version == 1
                && self.owner_limit.bounded()
                && self.project_limit.bounded()
                && self.project_limit.within(&self.owner_limit),
            "Invalid command aggregate admission limits"
        );
        Ok(())
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Allocation {
    group: Option<CommandResourceVector>,
    layer_bytes: Option<u64>,
    archive_bytes: u64,
}
impl Allocation {
    fn from_policy(policy: &Value, archive_bytes: u64) -> Result<Self> {
        ensure!(
            archive_bytes <= super::super::project::MAX_ARCHIVE as u64,
            "Command archive allocation exceeds its bound"
        );
        let group = if policy["resources"]["group"].is_object() {
            let g = &policy["resources"]["group"];
            let group = CommandResourceVector {
                jobs: 1,
                cpu_millis: g["cpu_millis"]
                    .as_u64()
                    .context("Command CPU allocation missing")?,
                memory_bytes: g["memory_bytes"]
                    .as_u64()
                    .context("Command RAM allocation missing")?,
                processes: g["processes"]
                    .as_u64()
                    .context("Command process allocation missing")?,
                storage_bytes: 0,
            };
            ensure!(
                g["version"] == 1
                    && (100..=32000).contains(&group.cpu_millis)
                    && (64 * MIB..=8192 * MIB).contains(&group.memory_bytes)
                    && (2..=256).contains(&group.processes),
                "Stored command group allocation invalid"
            );
            Some(group)
        } else {
            None
        };
        let layer_bytes = policy["max_layer_bytes"]
            .as_u64()
            .filter(|v| (1..=1024 * MIB).contains(v));
        Ok(Self {
            group,
            layer_bytes,
            archive_bytes,
        })
    }
    fn held(
        &self,
        compute_released: bool,
        layer_released: bool,
    ) -> Result<(CommandResourceVector, bool)> {
        let mut used = if compute_released {
            CommandResourceVector::default()
        } else {
            self.group.clone().unwrap_or_else(|| CommandResourceVector {
                jobs: 1,
                ..Default::default()
            })
        };
        let unknown = (!compute_released && self.group.is_none())
            || (!layer_released && self.layer_bytes.is_none());
        used.storage_bytes = self
            .archive_bytes
            .checked_mul(2)
            .context("Command storage allocation overflow")?
            .checked_add(if layer_released {
                0
            } else {
                self.layer_bytes.unwrap_or(0)
            })
            .context("Command storage allocation overflow")?;
        Ok((used, unknown))
    }
}
fn admission(policy: &Value) -> Result<Option<AdmissionPolicy>> {
    if policy.get("admission").is_none_or(Value::is_null) {
        return Ok(None);
    }
    let approved: AdmissionPolicy = serde_json::from_value(policy["admission"].clone())?;
    approved.validate()?;
    Ok(Some(approved))
}
pub(crate) fn validate_policy(policy: &Value) -> Result<()> {
    if let Some(approved) = admission(policy)? {
        let allocation = Allocation::from_policy(policy, 0)?;
        let (used, unknown) = allocation.held(false, false)?;
        ensure!(
            !unknown,
            "Command admission requires approved aggregate cgroup limits and a native layer bound"
        );
        ensure!(
            used.within(&approved.project_limit),
            "Command allocation exceeds its admission limits"
        );
    }
    Ok(())
}
struct Hold {
    project: Uuid,
    allocation: Allocation,
    policy: Option<AdmissionPolicy>,
    compute_released: bool,
    layer_released: bool,
}
fn holds(db: &Connection, excluding: Option<Uuid>) -> Result<Vec<Hold>> {
    // Parse one stored witness at a time. A large historical receipt is not
    // multiplied by the collection size in memory; render never reaches this.
    let mut statement=db.prepare(
        "SELECT r.project_id,CASE WHEN json_valid(j.signature) THEN json_extract(j.signature,'$.policy') ELSE NULL END,
         j.state,a.allocation,a.policy,a.compute_released,a.layer_released,coalesce(length(p.archive),0),
         CASE WHEN a.allocation IS NULL AND j.state='settled' THEN j.request ELSE NULL END,
         CASE WHEN a.allocation IS NULL AND j.state='settled' THEN j.receipt ELSE NULL END,
         j.operation,a.project_id
         FROM ion_team_jobs j JOIN ion_team_runs r ON r.id=j.run_id
         LEFT JOIN ion_team_job_admission a ON a.request_id=j.request_id
         LEFT JOIN ion_team_job_projects p ON p.request_id=j.request_id
         WHERE (?1 IS NULL OR j.request_id!=?1)
           AND (a.request_id IS NULL OR a.compute_released=0 OR a.layer_released=0 OR length(p.archive)>0)
         ORDER BY j.rowid LIMIT 2049")?;
    let rows = statement.query_map([excluding.map(|id| id.to_string())], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, Option<bool>>(5)?,
            r.get::<_, Option<bool>>(6)?,
            r.get::<_, u64>(7)?,
            r.get::<_, Option<String>>(8)?,
            r.get::<_, Option<String>>(9)?,
            r.get::<_, Option<String>>(10)?,
            r.get::<_, Option<String>>(11)?,
        ))
    })?;
    let mut result = Vec::new();
    for row in rows {
        ensure!(
            result.len() < MAX_ROWS,
            "Command resource history exceeds its inspection bound; verified retention cleanup is required"
        );
        let (
            project,
            raw_policy,
            state,
            allocation,
            approved,
            compute,
            layer,
            archive,
            request,
            receipt,
            operation,
            saved_project,
        ) = row?;
        let raw_policy: Value = raw_policy
            .as_deref()
            .and_then(|p| serde_json::from_str(p).ok())
            .unwrap_or(Value::Null);
        let derived_allocation = Allocation::from_policy(&raw_policy, archive)?;
        let derived_policy = admission(&raw_policy)?;
        let (allocation, policy, compute_released, layer_released) =
            if let Some(allocation) = allocation {
                let allocation: Allocation = serde_json::from_str(&allocation)?;
                let policy = approved
                    .map(|p| serde_json::from_str::<AdmissionPolicy>(&p))
                    .transpose()?;
                ensure!(
                    allocation == derived_allocation
                        && policy == derived_policy
                        && saved_project.as_deref() == Some(project.as_str()),
                    "Command allocation differs from its original policy or archive"
                );
                (
                    allocation,
                    policy,
                    compute.context("Command compute hold missing")?,
                    layer.context("Command storage hold missing")?,
                )
            } else {
                let validated_receipt = state == "settled"
                    && operation
                        .as_deref()
                        .is_some_and(|op| Uuid::parse_str(op).is_ok_and(|id| !id.is_nil()))
                    && receipt
                        .as_ref()
                        .zip(request.as_ref())
                        .is_some_and(|(body, request)| {
                            serde_json::from_str::<super::JobResponse>(body)
                                .ok()
                                .zip(serde_json::from_str::<super::JobRequest>(request).ok())
                                .is_some_and(|(receipt, request)| {
                                    receipt.validate_against(&request).is_ok()
                                })
                        });
                (
                    derived_allocation,
                    derived_policy,
                    state == "rejected" || validated_receipt,
                    state == "rejected",
                )
            };
        result.push(Hold {
            project: Uuid::parse_str(&project)?,
            allocation,
            policy,
            compute_released,
            layer_released,
        });
    }
    Ok(result)
}
#[derive(Clone, Debug, Serialize)]
pub(crate) struct CommandResourceUsage {
    pub project_id: Option<Uuid>,
    pub used: CommandResourceVector,
    pub limit: Option<CommandResourceVector>,
    pub unknown_jobs: u32,
    pub retained_archive_bytes: u64,
    pub held_compute_jobs: u32,
}
fn usage(holds: &[Hold], project: Option<Uuid>) -> Result<CommandResourceUsage> {
    let mut result = CommandResourceUsage {
        project_id: project,
        used: Default::default(),
        limit: None,
        unknown_jobs: 0,
        retained_archive_bytes: 0,
        held_compute_jobs: 0,
    };
    for hold in holds
        .iter()
        .filter(|h| project.is_none_or(|p| p == h.project))
    {
        let (used, unknown) = hold
            .allocation
            .held(hold.compute_released, hold.layer_released)?;
        if used == CommandResourceVector::default() && !unknown {
            continue;
        }
        result.used.add(&used)?;
        result.unknown_jobs += u32::from(unknown);
        result.held_compute_jobs += u32::from(!hold.compute_released);
        result.retained_archive_bytes = result
            .retained_archive_bytes
            .checked_add(hold.allocation.archive_bytes)
            .context("Retained command archive overflow")?;
        if let Some(policy) = &hold.policy {
            let cap = if project.is_some() {
                &policy.project_limit
            } else {
                &policy.owner_limit
            };
            if let Some(limit) = &mut result.limit {
                limit.restrict(cap);
            } else {
                result.limit = Some(cap.clone());
            }
        }
    }
    Ok(result)
}
pub(super) fn reserve(
    tx: &Transaction<'_>,
    request: Uuid,
    project: Uuid,
    policy: &Value,
    archive_bytes: u64,
) -> Result<()> {
    validate_policy(policy)?;
    let approved = admission(policy)?;
    let allocation = Allocation::from_policy(policy, archive_bytes)?;
    let retained_cap:bool=tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM ion_team_job_admission a LEFT JOIN ion_team_job_projects p ON p.request_id=a.request_id
         WHERE a.policy IS NOT NULL AND (a.compute_released=0 OR a.layer_released=0 OR length(p.archive)>0))",[],|r|r.get(0))?;
    // Historical omission keeps its original path when no aggregate approval is
    // retained. Do not impose the checked inspection bound on unconfigured runs.
    let previous = if approved.is_some() || retained_cap {
        holds(tx, Some(request))?
    } else {
        Vec::new()
    };
    let owner = usage(&previous, None)?;
    let project_usage = usage(&previous, Some(project))?;
    if approved.is_some() || owner.limit.is_some() {
        let approved = approved
            .as_ref()
            .context("Command aggregate approval cannot be omitted while prior holds remain")?;
        let (new, unknown) = allocation.held(false, false)?;
        ensure!(
            !unknown && owner.unknown_jobs == 0,
            "Unknown historical command allocations must be resolved before checked admission"
        );
        for (mut used, mut limit, prior_limit, scope) in [
            (
                owner.used,
                approved.owner_limit.clone(),
                owner.limit,
                "owner",
            ),
            (
                project_usage.used,
                approved.project_limit.clone(),
                project_usage.limit,
                "project",
            ),
        ] {
            used.add(&new)?;
            if let Some(cap) = prior_limit {
                limit.restrict(&cap);
            }
            ensure!(
                used.within(&limit),
                "Command {scope} aggregate resource reservation limit reached"
            );
        }
    }
    tx.execute("INSERT INTO ion_team_job_admission(request_id,project_id,allocation,policy) VALUES(?,?,?,?)",
        params![request.to_string(),project.to_string(),serde_json::to_string(&allocation)?,approved.map(|p|serde_json::to_string(&p)).transpose()?])?;
    Ok(())
}
pub(super) fn reject(tx: &Transaction<'_>, request: Uuid) -> Result<()> {
    // No instance create happened. A staged upload may have succeeded, so its
    // conservative upload and retained local archive charges survive rejection.
    tx.execute(
        "UPDATE ion_team_job_admission SET compute_released=1,layer_released=1 WHERE request_id=?",
        [request.to_string()],
    )?;
    Ok(())
}
pub(super) fn release_compute(tx: &Transaction<'_>, request: Uuid) -> Result<()> {
    // Only finish_job after validating the original trusted supervisor receipt.
    // FAILED/CANCELLED statuses without that receipt do not enter this path.
    tx.execute(
        "UPDATE ion_team_job_admission SET compute_released=1 WHERE request_id=?",
        [request.to_string()],
    )?;
    Ok(())
}
impl TeamCoordinator {
    pub(crate) fn command_resource_usage(
        &self,
        project: Option<Uuid>,
    ) -> Result<waku_protocol::command_resources::CommandResourceUsage> {
        ensure!(
            project.is_none_or(|p| !p.is_nil()),
            "Command usage project identity is empty"
        );
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let summary = usage(&holds(&tx, None)?, project)?;
        Ok(waku_protocol::command_resources::CommandResourceUsage {
            project_id: summary.project_id,
            used: summary.used.into(),
            limit: summary.limit.map(Into::into),
            unknown_jobs: summary.unknown_jobs,
            retained_archive_bytes: summary.retained_archive_bytes,
            held_compute_jobs: summary.held_compute_jobs,
        })
    }
}

impl From<CommandResourceVector> for waku_protocol::command_resources::CommandResourceVector {
    fn from(value: CommandResourceVector) -> Self {
        Self {
            jobs: value.jobs,
            cpu_millis: value.cpu_millis,
            memory_bytes: value.memory_bytes,
            processes: value.processes,
            storage_bytes: value.storage_bytes,
        }
    }
}

#[cfg(test)]
mod tests;
