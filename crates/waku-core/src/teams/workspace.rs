//! One durable Git workspace per coding run. Writers are serialized and file
//! effects have receipts; an interrupted effect is never performed a second time.
use super::*;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path},
    sync::atomic::AtomicBool,
    time::Duration,
};

enum ReservedWorkspaceEdit {
    Receipt(String),
    Pending { id: Uuid, path: PathBuf },
}
pub(super) mod effect_recovery;
mod files;
mod integration;
pub(super) fn reconcile_integrations(tx: &Transaction<'_>, now: u64) -> Result<()> {
    integration::reconcile_owner_restart(tx, now)
}

#[derive(Serialize, Deserialize)]
struct WorkspaceRecord {
    workspace: TeamWorkspace,
    repository: PathBuf,
    tree: PathBuf,
}
pub(super) fn projection(db: &Connection, run: Uuid) -> Result<Option<TeamWorkspace>> {
    let row: Option<String> = db
        .query_row(
            "SELECT body FROM ion_team_workspaces WHERE run_id=? AND state='ready'",
            [run.to_string()],
            |r| r.get(0),
        )
        .optional()?;
    row.map(|s| Ok(serde_json::from_str::<WorkspaceRecord>(&s)?.workspace))
        .transpose()
}
pub(super) fn require_settled_effects(db: &Connection, run: Uuid) -> Result<()> {
    require_settled_effects_except_gpu(db, run, None)
}
/// Only suspension of its already bound GPU request may omit that one effect.
/// Unknown acknowledgements and unrelated file/app/command effects stay fenced.
pub(super) fn require_settled_effects_except_gpu(
    db: &Connection,
    run: Uuid,
    request: Option<Uuid>,
) -> Result<()> {
    let pending: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM ion_team_workspace_edits WHERE run_id=?1 AND state='pending') OR EXISTS(SELECT 1 FROM ion_team_app_effects WHERE run_id=?1 AND state='pending') OR EXISTS(SELECT 1 FROM ion_team_jobs WHERE run_id=?1 AND state IN ('staging','submitting','running','canceling')) OR EXISTS(SELECT 1 FROM ion_team_gpu_jobs WHERE run_id=?1 AND state NOT IN ('settled','cleaned') AND (?2 IS NULL OR request_id!=?2))",
        params![run.to_string(),request.map(|id| id.to_string())], |r| r.get(0))?;
    ensure!(
        !pending,
        "A started effect needs reconciliation before further work"
    );
    Ok(())
}
pub(super) fn check_writer_capacity(
    tx: &Transaction<'_>,
    run: Uuid,
    access: &AgentAccess,
) -> Result<()> {
    if access.workspace_write {
        let busy: bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM ion_team_work WHERE run_id=? AND status IN ('running','canceling','uncertain') AND json_extract(body,'$.access.workspaceWrite')=1) OR EXISTS(SELECT 1 FROM ion_team_workspace_edits WHERE run_id=? AND state='pending')",params![run.to_string(),run.to_string()],|r|r.get(0))?;
        ensure!(
            !busy,
            "A team writer is active or its file effect needs reconciliation"
        );
    }
    Ok(())
}
// Fixed Git operations only. Disable hooks/fsmonitor/global configuration and
// reject local checkout filters before allowing worktree creation.
pub(super) fn git(repository: &Path, args: &[&str]) -> Result<String> {
    let mut command = crate::command_env::plain_command("git");
    command
        .args([
            "-c",
            if cfg!(windows) {
                "core.hooksPath=NUL"
            } else {
                "core.hooksPath=/dev/null"
            },
            "-c",
            "core.fsmonitor=false",
            "-c",
            "submodule.recurse=false",
        ])
        .args(args)
        .current_dir(repository)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0");
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_")
            && !matches!(
                key.to_str(),
                Some(
                    "GIT_CONFIG_NOSYSTEM"
                        | "GIT_CONFIG_GLOBAL"
                        | "GIT_TERMINAL_PROMPT"
                        | "GIT_OPTIONAL_LOCKS"
                )
            )
        {
            command.env_remove(key);
        }
    }
    let output = crate::team_process::bounded_process(
        &mut command,
        b"",
        &AtomicBool::new(false),
        Duration::from_secs(30),
    )
    .with_context(|| {
        format!(
            "Teams Git operation {} failed; review workspace setup",
            args.first().copied().unwrap_or("unknown")
        )
    })?;
    Ok(String::from_utf8(output).context("Git workspace metadata is not UTF-8")?)
}
fn git_path(path: &Path) -> Result<String> {
    let path = path.to_str().context("Git workspace path is not UTF-8")?;
    // Git for Windows rejects extended paths when allocating worktree directories.
    #[cfg(windows)]
    let path = if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        path.strip_prefix(r"\\?\").unwrap_or(path).to_owned()
    };
    Ok(path.replace('\\', "/"))
}
fn canonical_git_root(path: &Path) -> Result<PathBuf> {
    Ok(fs::canonicalize(
        git(path, &["rev-parse", "--show-toplevel"])?.trim(),
    )?)
}
fn require_filter_free(repository: &Path) -> Result<()> {
    let config = git(repository, &["config", "--null", "--list", "--includes"])?;
    ensure!(
        !config.split('\0').any(|line| line.starts_with("filter.")
            && (line.contains(".smudge\n")
                || line.contains(".process\n")
                || line.contains(".clean\n"))),
        "Teams Git inspection requires a project without custom clean/smudge/process filters"
    );
    Ok(())
}
fn verify(record: &WorkspaceRecord) -> Result<()> {
    require_filter_free(&record.repository)?;
    require_filter_free(&record.tree)?;
    ensure!(
        record.workspace.path.is_dir(),
        "Team worktree is missing; restore it before continuing"
    );
    ensure!(
        canonical_git_root(&record.workspace.path)? == fs::canonicalize(&record.tree)?,
        "Team workspace is no longer its recorded Git worktree"
    );
    ensure!(
        git(&record.tree, &["symbolic-ref", "--quiet", "HEAD"])?.trim()
            == format!("refs/heads/{}", record.workspace.branch),
        "Team worktree branch changed; review before continuing"
    );
    ensure!(
        git(&record.tree, &["rev-parse", "HEAD"])?.trim() == record.workspace.base_commit,
        "Team worktree base changed; review before continuing"
    );
    let common = |path: &Path| -> Result<PathBuf> {
        let value = git(path, &["rev-parse", "--git-common-dir"])?;
        Ok(fs::canonicalize(path.join(value.trim()))?)
    };
    ensure!(
        common(&record.tree)? == common(&record.repository)?,
        "Team worktree belongs to a different repository"
    );
    ensure!(
        fs::canonicalize(&record.workspace.path)? == record.workspace.path,
        "Team workspace path changed"
    );
    Ok(())
}
impl TeamCoordinator {
    /// Provision before dispatch, with a deterministic reserved identity. Source
    /// changes must be committed first; we never omit or reset uncommitted work.
    /// Existing ready workspaces retain their original base when the source changes.
    pub fn prepare_local_workspace(
        &self,
        run_id: Uuid,
        source: &Path,
        now: u64,
    ) -> Result<PathBuf> {
        let source = fs::canonicalize(source).context("Project workspace is unavailable")?;
        let run = self.snapshot(run_id)?;
        active(&run, now)?;
        if !run.access.workspace_write || !run.profiles.iter().any(|p| p.access.workspace_write) {
            return Ok(source);
        }
        let existing: Option<(String, String)> = self
            .connect()?
            .query_row(
                "SELECT state,body FROM ion_team_workspaces WHERE run_id=?",
                [run_id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((state, body)) = existing {
            let mut record: WorkspaceRecord = serde_json::from_str(&body)?;
            ensure!(
                source == record.workspace.source_path || source == record.workspace.path,
                "Team source workspace changed"
            );
            // Preparing can be adopted only after Git has fully registered and
            // unlocked its worktree. We do not repeat an interrupted creation.
            if state != "ready" {
                let registered = git(&record.repository, &["worktree", "list", "--porcelain"])?;
                let tree = git_path(&record.tree)?;
                let block = registered
                    .split("\n\n")
                    .find(|b| b.lines().next() == Some(format!("worktree {tree}").as_str()));
                ensure!(
                    block.is_some_and(|b| !b.lines().any(|l| l.starts_with("locked"))),
                    "Team workspace creation is incomplete; review its recorded path before recovery"
                );
            }
            record.workspace.path = fs::canonicalize(&record.workspace.path)?;
            verify(&record)?;
            self.connect()?.execute(
                "UPDATE ion_team_workspaces SET body=? WHERE run_id=?",
                params![serde_json::to_string(&record)?, run_id.to_string()],
            )?;
            self.seal_workspace(run_id, now)?;
            return Ok(record.workspace.path);
        }
        let repository=canonical_git_root(&source).context("Coding teams require a Git project for isolated work; initialize/commit the project first")?;
        require_filter_free(&repository)?;
        ensure!(
            git(
                &repository,
                &["status", "--porcelain=v1", "--untracked-files=normal"]
            )?
            .trim()
            .is_empty(),
            "Commit or stash project changes before starting a coding team; its isolated workspace must have an explicit clean baseline"
        );
        ensure!(
            git(&repository, &["ls-files", "--stage"])?
                .lines()
                .all(|l| !l.starts_with("160000 ")),
            "Team coding workspaces with submodules require a reviewed snapshot adapter"
        );
        let base = git(&repository, &["rev-parse", "HEAD"])?.trim().to_owned();
        ensure!(
            matches!(base.len(), 40 | 64) && base.bytes().all(|c| c.is_ascii_hexdigit()),
            "Invalid team base commit"
        );
        let parent = self
            .control
            .path()
            .parent()
            .context("Control database folder is missing")?
            .join("ion-team-workspaces");
        fs::create_dir_all(&parent)?;
        let parent = fs::canonicalize(parent)?;
        let tree = parent.join(run_id.to_string());
        ensure!(
            !tree.exists(),
            "Reserved team workspace path already exists; review it before creating this run"
        );
        let relative = source.strip_prefix(&repository)?.to_path_buf();
        let record = WorkspaceRecord {
            workspace: TeamWorkspace {
                path: tree.join(relative),
                source_path: source,
                branch: format!("ion/team-{}", run_id.simple()),
                base_commit: base,
            },
            repository,
            tree,
        };
        {
            let mut db = self.connect()?;
            let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
            active(&load(&tx, run_id)?, now)?;
            tx.execute(
                "INSERT INTO ion_team_workspaces VALUES(?,'preparing',?)",
                params![run_id.to_string(), serde_json::to_string(&record)?],
            )
            .context("Team workspace is already being provisioned")?;
            audit(
                &tx,
                run_id,
                None,
                "workspace_reserved",
                &format!(
                    "Isolated branch {} from {}",
                    record.workspace.branch, record.workspace.base_commit
                ),
                now,
            )?;
            tx.commit()?;
        }
        git(
            &record.repository,
            &[
                "worktree",
                "add",
                "-b",
                &record.workspace.branch,
                &git_path(&record.tree)?,
                &record.workspace.base_commit,
            ],
        )?;
        // Canonicalize before sealing (Windows normalizes its extended path prefix).
        let mut record = record;
        record.workspace.path = fs::canonicalize(&record.workspace.path)?;
        verify(&record)?;
        self.connect()?.execute(
            "UPDATE ion_team_workspaces SET body=? WHERE run_id=? AND state='preparing'",
            params![serde_json::to_string(&record)?, run_id.to_string()],
        )?;
        self.seal_workspace(run_id, now)?;
        Ok(record.workspace.path)
    }
    fn seal_workspace(&self, run_id: Uuid, now: u64) -> Result<()> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        active(&load(&tx, run_id)?, now)?;
        if tx.execute(
            "UPDATE ion_team_workspaces SET state='ready' WHERE run_id=? AND state='preparing'",
            [run_id.to_string()],
        )? > 0
        {
            bump(&tx, run_id)?;
            audit(
                &tx,
                run_id,
                None,
                "workspace_ready",
                "Isolated coding workspace is ready; source checkout remains unchanged",
                now,
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    /// The lease chooses the workspace. Caller projections cannot redirect writes.
    pub fn assignment_workspace(
        &self,
        lease: &AssignmentLease,
        source: &Path,
        now: u64,
    ) -> Result<PathBuf> {
        let db = self.connect()?;
        let run = load(&db, lease.run_id)?;
        active(&run, now)?;
        let work = checked_lease(&run, lease, now, false)?;
        let source = fs::canonicalize(source)?;
        match run.workspace {
            Some(workspace) => {
                ensure!(
                    source == workspace.path || source == workspace.source_path,
                    "Worker workspace does not match the pinned run"
                );
                Ok(workspace.path)
            }
            None => {
                ensure!(
                    !work.access.workspace_write,
                    "Provision the isolated team workspace before executing a writer"
                );
                Ok(source)
            }
        }
    }
    /// Whole-file compare-and-replace, limited to UTF-8 code/text. Records an
    /// exact receipt before returning; publication and checks remain separate.
    pub fn edit_workspace_file(
        &self,
        lease: &AssignmentLease,
        request: Uuid,
        relative: &str,
        old: Option<&str>,
        new: &str,
        now: u64,
    ) -> Result<String> {
        ensure!(
            !request.is_nil()
                && !relative.is_empty()
                && relative.len() <= 512
                && new.len() <= 16 * 1024
                && old.is_none_or(|s| s.len() <= 16 * 1024),
            "Invalid or oversized team file edit"
        );
        ensure!(old != Some(new), "File edit must change content");
        ensure!(
            serde_json::to_vec(&old)?.len() <= 24 * 1024
                && serde_json::to_vec(&new)?.len() <= 24 * 1024,
            "Escaped edit content exceeds the document handoff bound"
        );
        let signature =
            serde_json::to_string(&serde_json::json!({"path":relative,"old":old,"new":new}))?;
        let (id, path) = match self.reserve_workspace_edit(
            lease,
            request,
            relative,
            &signature,
            |path| compare_file(path, old),
            now,
        )? {
            ReservedWorkspaceEdit::Receipt(receipt) => return Ok(receipt),
            ReservedWorkspaceEdit::Pending { id, path } => (id, path),
        };
        // Errors after reservation retain a pending effect and block later
        // writers. Never turn uncertain outcomes into permission to replay.
        let stage = path.with_file_name(format!(".ion-edit-{id}.pending"));
        let outcome = (|| -> Result<()> {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&stage)?;
            file.write_all(new.as_bytes())?;
            file.sync_all()?;
            if path.exists() {
                fs::set_permissions(&stage, fs::metadata(&path)?.permissions())?;
            }
            self.renew(lease, crate::model::unix_time().max(now), 180)?;
            ensure!(
                safe_edit_path(
                    path.parent().context("File parent missing")?,
                    path.file_name()
                        .unwrap()
                        .to_str()
                        .context("File name is not UTF-8")?
                )? == path,
                "File path changed"
            );
            compare_file(&path, old)?;
            if old.is_none() {
                // hard_link fails if another writer creates the destination.
                fs::hard_link(&stage, &path).context("New file appeared during creation")?;
                fs::remove_file(&stage)?;
            } else {
                fs::rename(&stage, &path).context("Could not atomically replace team file")?;
            }
            Ok(())
        })();
        if let Err(error) = outcome {
            return Err(anyhow::anyhow!(
                "Team file edit requires workspace reconciliation ({id}): {error}"
            ));
        }
        let receipt=serde_json::json!({"edit_id":id,"path":relative,"bytes":new.len(),"status":"applied","workspace":"isolated_team_run"}).to_string();
        // A stopped lease can account for this already-started edit only.
        self.finish_workspace_edit(lease, id, relative, receipt, now)
    }
    fn reserve_workspace_edit(
        &self,
        lease: &AssignmentLease,
        request: Uuid,
        relative: &str,
        signature: &str,
        preflight: impl FnOnce(&Path) -> Result<()>,
        now: u64,
    ) -> Result<ReservedWorkspaceEdit> {
        ensure!(
            !request.is_nil()
                && !relative.is_empty()
                && relative.len() <= 512
                && signature.len() <= 64 * 1024,
            "Invalid workspace edit request"
        );
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, lease.run_id)?;
        active(&run, now)?;
        let work = checked_lease(&run, lease, now, false)?;
        ensure!(
            work.access.workspace_write && work.access.workspace_read,
            "Workspace write access denied"
        );
        let workspace = run
            .workspace
            .as_ref()
            .context("Isolated team workspace is not ready")?;
        let path = safe_edit_path(&workspace.path, relative)?;
        if let Some((prior,state,receipt))=tx.query_row("SELECT signature,state,receipt FROM ion_team_workspace_edits WHERE work_id=? AND request_id=?",params![work.id.to_string(),request.to_string()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?))).optional()? {
                ensure!(prior==signature,"File edit request reused with changed arguments");
                ensure!(state=="applied","File edit outcome needs reconciliation; it was not repeated");
                return Ok(ReservedWorkspaceEdit::Receipt(receipt.context("File edit receipt missing")?));
            }
        require_settled_effects(&tx, run.id)?;
        let count: u32 = tx.query_row(
            "SELECT count(*) FROM ion_team_workspace_edits WHERE run_id=?",
            [run.id.to_string()],
            |r| r.get(0),
        )?;
        ensure!(count < 256, "Team file edit limit reached");
        preflight(&path)?;
        let id = Uuid::new_v4();
        tx.execute("INSERT INTO ion_team_workspace_edits(id,run_id,work_id,attempt_id,request_id,signature,state) VALUES(?,?,?,?,?,?,'pending')",params![id.to_string(),run.id.to_string(),work.id.to_string(),lease.attempt_id.to_string(),request.to_string(),signature])?;
        audit(
            &tx,
            run.id,
            Some(work.id),
            "file_edit_started",
            relative,
            now,
        )?;
        tx.commit()?;
        Ok(ReservedWorkspaceEdit::Pending { id, path })
    }
    fn finish_workspace_edit(
        &self,
        lease: &AssignmentLease,
        id: Uuid,
        relative: &str,
        receipt: String,
        now: u64,
    ) -> Result<String> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed=tx.execute("UPDATE ion_team_workspace_edits SET state='applied',receipt=? WHERE id=? AND run_id=? AND work_id=? AND attempt_id=? AND state='pending'",params![receipt,id.to_string(),lease.run_id.to_string(),lease.work_id.to_string(),lease.attempt_id.to_string()])?;
        if changed == 0 {
            let prior:Option<String>=tx.query_row("SELECT receipt FROM ion_team_workspace_edits WHERE id=? AND run_id=? AND work_id=? AND attempt_id=? AND state='applied'",params![id.to_string(),lease.run_id.to_string(),lease.work_id.to_string(),lease.attempt_id.to_string()],|r|r.get(0)).optional()?.flatten();
            ensure!(
                prior.as_deref() == Some(receipt.as_str()),
                "Workspace receipt changed or is missing"
            );
            return Ok(receipt);
        }
        bump(&tx, lease.run_id)?;
        audit(
            &tx,
            lease.run_id,
            Some(lease.work_id),
            "file_edit_applied",
            relative,
            now,
        )?;
        tx.commit()?;
        Ok(receipt)
    }
    /// Generated exact before/after change document, one path per bounded tool call.
    /// Read scope covers the shared run workspace; unrelated runs stay inaccessible.
    pub fn publish_workspace_change(
        &self,
        lease: &AssignmentLease,
        request: Uuid,
        relative: &str,
        now: u64,
    ) -> Result<Artifact> {
        let (name, content) = {
            let db = self.connect()?;
            let run = load(&db, lease.run_id)?;
            active(&run, now)?;
            let work = checked_lease(&run, lease, now, false)?;
            ensure!(work.access.workspace_read, "Workspace read access denied");
            let workspace = run
                .workspace
                .as_ref()
                .context("No coding workspace exists for this run")?;
            let path = safe_edit_path(&workspace.path, relative)?;
            let rows:Vec<(String,String,String)>=db.prepare("SELECT id,signature,state FROM ion_team_workspace_edits WHERE run_id=? ORDER BY rowid LIMIT 257")?.query_map([run.id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?.collect::<rusqlite::Result<_>>()?;
            let mut before = None;
            let mut after = None;
            let mut ids = vec![];
            for (id, signature, state) in rows {
                let value: serde_json::Value = serde_json::from_str(&signature)?;
                if state != "restored"
                    && value["path"] == relative
                    && value["operation"] != "createDirectory"
                {
                    ensure!(
                        state == "applied",
                        "Changed file has an uncertain edit; reconcile it before publication"
                    );
                    if before.is_none() {
                        before = Some(value["old"].clone());
                    }
                    after = Some(value["new"].clone());
                    ids.push(id);
                }
            }
            let after = after.context("No team edits exist for this path")?;
            let final_content = if after.is_null() {
                None
            } else {
                Some(after.as_str().context("Invalid edit receipt")?)
            };
            compare_file(&path, final_content)?;
            let content = serde_json::to_string_pretty(
                &serde_json::json!({"path":relative,"base_commit":workspace.base_commit,"branch":workspace.branch,"before":before,"after":after,"edit_ids":ids,"source_checkout_changed":false}),
            )?;
            ensure!(
                content.len() <= 64 * 1024,
                "Change document exceeds its bound"
            );
            (format!("Change-{}.json", request.simple()), content)
        };
        self.publish_document(lease, request, &name, "json", &content, now)
    }
    pub fn workspace_has_pending_effect(&self, run: Uuid) -> Result<bool> {
        Ok(self.connect()?.query_row("SELECT EXISTS(SELECT 1 FROM ion_team_workspace_edits WHERE run_id=? AND state='pending')",[run.to_string()],|r|r.get(0))?)
    }
}
fn compare_file(path: &Path, expected: Option<&str>) -> Result<()> {
    match expected {
        None => ensure!(!path.exists(), "New file already exists"),
        Some(old) => {
            let file = fs::File::open(path)?;
            ensure!(
                file.metadata()?.is_file() && file.metadata()?.len() <= 16 * 1024,
                "Edit an existing UTF-8 file smaller than 16 KiB"
            );
            let mut content = String::new();
            file.take(16 * 1024 + 1).read_to_string(&mut content)?;
            ensure!(
                content == old,
                "File changed; read its current content before proposing an edit"
            );
        }
    }
    Ok(())
}
pub(super) fn safe_edit_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let value = Path::new(relative);
    ensure!(
        !value.is_absolute()
            && value
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        "Use a project-relative file path"
    );
    let mut path = root.to_owned();
    for part in value.components() {
        let Component::Normal(name) = part else {
            unreachable!()
        };
        let text = name
            .to_str()
            .context("File path is not UTF-8")?
            .to_ascii_lowercase();
        ensure!(
            !text.contains([':', '\0'])
                && !text.chars().any(|c| c.is_control())
                && !matches!(
                    text.split('.').next(),
                    Some(
                        "con"
                            | "prn"
                            | "aux"
                            | "nul"
                            | "com1"
                            | "com2"
                            | "com3"
                            | "com4"
                            | "com5"
                            | "com6"
                            | "com7"
                            | "com8"
                            | "com9"
                            | "lpt1"
                            | "lpt2"
                            | "lpt3"
                            | "lpt4"
                            | "lpt5"
                            | "lpt6"
                            | "lpt7"
                            | "lpt8"
                            | "lpt9"
                    )
                )
                && !text.ends_with(['.', ' '])
                && !matches!(
                    text.as_str(),
                    ".git"
                        | ".ssh"
                        | ".aws"
                        | ".env"
                        | ".npmrc"
                        | ".pypirc"
                        | "credentials.json"
                        | "secrets.json"
                )
                && !text.starts_with(".env.")
                && !text.starts_with(".ion-edit-")
                && !text.ends_with(".pem")
                && !text.ends_with(".p12")
                && !text.ends_with(".pfx"),
            "File path is private or reserved"
        );
        path.push(name);
        if let Ok(meta) = fs::symlink_metadata(&path) {
            ensure!(!meta.file_type().is_symlink(), "File path contains a link");
        }
    }
    let parent = fs::canonicalize(path.parent().context("File parent missing")?)?;
    ensure!(
        parent.starts_with(root),
        "File path escapes the team workspace"
    );
    ensure!(parent.is_dir(), "File parent directory must exist");
    Ok(parent.join(path.file_name().context("File name missing")?))
}

#[cfg(test)]
mod tests;
