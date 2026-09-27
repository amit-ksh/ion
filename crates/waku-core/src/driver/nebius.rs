//! Native Nebius Token Factory coding transport. The journal contains model
//! messages and tool results, never credentials. A started write is never
//! replayed after a crash because its outcome might be uncertain.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow, bail};
use crossbeam_channel::{Receiver, Sender, unbounded};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{DriverControl, DriverEventSender, DriverStartOptions, SessionOptions};
use crate::credentials::{CredentialService, TOKEN_FACTORY_BASE_URL};
use crate::model::{ActivityKind, DriverEvent, PermissionOption, ProviderResumeCursor};

const MAX_FILE_BYTES: u64 = 256 * 1024;
const MAX_EDIT_BYTES: usize = 16 * 1024;
const MAX_STEPS: usize = 16;
const PREFERRED_MODEL: &str = "nvidia/NVIDIA-Nemotron-3-Super-120B-A12B";

enum Command {
    Prompt(String),
    Respond(String, String),
    Stop,
}

#[derive(Default, Serialize, Deserialize)]
struct Journal {
    messages: Vec<Value>,
    /// Written before a workspace mutation. On restart, require review rather
    /// than guessing whether the effect happened and risking a duplicate.
    pending_effect: Option<String>,
}

#[derive(Default)]
struct ToolCall {
    id: String,
    name: String,
    arguments: String,
    activity_id: String,
    announced: bool,
}

pub struct NebiusDriver {
    commands: Sender<Command>,
    cancelled: Arc<AtomicBool>,
}

impl NebiusDriver {
    pub fn start(options: DriverStartOptions, events: DriverEventSender) -> Result<Self> {
        let cwd = fs::canonicalize(&options.cwd).context("Nebius workspace is unavailable")?;
        if !cwd.is_dir() {
            bail!("Nebius workspace is not a directory");
        }
        let (session_id, resuming) = match options.provider_cursor {
            Some(ProviderResumeCursor::Nebius { session_id }) => (
                Uuid::parse_str(&session_id).context("invalid Nebius session cursor")?,
                true,
            ),
            Some(cursor) => bail!(
                "cannot resume Nebius from a {} cursor",
                cursor.provider().display_name()
            ),
            None => (Uuid::new_v4(), false),
        };
        let journal_path = journal_path(session_id)?;
        if resuming && !journal_path.exists() {
            bail!("Nebius task journal is missing; cannot safely resume this task")
        }
        let mut journal = if journal_path.exists() {
            serde_json::from_slice(&fs::read(&journal_path)?)
                .context("could not load the Nebius task journal")?
        } else {
            Journal::default()
        };
        reconcile_incomplete_calls(&cwd, &journal_path, &mut journal)?;

        let key = CredentialService::global().nebius_runtime_key()?;
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;
        let response = client
            .get(format!("{TOKEN_FACTORY_BASE_URL}/models"))
            .bearer_auth(&key)
            .send()
            .context("could not verify the Nebius model catalog")?;
        if !response.status().is_success() {
            bail!(
                "Nebius model verification failed (HTTP {})",
                response.status()
            );
        }
        let catalog: Value = response.json().context("invalid Nebius model catalog")?;
        let ids: Vec<&str> = catalog["data"]
            .as_array()
            .ok_or_else(|| anyhow!("Nebius model catalog has no data array"))?
            .iter()
            .filter_map(|entry| entry["id"].as_str())
            .collect();
        let model = if let Some(requested) = options.model {
            if !ids.contains(&requested.as_str()) {
                bail!(
                    "Selected Nebius model is absent from the live catalog; refresh Models & Speech"
                );
            }
            requested
        } else if ids.contains(&PREFERRED_MODEL) {
            PREFERRED_MODEL.to_owned()
        } else {
            ids.iter()
                .find(|id| id.to_ascii_lowercase().contains("nemotron"))
                .ok_or_else(|| anyhow!("No NVIDIA Nemotron model is available to this Nebius key"))?
                .to_string()
        };

        let (commands, receiver) = unbounded();
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        thread::Builder::new()
            .name(format!("nebius-{session_id}"))
            .spawn(move || {
                let _ = events.send(DriverEvent::Connected {
                    provider_cursor: Some(ProviderResumeCursor::Nebius {
                        session_id: session_id.to_string(),
                    }),
                });
                run_worker(
                    receiver,
                    worker_cancelled,
                    events,
                    client,
                    key,
                    model,
                    cwd,
                    journal_path,
                    journal,
                );
            })
            .context("could not start the Nebius worker")?;
        Ok(Self {
            commands,
            cancelled,
        })
    }
}

