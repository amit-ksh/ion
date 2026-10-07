//! Persisted owner maintenance continues when a task leaves the dispatch index.
//! This lease fences polling callbacks, never authorizes a new cloud job or spend.
use super::*;
#[derive(Clone, Debug)]
pub(crate) struct GpuMaintenanceClaim {
    pub request_id: Uuid,
    pub work_id: Uuid,
    pub attempt_id: Uuid,
    token: Uuid,
}
impl TeamCoordinator {
    pub(crate) fn claim_gpu_maintenance(
        &self,
        now: u64,
        busy_work: &[Uuid],
    ) -> Result<Option<GpuMaintenanceClaim>> {
        ensure!(
            busy_work.len() <= 256,
            "GPU maintenance exclusion set exceeds its bound"
        );
        let until = now
            .checked_add(600)
            .context("GPU maintenance clock overflow")?;
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // Lazy rows adopt previously saved bound jobs too. Active waits keep their
        // existing collector; terminal tasks need no runnable model or live lease.
        let excluded = if busy_work.is_empty() {
            String::new()
        } else {
            format!(
                " AND j.work_id NOT IN ({})",
                (2..=busy_work.len() + 1)
                    .map(|i| format!("?{i}"))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        let sql=format!("SELECT j.request_id,j.work_id,j.attempt_id FROM ion_team_gpu_jobs j JOIN ion_team_work w ON w.id=j.work_id JOIN ion_team_runs r ON r.id=j.run_id LEFT JOIN ion_team_gpu_maintenance m ON m.request_id=j.request_id
            WHERE j.state IN ('running','canceling','terminal') AND j.job_id IS NOT NULL AND j.creation_operation IS NOT NULL
            AND (w.status IN ('completed','inReview','blocked','failed','canceled','superseded') OR r.status NOT IN ('active','stopping'))
            AND coalesce(m.done,0)=0 AND coalesce(m.next_poll_at,0)<=?1 AND coalesce(m.busy_until,0)<=?1
            AND NOT EXISTS(SELECT 1 FROM ion_team_gpu_maintenance other JOIN ion_team_gpu_jobs saved ON saved.request_id=other.request_id WHERE saved.work_id=j.work_id AND other.token IS NOT NULL AND other.busy_until>?1)
            {excluded} ORDER BY coalesce(m.next_poll_at,0),j.request_id LIMIT 1");
        let mut values = vec![rusqlite::types::Value::Integer(i64::try_from(now)?)];
        values.extend(
            busy_work
                .iter()
                .map(|id| rusqlite::types::Value::Text(id.to_string())),
        );
        let row: Option<(String, String, String)> = tx
            .query_row(&sql, rusqlite::params_from_iter(values), |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .optional()?;
        let Some((request, work, attempt)) = row else {
            return Ok(None);
        };
        let token = Uuid::new_v4();
        tx.execute("INSERT INTO ion_team_gpu_maintenance(request_id,busy_until,token) VALUES(?,?,?) ON CONFLICT(request_id) DO UPDATE SET busy_until=excluded.busy_until,token=excluded.token",
            params![request,until,token.to_string()])?;
        let claim = GpuMaintenanceClaim {
            request_id: Uuid::parse_str(&request)?,
            work_id: Uuid::parse_str(&work)?,
            attempt_id: Uuid::parse_str(&attempt)?,
            token,
        };
        tx.commit()?;
        Ok(Some(claim))
    }
    /// An expired/replaced callback cannot overwrite a newer poll's schedule.
    /// Unknown/missing receipts use saved backoff and never fabricate completion.
    pub(crate) fn finish_gpu_maintenance(
        &self,
        claim: &GpuMaintenanceClaim,
        complete: bool,
        now: u64,
    ) -> Result<()> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let failures: u32 = tx
            .query_row(
                "SELECT failures FROM ion_team_gpu_maintenance WHERE request_id=? AND token=?",
                params![claim.request_id.to_string(), claim.token.to_string()],
                |r| r.get(0),
            )
            .context("GPU maintenance poll was superseded")?;
        let delay = if complete {
            0
        } else {
            (5u64 << failures.min(6)).min(300)
        };
        let next = now
            .checked_add(delay)
            .context("GPU maintenance backoff overflow")?;
        ensure!(tx.execute("UPDATE ion_team_gpu_maintenance SET next_poll_at=?,busy_until=0,token=NULL,failures=?,done=? WHERE request_id=? AND token=?",
            params![next,if complete{0}else{failures.saturating_add(1).min(6)},complete,claim.request_id.to_string(),claim.token.to_string()])?==1,"GPU maintenance poll was superseded");
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::team_gpu::tests::{Fixture, launch, launch_body};
    use crate::teams::AssignmentOutcome;
    use waku_protocol::teams::TeamRunStatus;

    fn saved(bound: bool) -> (Fixture, Uuid) {
        let f = Fixture::new(true, true);
        let l = launch();
        let now = crate::model::unix_time();
        f.coordinator
            .reserve_gpu_launch(&f.lease, &l.request, &launch_body(&l), &f.policy, now)
            .unwrap();
        if bound {
            f.coordinator
                .bind_gpu_acknowledgement(
                    l.request.request_id,
                    "operation-create",
                    "aijob-fixture",
                    now,
                )
                .unwrap();
        }
        (f, l.request.request_id)
    }
    fn stop(f: &Fixture) {
        let now = crate::model::unix_time();
        let run = f.coordinator.snapshot(f.lease.run_id()).unwrap();
        f.coordinator.stop(run.id, run.revision, now).unwrap();
        let canceled = f
            .coordinator
            .report(&f.lease, AssignmentOutcome::Canceled, now)
            .unwrap();
        assert_eq!(canceled.status, TeamRunStatus::Canceled);
    }
    #[test]
    fn team_gpu_maintenance_adopts_canceled_jobs_after_restart_with_persisted_backoff_and_original_ids()
     {
        let (f, request) = saved(true);
        stop(&f);
        let now = crate::model::unix_time();
        let reopened = TeamCoordinator::new(f.dir.join("control.db"));
        assert!(reopened.dispatch_runs(None, 32).unwrap().is_empty());
        assert!(
            reopened
                .claim_gpu_maintenance(now, &[f.lease.work_id()])
                .unwrap()
                .is_none()
        );
        let claim = reopened.claim_gpu_maintenance(now, &[]).unwrap().unwrap();
        assert_eq!(claim.request_id, request);
        assert_eq!(claim.attempt_id, f.lease.attempt_id());
        assert_eq!(claim.work_id, f.lease.work_id());
        assert!(
            f.coordinator
                .claim_gpu_maintenance(now, &[])
                .unwrap()
                .is_none()
        );
        reopened.finish_gpu_maintenance(&claim, false, now).unwrap();
        let restarted = TeamCoordinator::new(f.dir.join("control.db"));
        assert!(
            restarted
                .claim_gpu_maintenance(now + 4, &[])
                .unwrap()
                .is_none()
        );
        let next = restarted
            .claim_gpu_maintenance(now + 5, &[])
            .unwrap()
            .unwrap();
        assert_eq!(next.request_id, request);
        restarted
            .finish_gpu_maintenance(&next, true, now + 5)
            .unwrap();
        assert!(
            f.coordinator
                .claim_gpu_maintenance(now + 1000, &[])
                .unwrap()
                .is_none()
        );
        let db = rusqlite::Connection::open(f.dir.join("control.db")).unwrap();
        let saved: (String, String, u64, String) = db.query_row("SELECT job_id,creation_operation,reserved_micros,state FROM ion_team_gpu_jobs WHERE request_id=?", [request.to_string()], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
        assert_eq!(
            saved,
            (
                "aijob-fixture".into(),
                "operation-create".into(),
                f.policy.reservation_micros,
                "running".into()
            )
        );
    }
    #[test]
    fn team_gpu_maintenance_expired_callback_cannot_finish_replacement_or_clear_its_backoff() {
        let (f, _) = saved(true);
        stop(&f);
        let now = crate::model::unix_time();
        let old = f
            .coordinator
            .claim_gpu_maintenance(now, &[])
            .unwrap()
            .unwrap();
        assert!(
            f.coordinator
                .claim_gpu_maintenance(now + 599, &[])
                .unwrap()
                .is_none()
        );
        let replacement = f
            .coordinator
            .claim_gpu_maintenance(now + 600, &[])
            .unwrap()
            .unwrap();
        assert_ne!(old.token, replacement.token);
        assert!(
            f.coordinator
                .finish_gpu_maintenance(&old, true, now + 601)
                .is_err()
        );
        assert!(
            f.coordinator
                .claim_gpu_maintenance(now + 602, &[])
                .unwrap()
                .is_none()
        );
        f.coordinator
            .finish_gpu_maintenance(&replacement, false, now + 603)
            .unwrap();
        assert!(
            f.coordinator
                .claim_gpu_maintenance(now + 607, &[])
                .unwrap()
                .is_none()
        );
        let newer = f
            .coordinator
            .claim_gpu_maintenance(now + 608, &[])
            .unwrap()
            .unwrap();
        assert!(
            f.coordinator
                .finish_gpu_maintenance(&replacement, true, now + 609)
                .is_err()
        );
        f.coordinator
            .finish_gpu_maintenance(&newer, true, now + 609)
            .unwrap();
    }
    #[test]
    fn team_gpu_maintenance_never_recreates_unknown_launches_or_steals_active_work_and_bounds_retries()
     {
        let (unbound, _) = saved(false);
        stop(&unbound);
        let now = crate::model::unix_time();
        assert!(
            unbound
                .coordinator
                .claim_gpu_maintenance(now, &[])
                .unwrap()
                .is_none()
        );
        let (f, _) = saved(true);
        assert!(
            f.coordinator
                .claim_gpu_maintenance(now, &[])
                .unwrap()
                .is_none()
        );
        f.coordinator
            .report(
                &f.lease,
                AssignmentOutcome::Blocked(
                    "Host exited; original GPU job is still outstanding".into(),
                ),
                now,
            )
            .unwrap();
        let mut at = now;
        for delay in [5, 10, 20, 40, 80, 160, 300, 300] {
            let claim = TeamCoordinator::new(f.dir.join("control.db"))
                .claim_gpu_maintenance(at, &[])
                .unwrap()
                .unwrap();
            f.coordinator
                .finish_gpu_maintenance(&claim, false, at)
                .unwrap();
            assert!(
                f.coordinator
                    .claim_gpu_maintenance(at + delay - 1, &[])
                    .unwrap()
                    .is_none()
            );
            at += delay;
        }
        assert!(
            f.coordinator
                .claim_gpu_maintenance(at, &vec![Uuid::new_v4(); 257])
                .is_err()
        );
        assert!(
            f.coordinator
                .claim_gpu_maintenance(at, &[])
                .unwrap()
                .is_some()
        );
    }
}
