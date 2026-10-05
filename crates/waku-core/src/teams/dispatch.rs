//! Indexed dispatcher reads and trusted setup/model-call operations.
use super::*;
impl TeamCoordinator {
    /// One background collection pass over indexed identity/status rows. Team
    /// names are read once per distinct run; task transcripts stay unhydrated.
    pub fn task_projections(
        &self,
        ids: impl IntoIterator<Item = Uuid>,
    ) -> Result<std::collections::HashMap<Uuid, (TeamTaskBinding, AssignmentStatus)>> {
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let mut query = tx.prepare("SELECT run_id,id,status FROM ion_team_work WHERE task_id=?")?;
        let mut name_query =
            tx.prepare("SELECT json_extract(body,'$.team.name') FROM ion_team_runs WHERE id=?")?;
        let mut names: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let mut projections = std::collections::HashMap::new();
        for task in ids {
            let row: Option<(String, String, String)> = query
                .query_row([task.to_string()], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })
                .optional()?;
            if let Some((run, work, status)) = row {
                let name = if let Some(name) = names.get(&run) {
                    name.clone()
                } else {
                    let name: String = name_query.query_row([&run], |r| r.get(0))?;
                    names.insert(run.clone(), name.clone());
                    name
                };
                projections.insert(
                    task,
                    (
                        TeamTaskBinding {
                            run_id: Uuid::parse_str(&run)?,
                            assignment_id: Uuid::parse_str(&work)?,
                            team_name: name,
                        },
                        serde_json::from_value(serde_json::Value::String(status))?,
                    ),
                );
            }
        }
        drop(query);
        drop(name_query);
        tx.commit()?;
        Ok(projections)
    }
    pub fn dispatch_runs(&self, after: Option<Uuid>, limit: u16) -> Result<Vec<TeamRunSnapshot>> {
        ensure!((1..=128).contains(&limit), "Dispatcher batch must be 1-128");
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let ids:Vec<String>=tx.prepare("SELECT r.id FROM ion_team_runs r WHERE r.status IN ('active','stopping') AND r.id>? AND EXISTS(SELECT 1 FROM ion_team_work w WHERE w.run_id=r.id AND w.status IN ('queued','running','waiting','canceling','uncertain')) ORDER BY r.id LIMIT ?")?.query_map(params![after.map(|id|id.to_string()).unwrap_or_default(),limit],|r|r.get(0))?.collect::<rusqlite::Result<_>>()?;
        let runs = ids
            .into_iter()
            .map(|id| load(&tx, Uuid::parse_str(&id)?))
            .collect::<Result<_>>()?;
        tx.commit()?;
        Ok(runs)
    }
    /// Setup blockers retain the queue and consume no attempt, so fixing setup
    /// can start the same durable task. Exact repeats do not churn audit/revision.
    pub fn defer_dispatch(
        &self,
        run_id: Uuid,
        work_id: Uuid,
        reason: Option<String>,
        now: u64,
    ) -> Result<()> {
        if let Some(reason) = &reason {
            text(reason, 4096, "Setup blocker")?;
        }
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, run_id)?;
        active(&run, now)?;
        let mut work = run
            .work
            .iter()
            .find(|w| w.id == work_id)
            .context("Assignment missing")?
            .clone();
        ensure!(
            work.status == AssignmentStatus::Queued,
            "Only queued assignments may be deferred"
        );
        if work.reason == reason {
            return Ok(());
        }
        work.reason = reason;
        save_work(&tx, &work, false)?;
        bump(&tx, run_id)?;
        audit(
            &tx,
            run_id,
            Some(work_id),
            "readiness",
            work.reason.as_deref().unwrap_or("Setup is ready"),
            now,
        )?;
        tx.commit()?;
        Ok(())
    }
}