impl DriverControl for NebiusDriver {
    fn prompt(&self, prompt: String) {
        let _ = self.commands.send(Command::Prompt(prompt));
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
        let _ = self.commands.send(Command::Stop);
    }

    fn respond(&self, request_id: String, option_id: String) {
        let _ = self.commands.send(Command::Respond(request_id, option_id));
    }

    fn apply_options(&self, _options: SessionOptions) -> bool {
        false
    }

    fn rollback(&self, _turns: usize) -> Result<Option<ProviderResumeCursor>> {
        bail!("Nebius conversation rewind is not supported")
    }
}

fn run_worker(
    receiver: Receiver<Command>,
    cancelled: Arc<AtomicBool>,
    events: DriverEventSender,
    client: reqwest::blocking::Client,
    key: String,
    model: String,
    cwd: PathBuf,
    journal_path: PathBuf,
    mut journal: Journal,
) {
    while let Ok(command) = receiver.recv() {
        let prompt = match command {
            Command::Prompt(prompt) => prompt,
            Command::Stop => {
                cancelled.store(false, Ordering::Relaxed);
                continue;
            }
            Command::Respond(_, _) => continue,
        };
        if cancelled.load(Ordering::Relaxed) {
            let _ = events.send(DriverEvent::TurnFinished {
                success: false,
                summary: Some("Nebius task stopped".into()),
            });
            continue;
        }
        let _ = events.send(DriverEvent::TurnStarted);
        let outcome = run_turn(
            &receiver,
            &cancelled,
            &events,
            &client,
            &key,
            &model,
            &cwd,
            &journal_path,
            &mut journal,
            prompt,
        );
        match outcome {
            Ok(()) => {
                let _ = events.send(DriverEvent::TurnFinished {
                    success: true,
                    summary: None,
                });
            }
            Err(error) => {
                let message = error.to_string();
                let _ = events.send(DriverEvent::Error(message.clone()));
                let _ = events.send(DriverEvent::TurnFinished {
                    success: false,
                    summary: Some(message),
                });
            }
        }
    }
    let _ = events.send(DriverEvent::ProcessExited);
}

#[allow(clippy::too_many_arguments)]
fn run_turn(
    receiver: &Receiver<Command>,
    cancelled: &AtomicBool,
    events: &DriverEventSender,
    client: &reqwest::blocking::Client,
    key: &str,
    model: &str,
    cwd: &Path,
    journal_path: &Path,
    journal: &mut Journal,
    prompt: String,
) -> Result<()> {
    if journal.pending_effect.is_some() {
        bail!("Review the uncertain Nebius file action before continuing");
    }
    let repeated_after_failure = journal
        .messages
        .last()
        .is_some_and(|message| message["role"] == "user" && message["content"] == prompt);
    if !repeated_after_failure {
        journal
            .messages
            .push(json!({"role": "user", "content": prompt}));
        persist(journal_path, journal)?;
    }
    for _ in 0..MAX_STEPS {
        if cancelled.load(Ordering::Relaxed) {
            bail!("Nebius task stopped")
        }
        let (text, calls) = completion(client, key, model, &journal.messages, events, cancelled)?;
        let empty_text = text.is_empty();
        let mut message = json!({"role": "assistant", "content": text});
        if !calls.is_empty() {
            if empty_text {
                message["content"] = Value::Null
            }
            message["tool_calls"] = Value::Array(
                calls
                    .iter()
                    .map(|call| {
                        json!({
                            "id": call.id,
                            "type": "function",
                            "function": {"name": call.name, "arguments": call.arguments}
                        })
                    })
                    .collect(),
            );
        }
        journal.messages.push(message);
        persist(journal_path, journal)?;
        if calls.is_empty() {
            return Ok(());
        }
        for call in calls {
            if cancelled.load(Ordering::Relaxed) {
                bail!("Nebius task stopped")
            }
            let result = execute_tool(
                receiver,
                cancelled,
                events,
                cwd,
                journal_path,
                journal,
                &call,
            );
            if journal.pending_effect.is_some() && result.is_err() {
                return result.map(|_| ());
            }
            let result = result.unwrap_or_else(|error| format!("Tool failed: {error}"));
            journal.messages.push(json!({
                "role": "tool", "tool_call_id": call.id, "name": call.name, "content": result
            }));
            let pending_effect = journal.pending_effect.take();
            if let Err(error) = persist(journal_path, journal) {
                journal.pending_effect = pending_effect;
                journal.messages.pop();
                return Err(error);
            }
        }
    }
    bail!("Nebius reached the 16-step task limit; send a new prompt to continue")
}

