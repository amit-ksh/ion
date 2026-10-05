use super::*;
use waku_protocol::control::*;
use waku_protocol::model::RuntimeMode;

pub(crate) struct WorkspaceContext {
    pub task_id: Uuid,
    pub project_id: Uuid,
    pub control_path: PathBuf,
    pub settings_path: PathBuf,
    pub mode: RuntimeMode,
}
impl WorkspaceContext {
    fn settings(&self) -> Result<crate::DaemonSettings> {
        Ok(serde_json::from_slice(&fs::read(&self.settings_path)?)?)
    }
    fn sandbox_session(&self) -> Result<String> {
        let settings = self.settings()?;
        if settings
            .extra
            .get("nebius_sandbox_enabled")
            .and_then(Value::as_bool)
            != Some(true)
        {
            bail!("Token Factory sandbox is off in Settings");
        }
        let name = settings
            .extra
            .get("nebius_sandbox_session")
            .and_then(Value::as_str)
            .unwrap_or("ion");
        if name.is_empty()
            || name.len() > 64
            || !name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            bail!("Invalid sandbox session name");
        }
        Ok(name.into())
    }
}
pub(super) fn tools(mut tools: Value, context: Option<&WorkspaceContext>) -> Value {
    let Some(context) = context else {
        return tools;
    };
    let list = tools.as_array_mut().unwrap();
    list.extend(team_tools().as_array().unwrap().iter().cloned());
    list.push(json!({"type":"function","function":{"name":"request_decision","description":"Ask the user to approve or decline a decision before further task or workflow progress. Blocks until the user answers.","parameters":{"type":"object","properties":{"title":{"type":"string"},"detail":{"type":"string"}},"required":["title","detail"],"additionalProperties":false}}}));
    list.push(json!({"type":"function","function":{"name":"propose_task","description":"Extract a follow-up task for the user to accept or decline in Inbox. Does not execute the task.","parameters":{"type":"object","properties":{"title":{"type":"string"},"instructions":{"type":"string"}},"required":["title","instructions"],"additionalProperties":false}}}));
    list.push(json!({"type":"function","function":{"name":"save_document","description":"Publish a generated HTML, Markdown, text, CSV or JSON document to the project's Artifacts collection.","parameters":{"type":"object","properties":{"name":{"type":"string"},"format":{"type":"string","enum":["html","md","txt","csv","json"]},"content":{"type":"string"}},"required":["name","format","content"],"additionalProperties":false}}}));
    if context.sandbox_session().is_ok() {
        list.push(json!({"type":"function","function":{"name":"tokenfactory_exec","description":"Run exact arguments in the user's configured Token Factory ConTree session after explicit approval. Local project files are not synced. Never include secrets in arguments.","parameters":{"type":"object","properties":{"argv":{"type":"array","items":{"type":"string"}}},"required":["argv"],"additionalProperties":false}}}));
    }
    tools
}
pub(super) fn execute(
    context: &WorkspaceContext,
    receiver: &Receiver<Command>,
    cancelled: &AtomicBool,
    events: &DriverEventSender,
    journal_path: &Path,
    journal: &mut Journal,
    call: &ToolCall,
    args: &Value,
) -> Result<String> {
    anyhow::ensure!(!cancelled.load(Ordering::Relaxed), "Task stopped");
    let store = crate::control::ControlStore::new(context.control_path.clone());
    let result = match call.name.as_str() {
        "list_teams" => team_catalog(&store, args)?,
        "invoke_team" => {
            journal.pending_effect = Some(call.id.clone());
            persist(journal_path, journal)?;
            let result = invoke_team_authorized(context, &call.id, args, |profile, limits| {
                if matches!(
                    context.mode,
                    RuntimeMode::FullAccess | RuntimeMode::AutoAcceptEdits
                ) && !profile.access.shell
                    && !profile.access.network
                {
                    return Ok(profile.access.clone());
                }
                if profile.access == waku_protocol::teams::AgentAccess::default() {
                    return Ok(profile.access.clone());
                }
                let _ = events.send(DriverEvent::Permission {
                    request_id:call.id.clone(), title:format!("Use team agent {}?", profile.name),
                    detail:format!("Harness: {}\nModel: {}\nExecution environment: {}\nConfigured access: {}\nRun limits: {}\nEdits use an isolated branch. App actions require their own exact approval.",profile.provider.display_name(),profile.model.as_deref().unwrap_or("default"),profile.execution.environment.label(),serde_json::to_string(&profile.access)?,serde_json::to_string(limits)?),
                    options:vec![PermissionOption{id:"allow".into(),label:"Start team".into(),allow:true},PermissionOption{id:"reject".into(),label:"Decline".into(),allow:false}],
                });
                loop {
                    anyhow::ensure!(
                        !cancelled.load(Ordering::Relaxed),
                        "Stopped before team invocation"
                    );
                    match receiver.recv_timeout(Duration::from_millis(200)) {
                        Ok(Command::Respond(id, option)) if id == call.id => {
                            anyhow::ensure!(
                                option == "allow",
                                "Team invocation declined; no run was created"
                            );
                            return Ok(profile.access.clone());
                        }
                        Ok(Command::Stop)
                        | Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                            bail!("Stopped before team invocation")
                        }
                        _ => {}
                    }
                }
            });
            if cancelled.load(Ordering::Relaxed) {
                // Close the race where cancellation arrives while the queue transaction commits.
                crate::teams::TeamCoordinator::new(context.control_path.clone())
                    .stop_source_runs(context.task_id, crate::model::unix_time())?;
                bail!("Source task stopped; its team invocations were canceled");
            }
            if result.is_err() {
                journal.pending_effect = None;
                persist(journal_path, journal)?;
            }
            result?
        }
        "get_team_runs" => {
            anyhow::ensure!(
                args.as_object().is_some_and(|o| o.is_empty()),
                "No arguments accepted"
            );
            let runs = crate::teams::TeamCoordinator::new(context.control_path.clone())
                .runs_for_source(context.task_id)?;
            serde_json::to_string(&runs.iter().map(team_summary).collect::<Vec<_>>())?
        }
        "await_team" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Args {
                run_id: Uuid,
                #[serde(default = "team_wait_seconds")]
                max_wait_seconds: u64,
            }
            let a: Args = serde_json::from_value(args.clone())?;
            anyhow::ensure!(
                (1..=30).contains(&a.max_wait_seconds),
                "Wait duration must be 1–30 seconds"
            );
            let coordinator = crate::teams::TeamCoordinator::new(context.control_path.clone());
            let deadline = std::time::Instant::now() + Duration::from_secs(a.max_wait_seconds);
            loop {
                anyhow::ensure!(
                    !cancelled.load(Ordering::Relaxed),
                    "Source task stopped while waiting for its team"
                );
                let run = coordinator.source_run(context.task_id, context.project_id, a.run_id)?;
                if run.status != waku_protocol::teams::TeamRunStatus::Active
                    || run
                        .work
                        .iter()
                        .filter(|w| w.status != waku_protocol::teams::AssignmentStatus::Completed)
                        .all(|w| {
                            matches!(
                                w.status,
                                waku_protocol::teams::AssignmentStatus::Blocked
                                    | waku_protocol::teams::AssignmentStatus::Failed
                                    | waku_protocol::teams::AssignmentStatus::Uncertain
                            ) || w.status == waku_protocol::teams::AssignmentStatus::Queued
                                && w.reason.is_some()
                        })
                    || std::time::Instant::now() >= deadline
                {
                    break team_summary(&run).to_string();
                }
                // One collection poll per second on the provider worker, with prompt cancellation.
                for _ in 0..5 {
                    anyhow::ensure!(!cancelled.load(Ordering::Relaxed), "Source task stopped");
                    thread::sleep(Duration::from_millis(200));
                }
            }
        }
        "read_team_document" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Args {
                run_id: Uuid,
                artifact_id: Uuid,
            }
            let a: Args = serde_json::from_value(args.clone())?;
            let coordinator = crate::teams::TeamCoordinator::new(context.control_path.clone());
            let content = String::from_utf8(coordinator.read_source_document(
                context.task_id,
                context.project_id,
                a.run_id,
                a.artifact_id,
            )?)
            .context("Team document is not UTF-8")?;
            let excerpt = team::bounded_excerpt(&content, 8192);
            json!({"run_id":a.run_id,"artifact_id":a.artifact_id,"content":excerpt,"truncated":excerpt.len()!=content.len()}).to_string()
        }
        "request_decision" => {
            let title = required_str(args, "title")?;
            let detail = required_str(args, "detail")?;
            anyhow::ensure!(
                title.len() <= 160 && detail.len() <= 32 * 1024,
                "Decision details are too large"
            );
            let _ = events.send(DriverEvent::Permission {
                request_id: call.id.clone(),
                title: format!("Decision: {title}"),
                detail: detail.into(),
                options: vec![
                    PermissionOption {
                        id: "allow".into(),
                        label: "Approve".into(),
                        allow: true,
                    },
                    PermissionOption {
                        id: "reject".into(),
                        label: "Decline".into(),
                        allow: false,
                    },
                ],
            });
            loop {
                if cancelled.load(Ordering::Relaxed) {
                    bail!("Task stopped");
                }
                match receiver.recv_timeout(Duration::from_millis(200)) {
                    Ok(Command::Respond(id, decision)) if id == call.id => {
                        break if decision == "allow" {
                            "User approved this decision.".into()
                        } else {
                            "User declined this decision. Do not proceed with the proposed action."
                                .into()
                        };
                    }
                    Ok(Command::Stop) => bail!("Task stopped"),
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                        bail!("Task disconnected")
                    }
                    _ => {}
                }
            }
        }
        "propose_task" => {
            let title = required_str(args, "title")?;
            let detail = required_str(args, "instructions")?;
            store.add_agent_action(WorkspaceAction {
                id: Uuid::new_v4(),
                project_id: context.project_id,
                task_id: context.task_id,
                title: title.into(),
                detail: detail.into(),
                action_kind: WorkspaceActionKind::ProposedTask,
                status: WorkspaceActionStatus::Pending,
                created_at: crate::model::unix_time(),
            })?;
            "Task proposed in Inbox. Wait for the user's acceptance before performing it.".into()
        }
        "save_document" => {
            let name = required_str(args, "name")?;
            let format = required_str(args, "format")?;
            let content = required_str(args, "content")?;
            anyhow::ensure!(
                matches!(format, "html" | "md" | "txt" | "csv" | "json"),
                "Unsupported generated document format"
            );
            let snapshot = store.load()?;
            let name = if name.to_ascii_lowercase().ends_with(&format!(".{format}")) {
                name.to_owned()
            } else {
                format!("{name}.{format}")
            };
            store.import_document(
                snapshot.revision,
                name,
                format.into(),
                Some(context.project_id),
                Some(context.task_id),
                content.as_bytes().to_vec(),
            )?;
            "Generated document saved in Artifacts.".into()
        }
        "tokenfactory_exec" => {
            let session = context.sandbox_session()?;
            let argv = NemoClawAdapter::validate_args(args)?;
            let detail = serde_json::to_string(&argv)?;
            let _=events.send(DriverEvent::Permission { request_id:call.id.clone(), title:"Run in Token Factory sandbox?".into(), detail:format!("Session: {session}\nExact arguments: {detail}\nThe local project is not automatically uploaded."), options:vec![PermissionOption{id:"allow".into(),label:"Run in sandbox".into(),allow:true},PermissionOption{id:"reject".into(),label:"Decline".into(),allow:false}] });
            loop {
                if cancelled.load(Ordering::Relaxed) {
                    bail!("Task stopped");
                }
                match receiver.recv_timeout(Duration::from_millis(200)) {
                    Ok(Command::Respond(id, decision)) if id == call.id => {
                        if decision != "allow" {
                            bail!("User declined sandbox execution");
                        }
                        break;
                    }
                    Ok(Command::Stop) => bail!("Task stopped"),
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                        bail!("Task disconnected")
                    }
                    _ => {}
                }
            }
            // Recheck the live switch after approval, before any external effect.
            anyhow::ensure!(
                context.sandbox_session()? == session,
                "Sandbox configuration changed. Request approval again."
            );
            journal.pending_effect = Some(call.id.clone());
            persist(journal_path, journal)?;
            let outcome = run_contree(&session, &argv, cancelled);
            if outcome.as_ref().err().is_some_and(|e| {
                e.downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound)
            }) {
                journal.pending_effect = None;
                persist(journal_path, journal)?;
            }
            outcome?
        }
        _ => bail!("Unknown workspace tool"),
    };
    let _ = events.send(DriverEvent::Activity {
        id: Some(call.activity_id.clone()),
        kind: ActivityKind::Tool,
        title: call.name.replace('_', " "),
        detail: Some(result.clone()),
        complete: true,
    });
    Ok(result)
}
fn team_tools() -> Value {
    json!([
        {"type":"function","function":{"name":"list_teams","description":"Find saved agent teams and the definition revision. Ready saved harnesses execute through the daemon; unavailable setup appears in run status.","parameters":{"type":"object","properties":{"query":{"type":"string"}},"additionalProperties":false}}},
        {"type":"function","function":{"name":"invoke_team","description":"Invoke a saved team in this task’s project using its lead’s configured access and pinned limits. Supervised tasks ask before granting privileged team access. Maximum four invocations per source task. Inspect get_team_runs; a queue acknowledgement is not completed work.","parameters":{"type":"object","properties":{"team_id":{"type":"string"},"definition_revision":{"type":"integer"},"objective":{"type":"string"}},"required":["team_id","definition_revision","objective"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"get_team_runs","description":"Inspect the durable team runs invoked from this task. Inspect assignment state, setup blockers and completed document evidence; queued work is not completed.","parameters":{"type":"object","properties":{},"additionalProperties":false}}},
        {"type":"function","function":{"name":"await_team","description":"Wait up to 30 seconds for a team invoked by this task, without another model request. Returns real status, blockers and final document IDs; not a completion assertion.","parameters":{"type":"object","properties":{"run_id":{"type":"string"},"max_wait_seconds":{"type":"integer","minimum":1,"maximum":30}},"required":["run_id"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"read_team_document","description":"Read a bounded document from a completed team invoked by this source task/project. Inspect the actual evidence before using its result; superseded and unrelated documents are inaccessible.","parameters":{"type":"object","properties":{"run_id":{"type":"string"},"artifact_id":{"type":"string"}},"required":["run_id","artifact_id"],"additionalProperties":false}}}

    ])
}
fn team_wait_seconds() -> u64 {
    30
}
fn team_catalog(store: &crate::control::ControlStore, args: &Value) -> Result<String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Search {
        #[serde(default)]
        query: String,
    }
    let search: Search = serde_json::from_value(args.clone())?;
    anyhow::ensure!(search.query.len() <= 256, "Team query is too long");
    let snapshot = store.load()?;
    let query = search.query.to_lowercase();
    let teams: Vec<_> = snapshot
        .records
        .iter()
        .filter_map(|r| {
            if let ControlRecord::Team(t) = r {
                Some(t)
            } else {
                None
            }
        })
        .filter(|t| t.name.to_lowercase().contains(&query))
        .take(64)
        .map(|t| json!({"id":t.id,"name":t.name,"agents":t.members.len()}))
        .collect();
    Ok(
        json!({"definition_revision":snapshot.revision,"teams":teams,"dispatch_available":true})
            .to_string(),
    )
}
#[cfg(test)]
fn invoke_team(context: &WorkspaceContext, request_id: &str, args: &Value) -> Result<String> {
    invoke_team_authorized(context, request_id, args, |_, _| Ok(Default::default()))
}
fn invoke_team_authorized(
    context: &WorkspaceContext,
    request_id: &str,
    args: &Value,
    authorize: impl FnOnce(
        &AgentProfile,
        &waku_protocol::teams::TeamRunLimits,
    ) -> Result<waku_protocol::teams::AgentAccess>,
) -> Result<String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Invocation {
        team_id: Uuid,
        definition_revision: u64,
        objective: String,
    }
    let request: Invocation = serde_json::from_value(args.clone())?;
    let coordinator = crate::teams::TeamCoordinator::new(context.control_path.clone());
    // Exact already-created invocations keep their original permission receipt.
    if let Some(run) = coordinator
        .runs_for_source(context.task_id)?
        .into_iter()
        .find(|r| {
            r.origin
                .as_ref()
                .is_some_and(|o| o.request_id == request_id)
        })
    {
        anyhow::ensure!(
            run.team.id == request.team_id
                && run.definition_revision == request.definition_revision
                && run.work[0].objective == request.objective.trim()
                && run.project_id == context.project_id,
            "Invocation replay changed its team, revision, project or objective"
        );
        return Ok(team_summary(&run).to_string());
    }
    let snapshot = crate::control::ControlStore::new(context.control_path.clone()).load()?;
    anyhow::ensure!(
        snapshot.revision == request.definition_revision,
        "Team definition changed; list teams again"
    );
    let team = snapshot
        .records
        .iter()
        .find_map(|r| match r {
            ControlRecord::Team(t) if t.id == request.team_id => Some(t),
            _ => None,
        })
        .context("Team is unavailable")?;
    let lead = team
        .members
        .iter()
        .find(|m| m.parent_id.is_none())
        .context("Team lead missing")?;
    let profile = snapshot
        .records
        .iter()
        .find_map(|r| match r {
            ControlRecord::Agent(p) if p.id == lead.agent_id => Some(p),
            _ => None,
        })
        .context("Team lead profile missing")?;
    let access = authorize(profile, &team.limits.clone().unwrap_or_default())?;
    anyhow::ensure!(
        crate::teams::intersect_access(&access, &profile.access) == access,
        "Source authorization exceeds the saved team lead access"
    );
    let run = coordinator.queue_from_agent_with_access(
        context.task_id,
        context.project_id,
        request_id.into(),
        request.team_id,
        request.definition_revision,
        request.objective,
        access,
        crate::model::unix_time(),
    )?;
    Ok(team_summary(&run).to_string())
}
fn team_summary(run: &waku_protocol::teams::TeamRunSnapshot) -> Value {
    json!({"run_id":run.id,"task_id":run.root_task_id,"team":run.team.name,"status":run.status,
        "invocation_id":run.origin.as_ref().map(|origin|&origin.request_id),
        "objective":run.work.first().map(|w|w.objective.chars().take(512).collect::<String>()),
        "total_assignments":run.work.len(),"omitted_assignments":run.work.len().saturating_sub(8),
        "assignments":run.work.iter().take(8).map(|w|json!({"id":w.id,"task_id":w.task_id,"status":w.status,
            "reason":w.reason.as_ref().map(|r|r.chars().take(256).collect::<String>()),
            "summary":w.result.as_ref().map(|r|r.summary.chars().take(512).collect::<String>()),
            "artifacts":w.result.as_ref().map(|r|r.artifact_ids.iter().take(8).collect::<Vec<_>>())})).collect::<Vec<_>>(),
        "dispatch_available":true,"message":"Queued for daemon dispatch. Inspect assignment status for progress or setup requirements."})
}

struct ChildGuard(std::process::Child);
impl std::ops::Deref for ChildGuard {
    type Target = std::process::Child;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for ChildGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn run_contree(session: &str, argv: &[String], cancelled: &AtomicBool) -> Result<String> {
    use std::io::Read;
    use std::process::{Command as Process, Stdio};
    let mut command = Process::new("contree");
    command
        .args(["-S", session, "run", "--"])
        .args(argv)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child=ChildGuard(command.spawn().context("ConTree is unavailable. Install contree-cli, run contree auth, and initialize the ion session.")?);
    let mut stdout = child.stdout.take().context("Missing stdout")?;
    let mut stderr = child.stderr.take().context("Missing stderr")?;
    // Drain both streams, retaining only bounded output. Avoid pipe deadlocks.
    let read_out = thread::spawn(move || {
        let mut saved = Vec::new();
        let mut buffer = [0; 4096];
        while let Ok(n) = stdout.read(&mut buffer) {
            if n == 0 {
                break;
            }
            let keep = n.min(16384usize.saturating_sub(saved.len()));
            saved.extend_from_slice(&buffer[..keep]);
        }
        saved
    });
    let read_err = thread::spawn(move || {
        let mut buffer = [0; 4096];
        while let Ok(n) = stderr.read(&mut buffer) {
            if n == 0 {
                break;
            }
        }
    });
    let start = std::time::Instant::now();
    let status = loop {
        if cancelled.load(Ordering::Relaxed) || start.elapsed() > Duration::from_secs(120) {
            let _ = child.kill();
            let _ = child.wait();
            bail!(
                "Sandbox result is uncertain after interruption. Inspect ConTree; this command will not be repeated automatically."
            );
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        thread::sleep(Duration::from_millis(100));
    };
    let output = read_out
        .join()
        .map_err(|_| anyhow!("Could not read sandbox output"))?;
    let _ = read_err.join();
    Ok(format!(
        "Sandbox command exit code: {:?}\n{}",
        status.code(),
        String::from_utf8_lossy(&output)
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn team_tool_rejects_model_selected_task_or_project() {
        let context = WorkspaceContext {
            task_id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            control_path: PathBuf::new(),
            settings_path: PathBuf::new(),
            mode: RuntimeMode::Ask,
        };
        let args = json!({"team_id":Uuid::new_v4(),"definition_revision":0,"objective":"Research","project_id":Uuid::new_v4()});
        assert!(
            invoke_team(&context, "call", &args)
                .unwrap_err()
                .to_string()
                .contains("unknown field")
        );
        let names: Vec<_> = team_tools()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["function"]["name"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(
            names,
            [
                "list_teams",
                "invoke_team",
                "get_team_runs",
                "await_team",
                "read_team_document"
            ]
        );
    }

    #[test]
    fn sandbox_is_hidden_by_default_and_switch_is_read_live() {
        let path = std::env::temp_dir().join(format!("ion-sandbox-test-{}.json", Uuid::new_v4()));
        let context = WorkspaceContext {
            task_id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            control_path: path.with_extension("db"),
            settings_path: path.clone(),
            mode: RuntimeMode::Ask,
        };
        fs::write(&path, b"{}").unwrap();
        assert!(context.sandbox_session().is_err());
        fs::write(&path, b"{\"nebius_sandbox_enabled\":true}").unwrap();
        assert_eq!(context.sandbox_session().unwrap(), "ion");
        fs::write(&path, b"{\"nebius_sandbox_enabled\":false}").unwrap();
        assert!(context.sandbox_session().is_err());
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn normal_team_invocation_uses_source_permissions_and_preserves_approved_scope_on_replay() {
        use waku_protocol::teams::{AgentAccess, AgentExecution, TeamRunLimits};
        let dir = std::env::temp_dir().join(format!("ion-source-team-tests-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let store = crate::control::ControlStore::new(dir.join("control.db"));
        let agent = Uuid::new_v4();
        let team = Uuid::new_v4();
        store
            .save(
                0,
                ControlRecord::Agent(AgentProfile {
                    id: agent,
                    name: "Lead".into(),
                    expertise: "Coding".into(),
                    instructions: "Delegate".into(),
                    provider: waku_protocol::model::ProviderKind::Nebius,
                    model: Some("pinned-model".into()),
                    expected_output: "Reviewed change".into(),
                    execution: AgentExecution::default(),
                    access: AgentAccess {
                        workspace_write: true,
                        ..Default::default()
                    },
                    max_parallel_tasks: 1,
                }),
            )
            .unwrap();
        store
            .save(
                store.load().unwrap().revision,
                ControlRecord::Team(Team {
                    id: team,
                    name: "Engineering".into(),
                    objective: "Produce code".into(),
                    limits: Some(TeamRunLimits {
                        max_tokens: Some(500000),
                        ..Default::default()
                    }),
                    members: vec![TeamMember {
                        agent_id: agent,
                        parent_id: None,
                    }],
                }),
            )
            .unwrap();
        let task = Uuid::new_v4();
        let project = Uuid::new_v4();
        let mut context = WorkspaceContext {
            task_id: task,
            project_id: project,
            control_path: store.path(),
            settings_path: dir.join("settings.json"),
            mode: RuntimeMode::FullAccess,
        };
        let args = json!({"team_id":team,"definition_revision":store.load().unwrap().revision,"objective":"Implement a reviewed change"});
        let (sender, receiver) = unbounded();
        let (events, event_receiver) = crate::driver::test_event_channel();
        let mut journal = Journal::default();
        let path = dir.join("journal.json");
        let mut call = ToolCall {
            id: "invoke-one".into(),
            name: "invoke_team".into(),
            activity_id: "source-activity".into(),
            ..Default::default()
        };
        let body = execute(
            &context,
            &receiver,
            &AtomicBool::new(false),
            &events,
            &path,
            &mut journal,
            &call,
            &args,
        )
        .unwrap();
        assert!(
            !event_receiver
                .try_iter()
                .any(|e| matches!(e, DriverEvent::Permission { .. }))
        );
        let id = Uuid::parse_str(
            serde_json::from_str::<Value>(&body).unwrap()["run_id"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        let coordinator = crate::teams::TeamCoordinator::new(store.path());
        let run = coordinator.snapshot(id).unwrap();
        assert!(run.work[0].access.workspace_write);
        assert_eq!(run.limits.max_tokens, Some(500000));
        context.mode = RuntimeMode::Ask;
        assert_eq!(
            execute(
                &context,
                &receiver,
                &AtomicBool::new(false),
                &events,
                &path,
                &mut journal,
                &call,
                &args
            )
            .unwrap(),
            body
        );
        assert!(
            !event_receiver
                .try_iter()
                .any(|e| matches!(e, DriverEvent::Permission { .. }))
        );
        call.id = "invoke-denied".into();
        sender
            .send(Command::Respond(call.id.clone(), "reject".into()))
            .unwrap();
        assert!(
            execute(
                &context,
                &receiver,
                &AtomicBool::new(false),
                &events,
                &path,
                &mut journal,
                &call,
                &args
            )
            .is_err()
        );
        assert_eq!(coordinator.runs_for_source(task).unwrap().len(), 1);
        call.id = "invoke-approved".into();
        sender
            .send(Command::Respond(call.id.clone(), "allow".into()))
            .unwrap();
        execute(
            &context,
            &receiver,
            &AtomicBool::new(false),
            &events,
            &path,
            &mut journal,
            &call,
            &args,
        )
        .unwrap();
        assert_eq!(coordinator.runs_for_source(task).unwrap().len(), 2);
        assert!(event_receiver.try_iter().any(|e|matches!(e,DriverEvent::Permission{detail,..} if detail.contains("workspaceWrite") && detail.contains("500000"))));
        assert!(
            dir.starts_with(std::env::temp_dir())
                && dir
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("ion-source-team-tests-")
        );
        let _ = fs::remove_dir_all(dir);
    }
}
