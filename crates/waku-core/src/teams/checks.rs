//! Executed command checks use original trusted receipts and current source witnesses.
//! Runs only on owner workers; public projections contain names and request IDs.
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, io::Read, path::Path};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CommandCheckContext {
    path: Option<PathBuf>,
    epoch: String,
}
pub(super) fn validate_contract(contract: &OutputContract) -> Result<()> {
    ensure!(
        contract.required_commands.len() <= 16 && serde_json::to_vec(contract)?.len() <= 32 * 1024,
        "Required command contract exceeds its count/context bound"
    );
    let mut names = HashSet::new();
    for check in &contract.required_commands {
        text(&check.name, 256, "Command check name")?;
        ensure!(
            names.insert(&check.name),
            "Duplicate required command check"
        );
        ensure!(
            (1..=64).contains(&check.argv.len())
                && check
                    .argv
                    .iter()
                    .all(|v| v.len() <= 8192 && !v.contains('\0'))
                && check.argv.iter().map(String::len).sum::<usize>() <= 32 * 1024
                && check.argv[0].len() <= 512
                && !check.argv[0].chars().any(char::is_control)
                && check.argv[0].starts_with('/')
                && !check.argv[0].contains(['\\', ':'])
                && check.argv[0]
                    .split('/')
                    .skip(1)
                    .all(|s| !s.is_empty() && !matches!(s, "." | ".."))
                && (1..=120).contains(&check.timeout_seconds),
            "Required command arguments/time invalid"
        );
        for paths in [&check.input_paths, &check.output_paths] {
            ensure!(paths.len() <= 32, "Too many required check files");
            let mut seen = HashSet::new();
            for path in paths {
                jobs::path(path)?;
                ensure!(
                    seen.insert(path.to_ascii_lowercase()),
                    "Duplicate required check path"
                );
            }
        }
    }
    Ok(())
}
pub(super) fn validate_references(result: &AssignmentResult) -> Result<()> {
    ensure!(
        result.executed_checks.len() <= 16,
        "Too many executed command references"
    );
    let mut names = HashSet::new();
    let mut requests = HashSet::new();
    for check in &result.executed_checks {
        text(&check.name, 256, "Executed check name")?;
        ensure!(
            !check.request_id.is_nil()
                && names.insert(&check.name)
                && requests.insert(check.request_id),
            "Executed command references are empty or duplicated"
        );
    }
    Ok(())
}
fn epoch(db: &Connection, run: Uuid) -> Result<String> {
    let rows: Vec<(String, String)> = db
        .prepare(
            "SELECT id,state FROM ion_team_workspace_edits WHERE run_id=? ORDER BY rowid LIMIT 257",
        )?
        .query_map([run.to_string()], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    ensure!(
        rows.len() <= 256 && rows.iter().all(|(_, state)| state != "pending"),
        "Check source has unknown edits or exceeds its bound"
    );
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(&rows)?)))
}
fn context(
    db: &Connection,
    run: &TeamRunSnapshot,
    work: &TeamAssignment,
) -> Result<CommandCheckContext> {
    if work
        .output_contract
        .required_commands
        .iter()
        .any(|c| !c.input_paths.is_empty())
    {
        ensure!(
            run.workspace.is_some(),
            "Executed project checks require a sealed isolated run workspace"
        );
    }
    let path = run.workspace.as_ref().map(|w| w.path.clone());
    if let Some(root) = &path {
        ensure!(
            std::fs::canonicalize(root)? == *root && std::fs::symlink_metadata(root)?.is_dir(),
            "Command check workspace was redirected or removed"
        );
    }
    Ok(CommandCheckContext {
        path,
        epoch: epoch(db, run.id)?,
    })
}
impl TeamCoordinator {
    /// Capture before reading/bundling files. The same witness is rechecked before POST.
    pub(crate) fn command_check_context(
        &self,
        lease: &AssignmentLease,
        source: &Path,
        now: u64,
    ) -> Result<Option<CommandCheckContext>> {
        let db = self.connect()?;
        let run = load(&db, lease.run_id)?;
        active(&run, now)?;
        let work = checked_lease(&run, lease, now, false)?;
        if work.output_contract.required_commands.is_empty() {
            return Ok(None);
        }
        workspace::require_settled_effects(&db, run.id)?;
        let value = context(&db, &run, work)?;
        if let Some(path) = &value.path {
            ensure!(
                std::fs::canonicalize(source)? == *path,
                "Command check workspace differs from its isolated run"
            );
        }
        Ok(Some(value))
    }
}
pub(super) fn verify_context(
    db: &Connection,
    run: &TeamRunSnapshot,
    work: &TeamAssignment,
    saved: Option<&CommandCheckContext>,
) -> Result<()> {
    if work.output_contract.required_commands.is_empty() {
        ensure!(saved.is_none(), "Unexpected command check context");
        return Ok(());
    }
    ensure!(
        saved == Some(&context(db, run, work)?),
        "Command check source changed or its original context is missing"
    );
    Ok(())
}
pub(super) fn verify(
    db: &Connection,
    run: &TeamRunSnapshot,
    work: &TeamAssignment,
    result: &AssignmentResult,
) -> Result<()> {
    validate_references(result)?;
    if work.output_contract.required_commands.is_empty() {
        ensure!(
            result.executed_checks.is_empty(),
            "Executed checks must refer to pinned command requirements"
        );
        return Ok(());
    }
    ensure!(
        result.executed_checks.len() == work.output_contract.required_commands.len(),
        "Required executed command receipts are missing"
    );
    let attempt = work.attempts.last().context("Check attempt missing")?;
    ensure!(
        attempt.environment == ExecutionEnvironment::NebiusSandbox,
        "Executed remote checks cannot use a Local/fallback attempt"
    );
    workspace::require_settled_effects(db, run.id)?;
    let mut files: HashMap<String, Vec<u8>> = HashMap::new();
    for check in &result.executed_checks {
        let required = work
            .output_contract
            .required_commands
            .iter()
            .find(|c| c.name == check.name)
            .context("Executed check name is not required by this assignment")?;
        let (request,receipt,signature):(String,String,String)=db.query_row("SELECT request,receipt,signature FROM ion_team_jobs WHERE request_id=? AND run_id=? AND work_id=? AND attempt_id=? AND state='settled' AND operation IS NOT NULL",
            params![check.request_id.to_string(),run.id.to_string(),work.id.to_string(),attempt.id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).context("Check receipt is missing, unfinished or belongs to another assignment/attempt")?;
        let request: jobs::JobRequest = serde_json::from_str(&request)?;
        let receipt: jobs::JobResponse = serde_json::from_str(&receipt)?;
        receipt.validate_against(&request)?;
        ensure!(
            request.request_id == check.request_id
                && request.argv == required.argv
                && request.timeout_seconds == required.timeout_seconds
                && request.inputs.iter().map(|f| &f.path).collect::<Vec<_>>()
                    == required.input_paths.iter().collect::<Vec<_>>()
                && request.output_paths == required.output_paths,
            "Executed command differs from its pinned check"
        );
        ensure!(
            receipt.exit_code == 0
                && !receipt.timed_out
                && !receipt.canceled
                && receipt.output_errors.is_empty(),
            "Required command check failed, timed out, was canceled or lacks outputs"
        );
        let signature: serde_json::Value = serde_json::from_str(&signature)?;
        let saved: CommandCheckContext = serde_json::from_value(
            signature
                .get("check_context")
                .cloned()
                .context("Legacy command has no original check context")?,
        )?;
        verify_context(db, run, work, Some(&saved))?;
        let mut total = 0;
        for input in &request.inputs {
            jobs::path(&input.path)?;
            let root = saved
                .path
                .as_ref()
                .context("Project check source witness missing")?;
            if !files.contains_key(&input.path) {
                let path = workspace::safe_edit_path(root, &input.path)?;
                let meta = std::fs::symlink_metadata(&path)?;
                ensure!(
                    meta.is_file() && !meta.file_type().is_symlink() && meta.len() <= 64 * 1024,
                    "Check source is no longer a bounded regular file"
                );
                let mut bytes = Vec::new();
                std::fs::File::open(path)?
                    .take(64 * 1024 + 1)
                    .read_to_end(&mut bytes)?;
                ensure!(bytes.len() <= 64 * 1024, "Check source exceeded its bound");
                files.insert(input.path.clone(), bytes);
            }
            let actual = &files[&input.path];
            total += actual.len();
            ensure!(
                total <= 256 * 1024 && STANDARD.decode(&input.content_base64)? == *actual,
                "Source changed after the command check; run the check again against current files"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn command() -> RequiredCommandCheck {
        RequiredCommandCheck {
            name: "Unit tests".into(),
            argv: vec!["/usr/bin/python3".into(), "check.py".into()],
            input_paths: vec!["check.py".into()],
            output_paths: vec![],
            timeout_seconds: 10,
        }
    }
    #[test]
    fn command_check_contract_rejects_unbounded_or_redirected_requirements() {
        for mutation in 0..10 {
            let mut command = command();
            match mutation {
                0 => command.name.clear(),
                1 => command.argv.clear(),
                2 => command.argv[0] = "python3".into(),
                3 => command.argv[0] = "/usr/../bin/python3".into(),
                4 => command.argv.push("\0".into()),
                5 => command.timeout_seconds = 121,
                6 => command.input_paths.push("CHECK.py".into()),
                7 => command.input_paths = vec![".env".into()],
                8 => command.output_paths = vec!["../other".into()],
                _ => command.argv = vec!["/bin/python".into(); 65],
            }
            assert!(
                validate_contract(&OutputContract {
                    required_commands: vec![command],
                    ..Default::default()
                })
                .is_err()
            );
        }
        assert!(
            validate_contract(&OutputContract {
                required_commands: vec![command(), command()],
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            validate_contract(&OutputContract {
                required_commands: vec![command(); 17],
                ..Default::default()
            })
            .is_err()
        );
        let mut large = command();
        large
            .argv
            .extend(["x".repeat(8192), "y".repeat(8192), "z".repeat(8192)]);
        let mut other = large.clone();
        other.name = "Other check".into();
        assert!(
            validate_contract(&OutputContract {
                required_commands: vec![large, other],
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            validate_contract(&OutputContract {
                required_commands: vec![command()],
                ..Default::default()
            })
            .is_ok()
        );
    }
    #[test]
    fn command_check_empty_extension_preserves_existing_serialized_contracts_and_results() {
        let contract =
            serde_json::json!({"minimumDocuments":0,"requiredFormats":[],"requiredChecks":[]});
        let restored: OutputContract = serde_json::from_value(contract.clone()).unwrap();
        assert!(restored.required_commands.is_empty());
        assert_eq!(serde_json::to_value(restored).unwrap(), contract);
        let result =
            serde_json::json!({"summary":"Reviewed","artifactIds":[],"checks":["Source audit"]});
        let restored: AssignmentResult = serde_json::from_value(result.clone()).unwrap();
        assert!(restored.executed_checks.is_empty());
        assert_eq!(serde_json::to_value(restored).unwrap(), result);
    }
}
