//! Private host data is discarded before gateway transfer. Document bytes
//! require exact accepted-result membership and a matching immutable digest.
use super::*;
use crate::{control::*, teams::*};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowHistoryQuery {
    pub project_id: Option<Uuid>,
    pub definition_id: Option<Uuid>,
    pub pending_only: bool,
    pub after: Option<Uuid>,
    pub limit: u16,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowRunQuery {
    pub run_id: Uuid,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowEvidenceQuery {
    pub run_id: Uuid,
    pub after: Option<Uuid>,
    pub limit: u16,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowDocumentQuery {
    pub run_id: Uuid,
    pub artifact_id: Uuid,
    pub sha256: String,
    pub offset: u32,
    pub limit: u16,
}

fn summary(run: &AutomationRun) -> Value {
    json!({"id":run.id,"name":label(&run.name),"projectId":run.project_id,
        "definitionId":run.routine_id.or(run.workflow_id).unwrap_or(run.id),
        "revision":run.revision,"createdAt":run.created_at,"scheduledAt":run.scheduled_at,
        "deadline":run.deadline,"status":run.status,"currentStage":run.current_stage,
        "stageCount":run.stages.len()})
}
pub fn history_result(page: AutomationPage, served_at: u64) -> Result<Value> {
    ensure!(page.runs.len() <= 32, "Too many historical runs");
    for run in &page.runs {
        ensure!(
            !run.stages.is_empty() && run.stages.len() <= 24,
            "Invalid historical stages"
        );
    }
    let rows: Vec<_> = page.runs.iter().map(summary).collect();
    bounded(
        json!({"kind":"workflowHistory","page":{"runs":rows,"nextCursor":page.next_cursor,"pendingCount":page.pending_count},"pageTruncated":false,"servedAt":served_at}),
        "runs",
    )
}
pub fn run_result(run: &AutomationRun, served_at: u64) -> Result<Value> {
    ensure!(
        !run.stages.is_empty() && run.stages.len() <= 24,
        "Invalid historical stages"
    );
    let stages: Vec<_> = run.stages.iter().enumerate().map(|(index, stage)| {
        // Only the current pending gate carries full-review instructions, matching
        // the existing workflow read grant. Partial content is never approval.
        let gate = run.status == AutomationStatus::WaitingDecision && usize::from(run.current_stage) == index;
        json!({"index":index,"title":label(&stage.title),"teamRunId":stage.team_run_id,"taskId":stage.task_id,
            "requiresDecision":stage.requires_decision,"approved":stage.approved,"started":stage.started,
            "completed":stage.completed,"skipped":stage.skipped,
            "workspaceFrom":stage.workspace_from,
            "route":stage.route.as_ref().map(|r|json!({"target":r.target,"artifactId":r.artifact_id,"sha256":r.sha256,"matched":r.matched})),
            "review":if gate {Some(json!({"instructions":stage.instructions.chars().take(1000).collect::<String>(),"complete":stage.instructions.chars().count()<=1000}))} else {None}})
    }).collect();
    let value = json!({"kind":"workflowRun","page":{"run":summary(run),"stages":stages},"pageTruncated":false,"servedAt":served_at});
    ensure!(
        value.to_string().len() <= 60_000,
        "Historical run exceeds transfer bound"
    );
    Ok(value)
}
/// Metadata can be listed only for artifacts an accepted completed assignment
/// actually reported in a pinned, started stage of this exact automation.
pub fn accepted_artifacts(
    run: &AutomationRun,
    teams: &[TeamRunSnapshot],
    snapshot: &ControlSnapshot,
) -> Result<Vec<Value>> {
    let stages: HashMap<_, _> = run
        .stages
        .iter()
        .enumerate()
        .filter(|(_, s)| s.started && !s.skipped)
        .map(|(i, s)| (s.team_run_id, (i, s)))
        .collect();
    let artifacts: HashMap<_, _> = snapshot
        .records
        .iter()
        .filter_map(|r| {
            if let ControlRecord::Artifact(a) = r {
                Some((a.id, a))
            } else {
                None
            }
        })
        .collect();
    let mut seen = HashSet::new();
    let mut rows = Vec::new();
    for team in teams {
        let Some((index, stage)) = stages.get(&team.id) else {
            anyhow::bail!("Evidence team belongs to another workflow");
        };
        ensure!(
            team.project_id == run.project_id
                && team.root_task_id == stage.task_id
                && team.team.id == stage.team_id,
            "Evidence stage identity changed"
        );
        for work in &team.work {
            if work.status != AssignmentStatus::Completed {
                continue;
            }
            let Some(result) = &work.result else {
                continue;
            };
            for id in &result.artifact_ids {
                let Some(artifact) = artifacts.get(id) else {
                    continue;
                };
                let Some(evidence) = &artifact.evidence else {
                    continue;
                };
                if artifact.project_id != Some(run.project_id)
                    || artifact.reference != format!("ion-document:{id}")
                    || evidence.version != 1
                    || evidence.bytes == 0
                    || evidence.bytes > 16 * 1024 * 1024
                    || evidence.producer.run_id != Some(team.id)
                    || evidence.producer.assignment_id.is_none()
                    || !team.work.iter().any(|producer| {
                        producer.id == evidence.producer.assignment_id.unwrap()
                            && producer.status == AssignmentStatus::Completed
                    })
                    || !seen.insert(*id)
                {
                    continue;
                }
                rows.push(json!({"id":id,"name":label(&artifact.name),"format":artifact.format,"projectId":run.project_id,
                    "taskId":artifact.task_id,"stage":index,"teamRunId":team.id,
                    "assignmentId":evidence.producer.assignment_id,"bytes":evidence.bytes,"sha256":evidence.sha256,"createdAt":artifact.created_at}));
            }
        }
    }
    rows.sort_by_key(|row| row["id"].as_str().unwrap().to_owned());
    Ok(rows)
}
pub fn evidence_result(
    run: &AutomationRun,
    teams: &[TeamRunSnapshot],
    snapshot: &ControlSnapshot,
    query: &WorkflowEvidenceQuery,
    served_at: u64,
) -> Result<Value> {
    ensure!(
        run.id == query.run_id && (1..=32).contains(&query.limit),
        "Evidence request changed"
    );
    let all = accepted_artifacts(run, teams, snapshot)?;
    let total = all.len();
    let mut rows: Vec<_> = all
        .into_iter()
        .filter(|row| {
            query
                .after
                .is_none_or(|after| row["id"].as_str().unwrap() > after.to_string().as_str())
        })
        .collect();
    let more = rows.len() > usize::from(query.limit);
    rows.truncate(usize::from(query.limit));
    let next = if more {
        rows.last().map(|r| r["id"].clone())
    } else {
        None
    };
    bounded(
        json!({"kind":"workflowEvidence","page":{"runId":run.id,"revision":run.revision,"documents":rows,"nextCursor":next,"total":total},"pageTruncated":false,"servedAt":served_at}),
        "documents",
    )
}
pub fn document_result(
    run: &AutomationRun,
    teams: &[TeamRunSnapshot],
    snapshot: &ControlSnapshot,
    query: &WorkflowDocumentQuery,
    content: &[u8],
    served_at: u64,
) -> Result<Value> {
    ensure!(
        run.id == query.run_id && (1..=16_384).contains(&query.limit),
        "Document request changed"
    );
    let document = accepted_artifacts(run, teams, snapshot)?
        .into_iter()
        .find(|row| row["id"] == query.artifact_id.to_string())
        .ok_or_else(|| anyhow::anyhow!("Document is not accepted evidence for this workflow"))?;
    let evidence = snapshot
        .records
        .iter()
        .find_map(|r| {
            if let ControlRecord::Artifact(a) = r {
                if a.id == query.artifact_id {
                    a.evidence.as_ref()
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap();
    ensure!(
        evidence.sha256 == query.sha256 && evidence.matches(content),
        "Document content or digest changed"
    );
    let start = query.offset as usize;
    ensure!(start < content.len(), "Document offset exceeds content");
    let end = content.len().min(start + usize::from(query.limit));
    Ok(
        json!({"kind":"workflowDocument","page":{"runId":run.id,"document":document,"offset":start,"nextOffset":if end<content.len(){Some(end)}else{None},"data":STANDARD.encode(&content[start..end])},"pageTruncated":false,"servedAt":served_at}),
    )
}

#[cfg(test)]
mod tests;
