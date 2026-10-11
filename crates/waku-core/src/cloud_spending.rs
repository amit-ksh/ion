//! Shared cloud allowances commit with GPU launches and survive every runtime state.
use anyhow::{Result, ensure};
use rusqlite::{Connection, params};
use uuid::Uuid;
use waku_protocol::{cloud_spending::*, control::ControlRecord};
pub(crate) fn schema(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS ion_cloud_spending_holds(request_id TEXT PRIMARY KEY,budget_id TEXT NOT NULL,run_id TEXT NOT NULL,cloud_project TEXT NOT NULL,target TEXT NOT NULL,currency TEXT NOT NULL,held_micros INTEGER NOT NULL,budget_snapshot TEXT NOT NULL); CREATE INDEX IF NOT EXISTS ion_cloud_spending_budget ON ion_cloud_spending_holds(budget_id); CREATE INDEX IF NOT EXISTS ion_cloud_spending_project ON ion_cloud_spending_holds(cloud_project,currency);")?;
    Ok(())
}
pub(crate) fn validate(budget: &CloudBudget) -> Result<()> {
    ensure!(
        !budget.id.is_nil()
            && !budget.name.trim().is_empty()
            && budget.name.len() <= 160
            && !budget.name.chars().any(char::is_control),
        "A GPU budget needs a name of at most 160 bytes"
    );
    ensure!(
        !budget.cloud_project.is_empty()
            && budget.cloud_project.len() <= 160
            && budget
                .cloud_project
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b)),
        "Enter the exact Nebius project ID"
    );
    ensure!(
        budget.currency.len() == 3
            && budget.currency.bytes().all(|b| b.is_ascii_uppercase())
            && (1..=1_000_000_000_000).contains(&budget.limit_micros),
        "GPU budget currency or amount is invalid"
    );
    ensure!(
        (1..=16).contains(&budget.targets.len()),
        "Select 1–16 approved sandbox target names"
    );
    let mut seen = std::collections::HashSet::new();
    for target in &budget.targets {
        ensure!(
            !target.trim().is_empty()
                && target.trim() == target
                && target.len() <= 160
                && !target.chars().any(char::is_control)
                && seen.insert(target),
            "GPU budget target names must be distinct and nonempty"
        );
    }
    Ok(())
}
fn totals(db: &Connection, id: Uuid) -> Result<(u64, u64)> {
    Ok(db.query_row("SELECT coalesce(sum(held_micros),0),count(*) FROM ion_cloud_spending_holds WHERE budget_id=?1",[id.to_string()],|row|Ok((row.get(0)?,row.get(1)?)))?)
}
pub(crate) fn validate_change(db: &Connection, record: &ControlRecord) -> Result<()> {
    if let ControlRecord::CloudBudget(budget) = record {
        // Adopt pre-existing launches before accepting a cap; changing definitions must not reset spending.
        let has_jobs:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='ion_team_gpu_jobs')",[],|r|r.get(0))?;
        if has_jobs {
            let legacy:Vec<(String,String,String,u64,String)>=db.prepare("SELECT j.request_id,j.run_id,j.currency,j.reserved_micros,j.policy FROM ion_team_gpu_jobs j LEFT JOIN ion_cloud_spending_holds h ON h.request_id=j.request_id WHERE j.project=?1 AND h.request_id IS NULL")?.query_map([&budget.cloud_project],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?.collect::<rusqlite::Result<_>>()?;
            for (request, run, currency, amount, policy) in legacy {
                ensure!(
                    currency == budget.currency,
                    "Existing GPU allowances use a different currency; billing reconciliation is required"
                );
                let target: serde_json::Value = serde_json::from_str(&policy)?;
                let target = target["target"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Existing GPU allowance has no target"))?;
                db.execute(
                    "INSERT INTO ion_cloud_spending_holds VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                    params![
                        request,
                        budget.id.to_string(),
                        run,
                        budget.cloud_project,
                        target,
                        currency,
                        amount,
                        serde_json::to_string(budget)?
                    ],
                )?;
            }
        }
        let (held, _) = totals(db, budget.id)?;
        ensure!(
            held <= budget.limit_micros,
            "Budget cannot be lowered below its held allowances"
        );
        let identities:Vec<(String,String)>=db.prepare("SELECT DISTINCT cloud_project,currency FROM ion_cloud_spending_holds WHERE budget_id=?1 LIMIT 2")?.query_map([budget.id.to_string()],|r|Ok((r.get(0)?,r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        ensure!(
            identities
                .iter()
                .all(|(project, currency)| project == &budget.cloud_project
                    && currency == &budget.currency),
            "A budget with held allowances cannot change project or currency"
        );
    }
    Ok(())
}
pub(crate) fn ensure_removable(db: &Connection, id: Uuid) -> Result<()> {
    ensure!(
        totals(db, id)?.1 == 0,
        "Budget has held allowances. Pause it instead of deleting it"
    );
    Ok(())
}
fn approved(
    db: &Connection,
    project: &str,
    target: &str,
    currency: &str,
    amount: u64,
) -> Result<CloudBudget> {
    let bodies:Vec<String>=db.prepare("SELECT body FROM ion_control_records WHERE kind='cloudBudget' AND json_extract(body,'$.cloudProject')=?1 LIMIT 2")?.query_map([project],|r|r.get(0))?.collect::<rusqlite::Result<_>>()?;
    ensure!(
        bodies.len() == 1,
        "Configure one shared GPU budget for the exact Nebius project before launch"
    );
    let ControlRecord::CloudBudget(budget) = serde_json::from_str(&bodies[0])? else {
        anyhow::bail!("Invalid shared GPU budget");
    };
    validate(&budget)?;
    ensure!(
        !budget.paused && budget.targets.iter().any(|t| t == target),
        "Shared GPU budget is paused or does not authorize this target"
    );
    ensure!(
        budget.currency == currency,
        "GPU allowance currency differs from the shared budget"
    );
    let (held, _) = totals(db, budget.id)?;
    ensure!(
        held.checked_add(amount)
            .is_some_and(|total| total <= budget.limit_micros),
        "Shared GPU budget reached; unknown and terminal charges remain held"
    );
    Ok(budget)
}
pub(crate) fn authorize(
    db: &Connection,
    project: &str,
    target: &str,
    currency: &str,
    amount: u64,
) -> Result<()> {
    approved(db, project, target, currency, amount).map(|_| ())
}
pub(crate) fn hold(
    db: &Connection,
    request: Uuid,
    run: Uuid,
    project: &str,
    target: &str,
    currency: &str,
    amount: u64,
) -> Result<()> {
    ensure!(amount > 0, "GPU allowance must be positive");
    let budget = approved(db, project, target, currency, amount)?;
    db.execute(
        "INSERT INTO ion_cloud_spending_holds VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            request.to_string(),
            budget.id.to_string(),
            run.to_string(),
            project,
            target,
            currency,
            amount,
            serde_json::to_string(&budget)?
        ],
    )?;
    Ok(())
}
impl crate::control::ControlStore {
    pub fn cloud_budgets(&self, after: Option<Uuid>, limit: u16) -> Result<CloudBudgetPage> {
        ensure!(
            (1..=64).contains(&limit),
            "GPU budget page must contain 1–64 rows"
        );
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let total: u64 = tx.query_row(
            "SELECT count(*) FROM ion_control_records WHERE kind='cloudBudget'",
            [],
            |r| r.get(0),
        )?;
        let bodies:Vec<String>=tx.prepare("SELECT body FROM ion_control_records WHERE kind='cloudBudget' AND (?1 IS NULL OR id>?1) ORDER BY id LIMIT ?2")?.query_map(params![after.map(|id|id.to_string()),u64::from(limit)+1],|r|r.get(0))?.collect::<rusqlite::Result<_>>()?;
        let more = bodies.len() > usize::from(limit);
        let mut budgets = Vec::new();
        for body in bodies.into_iter().take(usize::from(limit)) {
            let ControlRecord::CloudBudget(budget) = serde_json::from_str(&body)? else {
                anyhow::bail!("Invalid shared GPU budget");
            };
            let (held_micros, requests) = totals(&tx, budget.id)?;
            budgets.push(CloudBudgetStatus {
                available_micros: budget.limit_micros.saturating_sub(held_micros),
                budget,
                held_micros,
                requests,
            });
        }
        let next_cursor = if more {
            budgets.last().map(|b| b.budget.id)
        } else {
            None
        };
        tx.commit()?;
        Ok(CloudBudgetPage {
            budgets,
            next_cursor,
            total,
        })
    }
}

#[cfg(test)]
mod tests;
