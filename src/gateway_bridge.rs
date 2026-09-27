//! Outbound desktop-to-gateway task mirror. Runs only on a dedicated worker;
//! no gateway or daemon I/O is reachable from a GPUI frame.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use uuid::Uuid;
use waku_client::{Command, DaemonClient, DaemonSupervisor, ResponsePayload};

use crate::credentials::{CredentialService, GatewayApi, GatewayRelayTask};
use crate::model::{ActivityKind, AgentSession, MessageRole};

pub struct GatewayBridge {
    stop: Arc<AtomicBool>,
}

impl GatewayBridge {
    pub fn start(daemon: DaemonSupervisor) -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        std::thread::Builder::new()
            .name("ion-gateway-bridge".into())
            .spawn(move || bridge_loop(daemon, worker_stop))
            .context("Could not start personal gateway bridge")?;
        Ok(Self { stop })
    }
}

impl Drop for GatewayBridge {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

#[derive(Default)]
struct Published {
    identity: String,
    tasks: HashMap<Uuid, String>,
    hydrated: HashMap<Uuid, String>,
    events: HashSet<String>,
}

fn bridge_loop(daemon: DaemonSupervisor, stop: Arc<AtomicBool>) {
    let mut published = Published::default();
    while !stop.load(Ordering::Acquire) {
        if let Ok(api) = CredentialService::global().gateway_api()
            && api.has_publish_grant().unwrap_or(false)
        {
            let identity = format!("{}:{}", api.identity().0, api.identity().1);
            if published.identity != identity {
                published = Published {
                    identity,
                    ..Published::default()
                };
            }
            let _ = sync_once(&daemon.client(), &api, &mut published);
        }
        for _ in 0..50 {
            if stop.load(Ordering::Acquire) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

fn sync_once(client: &DaemonClient, api: &GatewayApi, published: &mut Published) -> Result<()> {
    dispatch_commands(client, api)?;
    let ResponsePayload::TaskState {
        projects,
        mut sessions,
        ..
    } = client.request(Uuid::nil(), Uuid::nil(), Command::LoadTaskState)?
    else {
        bail!("Daemon returned an invalid task list");
    };
    let projects = projects
        .into_iter()
        .map(|project| (project.id, project.name))
        .collect::<HashMap<_, _>>();
    sessions.sort_by_key(|session| std::cmp::Reverse(session.updated_at));
    let mut active_ids = Vec::new();
    for session in sessions.into_iter().take(200) {
        active_ids.push(session.id.to_string());
        let runtime_id = if session.status.is_busy() {
            match client.request(session.id, Uuid::nil(), Command::AttachSession) {
                Ok(ResponsePayload::SessionRuntime { runtime_id, .. }) => runtime_id,
                _ => None,
            }
        } else {
            None
        };
        let task = GatewayRelayTask {
            id: session.id.to_string(),
            project_id: session.project_id.to_string(),
            project_name: projects
                .get(&session.project_id)
                .cloned()
                .unwrap_or_default()
                .chars()
                .take(240)
                .collect(),
            title: session
                .auto_title
                .as_deref()
                .unwrap_or(&session.title)
                .chars()
                .take(240)
                .collect(),
            provider: serde_json::to_value(session.provider)?
                .as_str()
                .unwrap_or("unknown")
                .to_owned(),
            status: serde_json::to_value(session.status)?
                .as_str()
                .unwrap_or("idle")
                .to_owned(),
            runtime_id: runtime_id.map(|id| id.to_string()),
            updated_at: session.updated_at.saturating_mul(1_000),
        };
        let task_json = serde_json::to_string(&task)?;
        if published.tasks.get(&session.id) != Some(&task_json) {
            api.publish_task(&task)?;
            published.tasks.insert(session.id, task_json);
        }
        let signature = format!("{}:{:?}", session.updated_at, session.runtime_event_cursor);
        if published.hydrated.get(&session.id) != Some(&signature) {
            if let ResponsePayload::Session {
                session: Some(detail),
            } = client.request(
                Uuid::nil(),
                Uuid::nil(),
                Command::HydrateSession {
                    session_id: session.id,
                },
            )? {
                publish_events(api, &detail, &mut published.events)?;
                published.hydrated.insert(session.id, signature);
            }
        }
    }
    api.reconcile_tasks(&active_ids)?;
    api.heartbeat()?;
    Ok(())
}

fn dispatch_commands(client: &DaemonClient, api: &GatewayApi) -> Result<()> {
    for pending in api.pending_commands()? {
        if pending.status != "queued" {
            continue;
        }
        let claimed = api.claim_command(&pending.id)?;
        if claimed.status != "uncertain" {
            continue;
        }
        // Revalidate against the daemon actor after the durable gateway claim.
        // A lost daemon acknowledgement leaves the command uncertain, never
        // silently replays a steer or other consequential action.
        let session_id = Uuid::parse_str(&claimed.task_id)?;
        let runtime_id = Uuid::parse_str(&claimed.runtime_id)?;
        let current = client.request(session_id, Uuid::nil(), Command::AttachSession)?;
        let same_runtime = matches!(current, ResponsePayload::SessionRuntime { runtime_id: Some(id), .. } if id == runtime_id);
        if !same_runtime {
            api.settle_command(&claimed.id, false)?;
            continue;
        }
        let command = match claimed.kind.as_str() {
            "stop" => {
                // The local computer-control emergency stop has its own actor
                // command. Attempt both even if one request fails: an error
                // stopping computer control must not suppress turn cancel.
                let computer_stop =
                    client.request(session_id, runtime_id, Command::CancelComputerUse);
                let turn_stop = client.request(session_id, runtime_id, Command::Cancel);
                if computer_stop.is_ok() && turn_stop.is_ok() {
                    api.settle_command(&claimed.id, true)?;
                }
                continue;
            }
            "steer" => Command::Steer {
                prompt: claimed
                    .payload
                    .get("prompt")
                    .cloned()
                    .context("Missing steer prompt")?,
            },
            _ => {
                api.settle_command(&claimed.id, false)?;
                continue;
            }
        };
        if client.request(session_id, runtime_id, command).is_ok() {
            api.settle_command(&claimed.id, true)?;
        }
    }
    Ok(())
}

fn publish_events(
    api: &GatewayApi,
    session: &AgentSession,
    seen: &mut HashSet<String>,
) -> Result<()> {
    let mut blocks = HashMap::<usize, Vec<_>>::new();
    for block in &session.transcript_blocks {
        blocks.entry(block.after_message).or_default().push(block);
    }
    for index in 0..=session.messages.len() {
        if let Some(group) = blocks.remove(&index) {
            for block in group {
                for activity in &block.activities {
                    if !activity.complete || activity.kind == ActivityKind::Reasoning {
                        continue;
                    }
                    let summary = format!(
                        "{} {}",
                        activity.title,
                        activity.display_target.as_deref().unwrap_or("")
                    );
                    send_event(
                        api,
                        seen,
                        session.id,
                        &format!("activity:{}", activity.id),
                        "activity",
                        &summary,
                        session.created_at.saturating_mul(1_000),
                    )?;
                    for (change_index, change) in activity.file_changes.iter().enumerate() {
                        if let Some(diff) = change.diff.as_deref() {
                            send_event(
                                api,
                                seen,
                                session.id,
                                &format!("diff:{}:{change_index}", activity.id),
                                "diff",
                                &format!("{}\n{diff}", change.path),
                                session.created_at.saturating_mul(1_000),
                            )?;
                        }
                    }
                }
            }
        }
        if let Some(message) = session.messages.get(index) {
            if message.streaming || message.role == MessageRole::System {
                continue;
            }
            let role = if message.role == MessageRole::User {
                "You"
            } else {
                "Assistant"
            };
            send_event(
                api,
                seen,
                session.id,
                &format!("message:{}", message.id),
                "message",
                &format!("{role}: {}", message.visible_content()),
                message.created_at.saturating_mul(1_000),
            )?;
        }
    }
    Ok(())
}

fn send_event(
    api: &GatewayApi,
    seen: &mut HashSet<String>,
    task_id: Uuid,
    source: &str,
    kind: &str,
    text: &str,
    at: u64,
) -> Result<()> {
    let text = if text.chars().count() > 45_000 {
        format!(
            "{}… [truncated]",
            text.chars().take(45_000).collect::<String>()
        )
    } else {
        text.to_owned()
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    let event_id = format!("{source}:{:016x}", hasher.finish());
    if seen.contains(&event_id) {
        return Ok(());
    }
    api.publish_event(&event_id, &task_id.to_string(), kind, &text, at)?;
    seen.insert(event_id);
    Ok(())
}
