//! Antigravity's documented NDJSON print transport. CLI auth stays with agy.
use super::{DriverControl, DriverEventSender, DriverStartOptions};
use crate::model::{ActivityKind, DriverEvent, ProviderResumeCursor, RuntimeMode};
use crossbeam_channel::{Sender, unbounded};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::Stdio;
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

pub struct AntigravityDriver {
    commands: Sender<String>,
    pid: Arc<AtomicU32>,
}

struct RunningChild(std::process::Child);
impl std::ops::Deref for RunningChild {
    type Target = std::process::Child;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for RunningChild {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Drop for RunningChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl AntigravityDriver {
    pub fn start(options: DriverStartOptions, events: DriverEventSender) -> anyhow::Result<Self> {
        let (commands, requests) = unbounded::<String>();
        let pid = Arc::new(AtomicU32::new(0));
        let active = pid.clone();
        std::thread::Builder::new().name("ion-antigravity".into()).spawn(move || {
            let mut cursor = options.provider_cursor;
            let _ = events.send(DriverEvent::Connected { provider_cursor: cursor.clone() });
            for prompt in requests {
                let _ = events.send(DriverEvent::TurnStarted);
                let result = (|| -> anyhow::Result<bool> {
                    let mut command = crate::command_env::command(&options.binary);
                    command.current_dir(&options.cwd)
                        .args(["--input-format", "stream-json", "--output-format", "stream-json"])
                        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
                    if let Some(model) = options.model.as_deref().filter(|id| *id != "default") { command.args(["--model", model]); }
                    if let Some(ProviderResumeCursor::Antigravity { session_id }) = &cursor { command.args(["--conversation", session_id]); }
                    // Only the user's explicit Full Access mode disables CLI review.
                    if options.mode == RuntimeMode::FullAccess { command.arg("--dangerously-skip-permissions"); }
                    let mut child = RunningChild(command.spawn()?);
                    active.store(child.id(), Ordering::Release);
                    let mut input = child.stdin.take().ok_or_else(|| anyhow::anyhow!("Antigravity stdin unavailable"))?;
                    writeln!(input, "{}", json!({"event":"user","message":{"content":prompt}}))?;
                    drop(input);
                    let output = child.stdout.take().ok_or_else(|| anyhow::anyhow!("Antigravity stdout unavailable"))?;
                    let mut succeeded = false;
                    let mut streamed = false;
                    let mut saw_result = false;
                    for line in BufReader::new(output).lines() {
                        let event: Value = match serde_json::from_str(&line?) { Ok(value) => value, Err(_) => continue };
                        let decoded = decode_event(&event);
                        if let Some(id) = decoded.cursor {
                            cursor = Some(ProviderResumeCursor::Antigravity { session_id: id });
                            let _ = events.send(DriverEvent::Connected { provider_cursor: cursor.clone() });
                        }
                        if let Some(text) = decoded.text { streamed = true; let _ = events.send(DriverEvent::TextDelta(text)); }
                        if let Some(name) = decoded.tool { let _ = events.send(DriverEvent::Activity { id: None, kind: ActivityKind::Tool, title: name, detail: None, complete: true }); }
                        if let Some(result) = decoded.result {
                            saw_result = true; succeeded = result.0;
                            if !streamed && let Some(text) = result.1 { let _ = events.send(DriverEvent::TextDelta(text)); }
                            if !succeeded { let _ = events.send(DriverEvent::Error("Antigravity stopped the run. Check CLI authentication and permission policy.".into())); }
                        }
                    }
                    let status = child.wait()?;
                    Ok(status.success() && succeeded && saw_result)
                })();
                active.store(0, Ordering::Release);
                let success = match result { Ok(success) => success, Err(error) => { let _ = events.send(DriverEvent::Error(error.to_string())); false } };
                let _ = events.send(DriverEvent::TurnFinished { success, summary: None });
            }
            let _ = events.send(DriverEvent::ProcessExited);
        })?;
        Ok(Self { commands, pid })
    }
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
            // Preserve only public agent response text; never expose private reasoning.
            if step["step_type"] == "agent_response" {
                event.text = step["text_delta"].as_str().map(str::to_owned);
            }
            if step["step_type"] == "tool" && step["state"] == "DONE" {
                event.tool = step["tool_name"].as_str().map(str::to_owned);
            }
        }
        Some("result") => {
            event.result = Some((
                value["result"]["status"] == "SUCCESS",
                value["result"]["response"].as_str().map(str::to_owned),
            ));
        }
        _ => {}
    }
    event
}
impl DriverControl for AntigravityDriver {
    fn prompt(&self, prompt: String) {
        let _ = self.commands.send(prompt);
    }
    fn cancel(&self) {
        let pid = self.pid.load(Ordering::Acquire);
        if pid == 0 {
            return;
        }
        #[cfg(windows)]
        let _ = crate::command_env::command("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        #[cfg(unix)]
        let _ = crate::command_env::command("/bin/kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
    fn respond(&self, _request_id: String, _option_id: String) {}
    fn rollback(&self, _: usize) -> anyhow::Result<Option<ProviderResumeCursor>> {
        anyhow::bail!("Antigravity does not expose turn rollback")
    }
}
impl Drop for AntigravityDriver {
    fn drop(&mut self) {
        self.cancel();
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
        assert_eq!(decode_event(&json!({"event":"step_update","step_update":{"step_type":"agent_response","text_delta":"hello"}})).text.as_deref(), Some("hello"));
        assert!(decode_event(&json!({"event":"step_update","step_update":{"step_type":"thinking","text_delta":"private"}})).text.is_none());
        assert!(
            decode_event(
                &json!({"event":"result","result":{"status":"SUCCESS","response":"done"}})
            )
            .result
            .unwrap()
            .0
        );
    }
}
