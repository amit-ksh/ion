//! Connector transport seam; the coordinator, not the provider, authorizes effects.
use anyhow::Result;
use serde_json::{Value, json};
use waku_protocol::teams::TeamAppAction;
pub(crate) type PreparedApp = Box<dyn FnOnce() -> Result<String>>;
pub(crate) trait TeamApps {
    fn describe(&self, action: &TeamAppAction) -> Result<Value>;
    /// Performs metadata/account checks only. The returned effect starts after reservation.
    fn prepare(&self, action: &TeamAppAction) -> Result<PreparedApp>;
}
pub(super) struct ComposioApps;
impl TeamApps for ComposioApps {
    fn describe(&self, action: &TeamAppAction) -> Result<Value> {
        let g = &action.grant;
        let tool = crate::credentials::CredentialService::global().composio_tool(
            &g.account_id,
            &g.action,
            &g.version,
        )?;
        Ok(
            json!({"account_id":g.account_id,"action":tool.slug,"name":tool.name,"description":tool.description,"version":tool.version,"input_parameters":tool.input_parameters}),
        )
    }
    fn prepare(&self, action: &TeamAppAction) -> Result<PreparedApp> {
        let g = &action.grant;
        let arguments = serde_json::from_str(&action.arguments_json)?;
        let prepared = crate::credentials::CredentialService::global().prepare_composio_action(
            &g.account_id,
            &g.action,
            &g.version,
            &arguments,
        )?;
        Ok(Box::new(move || prepared.execute()))
    }
}
