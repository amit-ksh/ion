//! Outbound desktop-to-gateway task mirror. Runs only on a dedicated worker;
//! no gateway or daemon I/O is reachable from a GPUI frame.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use crossbeam_channel::{Receiver, Sender, unbounded};
use uuid::Uuid;
use waku_client::{Command, DaemonClient, DaemonSupervisor, ResponsePayload};

use crate::credentials::{
    CredentialService, GatewayApi, GatewayRelayCommand, GatewayRelayComputerTarget,
    GatewayRelayInteraction, GatewayRelayTask,
};
use crate::model::{ActivityKind, AgentSession, MessageRole};

pub struct GatewayBridge {
    stop: Arc<AtomicBool>,
    interactions: Sender<InteractionUpdate>,
}

pub enum InteractionUpdate {
    Reset,
    Publish(GatewayRelayInteraction),
    Computer(GatewayRelayComputerTarget),
    Resolve {
        task_id: Uuid,
        runtime_id: Uuid,
        request_id: String,
    },
}

impl InteractionUpdate {
    fn key(&self) -> Option<(String, String, String)> {
        match self {
            Self::Reset => None,
            Self::Publish(value) => Some((
                value.task_id.clone(),
                value.runtime_id.clone(),
                format!("interaction:{}", value.request_id),
            )),
            Self::Computer(value) => Some((
                value.task_id.clone(),
                value.runtime_id.clone(),
                "computer-target".into(),
            )),
            Self::Resolve {
                task_id,
                runtime_id,
                request_id,
            } => Some((
                task_id.to_string(),
                runtime_id.to_string(),
                format!("interaction:{request_id}"),
            )),
        }
    }
}

fn collect_interaction(
    interactions: &mut HashMap<(String, String, String), InteractionUpdate>,
    update: InteractionUpdate,
) {
    if let Some(key) = update.key() {
        interactions.insert(key, update);
        if interactions.len() > 1_000 {
            interactions.clear();
        }
    } else {
        interactions.clear();
    }
}

impl GatewayBridge {
    pub fn start(daemon: DaemonSupervisor) -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let (interactions, receiver) = unbounded();
        std::thread::Builder::new()
            .name("ion-gateway-bridge".into())
            .spawn(move || bridge_loop(daemon, worker_stop, receiver))
            .context("Could not start personal gateway bridge")?;
        Ok(Self { stop, interactions })
    }

    pub fn update_interaction(&self, update: InteractionUpdate) {
        let _ = self.interactions.send(update);
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

fn bridge_loop(
    daemon: DaemonSupervisor,
    stop: Arc<AtomicBool>,
    receiver: Receiver<InteractionUpdate>,
) {
    let mut published = Published::default();
    let mut interactions = HashMap::new();
    let mut last_error = String::new();
    while !stop.load(Ordering::Acquire) {
        if let Ok(api) = CredentialService::global().gateway_api()
            && api.has_publish_grant().unwrap_or(false)
        {
            let identity = format!("{}:{}", api.identity().0, api.identity().1);
            if published.identity != identity {
                if !published.identity.is_empty() {
                    interactions.clear();
                }
                published = Published {
                    identity,
                    ..Published::default()
                };
            }
            while let Ok(update) = receiver.try_recv() {
                collect_interaction(&mut interactions, update);
            }
            if let Err(error) = sync_once(&daemon.client(), &api, &mut published) {
                // A gateway restore or a partial network failure can erase
                // server state that our in-memory cache believed existed.
                // Replay the bounded snapshot and deduped event IDs next pass.
                published.tasks.clear();
                published.hydrated.clear();
                published.events.clear();
                let message = error.to_string();
                if last_error != message {
                    eprintln!("Personal gateway task bridge: {message}");
                    last_error = message;
                }
            } else {
                last_error.clear();
                interactions.retain(|_, update| {
                    let result = match update {
                        InteractionUpdate::Reset => Ok(()),
                        InteractionUpdate::Publish(value) => api.publish_interaction(value),
                        InteractionUpdate::Computer(value) => api.publish_computer_target(value),
                        InteractionUpdate::Resolve {
                            task_id,
                            runtime_id,
                            request_id,
                        } => api.resolve_interaction(
                            &task_id.to_string(),
                            &runtime_id.to_string(),
                            request_id,
                        ),
                    };
                    result.is_err()
                });
            }
        } else {
            while let Ok(update) = receiver.try_recv() {
                collect_interaction(&mut interactions, update);
            }
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
                .take(120)
                .collect(),
            title: session
                .auto_title
                .as_deref()
                .unwrap_or(&session.title)
                .chars()
                .take(120)
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
        let request_id = if matches!(pending.kind.as_str(), "permission" | "answer") {
            match gateway_request_id(api, &pending) {
                Ok(id) => Some(id),
                Err(_) => continue,
            }
        } else {
            None
        };
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
            "permission" => Command::Respond {
                request_id: request_id.clone().context("Missing provider request")?,
                option_id: claimed
                    .payload
                    .get("optionId")
                    .cloned()
                    .context("Missing permission option")?,
            },
            "answer" => Command::RespondUserInput {
                request_id: request_id.context("Missing provider request")?,
                answers: serde_json::from_str(
                    claimed
                        .payload
                        .get("answers")
                        .context("Missing question answers")?,
                )?,
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

fn gateway_request_id(api: &GatewayApi, command: &GatewayRelayCommand) -> Result<String> {
    api.interaction_request_id(
        command
            .payload
            .get("interactionId")
            .context("Missing provider interaction")?,
    )
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
                        send_event(
                            api,
                            seen,
                            session.id,
                            &format!("file:{}:{change_index}", activity.id),
                            "artifact",
                            &format!("Changed file: {}", change.path),
                            session.created_at.saturating_mul(1_000),
                        )?;
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
            for (attachment_index, attachment) in message.attachments.iter().enumerate() {
                // The gateway receives only a display name, never the local
                // path or daemon blob reference from a composer attachment.
                send_event(
                    api,
                    seen,
                    session.id,
                    &format!("attachment:{}:{attachment_index}", message.id),
                    "artifact",
                    &format!("Attachment: {}", attachment.name),
                    message.created_at.saturating_mul(1_000),
                )?;
            }
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
    // A JSON control character can expand to six ASCII bytes. Keep each
    // event below the gateway's 70 KiB request cap even in that worst case.
    let text = if text.chars().count() > 8_000 {
        format!(
            "{}… [truncated]",
            text.chars().take(8_000).collect::<String>()
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
