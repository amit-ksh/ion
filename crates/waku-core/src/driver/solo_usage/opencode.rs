//! Count only live provider steps, never assistant/context totals or fork copies.
use super::*;

pub(super) struct OpenCodeUsage {
    pub(super) opened_at: u64,
    messages: VecDeque<Message>,
}
impl Default for OpenCodeUsage {
    fn default() -> Self {
        Self {
            opened_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|time| time.as_millis().min(u64::MAX as u128) as u64)
                .unwrap_or(u64::MAX),
            messages: VecDeque::new(),
        }
    }
}
struct Message {
    session: String,
    id: String,
    model: String,
    created: u64,
    closed: bool,
    last_start: Option<String>,
    step_open: bool,
}
impl NativeUsage {
    pub(in crate::driver) fn observe_opencode(&mut self, value: &Value, session: &str) {
        let kind = value.get("type").and_then(Value::as_str);
        if !matches!(kind, Some("message.updated" | "message.part.updated")) {
            return;
        }
        if session.is_empty() || session.len() > 256 || session.chars().any(char::is_control) {
            return;
        }
        let Some(properties) = value.get("properties") else {
            return;
        };
        // The resident server carries many tasks. Nested and outer scope must agree.
        if let Some(outer) = properties.get("sessionID")
            && identifier(Some(outer)) != Some(session)
        {
            return;
        }
        match kind {
            Some("message.updated") => {
                let Some(info) = properties.get("info") else {
                    return;
                };
                if identifier(info.get("sessionID")) != Some(session)
                    || info.get("role").and_then(Value::as_str) != Some("assistant")
                {
                    return;
                }
                let Some(id) = identifier(info.get("id")) else {
                    return;
                };
                let existing = self
                    .opencode
                    .messages
                    .iter_mut()
                    .find(|message| message.session == session && message.id == id);
                // Fork emits copied completed messages under NEW IDs. It is history.
                // Keep a tombstone for observed messages; late updates cannot reopen them.
                if info.pointer("/time/completed").is_some() {
                    if let Some(message) = existing {
                        message.closed = true;
                        message.step_open = false;
                    } else {
                        self.opencode.messages.push_back(Message {
                            session: session.to_owned(),
                            id: id.to_owned(),
                            model: String::new(),
                            created: 0,
                            closed: true,
                            last_start: None,
                            step_open: false,
                        });
                        if self.opencode.messages.len() > RECENT {
                            self.opencode.messages.pop_front();
                        }
                    }
                    return;
                }
                let Some(created) = info
                    .pointer("/time/created")
                    .and_then(Value::as_u64)
                    .filter(|created| *created >= self.opencode.opened_at)
                else {
                    return;
                };
                let Some(provider) = identifier(info.get("providerID")) else {
                    return;
                };
                let Some(model) = info.get("modelID").and_then(Value::as_str).filter(|id| {
                    !id.trim().is_empty() && id.len() <= 512 && !id.chars().any(char::is_control)
                }) else {
                    return;
                };
                let model = format!("{provider}/{model}");
                if model.len() > 512 {
                    return;
                }
                if let Some(message) = existing {
                    if message.model != model || message.created != created {
                        message.closed = true;
                        message.step_open = false;
                    }
                    return;
                }
                self.opencode.messages.push_back(Message {
                    session: session.to_owned(),
                    id: id.to_owned(),
                    model,
                    created,
                    closed: false,
                    last_start: None,
                    step_open: false,
                });
                if self.opencode.messages.len() > RECENT {
                    self.opencode.messages.pop_front();
                }
            }
            Some("message.part.updated") => {
                let Some(part) = properties.get("part") else {
                    return;
                };
                if identifier(part.get("sessionID")) != Some(session) {
                    return;
                }
                let Some(message_id) = identifier(part.get("messageID")) else {
                    return;
                };
                let Some(part_id) = identifier(part.get("id")) else {
                    return;
                };
                let Some(message) = self
                    .opencode
                    .messages
                    .iter_mut()
                    .find(|message| message.session == session && message.id == message_id)
                else {
                    return;
                };
                if message.closed {
                    return;
                }
                match part.get("type").and_then(Value::as_str) {
                    Some("step-start") => {
                        if message.last_start.as_deref() != Some(part_id) {
                            message.last_start = Some(part_id.to_owned());
                            message.step_open = true;
                        }
                    }
                    Some("step-finish") if message.step_open => {
                        let Some(usage) = part.get("tokens").and_then(tokens) else {
                            return;
                        };
                        let model = message.model.clone();
                        // IDs are hashed before persistence. A copied fork cannot
                        // establish a fresh message or live step with historical time.
                        if self.record(
                            identity(&["opencode-step-v1", session, message_id, part_id]),
                            ProviderKind::OpenCode,
                            Some(model),
                            usage,
                        ) {
                            let message = self
                                .opencode
                                .messages
                                .iter_mut()
                                .find(|message| {
                                    message.session == session && message.id == message_id
                                })
                                .unwrap();
                            message.step_open = false;
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
fn tokens(value: &Value) -> Option<ModelTokenUsage> {
    let cache = value.get("cache")?;
    let input = count(value, "input")?
        .checked_add(count(cache, "read")?)?
        .checked_add(count(cache, "write")?)?;
    let output = count(value, "output")?;
    let reasoning = count(value, "reasoning")?;
    if input > LIMIT {
        return None;
    }
    let base = input.checked_add(output)?;
    // OpenCode's older v1 payload included reasoning in output. Newer v1
    // compatibility payloads separate it. The reported total is the witness;
    // no version guess or inferred model price can resolve an ambiguous count.
    let output = match value.get("total") {
        None if reasoning == 0 => output,
        Some(_) if count(value, "total")? == base && reasoning <= output => output,
        Some(_) if reasoning > 0 && count(value, "total")? == base.checked_add(reasoning)? => {
            output.checked_add(reasoning)?
        }
        _ => return None,
    };
    (output <= LIMIT).then_some(ModelTokenUsage {
        input_tokens: input,
        output_tokens: output,
    })
}