fn completion(
    client: &reqwest::blocking::Client,
    key: &str,
    model: &str,
    messages: &[Value],
    events: &DriverEventSender,
    cancelled: &AtomicBool,
) -> Result<(String, Vec<ToolCall>)> {
    let tools = tool_definitions();
    let mut request_messages = vec![json!({
        "role": "system",
        "content": "You are ion's coding assistant. Use tools only for task-relevant local files. Read before editing. A file edit needs user approval. Never put secrets or internal tool markers in final text. Keep answers concise."
    })];
    request_messages.extend_from_slice(messages);
    let response = client
        .post(format!("{TOKEN_FACTORY_BASE_URL}/chat/completions"))
        .bearer_auth(key)
        .json(&json!({
            "model": model, "messages": request_messages, "tools": tools,
            "tool_choice": "auto", "stream": true
        }))
        .send()
        .context("Nebius completion request failed")?;
    if !response.status().is_success() {
        bail!("Nebius completion failed (HTTP {})", response.status());
    }
    parse_stream(BufReader::new(response), events, cancelled)
}

fn parse_stream(
    reader: impl BufRead,
    events: &DriverEventSender,
    cancelled: &AtomicBool,
) -> Result<(String, Vec<ToolCall>)> {
    let mut text = String::new();
    let mut calls: Vec<ToolCall> = Vec::new();
    let mut done = false;
    for line in reader.lines() {
        if cancelled.load(Ordering::Relaxed) {
            bail!("Nebius task stopped")
        }
        let line = line.context("Nebius stream ended unexpectedly")?;
        let Some(data) = line.strip_prefix("data:").map(str::trim_start) else {
            continue;
        };
        if data == "[DONE]" {
            done = true;
            break;
        }
        let packet: Value = serde_json::from_str(data).context("invalid Nebius stream packet")?;
        if let Some(message) = packet["error"]["message"].as_str() {
            bail!("Nebius stream error: {message}");
        }
        let delta = &packet["choices"][0]["delta"];
        if let Some(reasoning) = delta["reasoning_content"].as_str() {
            let _ = events.send(DriverEvent::ReasoningDelta(reasoning.to_owned()));
        }
        if let Some(content) = delta["content"].as_str() {
            text.push_str(content);
            let _ = events.send(DriverEvent::TextDelta(content.to_owned()));
        }
        if let Some(parts) = delta["tool_calls"].as_array() {
            for part in parts {
                let index = part["index"]
                    .as_u64()
                    .ok_or_else(|| anyhow!("Nebius tool call has no index"))?
                    as usize;
                if index > 31 {
                    bail!("Nebius returned too many tool calls")
                }
                while calls.len() <= index {
                    let mut call = ToolCall::default();
                    call.activity_id = format!("nebius-tool-{}", Uuid::new_v4());
                    calls.push(call)
                }
                let call = &mut calls[index];
                if !call.announced {
                    let _ = events.send(DriverEvent::Activity {
                        id: Some(call.activity_id.clone()),
                        kind: ActivityKind::Tool,
                        title: "Preparing Nebius tool call".into(),
                        detail: None,
                        complete: false,
                    });
                    call.announced = true;
                }
                if let Some(id) = part["id"].as_str() {
                    call.id.push_str(id)
                }
                if let Some(name) = part["function"]["name"].as_str() {
                    call.name.push_str(name)
                }
                if let Some(arguments) = part["function"]["arguments"].as_str() {
                    call.arguments.push_str(arguments)
                }
            }
        }
        if let Some(usage) = packet.get("usage") {
            let context_tokens = usage["prompt_tokens"]
                .as_u64()
                .zip(usage["completion_tokens"].as_u64())
                .map(|(prompt, completion)| prompt.saturating_add(completion));
            if context_tokens.is_some() {
                let _ = events.send(DriverEvent::UsageUpdated {
                    context_tokens,
                    context_window: None,
                });
            }
        }
    }
    if !done {
        bail!("Nebius stream ended without completion; retry the task")
    }
    for call in &calls {
        if call.id.is_empty() || call.name.is_empty() {
            bail!("Nebius returned an incomplete tool call")
        }
    }
    Ok((text, calls))
}

