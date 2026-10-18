//! Budget reads run on workers; rows use cached status and integer money labels.
use super::*;
use waku_client::cloud_spending::{
    CloudBudget, CloudBudgetPage, CloudBudgetStatus, format_micros, parse_micros,
};
pub(super) struct BudgetUi {
    pub(super) open: bool,
    pub(super) editing: bool,
    id: Uuid,
    revision: u64,
    name: Entity<TextInput>,
    project: Entity<TextInput>,
    currency: Entity<TextInput>,
    amount: Entity<TextInput>,
    targets: Entity<TextInput>,
    paused: bool,
    rows: Rc<Vec<CloudBudgetStatus>>,
    list: ListState,
    list_focus: FocusHandle,
    selected: Option<Uuid>,
    busy: bool,
    generation: u64,
    error: Option<String>,
}
impl BudgetUi {
    pub(super) fn new(window: &mut Window, cx: &mut App) -> Self {
        Self {
            open: false,
            editing: false,
            id: Uuid::nil(),
            revision: 0,
            name: cx.new(|cx| TextInput::new(window, cx).placeholder("GPU budget name")),
            project: cx.new(|cx| TextInput::new(window, cx).placeholder("Exact Nebius project ID")),
            currency: cx.new(|cx| TextInput::new(window, cx).placeholder("USD")),
            amount: cx.new(|cx| TextInput::new(window, cx).placeholder("100.000000")),
            targets: cx.new(|cx| {
                TextInput::new(window, cx).placeholder("Approved target names, separated by commas")
            }),
            paused: true,
            rows: Rc::new(vec![]),
            list: ListState::new(0, ListAlignment::Top, px(130.0)),
            list_focus: cx.focus_handle(),
            selected: None,
            busy: false,
            generation: 0,
            error: None,
        }
    }
}
impl Waku {
    pub(super) fn refresh_cloud_budgets(&mut self, cx: &mut Context<Self>) {
        if self.settings_page != Some(SettingsPage::Usage) || self.control_ui.budgets.busy {
            return;
        }
        let ui = &mut self.control_ui.budgets;
        ui.busy = true;
        ui.generation += 1;
        let generation = ui.generation;
        let client = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let response = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || -> anyhow::Result<CloudBudgetPage> {
                        let ResponsePayload::CloudBudgets { page } = client.request(
                            Uuid::nil(),
                            Uuid::nil(),
                            Rpc::LoadCloudBudgets {
                                after: None,
                                limit: 64,
                            },
                        )?
                        else {
                            anyhow::bail!("Unexpected GPU budget response");
                        };
                        anyhow::ensure!(
                            page.next_cursor.is_none(),
                            "GPU budget collection exceeds bound"
                        );
                        Ok(page)
                    })
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let ui = &mut this.control_ui.budgets;
                if ui.generation != generation {
                    return;
                }
                ui.busy = false;
                match response {
                    Ok(page) => {
                        ui.list.reset(page.budgets.len());
                        ui.rows = Rc::new(page.budgets);
                        ui.error = None;
                    }
                    Err(e) => ui.error = Some(format!("Could not load GPU budgets: {e}")),
                };
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn begin_cloud_budget(
        &mut self,
        budget: Option<CloudBudget>,
        cx: &mut Context<Self>,
    ) {
        if self.control_ui.busy || !self.control_ui.snapshot_loaded {
            return;
        }
        let budget = if let Some(requested) = budget {
            let saved = self.control_ui.snapshot.records.iter().find_map(|r| {
                if let ControlRecord::CloudBudget(b) = r {
                    (b.id == requested.id).then(|| b.clone())
                } else {
                    None
                }
            });
            let Some(saved) = saved else {
                self.control_ui.budgets.error = Some(
                    "Refresh before editing this budget; its saved definition is unavailable."
                        .into(),
                );
                cx.notify();
                return;
            };
            Some(saved)
        } else {
            None
        };
        let budget = budget.unwrap_or(CloudBudget {
            id: Uuid::new_v4(),
            name: String::new(),
            cloud_project: String::new(),
            currency: "USD".into(),
            limit_micros: 100_000_000,
            targets: vec![],
            paused: true,
        });
        let ui = &mut self.control_ui.budgets;
        ui.editing = true;
        ui.id = budget.id;
        ui.revision = self.control_ui.snapshot.revision;
        ui.paused = budget.paused;
        ui.error = None;
        for (input, value) in [
            (&ui.name, budget.name),
            (&ui.project, budget.cloud_project),
            (&ui.currency, budget.currency),
            (&ui.amount, format_micros(budget.limit_micros)),
            (&ui.targets, budget.targets.join(", ")),
        ] {
            input.update(cx, |input, cx| input.set_content(value, cx));
        }
        cx.notify();
    }
    pub(super) fn cloud_budget_draft(&self, cx: &App) -> anyhow::Result<CloudBudget> {
        let ui = &self.control_ui.budgets;
        Ok(CloudBudget {
            id: ui.id,
            name: ui.name.read(cx).content().trim().into(),
            cloud_project: ui.project.read(cx).content().trim().into(),
            currency: ui.currency.read(cx).content().trim().into(),
            limit_micros: parse_micros(ui.amount.read(cx).content()).map_err(anyhow::Error::msg)?,
            targets: ui
                .targets
                .read(cx)
                .content()
                .split(',')
                .map(|t| t.trim().to_string())
                .collect(),
            paused: ui.paused,
        })
    }
    fn save_cloud_budget(&mut self, cx: &mut Context<Self>) {
        if !self.control_ui.budgets.editing
            || !self.control_ui.budgets.open
            || self.control_ui.busy
            || self.settings_page != Some(SettingsPage::Usage)
        {
            return;
        }
        match self.cloud_budget_draft(cx) {
            Ok(budget) => self.control_request(
                Rpc::SaveWorkspaceRecord {
                    expected_revision: self.control_ui.budgets.revision,
                    record: ControlRecord::CloudBudget(budget),
                },
                cx,
            ),
            Err(e) => {
                self.control_ui.budgets.error = Some(e.to_string());
                cx.notify();
            }
        }
    }
    fn cloud_budget_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let ui = &mut self.control_ui.budgets;
        if ui.rows.is_empty() {
            return;
        }
        let last = ui.rows.len() - 1;
        let current = ui
            .selected
            .and_then(|id| ui.rows.iter().position(|r| r.budget.id == id));
        let index = match event.keystroke.key.as_str() {
            "down" => current.map_or(0, |i| (i + 1).min(last)),
            "up" => current.map_or(last, |i| i.saturating_sub(1)),
            "home" => 0,
            "end" => last,
            "pagedown" => current.unwrap_or(0).saturating_add(4).min(last),
            "pageup" => current.unwrap_or(0).saturating_sub(4),
            "enter" | "space" => {
                let b = ui.rows[current.unwrap_or(0)].budget.clone();
                self.begin_cloud_budget(Some(b), cx);
                cx.stop_propagation();
                return;
            }
            _ => return,
        };
        ui.selected = Some(ui.rows[index].budget.id);
        ui.list.scroll_to_reveal_item(index);
        cx.stop_propagation();
        cx.notify();
    }
    pub(super) fn render_cloud_budgets(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let ui = &self.control_ui.budgets;
        let mut body=div().track_focus(&self.control_ui.focus).tab_group().flex_1().min_h_0().flex().flex_col().gap(px(14.0)).p_0()
   .child(div().flex().gap(px(8.0))
    .child(self.control_button("budget-usage","Usage",cx,|this,_,cx|{this.control_ui.budgets.open=false;cx.notify();}))
    .child(self.control_button("budget-new","New GPU budget",cx,|this,_,cx|this.begin_cloud_budget(None,cx)))
    .child(self.control_button("budget-refresh","",cx,|this,_,cx|{this.refresh_workspace_control(cx);this.refresh_cloud_budgets(cx);}).tooltip(Tooltip::text("Refresh GPU budgets")).child(icon("icons/rotate-cw.svg",16.0,theme.text_secondary))))
   .child(div().text_size(sp(13.0)).text_color(theme.text_secondary).child("Shared GPU allowances across runs. Held allowances remain reserved until provider billing is reconciled."));
        if let Some(e) = ui.error.as_ref().or(self.control_ui.error.as_ref()) {
            body = body.child(div().text_color(theme.danger).child(e.clone()));
        }
        if ui.busy {
            body = body.child(
                div()
                    .text_color(theme.text_secondary)
                    .child("Refreshing GPU budgets…"),
            );
        }
        let mut columns = div().flex_1().min_h_0().flex().gap(px(24.0));
        if ui.rows.is_empty() {
            columns = columns.child(
                div()
                    .flex_1()
                    .text_color(theme.text_secondary)
                    .child("No shared GPU budgets configured."),
            );
        } else {
            let weak = cx.entity().downgrade();
            let rows = ui.rows.clone();
            columns = columns.child(
                div()
                    .id("gpu-budget-list")
                    .track_focus(&ui.list_focus)
                    .tab_index(0)
                    .border_1()
                    .border_color(theme.border)
                    .focus_visible(|style| style.border_color(theme.accent))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        this.cloud_budget_key(event, cx)
                    }))
                    .flex_1()
                    .min_h_0()
                    .child(
                        list(ui.list.clone(), move |index, _, cx| {
                            weak.upgrade()
                                .map(|e| {
                                    e.update(cx, |this, cx| {
                                        rows.get(index)
                                            .cloned()
                                            .map(|row| this.render_cloud_budget_row(row, cx))
                                            .unwrap_or_else(|| div().into_any_element())
                                    })
                                })
                                .unwrap_or_else(|| div().into_any_element())
                        })
                        .size_full(),
                    ),
            );
        }
        if ui.editing {
            let mut editor = div()
                .id("budget-editor")
                .w(px(370.0))
                .min_h_0()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(control_field("Name", ui.name.clone(), "budget-name"))
                .child(control_field(
                    "Nebius project ID",
                    ui.project.clone(),
                    "budget-project",
                ))
                .child(control_field(
                    "Currency",
                    ui.currency.clone(),
                    "budget-currency",
                ))
                .child(control_field(
                    "Total allowance",
                    ui.amount.clone(),
                    "budget-amount",
                ))
                .child(control_field(
                    "Approved sandbox targets",
                    ui.targets.clone(),
                    "budget-targets",
                ));
            editor = editor.child(self.control_button(
                "budget-pause",
                if ui.paused {
                    "New launches paused · Enable"
                } else {
                    "New launches enabled · Pause"
                },
                cx,
                |this, _, cx| {
                    this.control_ui.budgets.paused = !this.control_ui.budgets.paused;
                    cx.notify();
                },
            ));
            editor = editor.child(
                div()
                    .flex()
                    .gap(px(8.0))
                    .child(self.control_button(
                        "budget-save",
                        if self.control_ui.busy {
                            "Saving…"
                        } else {
                            "Save budget"
                        },
                        cx,
                        |this, _, cx| this.save_cloud_budget(cx),
                    ))
                    .child(
                        self.control_button("budget-cancel", "Cancel", cx, |this, _, cx| {
                            this.control_ui.budgets.editing = false;
                            cx.notify();
                        }),
                    ),
            );
            columns = columns.child(editor);
        }
        body.child(columns).into_any_element()
    }
    fn render_cloud_budget_row(
        &self,
        row: CloudBudgetStatus,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        let budget = row.budget;
        div()
            .when(self.control_ui.budgets.selected == Some(budget.id), |row| {
                row.bg(theme.raised)
            })
            .py(px(16.0))
            .border_b_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .text_color(theme.text)
                    .child(format!(
                        "{} · {}",
                        budget.name,
                        if budget.paused { "Paused" } else { "Enabled" }
                    ))
                    .child(
                        div()
                            .text_size(sp(12.5))
                            .text_color(theme.text_secondary)
                            .child(format!(
                                "{} · {}",
                                budget.cloud_project,
                                budget.targets.join(", ")
                            )),
                    )
                    .child(format!(
                        "{} {} held · {} available · {} limit · {} requests",
                        budget.currency,
                        format_micros(row.held_micros),
                        format_micros(row.available_micros),
                        format_micros(budget.limit_micros),
                        row.requests
                    )),
            )
            .child(self.control_button(
                format!("budget-edit-{}", budget.id),
                "Edit",
                cx,
                move |this, _, cx| this.begin_cloud_budget(Some(budget.clone()), cx),
            ))
            .into_any_element()
    }
}
