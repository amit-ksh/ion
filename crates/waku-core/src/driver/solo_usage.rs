//! Whitelisted accounting metadata observed on the native provider reader worker.
//! Text deltas, context occupancy and cumulative SDK dollar totals are not calls.
use super::WorkspaceContext;
use crate::{control::ControlStore, teams::ModelTokenUsage, usage_ledger::NativeSoloReceipt};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashSet, VecDeque};
use uuid::Uuid;
use waku_protocol::model::ProviderKind;

const LIMIT: u64 = 1_000_000_000_000;
const RECENT: usize = 256;
pub(super) struct NativeUsage {
    store: ControlStore,
    project: Uuid,
    task: Uuid,
    // Only completed receipt identities are cached; SQLite is the durable authority.
    seen: HashSet<Uuid>,
    order: VecDeque<Uuid>,
    codex_turns: VecDeque<String>,
}
impl NativeUsage {
    pub(super) fn new(context: &WorkspaceContext) -> Self {
        Self {
            store: ControlStore::new(context.control_path.clone()),
            project: context.project_id,
            task: context.task_id,
            seen: HashSet::new(),
            order: VecDeque::new(),
            codex_turns: VecDeque::new(),
        }
    }
    fn record(
        &mut self,
        source_id: Uuid,
        provider: ProviderKind,
        model: Option<String>,
        usage: ModelTokenUsage,
    ) {
        if self.seen.contains(&source_id) {
            return;
        }
        let receipt = NativeSoloReceipt {
            source_id,
            project_id: self.project,
            task_id: self.task,
            provider,
            model,
            usage,
        };
        if self
            .store
            .record_native_solo_usage(&receipt, crate::model::unix_time())
            .is_err()
        {
            // Do not expose provider payloads or convert an accounting failure into
            // a provider action retry. A repeated receipt can retry persistence.
            eprintln!("[ion-usage] Native usage receipt could not be recorded");
            return;
        }
        self.seen.insert(source_id);
        self.order.push_back(source_id);
        if self.order.len() > RECENT
            && let Some(old) = self.order.pop_front()
        {
            self.seen.remove(&old);
        }
    }
    pub(super) fn observe_claude(&mut self, value: &Value, session: &str) {
        if value.get("type").and_then(Value::as_str) != Some("assistant") {
            return;
        }
        // Partial stream frames and the session's cumulative `result` are excluded.
        // CLI content-block frames repeat the same completed message usage.
        if let Some(wire_session) = value.get("session_id")
            && wire_session.as_str() != Some(session)
        {
            return;
        }
        let Some(message) = value.get("message") else {
            return;
        };
        let Some(id) = identifier(message.get("id")) else {
            return;
        };
        let Some(model) = message
            .get("model")
            .and_then(Value::as_str)
            .filter(|m| !m.trim().is_empty() && m.len() <= 512 && !m.chars().any(char::is_control))
        else {
            return;
        };
        let Some(usage) = message.get("usage").and_then(claude_tokens) else {
            return;
        };
        // Anthropic message IDs identify the upstream request across native
        // session forks; a copied message is not a new billable request.
        self.record(
            identity(&["claude-message-v1", id]),
            ProviderKind::Claude,
            Some(model.to_owned()),
            usage,
        );
    }
    pub(super) fn observe_codex(&mut self, value: &Value, thread: Option<&str>) {
        let Some(thread) = thread else {
            return;
        };
        let Some(params) = value.get("params") else {
            return;
        };
        if identifier(params.get("threadId")) != Some(thread) {
            return;
        }
        match value.get("method").and_then(Value::as_str) {
            Some("turn/started") => {
                let Some(turn) = identifier(params.pointer("/turn/id")) else {
                    return;
                };
                if !self.codex_turns.iter().any(|t| t == turn) {
                    self.codex_turns.push_back(turn.to_owned());
                    // Retain bounded recently completed turns for late accounting,
                    // while excluding native historical usage replay on resume/fork.
                    if self.codex_turns.len() > 4 {
                        self.codex_turns.pop_front();
                    }
                }
            }
            Some("thread/tokenUsage/updated") => {
                let Some(turn) = identifier(params.get("turnId")) else {
                    return;
                };
                if !self.codex_turns.iter().any(|t| t == turn) {
                    return;
                }
                let Some(last) = params.pointer("/tokenUsage/last").and_then(codex_tokens) else {
                    return;
                };
                let Some(total) = params.pointer("/tokenUsage/total").and_then(codex_tokens) else {
                    return;
                };
                // `fill_to_context_window` emits synthetic totals with zero input
                // and output. A rate-limit update can repeat the last call, so the
                // unchanged cumulative input/output checkpoint is the same receipt.
                if last.input_tokens == 0 && last.output_tokens == 0 {
                    return;
                }
                if total.input_tokens < last.input_tokens
                    || total.output_tokens < last.output_tokens
                {
                    return;
                }
                let checkpoint = format!("{}:{}", total.input_tokens, total.output_tokens);
                self.record(
                    identity(&["codex-call-v1", thread, turn, &checkpoint]),
                    ProviderKind::Codex,
                    None,
                    last,
                );
            }
            _ => {}
        }
    }
}
fn identifier(value: Option<&Value>) -> Option<&str> {
    value?
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control))
}
fn count(value: &Value, name: &str) -> Option<u64> {
    value.get(name)?.as_u64().filter(|n| *n <= LIMIT)
}
fn optional_count(value: &Value, name: &str) -> Option<u64> {
    match value.get(name) {
        None => Some(0),
        Some(_) => count(value, name),
    }
}
fn claude_tokens(value: &Value) -> Option<ModelTokenUsage> {
    let input = count(value, "input_tokens")?
        .checked_add(optional_count(value, "cache_read_input_tokens")?)?
        .checked_add(optional_count(value, "cache_creation_input_tokens")?)?;
    (input <= LIMIT).then_some(ModelTokenUsage {
        input_tokens: input,
        output_tokens: count(value, "output_tokens")?,
    })
}
fn codex_tokens(value: &Value) -> Option<ModelTokenUsage> {
    let input = count(value, "inputTokens")?;
    let output = count(value, "outputTokens")?;
    // Cached input and reasoning output are already included, unlike Claude's
    // separate cache fields. Optional cache-write is absent on older native CLIs.
    if count(value, "cachedInputTokens")? > input
        || optional_count(value, "cacheWriteInputTokens")? > input
        || count(value, "reasoningOutputTokens")? > output
        || count(value, "totalTokens")? != input.checked_add(output)?
    {
        return None;
    }
    Some(ModelTokenUsage {
        input_tokens: input,
        output_tokens: output,
    })
}
fn identity(parts: &[&str]) -> Uuid {
    let mut hash = Sha256::new();
    hash.update(b"ion-native-usage-v1");
    for part in parts {
        hash.update((part.len() as u64).to_be_bytes());
        hash.update(part.as_bytes());
    }
    let digest = hash.finalize();
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 15) | 0x80; // UUID v8: application-defined SHA-256 identity.
    bytes[8] = (bytes[8] & 63) | 0x80;
    Uuid::from_bytes(bytes)
}

#[cfg(test)]
mod tests;
