//! Human-reviewed integration of receipted native changes. Models never choose
//! the source checkout, manifest, approval identity, or application state.
use super::*;
use serde_json::json;
use std::collections::{BTreeMap, HashSet};

pub(super) fn reconcile_owner_restart(tx: &Transaction<'_>, now: u64) -> Result<()> {
    let runs: Vec<String> = tx
        .prepare("SELECT run_id FROM ion_team_integrations WHERE state='applying'")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for id in runs {
        let run = Uuid::parse_str(&id)?;
        tx.execute("UPDATE ion_team_integrations SET state='uncertain',revision=revision+1 WHERE run_id=? AND state='applying'",[id])?;
        audit(
            tx,
            run,
            None,
            "integration_owner_restarted",
            "Source application outcome requires user reconciliation; filesystem effects were not replayed",
            now,
        )?;
        bump(tx, run)?;
    }
    Ok(())
}
fn record(db: &Connection, run: Uuid) -> Result<WorkspaceRecord> {
    let body: String = db.query_row(
        "SELECT body FROM ion_team_workspaces WHERE run_id=? AND state='ready'",
        [run.to_string()],
        |r| r.get(0),
    )?;
    Ok(serde_json::from_str(&body)?)
}
fn read(db: &Connection, id: Uuid) -> Result<TeamIntegration> {
    let (body, state, revision): (String, String, u64) = db.query_row(
        "SELECT body,state,revision FROM ion_team_integrations WHERE id=?",
        [id.to_string()],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let mut plan: TeamIntegration = serde_json::from_str(&body)?;
    plan.status = serde_json::from_value(serde_json::Value::String(state))?;
    plan.revision = revision;
    plan.applied_changes = db.query_row("SELECT count(*) FROM ion_team_integration_steps WHERE integration_id=? AND state='applied'",[id.to_string()],|r|r.get(0))?;
    Ok(plan)
}
fn state(status: TeamIntegrationStatus) -> &'static str {
    match status {
        TeamIntegrationStatus::Ready => "ready",
        TeamIntegrationStatus::Applying => "applying",
        TeamIntegrationStatus::Uncertain => "uncertain",
        TeamIntegrationStatus::Applied => "applied",
        TeamIntegrationStatus::Reverted => "reverted",
    }
}
fn bound_source(record: &WorkspaceRecord, source: &Path) -> Result<PathBuf> {
    let source = fs::canonicalize(source).context("Original project is unavailable")?;
    ensure!(
        source == record.workspace.source_path,
        "Project moved since the run; review its original source binding"
    );
    ensure!(
        canonical_git_root(&source)? == fs::canonicalize(&record.repository)?,
        "Source repository identity changed"
    );
    Ok(source)
}
// Parents created by the reviewed manifest may not exist yet. Every existing
// component must still be a real directory/file within the recorded source.
fn prospective_path(record: &WorkspaceRecord, relative: &str) -> Result<PathBuf> {
    safe_edit_path(&record.workspace.path, relative)?;
    let root = &record.workspace.source_path;
    let mut path = root.clone();
    for component in Path::new(relative).components() {
        let Component::Normal(name) = component else {
            anyhow::bail!("Invalid integration path")
        };
        path.push(name);
        if let Ok(meta) = fs::symlink_metadata(&path) {
            ensure!(
                !meta.file_type().is_symlink(),
                "Source path contains a link"
            );
            ensure!(
                fs::canonicalize(&path)? == path && path.starts_with(root),
                "Source path identity changed"
            );
        }
    }
    Ok(path)
}
fn source_conflicts(record: &WorkspaceRecord, plan: &TeamIntegration) -> Vec<String> {
    let mut errors = Vec::new();
    let outcome = (|| -> Result<()> {
        bound_source(record, &plan.source_path)?;
        require_filter_free(&record.repository)?;
        ensure!(
            git(&record.repository, &["rev-parse", "HEAD"])?.trim() == plan.base_commit,
            "Source commit changed; reconcile against the reviewed base before applying"
        );
        ensure!(
            git(
                &record.repository,
                &["status", "--porcelain=v1", "--untracked-files=all"]
            )?
            .trim()
            .is_empty(),
            "Source has uncommitted changes; commit or stash them before applying"
        );
        Ok(())
    })();
    if let Err(error) = outcome {
        errors.push(error.to_string());
    }
    for change in &plan.changes {
        let outcome = (|| -> Result<()> {
            let path = prospective_path(record, &change.path)?;
            if change.directory {
                ensure!(!path.exists(), "Directory already exists");
            } else {
                compare_file(&path, change.before.as_deref())?;
            }
            Ok(())
        })();
        if let Err(error) = outcome {
            errors.push(format!("{}: {error}", change.path));
        }
    }
    errors
}
fn verify_result(record: &WorkspaceRecord, plan: &TeamIntegration, source: bool) -> Result<()> {
    for change in &plan.changes {
        let path = if source {
            prospective_path(record, &change.path)?
        } else {
            safe_edit_path(&record.workspace.path, &change.path)?
        };
        if change.directory {
            ensure!(
                path.is_dir(),
                "Reviewed directory is missing: {}",
                change.path
            );
        } else {
            compare_file(&path, change.after.as_deref())
                .with_context(|| format!("Reviewed result changed: {}", change.path))?;
        }
    }
    Ok(())
}
fn manifest(db: &Connection, run: Uuid, record: &WorkspaceRecord) -> Result<Vec<TeamSourceChange>> {
    let rows:Vec<(String,String,String)>=db.prepare("SELECT id,signature,state FROM ion_team_workspace_edits WHERE run_id=? ORDER BY rowid LIMIT 257")?.query_map([run.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?.collect::<rusqlite::Result<_>>()?;
    ensure!(rows.len() <= 256, "Integration edit bound exceeded");
    let mut changes: BTreeMap<String, TeamSourceChange> = BTreeMap::new();
    for (id, signature, state) in rows {
        ensure!(
            state == "applied",
            "File effect needs reconciliation before source review"
        );
        let value: serde_json::Value = serde_json::from_str(&signature)?;
        let relative = value["path"].as_str().context("Edit path missing")?;
        let path = safe_edit_path(&record.workspace.path, relative)?;
        let normalized = path
            .strip_prefix(&record.workspace.path)?
            .components()
            .map(|c| c.as_os_str().to_str().context("Path is not UTF-8"))
            .collect::<Result<Vec<_>>>()?
            .join("/");
        let key = if cfg!(windows) {
            normalized.to_lowercase()
        } else {
            normalized.clone()
        };
        let directory = value["operation"] == "createDirectory";
        let text = |field: &str| -> Result<Option<String>> {
            if value[field].is_null() {
                Ok(None)
            } else {
                Ok(Some(
                    value[field]
                        .as_str()
                        .context("Invalid edit content")?
                        .to_owned(),
                ))
            }
        };
        let before = text("old")?;
        let after = text("new")?;
        let id = Uuid::parse_str(&id)?;
        if let Some(change) = changes.get_mut(&key) {
            ensure!(
                !directory && !change.directory && change.after == before,
                "Receipted edit chain changed outside the team"
            );
            change.after = after;
            change.edit_ids.push(id);
        } else {
            changes.insert(
                key,
                TeamSourceChange {
                    path: normalized,
                    directory,
                    before,
                    after,
                    edit_ids: vec![id],
                },
            );
        }
    }
    let all_paths: HashSet<_> = changes.values().map(|c| c.path.clone()).collect();
    // Reject extra staged, unstaged, and untracked changes instead of silently
    // bringing files edited outside the receipted tool boundary into the plan.
    let prefix = record
        .workspace
        .path
        .strip_prefix(&record.tree)?
        .components()
        .map(|c| c.as_os_str().to_str().context("Path is not UTF-8"))
        .collect::<Result<Vec<_>>>()?
        .join("/");
    let status = git(
        &record.tree,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--no-renames",
        ],
    )?;
    for entry in status.split('\0').filter(|s| !s.is_empty()) {
        ensure!(entry.len() >= 3, "Invalid Git change metadata");
        let path = &entry[3..];
        let relative = if prefix.is_empty() {
            path
        } else {
            path.strip_prefix(&format!("{prefix}/"))
                .context("Unreceipted change outside the project")?
        };
        ensure!(
            all_paths.contains(relative),
            "Unreceipted workspace change: {relative}"
        );
    }
    let mut changes: Vec<_> = changes
        .into_values()
        .filter(|c| c.directory || c.before != c.after)
        .collect();
    changes.sort_by_key(|c| {
        (
            !c.directory,
            Path::new(&c.path).components().count(),
            c.path.clone(),
        )
    });
    ensure!(!changes.is_empty(), "No receipted changes remain to apply");
    ensure!(
        serde_json::to_vec(&changes)?.len() <= 16 * 1024 * 1024,
        "Integration exceeds its preview bound"
    );
    Ok(changes)
}
impl TeamCoordinator {
    pub fn source_integration_run(&self, id: Uuid) -> Result<Uuid> {
        Ok(read(&self.connect()?, id)?.run_id)
    }
    /// Reopening a review returns the same immutable manifest and current source
    /// conflicts. Preparing never changes the project or grants model access.
    pub fn prepare_source_integration(
        &self,
        run_id: Uuid,
        expected_revision: u64,
        source: &Path,
        now: u64,
    ) -> Result<TeamIntegration> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, run_id)?;
        ensure!(
            run.revision == expected_revision,
            "Team run changed; refresh before reviewing"
        );
        ensure!(
            run.status == TeamRunStatus::Completed,
            "Accept all team work before reviewing source integration"
        );
        require_settled_effects(&tx, run_id)?;
        let record = record(&tx, run_id)?;
        bound_source(&record, source)?;
        if let Some(id) = tx
            .query_row(
                "SELECT id FROM ion_team_integrations WHERE run_id=?",
                [run_id.to_string()],
                |r| r.get::<_, String>(0),
            )
            .optional()?
        {
            let mut plan = read(&tx, Uuid::parse_str(&id)?)?;
            plan.conflicts = if plan.status == TeamIntegrationStatus::Ready {
                source_conflicts(&record, &plan)
            } else {
                vec![]
            };
            return Ok(plan);
        }
        verify(&record)?;
        let mut plan = TeamIntegration {
            id: Uuid::new_v4(),
            run_id,
            revision: 1,
            source_path: record.workspace.source_path.clone(),
            base_commit: record.workspace.base_commit.clone(),
            branch: record.workspace.branch.clone(),
            status: TeamIntegrationStatus::Ready,
            changes: manifest(&tx, run_id, &record)?,
            conflicts: vec![],
            applied_changes: 0,
            created_at: now,
        };
        verify_result(&record, &plan, false)?;
        tx.execute("INSERT INTO ion_team_integrations(id,run_id,repository,state,revision,body) VALUES(?,?,?,'ready',1,?)",params![plan.id.to_string(),run_id.to_string(),git_path(&record.repository)?,serde_json::to_string(&plan)?])?;
        audit(
            &tx,
            run_id,
            None,
            "integration_review_prepared",
            &format!(
                "{} exact source changes; review {}",
                plan.changes.len(),
                plan.id
            ),
            now,
        )?;
        bump(&tx, run_id)?;
        tx.commit()?;
        plan.conflicts = source_conflicts(&record, &plan);
        Ok(plan)
    }
    /// This entry point is exclusively a human RPC action. A durable reservation
    /// precedes the first filesystem effect; unknown outcomes cannot be replayed.
    pub fn apply_source_integration(
        &self,
        id: Uuid,
        expected_revision: u64,
        source: &Path,
        now: u64,
    ) -> Result<TeamIntegration> {
        let (plan, record) = {
            let mut db = self.connect()?;
            let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let plan = read(&tx, id)?;
            let record = record(&tx, plan.run_id)?;
            bound_source(&record, source)?;
            if plan.status == TeamIntegrationStatus::Applied {
                return Ok(plan);
            }
            ensure!(
                plan.revision == expected_revision,
                "Source review changed; reopen before applying"
            );
            ensure!(
                plan.status == TeamIntegrationStatus::Ready,
                "Source application needs reconciliation; it was not repeated"
            );
            ensure!(
                load(&tx, plan.run_id)?.status == TeamRunStatus::Completed,
                "Team is no longer completed"
            );
            require_settled_effects(&tx, plan.run_id)?;
            verify(&record)?;
            verify_result(&record, &plan, false)?;
            ensure!(
                manifest(&tx, plan.run_id, &record)? == plan.changes,
                "Team workspace change set drifted after review"
            );
            let errors = source_conflicts(&record, &plan);
            ensure!(errors.is_empty(), "Source conflicts: {}", errors.join("; "));
            let busy:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM ion_team_integrations WHERE repository=? AND state IN ('applying','uncertain'))",[git_path(&record.repository)?],|r|r.get(0))?;
            ensure!(
                !busy,
                "Another source application needs reconciliation in this repository"
            );
            tx.execute(
                "UPDATE ion_team_integrations SET state='applying',revision=revision+1 WHERE id=?",
                [id.to_string()],
            )?;
            audit(
                &tx,
                plan.run_id,
                None,
                "integration_approved",
                &format!("User approved exact review {id} revision {expected_revision}"),
                now,
            )?;
            bump(&tx, plan.run_id)?;
            tx.commit()?;
            (plan, record)
        };
        let outcome = (|| -> Result<()> {
            for (index, change) in plan.changes.iter().enumerate() {
                // Start is committed separately from effect acknowledgement.
                self.connect()?.execute("INSERT INTO ion_team_integration_steps(integration_id,step,state) VALUES(?,?,'pending')",params![id.to_string(),index])?;
                apply_change(&record, id, index, change)?;
                self.connect()?.execute("UPDATE ion_team_integration_steps SET state='applied',receipt=? WHERE integration_id=? AND step=? AND state='pending'",params![json!({"path":change.path,"status":"applied","before_saved_in_review":true}).to_string(),id.to_string(),index])?;
            }
            verify_result(&record, &plan, true)?;
            Ok(())
        })();
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let status = if outcome.is_ok() {
            TeamIntegrationStatus::Applied
        } else {
            TeamIntegrationStatus::Uncertain
        };
        tx.execute(
            "UPDATE ion_team_integrations SET state=?,revision=revision+1 WHERE id=?",
            params![state(status), id.to_string()],
        )?;
        audit(
            &tx,
            plan.run_id,
            None,
            if outcome.is_ok() {
                "integration_applied"
            } else {
                "integration_uncertain"
            },
            &outcome.err().map(|e| e.to_string()).unwrap_or_else(|| {
                format!("Review {id} applied to source; files remain uncommitted")
            }),
            now,
        )?;
        bump(&tx, plan.run_id)?;
        tx.commit()?;
        let mut result = read(&db, id)?;
        if status == TeamIntegrationStatus::Uncertain {
            result.conflicts.push("Application stopped after a reserved effect. Inspect source and private .ion-edit stages, then reconcile; Apply will not replay it.".into());
        }
        Ok(result)
    }
    /// Explicit human confirmation after inspecting a partial/interrupted apply.
    /// Reads and verifies every final byte; it never repeats filesystem effects.
    pub fn reconcile_source_integration(
        &self,
        id: Uuid,
        expected_revision: u64,
        keep_changes: bool,
        source: &Path,
        now: u64,
    ) -> Result<TeamIntegration> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let plan = read(&tx, id)?;
        let record = record(&tx, plan.run_id)?;
        bound_source(&record, source)?;
        if plan.status == TeamIntegrationStatus::Applied
            || plan.status == TeamIntegrationStatus::Reverted
        {
            ensure!(
                keep_changes == (plan.status == TeamIntegrationStatus::Applied),
                "Recorded reconciliation chose a different outcome"
            );
            return Ok(plan);
        }
        ensure!(
            plan.revision == expected_revision,
            "Review changed; reopen before reconciliation"
        );
        ensure!(
            plan.status == TeamIntegrationStatus::Uncertain,
            "Application is still running or has not started"
        );
        ensure!(
            git(&record.repository, &["rev-parse", "HEAD"])?.trim() == plan.base_commit,
            "Source commit changed during application; inspect its history before reconciliation"
        );
        if keep_changes {
            verify_result(&record, &plan, true)?;
        } else {
            for change in &plan.changes {
                let path = prospective_path(&record, &change.path)?;
                if change.directory {
                    ensure!(
                        !path.exists(),
                        "Created source directory remains: {}",
                        change.path
                    );
                } else {
                    compare_file(&path, change.before.as_deref()).with_context(|| {
                        format!("Original source is not restored: {}", change.path)
                    })?;
                }
            }
        }

        for (index, change) in plan.changes.iter().enumerate() {
            let path = prospective_path(&record, &change.path)?;
            for suffix in ["integration", "source-backup"] {
                ensure!(
                    !path
                        .with_file_name(format!(".ion-edit-{id}-{index}.{suffix}"))
                        .exists(),
                    "Private source stage remains; inspect and preserve its contents before clearing it"
                );
            }
        }
        let status = if keep_changes {
            TeamIntegrationStatus::Applied
        } else {
            TeamIntegrationStatus::Reverted
        };
        for (index, change) in plan.changes.iter().enumerate() {
            let receipt=json!({"path":change.path,"status":if keep_changes {"confirmed_by_user"}else{"source_restored_by_user"},"before_saved_in_review":true}).to_string();
            tx.execute("INSERT INTO ion_team_integration_steps(integration_id,step,state,receipt) VALUES(?,?,?,?) ON CONFLICT(integration_id,step) DO UPDATE SET state=excluded.state,receipt=excluded.receipt",params![id.to_string(),index,state(status),receipt])?;
        }
        tx.execute(
            "UPDATE ion_team_integrations SET state=?,revision=revision+1 WHERE id=?",
            params![state(status), id.to_string()],
        )?;
        audit(
            &tx,
            plan.run_id,
            None,
            if keep_changes {
                "integration_reconciled"
            } else {
                "integration_reverted"
            },
            &format!(
                "User confirmed {} for exact review {id}",
                if keep_changes {
                    "final contents"
                } else {
                    "restored original source"
                }
            ),
            now,
        )?;
        bump(&tx, plan.run_id)?;
        tx.commit()?;
        read(&db, id)
    }
}
fn apply_change(
    record: &WorkspaceRecord,
    id: Uuid,
    index: usize,
    change: &TeamSourceChange,
) -> Result<()> {
    let path = prospective_path(record, &change.path)?;
    // The parent now exists only because its earlier reviewed directory step did.
    let path = safe_edit_path(&record.workspace.source_path, &change.path).and_then(|p| {
        ensure!(p == path, "Source path changed");
        Ok(p)
    })?;
    if change.directory {
        fs::create_dir(&path)?;
        return Ok(());
    }
    compare_file(&path, change.before.as_deref())?;
    let stage = path.with_file_name(format!(".ion-edit-{id}-{index}.integration"));
    let backup = path.with_file_name(format!(".ion-edit-{id}-{index}.source-backup"));
    if let Some(after) = &change.after {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&stage)?;
        file.write_all(after.as_bytes())?;
        file.sync_all()?;
        if path.exists() {
            fs::set_permissions(&stage, fs::metadata(&path)?.permissions())?;
        }
    }
    if let Some(before) = &change.before {
        ensure!(
            !fs::metadata(&path)?.permissions().readonly(),
            "Source file is read-only"
        );
        drop(
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&backup)?,
        );
        fs::rename(&path, &backup)?;
        // Preserve concurrent unexpected content in the private backup, with a
        // pending step. Never overwrite the surprise edit or claim success.
        compare_file(&backup, Some(before))?;
    }
    if change.after.is_some() {
        fs::hard_link(&stage, &path)?;
        fs::remove_file(&stage)?;
    } else {
        ensure!(!path.exists(), "Deleted source file reappeared");
    }
    compare_file(&path, change.after.as_deref())?;
    if change.before.is_some() {
        fs::remove_file(&backup)?;
    }
    Ok(())
}