fn tool_definitions() -> Value {
    json!([
        {"type":"function","function":{"name":"list_files","description":"List relative project files (at most 200). Optionally narrow to a directory.","parameters":{"type":"object","properties":{"directory":{"type":"string"}},"additionalProperties":false}}},
        {"type":"function","function":{"name":"read_file","description":"Read one UTF-8 project file, up to 256 KiB","parameters":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"write_file","description":"Replace one project file after user approval. Supply its exact old content, or an empty string to create a new file.","parameters":{"type":"object","properties":{"path":{"type":"string"},"old_content":{"type":"string"},"new_content":{"type":"string"}},"required":["path","old_content","new_content"],"additionalProperties":false}}}
    ])
}

fn execute_tool(
    receiver: &Receiver<Command>,
    cancelled: &AtomicBool,
    events: &DriverEventSender,
    cwd: &Path,
    journal_path: &Path,
    journal: &mut Journal,
    call: &ToolCall,
) -> Result<String> {
    let args: Value = serde_json::from_str(&call.arguments).context("invalid tool arguments")?;
    let kind = ActivityKind::from_tool_name(&call.name);
    let title = match call.name.as_str() {
        "list_files" => "List project files".to_owned(),
        "read_file" => format!("Read {}", required_str(&args, "path")?),
        "write_file" => format!("Edit {}", required_str(&args, "path")?),
        _ => bail!("unrecognized Nebius tool"),
    };
    let _ = events.send(DriverEvent::Activity {
        id: Some(call.activity_id.clone()),
        kind,
        title: title.clone(),
        detail: None,
        complete: false,
    });
    let result = match call.name.as_str() {
        "list_files" => {
            let mut names = Vec::new();
            let directory = args["directory"]
                .as_str()
                .map(|relative| checked_path(cwd, relative))
                .transpose()?
                .unwrap_or_else(|| cwd.to_owned());
            if !directory.is_dir() {
                bail!("not a project directory")
            }
            visit_files(cwd, &directory, &mut names)?;
            names.join("\n")
        }
        "read_file" => {
            let path = checked_path(cwd, required_str(&args, "path")?)?;
            if fs::metadata(&path)?.len() > MAX_FILE_BYTES {
                bail!("file exceeds 256 KiB")
            }
            fs::read_to_string(path).context("could not read UTF-8 project file")?
        }
        "write_file" => {
            let relative = required_str(&args, "path")?;
            let path = checked_path(cwd, relative)?;
            let old = required_str(&args, "old_content")?;
            let new = required_str(&args, "new_content")?;
            if new.len() > MAX_EDIT_BYTES || old.len() > MAX_EDIT_BYTES {
                bail!("Nebius file edit exceeds the 16 KiB approval limit")
            }
            let current = match fs::read_to_string(&path) {
                Ok(content) => content,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(error) => return Err(error.into()),
            };
            if current != old {
                bail!("file changed since it was read; read it again")
            }
            let request_id = call.id.clone();
            let _ = events.send(DriverEvent::Permission {
                request_id: request_id.clone(),
                title: format!("Allow Nebius to edit {relative}?"),
                detail: format!("Proposed complete content for {relative}:\n\n{new}"),
                options: vec![
                    PermissionOption {
                        id: "allow".into(),
                        label: "Allow once".into(),
                        allow: true,
                    },
                    PermissionOption {
                        id: "reject".into(),
                        label: "Deny".into(),
                        allow: false,
                    },
                ],
            });
            loop {
                if cancelled.load(Ordering::Relaxed) {
                    bail!("Nebius task stopped")
                }
                match receiver.recv_timeout(Duration::from_millis(200)) {
                    Ok(Command::Respond(id, decision)) if id == request_id => {
                        if decision != "allow" {
                            bail!("user denied file edit")
                        }
                        break;
                    }
                    Ok(Command::Stop) => bail!("Nebius task stopped"),
                    Ok(_) => {}
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                    Err(_) => bail!("Nebius task disconnected while waiting for approval"),
                }
            }
            // Mark the effect before touching the filesystem. An interrupted
            // write is left for human review and never automatically replayed.
            journal.pending_effect = Some(call.id.clone());
            persist(journal_path, journal)?;
            let latest = match fs::read_to_string(&path) {
                Ok(content) => content,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(error) => return Err(error.into()),
            };
            if latest != old {
                bail!("file changed while awaiting approval")
            }
            if !path.parent().is_some_and(Path::is_dir) {
                bail!("parent directory does not exist")
            }
            fs::write(&path, new).context("could not write approved file edit")?;
            format!("Wrote {relative}")
        }
        _ => unreachable!(),
    };
    let detail = match call.name.as_str() {
        "read_file" => format!("Read {} bytes", result.len()),
        "list_files" => format!("Listed {} files", result.lines().count()),
        _ => result.clone(),
    };
    let _ = events.send(DriverEvent::Activity {
        id: Some(call.activity_id.clone()),
        kind,
        title,
        detail: Some(detail),
        complete: true,
    });
    Ok(result)
}

