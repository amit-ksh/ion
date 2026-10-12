//! Bounded, indexed collection of persisted operations; no provider I/O or mutation.
use super::*;
use waku_protocol::remote_control::*;
fn rank(kind: RemoteExecutionKind) -> u8 {
    match kind {
        RemoteExecutionKind::Model => 0,
        RemoteExecutionKind::Command => 1,
        RemoteExecutionKind::Gpu => 2,
    }
}
// Sort by immutable (kind, request UUID); changing terminal state cannot reorder a page.
const COLLECTION: &str = "WITH operations AS NOT MATERIALIZED (
 SELECT 0 kind,c.call_id id,m.run_id,c.work_id,c.attempt_id,c.project,c.operation,NULL job_id,NULL cancellation_operation,NULL cleanup_state,c.state,
 c.state IN ('SUCCESS','FAILED','CANCELLED','rejected') settled,c.state='canceling' stop_requested
 FROM ion_team_remote_calls c JOIN ion_team_model_calls m ON m.id=c.call_id
 UNION ALL
 SELECT 1,j.request_id,j.run_id,j.work_id,j.attempt_id,j.project,j.operation,NULL,NULL,NULL,j.state,
 j.state IN ('settled','failed','rejected'),j.state='canceling' FROM ion_team_jobs j
 UNION ALL
 SELECT 2,j.request_id,j.run_id,j.work_id,j.attempt_id,j.project,j.creation_operation,j.job_id,j.cancellation_operation,c.state,j.state,
 j.state='cleaned' OR COALESCE(c.state='verified',0),j.cancel_requested FROM ion_team_gpu_jobs j
 LEFT JOIN ion_team_gpu_cleanup c ON c.request_id=j.request_id
)";
impl TeamCoordinator {
    pub fn list_remote_executions(
        &self,
        query: RemoteExecutionQuery,
    ) -> Result<RemoteExecutionPage> {
        ensure!(
            (1..=64).contains(&query.limit),
            "Remote page size must be 1–64"
        );
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let project = query.project_id.map(|id| id.to_string());
        let filter = query.kind.map(rank);
        // Cursor may become settled while paging. Validate its immutable collection/filter identity.
        if let Some(cursor) = &query.after {
            let present: bool=tx.query_row(&format!("{COLLECTION} SELECT EXISTS(SELECT 1 FROM operations o JOIN ion_team_runs r ON r.id=o.run_id WHERE o.kind=?1 AND o.id=?2 AND (?3 IS NULL OR r.project_id=?3) AND (?4 IS NULL OR o.kind=?4))"),params![rank(cursor.kind),cursor.request_id.to_string(),project,filter],|r|r.get(0))?;
            ensure!(
                present,
                "Remote page cursor belongs to another collection or no longer exists"
            );
        }
        let after_kind = query.after.as_ref().map(|c| rank(c.kind));
        let after_id = query.after.as_ref().map(|c| c.request_id.to_string());
        // Each indexed kind contributes at most limit+1 candidates before the final merge.
        let predicate = "WHERE (?1 IS NULL OR r.project_id=?1) AND (?2 IS NULL OR o.kind=?2) AND (NOT ?3 OR NOT o.settled) AND (?4 IS NULL OR o.kind>?4 OR (o.kind=?4 AND o.id>?5))";
        let mut stmt=tx.prepare(&format!("{COLLECTION},
   model_page AS (SELECT o.* FROM operations o JOIN ion_team_runs r ON r.id=o.run_id {predicate} AND o.kind=0 ORDER BY o.id LIMIT ?6),
   command_page AS (SELECT o.* FROM operations o JOIN ion_team_runs r ON r.id=o.run_id {predicate} AND o.kind=1 ORDER BY o.id LIMIT ?6),
   gpu_page AS (SELECT o.* FROM operations o JOIN ion_team_runs r ON r.id=o.run_id {predicate} AND o.kind=2 ORDER BY o.id LIMIT ?6),
   selected AS (SELECT * FROM model_page UNION ALL SELECT * FROM command_page UNION ALL SELECT * FROM gpu_page)
   SELECT json_object(
   'kind',CASE o.kind WHEN 0 THEN 'model' WHEN 1 THEN 'command' ELSE 'gpu' END,
   'requestId',o.id,'runId',o.run_id,'rootTaskId',r.root_task_id,'projectId',r.project_id,
   'assignmentId',o.work_id,'taskId',w.task_id,'attemptId',o.attempt_id,'agentId',w.agent_id,
   'teamName',json_extract(r.body,'$.team.name'),'objective',json_extract(w.body,'$.objective'),
   'state',o.state,'remoteProject',o.project,'operationId',o.operation,'jobId',o.job_id,
   'cancellationOperationId',o.cancellation_operation,'cleanupState',o.cleanup_state,
   'stopRequested',json(CASE WHEN o.stop_requested THEN 'true' ELSE 'false' END),
   'settled',json(CASE WHEN o.settled THEN 'true' ELSE 'false' END))
   FROM selected o JOIN ion_team_runs r ON r.id=o.run_id JOIN ion_team_work w ON w.id=o.work_id AND w.run_id=o.run_id
   ORDER BY o.kind,o.id LIMIT ?6"))?;
        let rows = stmt.query_map(
            params![
                project,
                filter,
                query.unresolved_only,
                after_kind,
                after_id,
                usize::from(query.limit) + 1
            ],
            |r| r.get::<_, String>(0),
        )?;
        let mut entries = rows
            .map(|row| Ok(serde_json::from_str::<RemoteExecution>(&row?)?))
            .collect::<Result<Vec<_>>>()?;
        let next_cursor = if entries.len() > usize::from(query.limit) {
            entries.truncate(usize::from(query.limit));
            entries.last().map(|e| RemoteExecutionCursor {
                kind: e.kind,
                request_id: e.request_id,
            })
        } else {
            None
        };
        drop(stmt);
        tx.commit()?;
        Ok(RemoteExecutionPage {
            entries,
            next_cursor,
        })
    }
}
