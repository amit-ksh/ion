//! Antigravity's native NDJSON transport. Authentication and ordinary tools stay with agy.
use super::{
    DriverControl, DriverEventSender, DriverStartOptions,
    source_teams::{WorkspaceContext, antigravity::AntigravityBridge},
};
use crate::model::{ActivityKind, DriverEvent, ProviderResumeCursor, RuntimeMode};
use anyhow::{Context, Result, ensure};
use crossbeam_channel::{Sender, bounded};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Command, Stdio},
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

const RUN_ERROR: &str = "Antigravity stopped the run. Check native CLI authentication, permission policy and output limits.";
const POLICY_ERROR: &str =
    "Antigravity stopped the run. Check native authentication and permission policy.";
const TEAMS_UNAVAILABLE: &str = "Antigravity Teams requires native CLI 1.3.1 and an available workspace plugin directory. The selected native account, model and permission policy remain in use.";

type Configure = Arc<dyn Fn(&mut Command) + Send + Sync>;
struct Job {
    prompt: String,
    cancelled: Arc<AtomicBool>,
}
#[derive(Default)]
struct State {
    jobs: Mutex<Vec<Weak<AtomicBool>>>,
    bridge: Mutex<Option<AntigravityBridge>>,
    closed: AtomicBool,
}
impl State {
    fn cancel(&self) {
        for flag in self.jobs.lock().iter().filter_map(Weak::upgrade) {
            flag.store(true, Ordering::Release);
        }
        if let Some(bridge) = self.bridge.lock().as_ref() {
            bridge.cancel();
        }
    }
}
pub struct AntigravityDriver {
    commands: Sender<Job>,
    state: Arc<State>,
    events: DriverEventSender,
}
impl AntigravityDriver {
    pub(crate) fn start_with_workspace(
        options: DriverStartOptions,
        events: DriverEventSender,
        workspace: Option<WorkspaceContext>,
    ) -> Result<Self> {
        Self::start_configured(options, events, workspace, Arc::new(|_| {}))
    }
    pub(super) fn start_configured(
        options: DriverStartOptions,
        events: DriverEventSender,
        workspace: Option<WorkspaceContext>,
        configure: Configure,
    ) -> Result<Self> {
        let (commands, requests) = bounded::<Job>(16);
        let state = Arc::new(State::default());
        let worker = state.clone();
        let delivered = events.clone();
        thread::Builder::new()
            .name("ion-antigravity".into())
            .spawn(move || {
                let mut cursor = options.provider_cursor.clone();
                let mut setup = false;
                let _ = events.send(DriverEvent::Connected {
                    provider_cursor: cursor.clone(),
                });
                for job in requests {
                    if job.cancelled.load(Ordering::Acquire)
                        || worker.closed.load(Ordering::Acquire)
                    {
                        continue;
                    }
                    let _ = events.send(DriverEvent::TurnStarted);
                    if !setup && workspace.is_some() {
                        let result = (|| -> Result<AntigravityBridge> {
                            let mut probe = crate::command_env::command(&options.binary);
                            probe
                                .current_dir(&options.cwd)
                                .arg("--version")
                                .env("AGY_CLI_DISABLE_AUTO_UPDATE", "true");
                            configure(&mut probe);
                            let version = crate::team_process::bounded_process(
                                &mut probe,
                                b"",
                                &job.cancelled,
                                Duration::from_secs(15),
                            )?;
                            ensure!(
                                String::from_utf8(version)?.trim() == "1.3.1",
                                "Native Antigravity Teams version is not verified"
                            );
                            ensure!(!job.cancelled.load(Ordering::Acquire), "Source stopped");
                            AntigravityBridge::start(
                                workspace.clone().unwrap(),
                                events.clone(),
                                &options.cwd,
                            )
                        })();
                        if !job.cancelled.load(Ordering::Acquire) {
                            setup = true;
                            match result {
                                Ok(bridge) => *worker.bridge.lock() = Some(bridge),
                                Err(_) => {
                                    let _ = events.send(DriverEvent::Activity {
                                        id: None,
                                        kind: ActivityKind::Tool,
                                        title: "Team tools unavailable".into(),
                                        detail: Some(TEAMS_UNAVAILABLE.into()),
                                        complete: true,
                                    });
                                }
                            }
                        }
                    }
                    let bridge = worker.bridge.lock().clone();
                    let result = if job.cancelled.load(Ordering::Acquire)
                        || worker.closed.load(Ordering::Acquire)
                    {
                        Ok(false)
                    } else {
                        run(&options, &job, &events, &mut cursor, &bridge, &configure)
                    };
                    if let Some(bridge) = &bridge {
                        bridge.finish();
                    }
                    let success = match result {
                        Ok(success) => success && !job.cancelled.load(Ordering::Acquire),
                        Err(_) => {
                            if !job.cancelled.load(Ordering::Acquire) {
                                let _ = events.send(DriverEvent::Error(RUN_ERROR.into()));
                            }
                            false
                        }
                    };
                    let _ = events.send(DriverEvent::TurnFinished {
                        success,
                        summary: None,
                    });
                }
                if let Some(bridge) = worker.bridge.lock().take() {
                    bridge.close();
                }
                let _ = events.send(DriverEvent::ProcessExited);
            })?;
        Ok(Self {
            commands,
            state,
            events: delivered,
        })
    }
}
fn run(
    options: &DriverStartOptions,
    job: &Job,
    events: &DriverEventSender,
    cursor: &mut Option<ProviderResumeCursor>,
    bridge: &Option<AntigravityBridge>,
    configure: &Configure,
) -> Result<bool> {
    let mut command = crate::command_env::command(&options.binary);
    command
        .current_dir(&options.cwd)
        .args([
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(model) = options.model.as_deref().filter(|model| *model != "default") {
        command.args(["--model", model]);
    }
    let expected = match cursor {
        Some(ProviderResumeCursor::Antigravity { session_id }) => {
            command.args(["--conversation", session_id]);
            Some(session_id.clone())
        }
        _ => None,
    };
    if options.mode == RuntimeMode::FullAccess {
        command.arg("--dangerously-skip-permissions");
    }
    if bridge.is_some() {
        command.env("AGY_CLI_DISABLE_AUTO_UPDATE", "true");
    }
    configure(&mut command);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    ensure!(
        !job.cancelled.load(Ordering::Acquire),
        "Canceled before native launch"
    );
    if let Some(bridge) = bridge {
        ensure!(
            bridge.begin(job.cancelled.clone()),
            "Canceled before team launch"
        );
    }
    let mut child = crate::command_env::spawn(&mut command)?;
    #[cfg(windows)]
    let tree = match crate::command_env::ProcessTree::attach(&child) {
        Ok(tree) => tree,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let mut input = child.stdin.take().context("Native stdin unavailable")?;
    let output = child.stdout.take().context("Native stdout unavailable")?;
    let prompt = job.prompt.clone();
    let write = thread::spawn(move || -> std::io::Result<()> {
        writeln!(
            input,
            "{}",
            json!({"event":"user","message":{"content":prompt}})
        )
    });
    let read_failed = Arc::new(AtomicBool::new(false));
    let failed = read_failed.clone();
    let events = events.clone();
    let source = bridge.clone();
    let read_cancelled = job.cancelled.clone();
    let reader = thread::spawn(move || {
        let mut session = None;
        let result = (|| -> Result<bool> {
            let mut reader = BufReader::new(output);
            let (mut success, mut saw_result, mut streamed) = (false, false, false);
            loop {
                let mut line = Vec::new();
                let count = reader
                    .by_ref()
                    .take(1024 * 1024 + 1)
                    .read_until(b'\n', &mut line)?;
                if count == 0 {
                    break;
                }
                ensure!(
                    line.len() <= 1024 * 1024,
                    "Native event exceeded its byte limit"
                );
                let event: Value = match serde_json::from_slice(&line) {
                    Ok(event) => event,
                    Err(_) => continue,
                };
                let decoded = decode_event(&event);
                if let Some(source) = &source {
                    source.observe(&event)?;
                }
                if let Some(id) = &decoded.cursor {
                    ensure!(
                        expected.as_ref().is_none_or(|expected| expected == id),
                        "Native resumed conversation changed"
                    );
                    session = Some(id.clone());
                    let _ = events.send(DriverEvent::Connected {
                        provider_cursor: Some(ProviderResumeCursor::Antigravity {
                            session_id: id.clone(),
                        }),
                    });
                }
                if let Some(text) = decoded.text {
                    streamed = true;
                    let _ = events.send(DriverEvent::TextDelta(text));
                }
                if let Some(name) = decoded.tool {
                    let _ = events.send(DriverEvent::Activity {
                        id: None,
                        kind: ActivityKind::Tool,
                        title: name,
                        detail: None,
                        complete: true,
                    });
                }
                if let Some((ok, text)) = decoded.result {
                    saw_result = true;
                    success = ok;
                    if !ok && !read_cancelled.load(Ordering::Acquire) {
                        let _ = events.send(DriverEvent::Error(POLICY_ERROR.into()));
                    }
                    if !streamed && let Some(text) = text {
                        let _ = events.send(DriverEvent::TextDelta(text));
                    }
                }
            }
            Ok(success && saw_result)
        })();
        // EOF and read/protocol errors revoke the old source authority promptly.
        if let Some(source) = &source {
            source.finish();
        }
        if result.is_err() {
            failed.store(true, Ordering::Release);
        }
        (result, session)
    });
    let status = loop {
        if job.cancelled.load(Ordering::Acquire) || read_failed.load(Ordering::Acquire) {
            break None;
        }
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(_) => break None,
        }
    };
    #[cfg(unix)]
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    #[cfg(windows)]
    drop(tree);
    let _ = child.kill();
    let _ = child.wait();
    let written = write
        .join()
        .map_err(|_| anyhow::anyhow!("Native stdin worker failed"))?;
    let (read, session) = reader
        .join()
        .map_err(|_| anyhow::anyhow!("Native stdout worker failed"))?;
    // Keep the acknowledged native conversation even if Stop closes a pipe
    // during a read. A subsequent explicit prompt resumes this conversation.
    if let Some(session_id) = session {
        *cursor = Some(ProviderResumeCursor::Antigravity { session_id });
    }
    if job.cancelled.load(Ordering::Acquire) {
        return Ok(false);
    }
    let success = read?;
    written?;
    Ok(success && status.is_some_and(|status| status.success()))
}
#[derive(Default)]
struct Decoded {
    cursor: Option<String>,
    text: Option<String>,
    tool: Option<String>,
    result: Option<(bool, Option<String>)>,
}
fn decode_event(value: &Value) -> Decoded {
    let mut event = Decoded::default();
    match value["event"].as_str() {
        Some("init") => event.cursor = value["conversation_id"].as_str().map(str::to_owned),
        Some("step_update") => {
            let step = &value["step_update"];
            if step["step_type"] == "agent_response" {
                event.text = step["text_delta"].as_str().map(str::to_owned);
            }
            if step["step_type"] == "tool" && step["state"] == "DONE" {
                event.tool = step["tool_name"].as_str().map(str::to_owned);
            }
        }
        Some("result") => {
            event.result = Some((
                value["result"]["status"] == "SUCCESS"
                    && value["result"]["denied_actions"]
                        .as_array()
                        .is_none_or(Vec::is_empty),
                value["result"]["response"].as_str().map(str::to_owned),
            ))
        }
        _ => {}
    }
    event
}
impl DriverControl for AntigravityDriver {
    fn prompt(&self, prompt: String) {
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut jobs = self.state.jobs.lock();
        jobs.retain(|flag| flag.strong_count() > 0);
        jobs.push(Arc::downgrade(&cancelled));
        if self.commands.try_send(Job { prompt, cancelled }).is_err() {
            let _ = self.events.send(DriverEvent::Error(
                "Antigravity's prompt queue is full or closed.".into(),
            ));
        }
    }
    fn cancel(&self) {
        self.state.cancel();
    }
    fn respond(&self, id: String, option: String) {
        if let Some(bridge) = self.state.bridge.lock().as_ref() {
            bridge.respond(&id, &option);
        }
    }
    fn rollback(&self, _: usize) -> Result<Option<ProviderResumeCursor>> {
        anyhow::bail!("Antigravity does not expose turn rollback")
    }
}
impl Drop for AntigravityDriver {
    fn drop(&mut self) {
        self.state.closed.store(true, Ordering::Release);
        self.state.cancel();
        if let Some(bridge) = self.state.bridge.lock().as_ref() {
            bridge.close();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_documented_public_stream_without_private_steps() {
        assert_eq!(
            decode_event(&json!({"event":"init","conversation_id":"abc"}))
                .cursor
                .as_deref(),
            Some("abc")
        );
        assert_eq!(decode_event(&json!({"event":"step_update","step_update":{"step_type":"agent_response","text_delta":"hello"}})).text.as_deref(),Some("hello"));
        assert!(decode_event(&json!({"event":"step_update","step_update":{"step_type":"thinking","text_delta":"private"}})).text.is_none());
        assert!(
            decode_event(
                &json!({"event":"result","result":{"status":"SUCCESS","response":"done"}})
            )
            .result
            .unwrap()
            .0
        );
        assert!(!decode_event(&json!({"event":"result","result":{"status":"SUCCESS","denied_actions":[{"action":"mcp"}]}})).result.unwrap().0);
    }
}
