use super::*;
use waku_protocol::control::*;

pub(crate) struct WorkspaceContext {
    pub task_id: Uuid,
    pub project_id: Uuid,
    pub control_path: PathBuf,
    pub settings_path: PathBuf,
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
    let store = crate::control::ControlStore::new(context.control_path.clone());
    let result = match call.name.as_str() {
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
    fn sandbox_is_hidden_by_default_and_switch_is_read_live() {
        let path = std::env::temp_dir().join(format!("ion-sandbox-test-{}.json", Uuid::new_v4()));
        let context = WorkspaceContext {
            task_id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            control_path: path.with_extension("db"),
            settings_path: path.clone(),
        };
        fs::write(&path, b"{}").unwrap();
        assert!(context.sandbox_session().is_err());
        fs::write(&path, b"{\"nebius_sandbox_enabled\":true}").unwrap();
        assert_eq!(context.sandbox_session().unwrap(), "ion");
        fs::write(&path, b"{\"nebius_sandbox_enabled\":false}").unwrap();
        assert!(context.sandbox_session().is_err());
        fs::remove_file(path).unwrap();
    }
}
