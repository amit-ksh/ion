//! Direct native-chat app tools use the same credential boundary and exact
//! approvals as Teams. No project keys, connection credentials or proxies.
use super::*;

pub(super) fn accepts(name: &str) -> bool {
    matches!(
        name,
        "list_connected_apps" | "search_app_actions" | "describe_app_action" | "request_app_action"
    )
}
pub(super) fn tools() -> Value {
    json!([
        {"type":"function","function":{"name":"list_connected_apps","description":"List this user's connected app accounts. App sign-in happens in Apps; only active accounts may execute actions.","parameters":{"type":"object","properties":{"cursor":{"type":"string"}},"additionalProperties":false}}},
        {"type":"function","function":{"name":"search_app_actions","description":"Discover exact published action versions for an active account returned by list_connected_apps. This does not execute an action.","parameters":{"type":"object","properties":{"account_id":{"type":"string"},"toolkit":{"type":"string"},"query":{"type":"string"},"cursor":{"type":"string"}},"required":["account_id","toolkit","query"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"describe_app_action","description":"Read a pinned action schema for an exact connected account and published version. Do not invent slugs or versions.","parameters":{"type":"object","properties":{"account_id":{"type":"string"},"action":{"type":"string"},"version":{"type":"string"}},"required":["account_id","action","version"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"request_app_action","description":"Ask the user to approve exact account, action version and JSON arguments. Only approval runs the action once. A declined or uncertain effect must never be repeated automatically. Inspect the real provider receipt and log ID before claiming success.","parameters":{"type":"object","properties":{"account_id":{"type":"string"},"action":{"type":"string"},"version":{"type":"string"},"arguments":{"type":"object"}},"required":["account_id","action","version","arguments"],"additionalProperties":false}}}
    ])
}
fn account_guard(expected: Option<&str>) -> Result<()> {
    if let Some(expected) = expected {
        let current = waku_credentials::cloud::status()?;
        anyhow::ensure!(
            current.signed_in && current.workspace_id.as_deref() == Some(expected),
            "The app account changed; restore this conversation's original Ion account"
        );
    }
    Ok(())
}
pub(super) fn execute(
    receiver: &Receiver<Command>,
    cancelled: &AtomicBool,
    events: &DriverEventSender,
    journal_path: &Path,
    journal: &mut Journal,
    call: &ToolCall,
    args: &Value,
) -> Result<String> {
    account_guard(journal.managed_workspace.as_deref())?;
    let service = crate::credentials::CredentialService::global();
    let result = match call.name.as_str() {
        "list_connected_apps" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Args {
                cursor: Option<String>,
            }
            let a: Args = serde_json::from_value(args.clone())?;
            let page = service.composio_app_page(
                "",
                a.cursor.as_deref(),
                true,
                journal.managed_workspace.as_deref(),
            )?;
            json!({"accounts":page.accounts.iter().filter(|a|a.status=="ACTIVE").take(50).map(|a|json!({"account_id":a.id,"toolkit":a.toolkit})).collect::<Vec<_>>(),"next_cursor":page.next_cursor}).to_string()
        }
        "search_app_actions" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Args {
                account_id: String,
                toolkit: String,
                query: String,
                cursor: Option<String>,
            }
            let a: Args = serde_json::from_value(args.clone())?;
            let page = service.composio_tools_for(
                &a.account_id,
                &a.toolkit,
                &a.query,
                a.cursor.as_deref(),
                journal.managed_workspace.as_deref(),
            )?;
            let items:Vec<_>=page.items.iter().map(|t|json!({"action":t.slug,"name":t.name,"version":t.version,"description":t.description})).collect();
            json!({"items":items,"next_cursor":page.next_cursor}).to_string()
        }
        "describe_app_action" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Args {
                account_id: String,
                action: String,
                version: String,
            }
            let a: Args = serde_json::from_value(args.clone())?;
            let tool = service.composio_tool_for(
                &a.account_id,
                &a.action,
                &a.version,
                journal.managed_workspace.as_deref(),
            )?;
            json!({"action":tool.slug,"version":tool.version,"input_parameters":tool.input_parameters}).to_string()
        }
        "request_app_action" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Args {
                account_id: String,
                action: String,
                version: String,
                arguments: Value,
            }
            let a: Args = serde_json::from_value(args.clone())?;
            let effect = service.prepare_composio_action_for(
                &a.account_id,
                &a.action,
                &a.version,
                &a.arguments,
                journal.managed_workspace.as_deref(),
            )?;
            let _ = events.send(DriverEvent::Permission {
                request_id: call.id.clone(),
                title: format!("Run {}?", a.action),
                detail: format!(
                    "Connected account: {}\nTool version: {}\nExact arguments:\n{}",
                    a.account_id,
                    a.version,
                    serde_json::to_string_pretty(&a.arguments)?
                ),
                options: vec![
                    PermissionOption {
                        id: "allow".into(),
                        label: "Run once".into(),
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
                anyhow::ensure!(
                    !cancelled.load(Ordering::Acquire),
                    "Stopped before app approval"
                );
                match receiver.recv_timeout(Duration::from_millis(200)) {
                    Ok(Command::Respond(id, choice)) if id == call.id => {
                        anyhow::ensure!(
                            choice == "allow",
                            "App action declined; no effect was sent"
                        );
                        break;
                    }
                    Ok(Command::Stop) | Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                        bail!("Stopped before app approval")
                    }
                    _ => {}
                }
            }
            account_guard(journal.managed_workspace.as_deref())?;
            anyhow::ensure!(
                !cancelled.load(Ordering::Acquire),
                "Stopped before app execution"
            );
            journal.pending_effect = Some(call.id.clone());
            persist(journal_path, journal)?;
            effect.execute()?
        }
        _ => bail!("Unsupported app action"),
    };
    anyhow::ensure!(result.len() <= 64 * 1024, "App response exceeds its bound");
    Ok(result)
}
