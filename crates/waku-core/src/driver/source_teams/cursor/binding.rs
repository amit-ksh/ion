//! Cursor hook IDs identify hook executions, not native tool calls. Join a
//! one-use hook binding to both native ACP observation and permission content.
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

pub(super) const FIELD: &str = "__ion_native_binding";
const MAX_IDENTITIES: usize = 2048;
const MAX_ARGUMENT_BYTES: usize = 1024 * 1024;
const MAX_CALL_BYTES: usize = 40 * 1024;
const EXPIRY: Duration = Duration::from_secs(30);
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)
}
fn canonical(value: &Value) -> Result<Value> {
    let mut result = value
        .as_object()
        .context("Native tool arguments must be an object")?
        .clone();
    result.remove(FIELD);
    ensure!(
        serde_json::to_vec(&result)?.len() <= MAX_CALL_BYTES,
        "Native tool arguments exceed their bound"
    );
    Ok(Value::Object(result))
}
struct Call {
    name: String,
    arguments: Value,
    valid: bool,
    expires: Instant,
    binding: Option<String>,
    consumed: bool,
    approved: bool,
}
struct Hook {
    name: String,
    arguments: Value,
    nonce: String,
    expires: Instant,
    call: Option<String>,
    valid: bool,
}
pub(super) struct Bindings {
    session: String,
    server: String,
    prefix: String,
    running: bool,
    calls: HashMap<String, Call>,
    hooks: HashMap<String, Hook>,
    nonces: HashMap<String, String>,
    argument_bytes: usize,
}
#[derive(Debug, PartialEq)]
pub(super) struct Invocation {
    pub call_id: String,
    pub name: String,
    pub arguments: Value,
}
impl Bindings {
    pub(super) fn new(session: String, server: String, prefix: String) -> Result<Self> {
        ensure!(
            valid_id(&session) && valid_id(&server) && valid_id(&prefix),
            "Invalid Cursor binding identity"
        );
        Ok(Self {
            session,
            server,
            prefix,
            running: false,
            calls: HashMap::new(),
            hooks: HashMap::new(),
            nonces: HashMap::new(),
            argument_bytes: 0,
        })
    }
    pub(super) fn activate(&mut self) {
        self.cancel();
        self.running = true;
    }
    pub(super) fn cancel(&mut self) {
        self.running = false;
        self.argument_bytes = 0;
        for call in self.calls.values_mut() {
            call.valid = false;
            call.arguments = Value::Null;
        }
        for hook in self.hooks.values_mut() {
            hook.valid = false;
            hook.arguments = Value::Null;
        }
    }
    fn name(&self, native: &str) -> Result<String> {
        let name = native
            .strip_prefix(&self.prefix)
            .filter(|name| super::super::accepts(name))
            .context("Unrelated Cursor native tool")?;
        Ok(name.into())
    }
    fn retain(&mut self, arguments: &Value) -> Result<()> {
        let bytes = serde_json::to_vec(arguments)?.len();
        ensure!(
            self.argument_bytes.saturating_add(bytes) <= MAX_ARGUMENT_BYTES,
            "Cursor argument budget exhausted"
        );
        self.argument_bytes += bytes;
        Ok(())
    }
    pub(super) fn observe(&mut self, event: &Value) -> Result<()> {
        ensure!(
            self.running && event["sessionId"].as_str() == Some(&self.session),
            "Unrelated or stopped Cursor session"
        );
        let update = &event["update"];
        let id = update["toolCallId"]
            .as_str()
            .filter(|id| valid_id(id))
            .context("Invalid Cursor native call ID")?;
        if update["status"] == "failed" || update["status"] == "completed" {
            if let Some(call) = self.calls.get_mut(id) {
                call.valid = false;
                call.arguments = Value::Null;
            }
            return Ok(());
        }
        ensure!(
            update["sessionUpdate"] == "tool_call",
            "Not a native initial call"
        );
        let raw = &update["rawInput"];
        ensure!(
            raw["providerIdentifier"].as_str() == Some(&self.server),
            "Unrelated Cursor MCP server"
        );
        let name = self.name(
            raw["toolName"]
                .as_str()
                .context("Missing native tool name")?,
        )?;
        let arguments = canonical(&raw["args"])?;
        if let Some(call) = self.calls.get_mut(id) {
            call.valid = false;
            anyhow::bail!("Cursor native call ID was repeated");
        }
        ensure!(
            self.calls.len() < MAX_IDENTITIES,
            "Cursor native identity limit reached"
        );
        self.retain(&arguments)?;
        self.calls.insert(
            id.into(),
            Call {
                name,
                arguments,
                valid: true,
                expires: Instant::now() + EXPIRY,
                binding: None,
                consumed: false,
                approved: false,
            },
        );
        Ok(())
    }
    /// Called only after the private hook transport has authenticated the request.
    /// The hook UUID is never treated as a native call ID or durable invocation ID.
    pub(super) fn hook(&mut self, event: &Value) -> Result<Value> {
        ensure!(
            self.running
                && event["hook_event_name"] == "preToolUse"
                && event["conversation_id"].as_str() == Some(&self.session)
                && event["session_id"].as_str() == Some(&self.session),
            "Unrelated or stopped native hook"
        );
        let id = event["tool_use_id"]
            .as_str()
            .filter(|id| valid_id(id))
            .context("Invalid Cursor hook ID")?;
        if let Some(hook) = self.hooks.get_mut(id) {
            hook.valid = false;
            if let Some(call) = hook.call.as_ref().and_then(|id| self.calls.get_mut(id)) {
                call.valid = false;
            }
            anyhow::bail!("Cursor hook ID was repeated");
        }
        ensure!(
            self.hooks.len() < MAX_IDENTITIES,
            "Cursor hook identity limit reached"
        );
        let tool = event["tool_name"]
            .as_str()
            .and_then(|name| name.strip_prefix("MCP:"))
            .context("Not a Cursor MCP hook")?;
        let name = self.name(tool)?;
        let arguments = canonical(&event["tool_input"])?;
        self.retain(&arguments)?;
        let nonce = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        self.nonces.insert(nonce.clone(), id.into());
        self.hooks.insert(
            id.into(),
            Hook {
                name,
                arguments,
                nonce: nonce.clone(),
                expires: Instant::now() + EXPIRY,
                call: None,
                valid: true,
            },
        );
        Ok(json!({"permission":"allow","updated_input":{FIELD:nonce}}))
    }
    /// Native permission content is produced after input rewriting. It must
    /// agree with the separately observed original call before any work starts.
    pub(super) fn witness(&mut self, request: &Value) -> Result<()> {
        ensure!(
            self.running && request["sessionId"].as_str() == Some(&self.session),
            "Unrelated or stopped native permission"
        );
        let tool = &request["toolCall"];
        let id = tool["toolCallId"]
            .as_str()
            .filter(|id| valid_id(id))
            .context("Invalid native permission ID")?;
        let content = tool["content"]
            .as_array()
            .filter(|items| items.len() == 1)
            .context("Unexpected native permission content")?;
        ensure!(
            content[0]["type"] == "content" && content[0]["content"]["type"] == "text",
            "Unexpected native permission shape"
        );
        let text = content[0]["content"]["text"]
            .as_str()
            .filter(|text| text.len() <= MAX_CALL_BYTES + 1024)
            .context("Native permission content exceeds its bound")?;
        let source = text
            .strip_prefix("```json\n")
            .and_then(|text| text.strip_suffix("\n```"))
            .context("Invalid native argument witness")?;
        let input: Value = serde_json::from_str(source)?;
        let nonce = input[FIELD]
            .as_str()
            .context("Missing rewritten native binding")?;
        let arguments = canonical(&input)?;
        let hook_id = self
            .nonces
            .get(nonce)
            .context("Unknown native binding")?
            .clone();
        let hook = self
            .hooks
            .get_mut(&hook_id)
            .context("Unknown native hook")?;
        let call = self
            .calls
            .get_mut(id)
            .context("Native call has not been observed")?;
        let native_name = format!("{}{}", self.prefix, call.name);
        let expected_title = format!("{}-{}: {}", self.server, native_name, native_name);
        if tool["title"] != expected_title
            || !hook.valid
            || !call.valid
            || call.consumed
            || hook.call.is_some()
            || call.binding.is_some()
            || hook.expires < Instant::now()
            || call.expires < Instant::now()
            || hook.nonce != nonce
            || hook.name != call.name
            || hook.arguments != arguments
            || call.arguments != arguments
        {
            hook.valid = false;
            call.valid = false;
            anyhow::bail!("Changed, replayed or expired native permission");
        }
        hook.call = Some(id.into());
        call.binding = Some(nonce.into());
        // A native per-call approval may wait on the owner. Keep that wait
        // bounded separately from the short pre-hook correlation window.
        let expiry = Instant::now() + Duration::from_secs(300);
        hook.expires = expiry;
        call.expires = expiry;
        Ok(())
    }
    pub(super) fn approve(&mut self, id: &str, allowed: bool) -> Result<()> {
        ensure!(self.running, "Cursor source stopped");
        let call = self
            .calls
            .get_mut(id)
            .context("Unobserved native approval")?;
        ensure!(
            call.valid
                && !call.consumed
                && call.binding.is_some()
                && call.expires >= Instant::now(),
            "Unwitnessed or expired native approval"
        );
        if allowed {
            call.approved = true;
        } else {
            call.valid = false;
        }
        Ok(())
    }
    pub(super) fn consume(&mut self, native_name: &str, input: &Value) -> Result<Invocation> {
        ensure!(self.running, "Cursor source stopped");
        let name = self.name(native_name)?;
        let nonce = input[FIELD].as_str().context("Missing native binding")?;
        let arguments = canonical(input)?;
        let hook_id = self
            .nonces
            .get(nonce)
            .context("Unknown native binding")?
            .clone();
        let hook = self
            .hooks
            .get_mut(&hook_id)
            .context("Unknown native hook")?;
        let id = hook
            .call
            .as_ref()
            .context("Native binding was not witnessed")?
            .clone();
        let call = self.calls.get_mut(&id).context("Unknown native call")?;
        if !hook.valid
            || !call.valid
            || !call.approved
            || call.consumed
            || hook.expires < Instant::now()
            || call.expires < Instant::now()
            || hook.name != name
            || call.name != name
            || hook.arguments != arguments
            || call.arguments != arguments
            || call.binding.as_deref() != Some(nonce)
        {
            hook.valid = false;
            call.valid = false;
            anyhow::bail!("Changed, replayed or expired native callback");
        }
        call.consumed = true;
        hook.valid = false;
        Ok(Invocation {
            call_id: id,
            name,
            arguments,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> Bindings {
        let mut s = Bindings::new(
            "native-session".into(),
            "ion-server".into(),
            "ion_private_".into(),
        )
        .unwrap();
        s.activate();
        s
    }
    fn observed(id: &str, args: Value) -> Value {
        json!({"sessionId":"native-session","update":{"sessionUpdate":"tool_call","toolCallId":id,"title":"ion-server: ion_private_list_teams","kind":"other","status":"pending","rawInput":{"providerIdentifier":"ion-server","toolName":"ion_private_list_teams","args":args}}})
    }
    fn hook(id: &str, args: Value) -> Value {
        json!({"hook_event_name":"preToolUse","conversation_id":"native-session","session_id":"native-session","generation_id":"native-session","tool_use_id":id,"tool_name":"MCP:ion_private_list_teams","tool_input":args})
    }
    fn permission(id: &str, args: &Value) -> Value {
        json!({"sessionId":"native-session","toolCall":{"toolCallId":id,"title":"ion-server-ion_private_list_teams: ion_private_list_teams","kind":"other","status":"pending","content":[{"type":"content","content":{"type":"text","text":format!("```json\n{}\n```",serde_json::to_string_pretty(args).unwrap())}}]},"options":[{"optionId":"allow-once","kind":"allow_once"},{"optionId":"reject-once","kind":"reject_once"}]})
    }
    fn joined(s: &mut Bindings, id: &str, hook_id: &str, args: Value) -> Value {
        s.observe(&observed(id, args.clone())).unwrap();
        let output = s.hook(&hook(hook_id, args.clone())).unwrap();
        let mut input = args;
        input[FIELD] = output["updated_input"][FIELD].clone();
        s.witness(&permission(id, &input)).unwrap();
        input
    }
    #[test]
    fn cursor_binding_joins_distinct_native_calls_with_identical_original_arguments() {
        let mut s = state();
        let a = joined(
            &mut s,
            "native-call-a",
            "hook-uuid-a",
            json!({"query":"saved"}),
        );
        let b = joined(
            &mut s,
            "native-call-b",
            "hook-uuid-b",
            json!({"query":"saved"}),
        );
        assert_ne!(a[FIELD], b[FIELD]);
        for (id, input) in [("native-call-b", b), ("native-call-a", a)] {
            s.approve(id, true).unwrap();
            assert_eq!(
                s.consume("ion_private_list_teams", &input).unwrap(),
                Invocation {
                    call_id: id.into(),
                    name: "list_teams".into(),
                    arguments: json!({"query":"saved"})
                }
            );
            assert!(s.consume("ion_private_list_teams", &input).is_err());
        }
    }
    #[test]
    fn cursor_binding_repeated_initial_or_hook_id_revokes_prior_authority() {
        for duplicate in ["initial", "hook"] {
            let mut s = state();
            let input = joined(&mut s, "native-call", "hook-uuid", json!({}));
            s.approve("native-call", true).unwrap();
            match duplicate {
                "initial" => assert!(s.observe(&observed("native-call", json!({}))).is_err()),
                _ => assert!(s.hook(&hook("hook-uuid", json!({}))).is_err()),
            }
            assert!(s.consume("ion_private_list_teams", &input).is_err());
        }
    }
    #[test]
    fn cursor_binding_hook_uuid_cannot_stand_in_for_the_native_call_id() {
        let mut s = state();
        s.observe(&observed("native-call", json!({}))).unwrap();
        let reply = s.hook(&hook("random-hook-uuid", json!({}))).unwrap();
        let input = json!({FIELD:reply["updated_input"][FIELD]});
        assert!(s.witness(&permission("random-hook-uuid", &input)).is_err());
        assert!(s.consume("ion_private_list_teams", &input).is_err());
        assert!(s.approve("random-hook-uuid", true).is_err());
    }
    #[test]
    fn cursor_binding_unwitnessed_or_forged_callbacks_never_authorize_work() {
        let mut s = state();
        for value in [json!({}), json!({FIELD:"model-forged"}), json!({FIELD:7})] {
            assert!(s.consume("ion_private_list_teams", &value).is_err());
        }
        let reply = s.hook(&hook("random-hook-uuid", json!({}))).unwrap();
        let input = json!({FIELD:reply["updated_input"][FIELD]});
        assert!(
            s.witness(&permission("missing-native-call", &input))
                .is_err()
        );
        assert!(s.consume("ion_private_list_teams", &input).is_err());
    }
    #[test]
    fn cursor_binding_requires_native_approval_even_after_an_exact_witness() {
        let mut s = state();
        let input = joined(&mut s, "native-call", "hook-uuid", json!({}));
        assert!(s.consume("ion_private_list_teams", &input).is_err());
        assert!(s.approve("native-call", true).is_err());
    }
    #[test]
    fn cursor_binding_denial_stop_and_terminal_events_revoke_accepted_identity() {
        for reason in ["deny", "stop", "failed", "completed"] {
            let mut s = state();
            let input = joined(&mut s, "native-call", "hook-uuid", json!({}));
            s.approve("native-call", true).unwrap();
            match reason {
                "deny"=>s.approve("native-call",false).unwrap(),
                "stop"=>s.cancel(),
                status=>s.observe(&json!({"sessionId":"native-session","update":{"sessionUpdate":"tool_call_update","toolCallId":"native-call","status":status}})).unwrap(),
            }
            assert!(
                s.consume("ion_private_list_teams", &input).is_err(),
                "{reason}"
            );
        }
    }
    #[test]
    fn cursor_binding_old_call_hook_and_nonce_stay_revoked_across_new_turns() {
        let mut s = state();
        let input = joined(&mut s, "native-call", "hook-uuid", json!({}));
        s.approve("native-call", true).unwrap();
        s.activate();
        assert!(s.consume("ion_private_list_teams", &input).is_err());
        assert!(s.observe(&observed("native-call", json!({}))).is_err());
        assert!(s.hook(&hook("hook-uuid", json!({}))).is_err());
        let fresh = joined(&mut s, "new-call", "new-hook", json!({}));
        s.approve("new-call", true).unwrap();
        assert!(s.consume("ion_private_list_teams", &fresh).is_ok());
    }
    #[test]
    fn cursor_binding_changed_arguments_tool_or_server_fail_before_dispatch() {
        for change in ["arguments", "tool", "server", "title", "session"] {
            let mut s = state();
            s.observe(&observed("native-call", json!({"query":"original"})))
                .unwrap();
            let reply = s
                .hook(&hook("hook-uuid", json!({"query":"original"})))
                .unwrap();
            let input = json!({"query":"original",FIELD:reply["updated_input"][FIELD]});
            let mut witness = permission("native-call", &input);
            match change {
                "arguments" => {
                    witness = permission(
                        "native-call",
                        &json!({"query":"changed",FIELD:input[FIELD]}),
                    )
                }
                "tool" => {
                    witness["toolCall"]["title"] =
                        json!("ion-server-ion_private_invoke_team: ion_private_invoke_team")
                }
                "server" => {
                    witness["toolCall"]["title"] =
                        json!("other-server-ion_private_list_teams: ion_private_list_teams")
                }
                "title" => witness["toolCall"]["title"] = json!("ion_private_list_teams"),
                _ => witness["sessionId"] = json!("child-session"),
            }
            assert!(s.witness(&witness).is_err(), "{change}");
            assert!(
                s.consume("ion_private_list_teams", &input).is_err(),
                "{change}"
            );
        }
    }
    #[test]
    fn cursor_binding_repeated_witness_changed_callback_and_expired_authority_refuse() {
        for reason in ["witness", "callback", "expiry"] {
            let mut s = state();
            let mut input = joined(&mut s, "native-call", "hook-uuid", json!({}));
            s.approve("native-call", true).unwrap();
            match reason {
                "witness" => assert!(s.witness(&permission("native-call", &input)).is_err()),
                "callback" => input["query"] = json!("changed"),
                _ => {
                    s.calls.get_mut("native-call").unwrap().expires =
                        Instant::now() - Duration::from_secs(1)
                }
            }
            assert!(s.consume("ion_private_list_teams", &input).is_err());
        }
    }
    #[test]
    fn cursor_binding_model_transport_field_is_overwritten_and_never_reaches_worker() {
        let mut s = state();
        let input = joined(
            &mut s,
            "native-call",
            "hook-uuid",
            json!({"query":"saved",FIELD:"model-forged"}),
        );
        assert_ne!(input[FIELD], "model-forged");
        s.approve("native-call", true).unwrap();
        assert_eq!(
            s.consume("ion_private_list_teams", &input)
                .unwrap()
                .arguments,
            json!({"query":"saved"})
        );
    }
    #[test]
    fn cursor_binding_foreign_nested_or_malformed_native_envelopes_refuse() {
        for field in ["sessionId", "providerIdentifier", "toolName", "arguments"] {
            let mut s = state();
            let mut event = observed("native-call", json!({}));
            match field {
                "sessionId" => event[field] = json!("child-session"),
                "arguments" => event["update"]["rawInput"]["args"] = json!([]),
                _ => event["update"]["rawInput"][field] = json!("foreign"),
            };
            assert!(s.observe(&event).is_err());
        }
        for field in [
            "session_id",
            "conversation_id",
            "tool_name",
            "hook_event_name",
        ] {
            let mut s = state();
            let mut event = hook("hook-uuid", json!({}));
            event[field] = json!("foreign");
            assert!(s.hook(&event).is_err());
        }
        let mut s = state();
        let input = joined(&mut s, "native-call", "hook-uuid", json!({}));
        let mut event = permission("native-call", &input);
        event["toolCall"]["content"][0]["content"]["text"] = json!("not a native code block");
        assert!(s.witness(&event).is_err());
    }
    #[test]
    fn cursor_binding_limits_input_identity_count_and_aggregate_retention() {
        let mut s = state();
        assert!(
            s.observe(&observed(
                "large",
                json!({"query":"x".repeat(MAX_CALL_BYTES)})
            ))
            .is_err()
        );
        for index in 0..MAX_IDENTITIES {
            s.observe(&observed(&format!("call-{index}"), json!({})))
                .unwrap();
        }
        assert!(s.observe(&observed("extra", json!({}))).is_err());
        let mut s = state();
        let mut rejected = false;
        for index in 0..100 {
            if s.hook(&hook(
                &format!("hook-{index}"),
                json!({"query":"x".repeat(32*1024)}),
            ))
            .is_err()
            {
                rejected = true;
                break;
            }
        }
        assert!(rejected);
        assert!(s.argument_bytes <= MAX_ARGUMENT_BYTES);
    }
}
