//! Collection reads share one SQLite snapshot and never hydrate transcripts.
use super::*;
impl TeamCoordinator {
    pub fn list_runs(
        &self,
        project: Option<Uuid>,
        team: Option<Uuid>,
        after: Option<Uuid>,
        limit: usize,
    ) -> Result<TeamRunPage> {
        ensure!((1..=64).contains(&limit), "Run page size must be 1–64");
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let cursor = if let Some(id) = after {
            Some(
                tx.query_row(
                    "SELECT json_extract(body,'$.createdAt') FROM ion_team_runs WHERE id=?",
                    [id.to_string()],
                    |r| r.get::<_, u64>(0),
                )
                .context("Run page cursor no longer exists")?,
            )
        } else {
            None
        };
        let project = project.map(|id| id.to_string());
        let team = team.map(|id| id.to_string());
        let mut query = tx.prepare("SELECT r.id,r.root_task_id,r.project_id,
            json_extract(r.body,'$.team.id'),json_extract(r.body,'$.team.name'),
            json_extract(root.body,'$.objective'),r.status,r.revision,
            json_extract(r.body,'$.createdAt'),
            COALESCE((SELECT MAX(timestamp) FROM ion_team_audit WHERE run_id=r.id),json_extract(r.body,'$.createdAt')),
            (SELECT COUNT(*) FROM ion_team_work WHERE run_id=r.id),
            (SELECT COUNT(*) FROM ion_team_work WHERE run_id=r.id AND status IN ('running','canceling')),
            (SELECT COUNT(*) FROM ion_team_work WHERE run_id=r.id AND status IN ('waiting','inReview')),
            (SELECT COUNT(*) FROM ion_team_work WHERE run_id=r.id AND status='completed'),
            (SELECT COUNT(*) FROM ion_team_work WHERE run_id=r.id AND (status IN ('blocked','failed','uncertain') OR (status='queued' AND json_extract(body,'$.reason') IS NOT NULL)))
            FROM ion_team_runs r JOIN ion_team_work root ON root.run_id=r.id AND root.parent_id IS NULL
            WHERE (?1 IS NULL OR r.project_id=?1) AND (?2 IS NULL OR json_extract(r.body,'$.team.id')=?2)
              AND (?3 IS NULL OR json_extract(r.body,'$.createdAt')<?3 OR (json_extract(r.body,'$.createdAt')=?3 AND r.id<?4))
            ORDER BY json_extract(r.body,'$.createdAt') DESC,r.id DESC LIMIT ?5")?;
        let mut runs = query
            .query_map(
                params![
                    project,
                    team,
                    cursor,
                    after.map(|id| id.to_string()).unwrap_or_default(),
                    limit + 1
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, u64>(7)?,
                        row.get::<_, u64>(8)?,
                        row.get::<_, u64>(9)?,
                        row.get::<_, u32>(10)?,
                        row.get::<_, u32>(11)?,
                        row.get::<_, u32>(12)?,
                        row.get::<_, u32>(13)?,
                        row.get::<_, u32>(14)?,
                    ))
                },
            )?
            .map(|row| {
                let (
                    id,
                    root,
                    project,
                    team,
                    name,
                    objective,
                    status,
                    revision,
                    created_at,
                    updated_at,
                    assignments,
                    running,
                    waiting,
                    completed,
                    needs_attention,
                ) = row?;
                let id = Uuid::parse_str(&id)?;
                Ok(TeamRunSummary {
                    id,
                    root_task_id: Uuid::parse_str(&root)?,
                    project_id: Uuid::parse_str(&project)?,
                    team_id: Uuid::parse_str(&team)?,
                    team_name: name,
                    objective,
                    status: serde_json::from_value(serde_json::Value::String(status))?,
                    revision,
                    created_at,
                    updated_at,
                    assignments,
                    running,
                    waiting,
                    completed,
                    needs_attention,
                    usage: super::budget::usage(&tx, id)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let next_cursor = if runs.len() > limit {
            runs.truncate(limit);
            runs.last().map(|r| r.id)
        } else {
            None
        };
        drop(query);
        tx.commit()?;
        Ok(TeamRunPage { runs, next_cursor })
    }
}
