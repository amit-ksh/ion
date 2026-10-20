//! Connector transport seam; the coordinator, not the provider, authorizes effects.
use anyhow::Result;
use serde_json::{Value, json};
use waku_protocol::teams::TeamAppAction;
pub(crate) type PreparedApp = Box<dyn FnOnce() -> Result<String>>;
pub(crate) trait TeamApps {
    fn describe(&self, action: &TeamAppAction) -> Result<Value>;
    /// Performs metadata/account checks only. The returned effect starts after reservation.
    fn prepare(&self, action: &TeamAppAction) -> Result<PreparedApp>;
    fn describe_for(&self, action: &TeamAppAction, _workspace: Option<&str>) -> Result<Value> {
        self.describe(action)
    }
    fn prepare_for(&self, action: &TeamAppAction, _workspace: Option<&str>) -> Result<PreparedApp> {
        self.prepare(action)
    }
}
/// Preserve the run's original account through metadata preparation and delayed effects.
pub(super) struct ScopedApps<'a> {
    pub inner: &'a dyn TeamApps,
    pub workspace: Option<&'a str>,
}
impl TeamApps for ScopedApps<'_> {
    fn describe(&self, action: &TeamAppAction) -> Result<Value> {
        self.inner.describe_for(action, self.workspace)
    }
    fn prepare(&self, action: &TeamAppAction) -> Result<PreparedApp> {
        let effect = self.inner.prepare_for(action, self.workspace)?;
        let expected = self.workspace.map(str::to_owned);
        Ok(Box::new(move || {
            if let Some(expected) = &expected {
                let current = waku_credentials::cloud::status()?;
                anyhow::ensure!(
                    current.signed_in && current.workspace_id.as_ref() == Some(expected),
                    "The app account changed; restore this team's original Ion account"
                );
            }
            effect()
        }))
    }
}
pub(super) struct ComposioApps;
impl TeamApps for ComposioApps {
    fn describe(&self, action: &TeamAppAction) -> Result<Value> {
        self.describe_for(action, None)
    }
    fn describe_for(&self, action: &TeamAppAction, workspace: Option<&str>) -> Result<Value> {
        let g = &action.grant;
        let tool = crate::credentials::CredentialService::global().composio_tool_for(
            &g.account_id,
            &g.action,
            &g.version,
            workspace,
        )?;
        Ok(
            json!({"account_id":g.account_id,"action":tool.slug,"name":tool.name,"description":tool.description,"version":tool.version,"input_parameters":tool.input_parameters}),
        )
    }
    fn prepare(&self, action: &TeamAppAction) -> Result<PreparedApp> {
        self.prepare_for(action, None)
    }
    fn prepare_for(&self, action: &TeamAppAction, workspace: Option<&str>) -> Result<PreparedApp> {
        let g = &action.grant;
        let arguments = serde_json::from_str(&action.arguments_json)?;
        let prepared = crate::credentials::CredentialService::global()
            .prepare_composio_action_for(
                &g.account_id,
                &g.action,
                &g.version,
                &arguments,
                workspace,
            )?;
        Ok(Box::new(move || prepared.execute()))
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};
    struct Probe {
        seen: RefCell<Vec<Option<String>>>,
        executed: Rc<RefCell<bool>>,
    }
    impl TeamApps for Probe {
        fn describe(&self, _: &TeamAppAction) -> Result<Value> {
            unreachable!("scoped method must be used")
        }
        fn prepare(&self, _: &TeamAppAction) -> Result<PreparedApp> {
            unreachable!("scoped method must be used")
        }
        fn describe_for(&self, _: &TeamAppAction, scope: Option<&str>) -> Result<Value> {
            self.seen.borrow_mut().push(scope.map(str::to_owned));
            Ok(json!({}))
        }
        fn prepare_for(&self, _: &TeamAppAction, scope: Option<&str>) -> Result<PreparedApp> {
            self.seen.borrow_mut().push(scope.map(str::to_owned));
            let executed = self.executed.clone();
            Ok(Box::new(move || {
                *executed.borrow_mut() = true;
                Ok("receipt".into())
            }))
        }
    }
    #[test]
    fn managed_team_apps_keep_original_scope_through_prepare_and_delayed_dispatch() {
        let probe = Probe {
            seen: RefCell::new(vec![]),
            executed: Rc::new(RefCell::new(false)),
        };
        // Deliberately invalid identity cannot equal an admitted account workspace hash.
        let scoped = ScopedApps {
            inner: &probe,
            workspace: Some("not-an-admitted-workspace"),
        };
        let action: TeamAppAction = serde_json::from_value(json!({
            "grant":{"accountId":"account","action":"ACTION","version":"1"},
            "argumentsJson":"{}"
        }))
        .unwrap();
        scoped.describe(&action).unwrap();
        let prepared = scoped.prepare(&action).unwrap();
        assert_eq!(
            *probe.seen.borrow(),
            vec![Some("not-an-admitted-workspace".into()); 2]
        );
        assert!(prepared().is_err());
        assert!(!*probe.executed.borrow());
    }
}