fn required_str<'a>(args: &'a Value, field: &str) -> Result<&'a str> {
    args.get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing {field}"))
}

fn checked_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let relative = Path::new(relative);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        bail!("tool path must be a project-relative file path")
    }
    let mut path = root.to_owned();
    for part in relative.components() {
        if let Component::Normal(name) = part {
            let name = name.to_string_lossy().to_ascii_lowercase();
            if name == ".git"
                || name == ".ssh"
                || name == ".aws"
                || name == ".env"
                || name.starts_with(".env.")
                || name == ".npmrc"
                || name == ".pypirc"
                || name == "credentials.json"
                || name == "secrets.json"
                || name.ends_with(".pem")
                || name.ends_with(".p12")
                || name.ends_with(".pfx")
            {
                bail!("tool path is private or outside task scope")
            }
        }
        path.push(part);
        if let Ok(metadata) = fs::symlink_metadata(&path) {
            if metadata.file_type().is_symlink() {
                bail!("tool path contains a symlink")
            }
        }
    }
    Ok(path)
}

fn visit_files(root: &Path, directory: &Path, names: &mut Vec<String>) -> Result<()> {
    if names.len() >= 200 {
        return Ok(());
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if names.len() >= 200 {
            break;
        }
        let kind = entry.file_type()?;
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if kind.is_symlink()
            || matches!(
                name.as_str(),
                ".git" | "node_modules" | "target" | ".waku-cache" | ".ssh" | ".aws"
            )
            || name == ".env"
            || name.starts_with(".env.")
            || name == ".npmrc"
            || name == ".pypirc"
            || name == "credentials.json"
            || name == "secrets.json"
            || name.ends_with(".pem")
            || name.ends_with(".p12")
            || name.ends_with(".pfx")
        {
            continue;
        }
        if kind.is_dir() {
            visit_files(root, &entry.path(), names)?
        } else if kind.is_file() {
            names.push(
                entry
                    .path()
                    .strip_prefix(root)?
                    .to_string_lossy()
                    .to_string(),
            );
        }
    }
    Ok(())
}

