//! Source-task team tools shared by native harness transports. Project identity,
//! grants, queue replay and document scope are owned here, never model arguments.
pub(crate) mod amp;
pub(crate) mod antigravity;
pub(crate) mod grok;
pub(crate) mod opencode;
pub(crate) mod opencode2;
mod project_bundle;
pub(crate) mod worker;
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};
use uuid::Uuid;
use waku_protocol::{control::*, model::RuntimeMode};
#[derive(Clone)]
pub(crate) struct WorkspaceContext {
    pub task_id: Uuid,
    pub project_id: Uuid,
    pub control_path: PathBuf,
    pub settings_path: PathBuf,
    pub mode: RuntimeMode,
}
pub(crate) fn execute(
    context: &WorkspaceContext,
    request_id: &str,
    name: &str,
    args: &Value,
    cancelled: &AtomicBool,
    authorize: impl FnOnce(
        &AgentProfile,
        &waku_protocol::teams::TeamRunLimits,
    ) -> Result<waku_protocol::teams::AgentAccess>,
) -> Result<String> {
    anyhow::ensure!(!cancelled.load(Ordering::Acquire), "Source task stopped");
    anyhow::ensure!(
        serde_json::to_vec(args)?.len() <= 40 * 1024,
        "Source team arguments exceed their limit"
    );
    let result = (|| -> Result<String> {
        let result = match name {
            "list_teams" => team_catalog(
                &crate::control::ControlStore::new(context.control_path.clone()),
                args,
            )?,
            "invoke_team" => invoke_team_authorized(context, request_id, args, authorize)?,
            "get_team_runs" => {
                anyhow::ensure!(
                    args.as_object().is_some_and(|o| o.is_empty()),
                    "No arguments accepted"
                );
                let runs = crate::teams::TeamCoordinator::new(context.control_path.clone())
                    .runs_for_source(context.task_id)?;
                serde_json::to_string(&runs.iter().map(team_summary).collect::<Vec<_>>())?
            }
            "await_team" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Args {
                    run_id: Uuid,
                    #[serde(default = "team_wait_seconds")]
                    max_wait_seconds: u64,
                }
                let a: Args = serde_json::from_value(args.clone())?;
                anyhow::ensure!(
                    (1..=30).contains(&a.max_wait_seconds),
                    "Wait duration must be 1–30 seconds"
                );
                let coordinator = crate::teams::TeamCoordinator::new(context.control_path.clone());
                let deadline = std::time::Instant::now() + Duration::from_secs(a.max_wait_seconds);
                loop {
                    anyhow::ensure!(
                        !cancelled.load(Ordering::Relaxed),
                        "Source task stopped while waiting for its team"
                    );
                    let run =
                        coordinator.source_run(context.task_id, context.project_id, a.run_id)?;
                    if run.status != waku_protocol::teams::TeamRunStatus::Active
                        || run
                            .work
                            .iter()
                            .filter(|w| {
                                w.status != waku_protocol::teams::AssignmentStatus::Completed
                            })
                            .all(|w| {
                                matches!(
                                    w.status,
                                    waku_protocol::teams::AssignmentStatus::Blocked
                                        | waku_protocol::teams::AssignmentStatus::Failed
                                        | waku_protocol::teams::AssignmentStatus::Uncertain
                                ) || w.status == waku_protocol::teams::AssignmentStatus::Queued
                                    && w.reason.is_some()
                            })
                        || std::time::Instant::now() >= deadline
                    {
                        break team_summary(&run).to_string();
                    }
                    // One collection poll per second on the provider worker, with prompt cancellation.
                    for _ in 0..5 {
                        anyhow::ensure!(!cancelled.load(Ordering::Relaxed), "Source task stopped");
                        thread::sleep(Duration::from_millis(200));
                    }
                }
            }
            "read_team_document" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Args {
                    run_id: Uuid,
                    artifact_id: Uuid,
                }
                let a: Args = serde_json::from_value(args.clone())?;
                let coordinator = crate::teams::TeamCoordinator::new(context.control_path.clone());
                let bytes = coordinator.read_source_document(
                    context.task_id,
                    context.project_id,
                    a.run_id,
                    a.artifact_id,
                )?;
                let mut reply = coordinator.document_model_reply(a.artifact_id, &bytes)?;
                reply["run_id"] = json!(a.run_id);
                reply.to_string()
            }
            _ => anyhow::bail!("Unknown source team tool"),
        };
        Ok(result)
    })();
    if cancelled.load(Ordering::Acquire) {
        // Close cancellation racing with the durable queue transaction. Never
        // leave a newly committed invocation working after its source stopped.
        crate::teams::TeamCoordinator::new(context.control_path.clone())
            .stop_source_runs(context.task_id, crate::model::unix_time())?;
        anyhow::bail!("Source task stopped; its team invocations were canceled");
    }
    result
}
pub(crate) fn accepts(name: &str) -> bool {
    matches!(
        name,
        "list_teams" | "invoke_team" | "get_team_runs" | "await_team" | "read_team_document"
    )
}
pub(crate) fn tools() -> Value {
    json!([
        {"type":"function","function":{"name":"list_teams","description":"Find saved agent teams and the definition revision. Ready saved harnesses execute through the daemon; unavailable setup appears in run status.","parameters":{"type":"object","properties":{"query":{"type":"string"}},"additionalProperties":false}}},
        {"type":"function","function":{"name":"invoke_team","description":"Invoke a saved team in this task’s project using its lead’s configured access and pinned limits. Supervised tasks ask before granting privileged team access. Maximum four invocations per source task. Inspect get_team_runs; a queue acknowledgement is not completed work.","parameters":{"type":"object","properties":{"team_id":{"type":"string"},"definition_revision":{"type":"integer"},"objective":{"type":"string"}},"required":["team_id","definition_revision","objective"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"get_team_runs","description":"Inspect the durable team runs invoked from this task. Inspect assignment state, setup blockers and completed document evidence; queued work is not completed.","parameters":{"type":"object","properties":{},"additionalProperties":false}}},
        {"type":"function","function":{"name":"await_team","description":"Wait up to 30 seconds for a team invoked by this task, without another model request. Returns real status, blockers and final document IDs; not a completion assertion.","parameters":{"type":"object","properties":{"run_id":{"type":"string"},"max_wait_seconds":{"type":"integer","minimum":1,"maximum":30}},"required":["run_id"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"read_team_document","description":"Read a bounded document from a completed team invoked by this source task/project. Inspect the actual evidence before using its result; superseded and unrelated documents are inaccessible.","parameters":{"type":"object","properties":{"run_id":{"type":"string"},"artifact_id":{"type":"string"}},"required":["run_id","artifact_id"],"additionalProperties":false}}}

    ])
}
fn team_wait_seconds() -> u64 {
    30
}
fn team_catalog(store: &crate::control::ControlStore, args: &Value) -> Result<String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Search {
        #[serde(default)]
        query: String,
    }
    let search: Search = serde_json::from_value(args.clone())?;
    anyhow::ensure!(search.query.len() <= 256, "Team query is too long");
    let snapshot = store.load()?;
    let query = search.query.to_lowercase();
    let teams: Vec<_> = snapshot
        .records
        .iter()
        .filter_map(|r| {
            if let ControlRecord::Team(t) = r {
                Some(t)
            } else {
                None
            }
        })
        .filter(|t| t.name.to_lowercase().contains(&query))
        .take(64)
        .map(|t| json!({"id":t.id,"name":t.name,"agents":t.members.len()}))
        .collect();
    Ok(
        json!({"definition_revision":snapshot.revision,"teams":teams,"dispatch_available":true})
            .to_string(),
    )
}
fn invoke_team_authorized(
    context: &WorkspaceContext,
    request_id: &str,
    args: &Value,
    authorize: impl FnOnce(
        &AgentProfile,
        &waku_protocol::teams::TeamRunLimits,
    ) -> Result<waku_protocol::teams::AgentAccess>,
) -> Result<String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Invocation {
        team_id: Uuid,
        definition_revision: u64,
        objective: String,
    }
    let request: Invocation = serde_json::from_value(args.clone())?;
    anyhow::ensure!(
        !request_id.is_empty() && request_id.len() <= 256,
        "Invalid source invocation identity"
    );
    let coordinator = crate::teams::TeamCoordinator::new(context.control_path.clone());
    // Exact already-created invocations keep their original permission receipt.
    if let Some(run) = coordinator
        .runs_for_source(context.task_id)?
        .into_iter()
        .find(|r| {
            r.origin
                .as_ref()
                .is_some_and(|o| o.request_id == request_id)
        })
    {
        anyhow::ensure!(
            run.team.id == request.team_id
                && run.definition_revision == request.definition_revision
                && run.work[0].objective == request.objective.trim()
                && run.project_id == context.project_id,
            "Invocation replay changed its team, revision, project or objective"
        );
        return Ok(team_summary(&run).to_string());
    }
    let snapshot = crate::control::ControlStore::new(context.control_path.clone()).load()?;
    anyhow::ensure!(
        snapshot.revision == request.definition_revision,
        "Team definition changed; list teams again"
    );
    let team = snapshot
        .records
        .iter()
        .find_map(|r| match r {
            ControlRecord::Team(t) if t.id == request.team_id => Some(t),
            _ => None,
        })
        .context("Team is unavailable")?;
    let lead = team
        .members
        .iter()
        .find(|m| m.parent_id.is_none())
        .context("Team lead missing")?;
    let profile = snapshot
        .records
        .iter()
        .find_map(|r| match r {
            ControlRecord::Agent(p) if p.id == lead.agent_id => Some(p),
            _ => None,
        })
        .context("Team lead profile missing")?;
    let access = if profile.access == Default::default()
        || matches!(
            context.mode,
            RuntimeMode::FullAccess | RuntimeMode::AutoAcceptEdits
        ) && !profile.access.shell
            && !profile.access.network
    {
        profile.access.clone()
    } else {
        authorize(profile, &team.limits.clone().unwrap_or_default())?
    };
    anyhow::ensure!(
        crate::teams::intersect_access(&access, &profile.access) == access,
        "Source authorization exceeds the saved team lead access"
    );
    let run = coordinator.queue_from_agent_with_access(
        context.task_id,
        context.project_id,
        request_id.into(),
        request.team_id,
        request.definition_revision,
        request.objective,
        access,
        crate::model::unix_time(),
    )?;
    Ok(team_summary(&run).to_string())
}
fn team_summary(run: &waku_protocol::teams::TeamRunSnapshot) -> Value {
    json!({"run_id":run.id,"task_id":run.root_task_id,"team":run.team.name,"status":run.status,
        "invocation_id":run.origin.as_ref().map(|origin|&origin.request_id),
        "objective":run.work.first().map(|w|w.objective.chars().take(512).collect::<String>()),
        "total_assignments":run.work.len(),"omitted_assignments":run.work.len().saturating_sub(8),
        "assignments":run.work.iter().take(8).map(|w|json!({"id":w.id,"task_id":w.task_id,"status":w.status,
            "reason":w.reason.as_ref().map(|r|r.chars().take(256).collect::<String>()),
            "summary":w.result.as_ref().map(|r|r.summary.chars().take(512).collect::<String>()),
            "artifacts":w.result.as_ref().map(|r|r.artifact_ids.iter().take(8).collect::<Vec<_>>())})).collect::<Vec<_>>(),
        "dispatch_available":true,"message":"Queued for daemon dispatch. Inspect assignment status for progress or setup requirements."})
}
