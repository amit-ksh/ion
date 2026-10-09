//! Opt-in retry of the original assignment, never a new stage/team or fresh budget.
use super::*;
pub(super) fn advance(
    tx: &Transaction<'_>,
    team: &TeamRunSnapshot,
    policy: Option<WorkflowRetryPolicy>,
    now: u64,
) -> Result<Option<String>> {
    let Some(policy) = policy else {
        return Ok(None);
    };
    policy.validate().map_err(anyhow::Error::msg)?;
    ensure!(now < team.deadline, "Stage deadline reached before retry");
    ensure!(
        team.usage.allowance_violations == 0 && team.usage.unmetered_calls == 0,
        "Stage usage needs reconciliation before retry"
    );
    ensure!(
        team.usage.model_calls < u32::from(team.limits.max_model_calls),
        "Stage model-call budget reached before retry"
    );
    if let Some(cap) = team.limits.max_tokens {
        ensure!(
            team.usage
                .consumed_tokens
                .checked_add(team.usage.reserved_tokens)
                .is_some_and(|used| used < cap),
            "Stage token budget reached before retry"
        );
    }
    workspace::require_settled_effects(tx, team.id)?;
    let failed: Vec<_> = team
        .work
        .iter()
        .filter(|w| w.status == AssignmentStatus::Failed)
        .collect();
    let mut retry_at = now;
    // Validate every failure before creating a wait or queue mutation.
    for work in &failed {
        let safe: bool = tx.query_row(
            "SELECT retry_safe FROM ion_team_work WHERE id=?",
            [work.id.to_string()],
            |r| r.get(0),
        )?;
        let cap = usize::from(policy.max_attempts).min(usize::from(team.limits.max_attempts));
        if !safe || work.attempts.is_empty() || work.attempts.len() >= cap {
            return Ok(None);
        }
        // A native before-effects claim cannot replay a previously started app,
        // file, command/GPU action or child delegation from an earlier attempt.
        let effect: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM ion_team_workspace_edits WHERE work_id=?1) OR EXISTS(SELECT 1 FROM ion_team_app_effects WHERE work_id=?1) OR EXISTS(SELECT 1 FROM ion_team_jobs WHERE work_id=?1) OR EXISTS(SELECT 1 FROM ion_team_gpu_jobs WHERE work_id=?1) OR EXISTS(SELECT 1 FROM ion_team_work WHERE run_id=?2 AND parent_id=?1) OR EXISTS(SELECT 1 FROM ion_team_remote_calls WHERE work_id=?1 AND state NOT IN ('SUCCESS','FAILED','CANCELLED','rejected'))", params![work.id.to_string(),team.id.to_string()], |r|r.get(0))?;
        ensure!(
            !effect,
            "Stage has prior effects or an unknown host; review its original task"
        );
    }
    for work in &failed {
        let attempt = work.attempts.last().context("Failed attempt missing")?;
        let saved: Option<u64> = tx.query_row(
            "INSERT INTO ion_automation_retry_waits VALUES(?,?,?) ON CONFLICT(work_id) DO UPDATE SET attempt_id=excluded.attempt_id,retry_at=excluded.retry_at WHERE attempt_id!=excluded.attempt_id RETURNING retry_at",
            params![work.id.to_string(), attempt.id.to_string(), now.saturating_add(u64::from(policy.delay_seconds))], |r|r.get(0),
        ).optional()?;
        let at = match saved {
            Some(at) => at,
            None => tx.query_row(
                "SELECT retry_at FROM ion_automation_retry_waits WHERE work_id=? AND attempt_id=?",
                params![work.id.to_string(), attempt.id.to_string()],
                |r| r.get(0),
            )?,
        };
        retry_at = retry_at.max(at);
    }
    if now < retry_at {
        return Ok(Some(
            "Waiting for the configured retry delay after a confirmed failure before effects"
                .into(),
        ));
    }
    for work in failed {
        retry_in_transaction(tx, team, work.id, now)?;
        tx.execute(
            "DELETE FROM ion_automation_retry_waits WHERE work_id=?",
            [work.id.to_string()],
        )?;
    }
    Ok(Some(
        "Confirmed failures queued within the original attempt and usage limits".into(),
    ))
}
