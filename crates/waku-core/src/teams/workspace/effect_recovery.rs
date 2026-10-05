//! Inspect and acknowledge a stopped worker's exact reserved file effect.
//! Reconciliation reads files; it does not replay or clean up filesystem effects.
use super::*;
use serde_json::{Value, json};
pub(in crate::teams) fn files(
    db: &Connection,
    run: Uuid,
    work: Uuid,
    attempt: Uuid,
) -> Result<Vec<TeamFileRecovery>> {
    let rows:Vec<(String,String)>=db.prepare("SELECT id,signature FROM ion_team_workspace_edits WHERE run_id=? AND work_id=? AND attempt_id=? AND state='pending' ORDER BY rowid LIMIT 257")?.query_map(params![run.to_string(),work.to_string(),attempt.to_string()],|r|Ok((r.get(0)?,r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    ensure!(rows.len() <= 256, "Too many file recovery records");
    rows.into_iter()
        .map(|(id, signature)| {
            let id = Uuid::parse_str(&id)?;
            let value: Value = serde_json::from_str(&signature)?;
            let operation = value["operation"]
                .as_str()
                .unwrap_or("writeFile")
                .to_owned();
            let text = |key: &str| -> Result<Option<String>> {
                if value[key].is_null() {
                    Ok(None)
                } else {
                    Ok(Some(
                        value[key]
                            .as_str()
                            .context("Invalid file effect content")?
                            .into(),
                    ))
                }
            };
            let path = value["path"]
                .as_str()
                .context("Missing effect path")?
                .to_owned();
            let parent = Path::new(&path).parent().unwrap_or_else(|| Path::new(""));
            let stages = match operation.as_str() {
                "writeFile" => vec![parent.join(format!(".ion-edit-{id}.pending"))],
                "deleteFile" => vec![parent.join(format!(".ion-edit-{id}.deleted"))],
                "createDirectory" => vec![],
                _ => anyhow::bail!("Unknown file effect operation"),
            };
            Ok(TeamFileRecovery {
                id,
                path,
                operation,
                before: text("old")?,
                after: text("new")?,
                private_stages: stages
                    .into_iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect(),
            })
        })
        .collect()
}
pub(in crate::teams) fn confirm(
    tx: &Transaction<'_>,
    run: Uuid,
    work: Uuid,
    attempt: Uuid,
    id: Uuid,
    applied: bool,
    now: u64,
) -> Result<()> {
    let effect = files(tx, run, work, attempt)?
        .into_iter()
        .find(|e| e.id == id)
        .context("Pending file effect does not belong to this stopped attempt")?;
    let body: String = tx.query_row(
        "SELECT body FROM ion_team_workspaces WHERE run_id=? AND state='ready'",
        [run.to_string()],
        |r| r.get(0),
    )?;
    let record: WorkspaceRecord = serde_json::from_str(&body)?;
    verify(&record)?;
    let path = safe_edit_path(&record.workspace.path, &effect.path)?;
    if effect.operation == "createDirectory" {
        if applied {
            ensure!(
                path.is_dir() && fs::canonicalize(&path)? == path,
                "Recorded directory creation is not present"
            );
        } else {
            ensure!(
                matches!(fs::symlink_metadata(&path),Err(error) if error.kind()==std::io::ErrorKind::NotFound),
                "Created directory is still present or inaccessible"
            );
        }
    } else {
        let expected = if applied {
            effect.after.as_deref()
        } else {
            effect.before.as_deref()
        };
        if expected.is_none() {
            ensure!(
                matches!(fs::symlink_metadata(&path),Err(error) if error.kind()==std::io::ErrorKind::NotFound),
                "File is still present or inaccessible"
            );
        }
        compare_file(
            &path,
            if applied {
                effect.after.as_deref()
            } else {
                effect.before.as_deref()
            },
        )
        .context("File does not match the selected recovery outcome")?;
    }
    for stage in &effect.private_stages {
        // Validate reserved private names separately; public edit paths forbid them.
        let stage = record.workspace.path.join(stage);
        let parent = fs::canonicalize(stage.parent().context("Stage parent missing")?)?;
        ensure!(
            parent.starts_with(&record.workspace.path),
            "Recovery stage escaped its workspace"
        );
        ensure!(
            matches!(fs::symlink_metadata(&stage),Err(error) if error.kind()==std::io::ErrorKind::NotFound),
            "Private recovery stage remains; inspect and preserve its bytes before clearing it"
        );
    }
    let state = if applied { "applied" } else { "restored" };
    let receipt=json!({"edit_id":id,"path":effect.path,"operation":effect.operation,"status":state,"confirmed_by":"user","workspace":"isolated_team_run"}).to_string();
    ensure!(tx.execute("UPDATE ion_team_workspace_edits SET state=?,receipt=? WHERE id=? AND run_id=? AND work_id=? AND attempt_id=? AND state='pending'",params![state,receipt,id.to_string(),run.to_string(),work.to_string(),attempt.to_string()])?==1,"File effect changed during reconciliation");
    audit(
        tx,
        run,
        Some(work),
        "file_effect_reconciled",
        &format!("{id}: user confirmed {state}"),
        now,
    )?;
    Ok(())
}