fn reconcile_incomplete_calls(cwd: &Path, path: &Path, journal: &mut Journal) -> Result<()> {
    let Some(assistant) = journal
        .messages
        .iter()
        .rfind(|message| message["role"] == "assistant" && message["tool_calls"].is_array())
    else {
        if journal.pending_effect.is_some() {
            bail!("Nebius journal has an unrecognized pending file action")
        }
        return Ok(());
    };
    let calls = assistant["tool_calls"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut changed = false;
    for call in calls {
        let id = call["id"]
            .as_str()
            .ok_or_else(|| anyhow!("invalid Nebius journal tool ID"))?;
        let already_recorded = journal
            .messages
            .iter()
            .any(|message| message["role"] == "tool" && message["tool_call_id"] == id);
        if already_recorded {
            continue;
        }
        let outcome = if journal.pending_effect.as_deref() == Some(id) {
            let args: Value =
                serde_json::from_str(call["function"]["arguments"].as_str().unwrap_or(""))
                    .context("invalid pending Nebius file action")?;
            if call["function"]["name"] != "write_file" {
                bail!("Nebius journal has an unrecognized pending action")
            }
            let file = checked_path(cwd, required_str(&args, "path")?)?;
            let old = required_str(&args, "old_content")?;
            let new = required_str(&args, "new_content")?;
            let current = match fs::read_to_string(file) {
                Ok(content) => content,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(error) => return Err(error.into()),
            };
            if current == new {
                "Approved file edit was present after interruption; no edit was repeated".to_owned()
            } else if current == old {
                "Approved file edit was not present after interruption; no edit was repeated"
                    .to_owned()
            } else {
                bail!(
                    "An interrupted Nebius file edit has an uncertain result. Review the workspace file and restore either its previous or proposed content before resuming."
                )
            }
        } else {
            "Tool call was interrupted before execution; no action was repeated".to_owned()
        };
        journal.messages.push(json!({
            "role": "tool", "tool_call_id": id, "content": outcome,
        }));
        changed = true;
    }
    if changed || journal.pending_effect.is_some() {
        journal.pending_effect = None;
        persist(path, journal)?;
    }
    Ok(())
}

fn journal_path(id: Uuid) -> Result<PathBuf> {
    let base =
        dirs::data_local_dir().ok_or_else(|| anyhow!("local data directory is unavailable"))?;
    let directory = base.join("ion").join("nebius-sessions");
    fs::create_dir_all(&directory)?;
    Ok(directory.join(format!("{id}.json")))
}

fn persist(path: &Path, journal: &Journal) -> Result<()> {
    let staging = path.with_extension("json.pending");
    fs::write(&staging, serde_json::to_vec(journal)?)?;
    fs::rename(&staging, path).context("could not checkpoint Nebius task")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streamed_tool_fragments_keep_model_order() {
        let (events, received) = super::super::test_event_channel();
        let packets = [
            json!({"choices":[{"delta":{"content":"Checking. "}}]}),
            json!({"choices":[{"delta":{"tool_calls":[{
                "index":0,"id":"call-a","function":{"name":"read_file","arguments":"{\"path\":"}
            }]}}]}),
            json!({"choices":[{"delta":{"tool_calls":[{
                "index":0,"function":{"arguments":"\"src/lib.rs\"}"}
            }]}}]}),
        ];
        let input = format!(
            "data: {}\n\ndata: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
            packets[0], packets[1], packets[2],
        );
        let (text, calls) =
            parse_stream(input.as_bytes(), &events, &AtomicBool::new(false)).unwrap();
        assert_eq!(text, "Checking. ");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call-a");
        assert_eq!(calls[0].name, "read_file");
        assert_eq!(calls[0].arguments, "{\"path\":\"src/lib.rs\"}");
        assert!(
            matches!(received.try_recv(), Ok(DriverEvent::TextDelta(delta)) if delta == "Checking. ")
        );
        assert!(matches!(
            received.try_recv(),
            Ok(DriverEvent::Activity {
                complete: false,
                ..
            })
        ));
    }

    #[test]
    fn tools_reject_escape_and_private_paths() {
        let root = Path::new("C:/project");
        assert!(checked_path(root, "src/lib.rs").is_ok());
        assert!(checked_path(root, "../other.txt").is_err());
        assert!(checked_path(root, ".git/config").is_err());
        assert!(checked_path(root, ".env.local").is_err());
    }

    #[test]
    fn resume_records_completed_write_without_reapplying_it() {
        let temporary = std::env::temp_dir();
        let root = temporary.join(format!("waku-nebius-test-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let file = root.join("note.txt");
        fs::write(&file, "updated").unwrap();
        let path = root.join("journal.json");
        let mut journal = Journal {
            messages: vec![json!({
                "role": "assistant",
                "tool_calls": [{
                    "id": "call-write",
                    "type": "function",
                    "function": {
                        "name": "write_file",
                        "arguments": "{\"path\":\"note.txt\",\"old_content\":\"original\",\"new_content\":\"updated\"}"
                    }
                }]
            })],
            pending_effect: Some("call-write".into()),
        };
        persist(&path, &journal).unwrap();
        reconcile_incomplete_calls(&root, &path, &mut journal).unwrap();
        assert!(journal.pending_effect.is_none());
        assert_eq!(fs::read_to_string(&file).unwrap(), "updated");
        assert_eq!(journal.messages[1]["tool_call_id"], "call-write");
        assert!(
            journal.messages[1]["content"]
                .as_str()
                .unwrap()
                .contains("no edit was repeated")
        );
        assert!(root.starts_with(&temporary));
        assert!(
            root.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("waku-nebius-test-")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
