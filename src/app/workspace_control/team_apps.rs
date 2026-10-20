//! Exact connector grants. Rendering reads only cached account/action pages.
use super::*;
use waku_client::teams::AppActionGrant;
impl Waku {
    fn load_agent_app_actions(&mut self, cursor: Option<String>, cx: &mut Context<Self>) {
        let Some(account) = self.control_ui.app_action_account.clone() else {
            return;
        };
        let Some(toolkit) = self
            .control_ui
            .app_accounts
            .iter()
            .find(|a| a.id == account && a.status == "ACTIVE")
            .map(|a| a.toolkit.clone())
        else {
            return;
        };
        let query = self
            .control_ui
            .app_action_search
            .read(cx)
            .content()
            .trim()
            .to_owned();
        self.control_ui.app_action_generation += 1;
        let generation = self.control_ui.app_action_generation;
        let editor = self.control_ui.editor_generation;
        self.control_ui.app_action_busy = true;
        let expected_workspace = self.control_ui.apps_workspace.clone();
        let request_account = account.clone();
        self.control_ui.app_action_tools.clear();
        self.control_ui.app_action_next = None;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || {
                        crate::credentials::CredentialService::global().composio_tools_for(
                            &request_account,
                            &toolkit,
                            &query,
                            cursor.as_deref(),
                            expected_workspace.as_deref(),
                        )
                    })
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.control_ui.form != Some(FormKind::Agent)
                    || this.control_ui.app_action_generation != generation
                    || this.control_ui.editor_generation != editor
                    || this.control_ui.app_action_account.as_ref() != Some(&account)
                {
                    return;
                }
                this.control_ui.app_action_busy = false;
                match result {
                    Ok(page) => {
                        this.control_ui.app_action_tools = page.items;
                        this.control_ui.app_action_next = page.next_cursor;
                        this.control_ui.error = None;
                    }
                    Err(error) => this.control_ui.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn render_agent_app_grants(&self, cx: &mut Context<Self>) -> Div {
        let theme = Theme::workspace(cx);
        let mut panel = div().flex().flex_col().gap(px(10.0)).child(
            div()
                .text_size(sp(12.5))
                .text_color(theme.text_secondary)
                .child("Connected app actions"),
        );
        for (index, grant) in self.control_ui.access.app_actions.iter().enumerate() {
            let remove = grant.clone();
            panel = panel.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(3.0))
                            .child(
                                div()
                                    .text_size(sp(12.5))
                                    .text_color(theme.text)
                                    .child(grant.action.clone()),
                            )
                            .child(
                                div()
                                    .text_size(sp(11.5))
                                    .text_color(theme.text_secondary)
                                    .child(format!(
                                        "{} · {}",
                                        grant.account_id,
                                        if grant.version.is_empty() {
                                            "Reselect a version"
                                        } else {
                                            &grant.version
                                        }
                                    )),
                            ),
                    )
                    .child(self.control_button(
                        format!("agent-app-remove-{index}"),
                        "Remove",
                        cx,
                        move |this, _, cx| {
                            this.control_ui.access.app_actions.retain(|g| g != &remove);
                            cx.notify();
                        },
                    )),
            );
        }
        panel = panel.child(self.control_button(
            "agent-app-load",
            if self.control_ui.apps_busy {
                "Loading apps…"
            } else {
                "Load connected apps"
            },
            cx,
            |this, _, cx| this.refresh_composio_apps(cx),
        ));
        let accounts: Vec<_> = self
            .control_ui
            .app_accounts
            .iter()
            .filter(|a| a.status == "ACTIVE")
            .map(|a| (a.id.clone(), a.toolkit.clone()))
            .collect();
        if accounts.is_empty() {
            return panel;
        }
        let selected = self.control_ui.app_action_account.clone();
        let label = accounts
            .iter()
            .find(|a| Some(&a.0) == selected.as_ref())
            .map(|(id, toolkit)| format!("{} · {}", app_name(toolkit), id))
            .unwrap_or_else(|| "Choose connected account".into());
        let weak = cx.entity().downgrade();
        let menu = self.menu_handle("agent-app-account-menu", cx);
        panel = panel.child(dropdown_menu(
            self.control_button("agent-app-account", label, cx, |_, _, _| {}),
            "agent-app-account-menu",
            &menu,
            MenuAlign::BelowLeft,
            move |_| {
                accounts
                    .iter()
                    .map(|(id, toolkit)| {
                        let weak = weak.clone();
                        let id = id.clone();
                        let active = Some(&id) == selected.as_ref();
                        MenuItem::new(format!("{} · {}", app_name(toolkit), id), move |_, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.control_ui.app_action_account = Some(id.clone());
                                this.load_agent_app_actions(None, cx);
                            });
                        })
                        .selected(active)
                    })
                    .collect()
            },
        ));
        if self.control_ui.app_action_account.is_none() {
            return panel;
        }
        panel = panel
            .child(control_field(
                "Find actions",
                self.control_ui.app_action_search.clone(),
                "agent-app-search",
            ))
            .child(self.control_button(
                "agent-app-action-search",
                if self.control_ui.app_action_busy {
                    "Searching…"
                } else {
                    "Search actions"
                },
                cx,
                |this, _, cx| this.load_agent_app_actions(None, cx),
            ));
        for (index, tool) in self.control_ui.app_action_tools.iter().enumerate() {
            let grant = AppActionGrant {
                account_id: self.control_ui.app_action_account.clone().unwrap(),
                action: tool.slug.clone(),
                version: tool.version.clone(),
            };
            let granted = self.control_ui.access.app_actions.contains(&grant);
            panel = panel.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(sp(12.5))
                            .text_color(theme.text)
                            .child(tool.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(sp(11.5))
                            .text_color(theme.text_secondary)
                            .child(format!("{} · {}", tool.slug, tool.version)),
                    )
                    .child(self.control_button(
                        format!("agent-app-action-{index}"),
                        if granted { "Granted" } else { "Grant action" },
                        cx,
                        move |this, _, cx| {
                            if this.control_ui.access.app_actions.len() < 64
                                || this.control_ui.access.app_actions.iter().any(|g| {
                                    g.account_id == grant.account_id && g.action == grant.action
                                })
                            {
                                this.control_ui.access.app_actions.retain(|g| {
                                    g.account_id != grant.account_id || g.action != grant.action
                                });
                                this.control_ui.access.app_actions.push(grant.clone());
                                cx.notify();
                            }
                        },
                    )),
            );
        }
        if let Some(cursor) = &self.control_ui.app_action_next {
            let cursor = cursor.clone();
            panel = panel.child(self.control_button(
                "agent-app-actions-next",
                "More actions",
                cx,
                move |this, _, cx| this.load_agent_app_actions(Some(cursor.clone()), cx),
            ));
        }
        panel
    }
}
