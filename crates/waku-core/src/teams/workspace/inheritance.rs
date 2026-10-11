//! Immutable, owner-installed code inputs between workflow stages. Copying runs
//! before any lease exists and never writes to the user's source checkout.
use super::*;
use sha2::{Digest, Sha256};

#[derive(Serialize, Deserialize)]
struct Handoff {
    previous: Uuid,
    record: WorkspaceRecord,
    changes: Vec<TeamSourceChange>,
    sha256: String,
}
impl Handoff {
    fn verify(&self) -> Result<()> {
        let bytes = serde_json::to_vec(&self.changes)?;
        ensure!(
            self.changes.len() <= 256 && bytes.len() <= 16 * 1024 * 1024,
            "Inherited code exceeds its bound"
        );
        ensure!(
            input_hash(self.previous, &self.record, &self.changes)? == self.sha256,
            "Inherited code receipt changed"
        );
        Ok(())
    }
}
fn input_hash(
    previous: Uuid,
    record: &WorkspaceRecord,
    changes: &[TeamSourceChange],
) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(previous, record, changes))?)
    ))
}
fn row(db: &Connection, run: Uuid) -> Result<Option<(Uuid, String, Option<String>)>> {
    let row: Option<(String, String, Option<String>)> = db
        .query_row(
            "SELECT previous_run_id,state,body FROM ion_team_workspace_handoffs WHERE run_id=?",
            [run.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    row.map(|(previous, state, body)| Ok((Uuid::parse_str(&previous)?, state, body)))
        .transpose()
}
fn read(previous: Uuid, body: &str) -> Result<Handoff> {
    let handoff: Handoff = serde_json::from_str(body)?;
    ensure!(
        handoff.previous == previous,
        "Inherited stage identity changed"
    );
    handoff.verify()?;
    Ok(handoff)
}
pub(in crate::teams) fn reserve(
    tx: &Transaction<'_>,
    run: Uuid,
    previous: Uuid,
    now: u64,
) -> Result<()> {
    let next = load(tx, run)?;
    let prior = load(tx, previous)?;
    ensure!(
        prior.status == TeamRunStatus::Completed && prior.project_id == next.project_id,
        "Code inputs require a completed stage in the same project"
    );
    ensure!(
        prior.workspace.is_some(),
        "Inherit code requires a coding workspace in the previous completed stage"
    );
    ensure!(
        next.access.workspace_read,
        "Inherit code requires workspace read access"
    );
    tx.execute(
        "INSERT INTO ion_team_workspace_handoffs VALUES(?,?,'pending',NULL)",
        params![run.to_string(), previous.to_string()],
    )?;
    audit(
        tx,
        run,
        None,
        "workspace_input_reserved",
        &format!("Code inputs from completed stage {previous}; grants unchanged"),
        now,
    )?;
    Ok(())
}
pub(super) fn require_installed(db: &Connection, run: Uuid) -> Result<()> {
    let pending: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM ion_team_workspace_handoffs WHERE run_id=? AND state!='applied')",
        [run.to_string()],|r|r.get(0))?;
    ensure!(
        !pending,
        "Inherited code must be verified and installed before dispatch"
    );
    Ok(())
}
pub(in crate::teams) fn require_ready(db: &Connection, run: Uuid) -> Result<()> {
    require_installed(db, run)?;
    let unsealed: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM ion_team_workspace_handoffs h WHERE h.run_id=? AND NOT EXISTS(SELECT 1 FROM ion_team_workspaces w WHERE w.run_id=h.run_id AND w.state='ready'))",
        [run.to_string()],|r|r.get(0))?;
    ensure!(
        !unsealed,
        "Inherited code workspace must be sealed before dispatch"
    );
    Ok(())
}
pub(super) fn inherited_changes(db: &Connection, run: Uuid) -> Result<Vec<TeamSourceChange>> {
    let Some((previous, state, body)) = row(db, run)? else {
        return Ok(vec![]);
    };
    ensure!(state == "applied", "Inherited code is not ready for review");
    Ok(read(previous, &body.context("Inherited code receipt missing")?)?.changes)
}
/// Expensive Git/byte verification happens outside the dispatch transaction and
/// outside any immediate writer transaction. The plan is then sealed once.
pub(super) fn pin(
    owner: &TeamCoordinator,
    db: &Connection,
    run: Uuid,
    source: &Path,
    now: u64,
) -> Result<Option<WorkspaceRecord>> {
    let Some((previous, _, body)) = row(db, run)? else {
        return Ok(None);
    };
    let plan = if let Some(body) = body {
        read(previous, &body)?
    } else {
        let prior = load(db, previous)?;
        ensure!(
            prior.status == TeamRunStatus::Completed,
            "Previous code stage is no longer completed"
        );
        require_settled_effects(db, previous)?;
        let record = integration::record(db, previous)?;
        verify(&record)?;
        let changes = integration::receipted_changes(db, previous, &record)?;
        integration::verify_changes(&record, &changes, false)?;
        let sha256 = input_hash(previous, &record, &changes)?;
        let plan = Handoff {
            previous,
            record,
            changes,
            sha256,
        };
        let mut db = owner.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        active(&load(&tx, run)?, now)?;
        let current = load(&tx, previous)?;
        ensure!(
            current.revision == prior.revision && current.status == TeamRunStatus::Completed,
            "Previous code stage changed during handoff"
        );
        let installed = tx.execute("UPDATE ion_team_workspace_handoffs SET state='pinned',body=? WHERE run_id=? AND body IS NULL",
            params![serde_json::to_string(&plan)?,run.to_string()])?;
        if installed == 1 {
            audit(
                &tx,
                run,
                None,
                "workspace_input_pinned",
                &format!(
                    "Stage {previous}; {} paths; SHA-256 {}",
                    plan.changes.len(),
                    plan.sha256
                ),
                now,
            )?;
            bump(&tx, run)?;
        }
        let (_, _, body) = row(&tx, run)?.context("Code input reservation disappeared")?;
        let sealed = read(previous, &body.context("Code input was not pinned")?)?;
        tx.commit()?;
        sealed
    };
    let existing: Option<String> = db
        .query_row(
            "SELECT body FROM ion_team_workspaces WHERE run_id=?",
            [run.to_string()],
            |r| r.get(0),
        )
        .optional()?;
    let bound_target = existing
        .map(|body| serde_json::from_str::<WorkspaceRecord>(&body))
        .transpose()?;
    ensure!(
        source == plan.record.workspace.source_path
            || bound_target
                .as_ref()
                .is_some_and(|r| source == r.workspace.path
                    && r.workspace.source_path == plan.record.workspace.source_path),
        "Inherited code belongs to a different project folder"
    );
    Ok(Some(plan.record))
}
pub(super) fn reconcile_owner_restart(tx: &Transaction<'_>, now: u64) -> Result<()> {
    let ids: Vec<String> = tx
        .prepare("SELECT run_id FROM ion_team_workspace_handoffs WHERE state='applying'")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for id in ids {
        let run = Uuid::parse_str(&id)?;
        tx.execute("UPDATE ion_team_workspace_handoffs SET state='uncertain' WHERE run_id=? AND state='applying'",[id])?;
        audit(
            tx,
            run,
            None,
            "workspace_input_interrupted",
            "Code installation needs exact file verification; copy effects were not replayed",
            now,
        )?;
        bump(tx, run)?;
    }
    Ok(())
}
fn verify_stages(record: &WorkspaceRecord, run: Uuid, changes: &[TeamSourceChange]) -> Result<()> {
    for (index, change) in changes.iter().enumerate() {
        let path = safe_edit_path(&record.workspace.path, &change.path)?;
        let parent = path.parent().context("Code input parent missing")?;
        for suffix in ["integration", "source-backup"] {
            let stage = parent.join(format!(".ion-edit-{run}-{index}.{suffix}"));
            match fs::symlink_metadata(stage) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error).context("Code input staging metadata is unavailable");
                }
                Ok(_) => {
                    anyhow::bail!("An interrupted code input has a temporary file requiring review")
                }
            }
        }
    }
    Ok(())
}
fn verify_target(target: &WorkspaceRecord, run: Uuid, changes: &[TeamSourceChange]) -> Result<()> {
    integration::verify_changes(target, changes, false)?;
    integration::verify_paths(target, &changes.iter().map(|c| c.path.clone()).collect())?;
    verify_stages(target, run, changes)
}
pub(super) fn apply(
    owner: &TeamCoordinator,
    run: Uuid,
    target: &WorkspaceRecord,
    now: u64,
) -> Result<()> {
    let Some((previous, state, body)) = row(&owner.connect()?, run)? else {
        return Ok(());
    };
    let plan = read(previous, &body.context("Code input was not pinned")?)?;
    ensure!(
        target.repository == plan.record.repository
            && target.workspace.source_path == plan.record.workspace.source_path
            && target.workspace.base_commit == plan.record.workspace.base_commit,
        "Code input base or repository changed"
    );
    if state == "applied" {
        return Ok(());
    }
    verify(target)?;
    ensure!(
        state != "applying",
        "Another owner is installing the inherited code"
    );
    if state == "uncertain" {
        // Adoption is read-only: require the whole exact result and no private
        // staging files. A partial or unknown copy is never performed again.
        verify_target(target,run,&plan.changes)
            .with_context(||format!("Interrupted code handoff at {}; review stage {previous} and restore its exact final files before retrying",target.workspace.path.display()))?;
    } else {
        ensure!(state == "pinned", "Invalid code input installation state");
        // Compare every initial path before reserving any filesystem operation.
        // The fresh worktree is hidden from agents until installation is sealed.
        let mut copy = target.clone();
        copy.workspace.source_path = target.workspace.path.clone();
        for change in &plan.changes {
            let path = integration::prospective_path(&copy, &change.path)?;
            if change.directory {
                ensure!(!path.exists(), "Inherited directory already exists")
            } else {
                compare_file(&path, change.before.as_deref())?
            }
        }
        {
            let mut db = owner.connect()?;
            let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
            active(&load(&tx, run)?, now)?;
            ensure!(tx.execute("UPDATE ion_team_workspace_handoffs SET state='applying' WHERE run_id=? AND state='pinned'",[run.to_string()])? == 1,
                "Code input installation already reserved");
            tx.commit()?;
        }
        let db = owner.connect()?;
        let outcome = (|| -> Result<()> {
            for (index, change) in plan.changes.iter().enumerate() {
                let allowed: bool = db.query_row("SELECT status='active' AND json_extract(body,'$.deadline')>?2 FROM ion_team_runs WHERE id=?1",params![run.to_string(),crate::model::unix_time().max(now)],|r|r.get(0))?;
                ensure!(
                    allowed,
                    "Team stopped or reached its deadline during code installation"
                );
                integration::apply_change(&copy, run, index, change)?;
            }
            verify_target(target, run, &plan.changes)
        })();
        if let Err(error) = outcome {
            let mut db = owner.connect()?;
            let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
            if tx.execute("UPDATE ion_team_workspace_handoffs SET state='uncertain' WHERE run_id=? AND state='applying'",[run.to_string()])? == 1 {
                audit(&tx,run,None,"workspace_input_uncertain","Code installation failed; inspect its recorded workspace before recovery",now)?;
                bump(&tx,run)?;
            }
            tx.commit()?;
            return Err(error).with_context(|| {
                format!(
                    "Code handoff needs review at {}; no agents were dispatched",
                    target.workspace.path.display()
                )
            });
        }
    }
    let mut db = owner.connect()?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    active(&load(&tx, run)?, now)?;
    ensure!(tx.execute("UPDATE ion_team_workspace_handoffs SET state='applied' WHERE run_id=? AND state IN ('applying','uncertain')",[run.to_string()])? == 1,
        "Code input state changed before sealing");
    audit(
        &tx,
        run,
        None,
        "workspace_input_ready",
        &format!(
            "Verified {} inherited paths from {previous}; source unchanged",
            plan.changes.len()
        ),
        now,
    )?;
    bump(&tx, run)?;
    tx.commit()?;
    Ok(())
}
