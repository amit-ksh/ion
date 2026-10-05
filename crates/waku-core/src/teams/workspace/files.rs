//! Structural changes in the isolated coding workspace share the exact file
//! receipt boundary. No recursive deletion or unchecked filesystem targets.
use super::*;
use serde_json::json;

fn recheck_path(path: &Path) -> Result<()> {
    ensure!(
        safe_edit_path(
            path.parent().context("File parent missing")?,
            path.file_name()
                .context("File name missing")?
                .to_str()
                .context("Path is not UTF-8")?
        )? == path,
        "File path changed"
    );
    Ok(())
}
impl TeamCoordinator {
    /// One directory per call. Parents must already exist, so each step has its
    /// own exclusive creation and durable receipt rather than recursive mutation.
    pub fn create_workspace_directory(
        &self,
        lease: &AssignmentLease,
        request: Uuid,
        relative: &str,
        now: u64,
    ) -> Result<String> {
        let signature = json!({"path":relative,"operation":"createDirectory"}).to_string();
        let (id, path) = match self.reserve_workspace_edit(
            lease,
            request,
            relative,
            &signature,
            |p| {
                ensure!(!p.exists(), "Directory already exists");
                Ok(())
            },
            now,
        )? {
            ReservedWorkspaceEdit::Receipt(receipt) => return Ok(receipt),
            ReservedWorkspaceEdit::Pending { id, path } => (id, path),
        };
        let outcome = (|| -> Result<()> {
            self.renew(lease, crate::model::unix_time().max(now), 180)?;
            recheck_path(&path)?;
            fs::create_dir(&path)?;
            ensure!(
                fs::symlink_metadata(&path)?.is_dir() && fs::canonicalize(&path)? == path,
                "Created directory path changed"
            );
            Ok(())
        })();
        outcome.with_context(|| {
            format!(
                "Directory creation requires workspace reconciliation ({id}); it was not repeated"
            )
        })?;
        self.finish_workspace_edit(lease,id,relative,json!({"edit_id":id,"path":relative,"operation":"createDirectory","status":"applied","workspace":"isolated_team_run"}).to_string(),now)
    }
    /// Remove a bounded UTF-8 file, preserving its exact bytes in the durable
    /// edit signature. An unexpected removed file stays at the private stage.
    pub fn delete_workspace_file(
        &self,
        lease: &AssignmentLease,
        request: Uuid,
        relative: &str,
        old: &str,
        now: u64,
    ) -> Result<String> {
        ensure!(
            old.len() <= 16 * 1024 && serde_json::to_vec(old)?.len() <= 24 * 1024,
            "Delete content exceeds the edit bound"
        );
        let signature =
            json!({"path":relative,"operation":"deleteFile","old":old,"new":null}).to_string();
        let (id, path) = match self.reserve_workspace_edit(
            lease,
            request,
            relative,
            &signature,
            |p| {
                compare_file(p, Some(old))?;
                ensure!(
                    !fs::metadata(p)?.permissions().readonly(),
                    "Read-only files cannot be removed by the team"
                );
                Ok(())
            },
            now,
        )? {
            ReservedWorkspaceEdit::Receipt(receipt) => return Ok(receipt),
            ReservedWorkspaceEdit::Pending { id, path } => (id, path),
        };
        let backup = path.with_file_name(format!(".ion-edit-{id}.deleted"));
        let outcome = (|| -> Result<()> {
            self.renew(lease, crate::model::unix_time().max(now), 180)?;
            recheck_path(&path)?;
            compare_file(&path, Some(old))?;
            // Reserve the backup name without following/overwriting an existing link.
            drop(
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&backup)?,
            );
            fs::rename(&path, &backup)?;
            // Never destroy the removed bytes, including a concurrent user edit.
            compare_file(&backup, Some(old))?;
            ensure!(
                !path.exists(),
                "File reappeared during deletion; inspect the saved backup"
            );
            // The committed reservation already stores these exact bytes in
            // signature.old. Keep unexpected content above; remove a matching
            // stage so recovery bookkeeping never contaminates the Git diff.
            fs::remove_file(&backup)?;
            Ok(())
        })();
        outcome.with_context(||format!("File deletion requires workspace reconciliation ({id}); inspect its private .ion-edit backup before recovery"))?;
        self.finish_workspace_edit(lease,id,relative,json!({"edit_id":id,"path":relative,"operation":"deleteFile","bytes":old.len(),"status":"applied","workspace":"isolated_team_run","backup":"durable_edit_receipt"}).to_string(),now)
    }
}
