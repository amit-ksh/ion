//! Cached bounded usage pages. Queries and polling never run from render.
use super::*;
use waku_client::usage_ledger::*;
pub(super) struct UsageUi {
    history: bool,
    list: ListState,
    scrollbar: Rc<ScrollbarState>,
    page: Option<Rc<UsageLedgerPage>>,
    rows: Rc<Vec<Rc<UsageRow>>>,
    busy: bool,
    generation: u64,
    polling: bool,
    error: Option<String>,
    project: Option<Uuid>,
    provider: Option<ProviderKind>,
    since: Option<u64>,
    range: &'static str,
    after: Option<u64>,
    previous: Vec<Option<u64>>,
}
pub(super) struct UsageRow {
    entry: UsageLedgerEntry,
    title: String,
    detail: String,
    meter: String,
    cost: String,
}
impl UsageUi {
    pub(super) fn new(_window: &mut Window, _cx: &mut App) -> Self {
        Self {
            history: false,
            list: ListState::new(0, ListAlignment::Top, px(128.0)),
            scrollbar: ScrollbarState::new(),
            page: None,
            rows: Rc::new(vec![]),
            busy: false,
            generation: 0,
            polling: false,
            error: None,
            project: None,
            provider: None,
            since: None,
            range: "All time",
            after: None,
            previous: vec![],
        }
    }
    fn query(&self) -> UsageLedgerQuery {
        UsageLedgerQuery {
            project_id: self.project,
            task_id: None,
            agent_id: None,
            provider: self.provider,
            environment: None,
            kind: None,
            model: None,
            since: self.since,
            after: self.after,
            limit: 32,
        }
    }
}
fn money(amount: &UsageMoney) -> String {
    format!(
        "{} {:.6}",
        amount.currency,
        amount.micros as f64 / 1_000_000.0
    )
}
fn cost_label(cost: &UsageCost) -> String {
    format!(
        "{} · {} · {}",
        money(&cost.amount),
        if cost.quality == UsageCostQuality::Estimated {
            "Estimated"
        } else {
            "Reported"
        },
        if cost.finality == UsageCostFinality::Final {
            "Final"
        } else {
            "Provisional"
        }
    )
}
impl UsageRow {
    fn new(entry: UsageLedgerEntry) -> Self {
        let title = entry.model.clone().unwrap_or_else(|| {
            match entry.kind {
                UsageLedgerKind::RemoteCommand => "Remote command",
                UsageLedgerKind::GpuJob => "GPU job",
                _ => "Model request",
            }
            .into()
        });
        let provider = entry
            .provider
            .map(|p| p.display_name())
            .unwrap_or("Unknown harness");
        let detail = format!(
            "{} · {provider} · {} · {} · {}",
            entry.agent_name.as_deref().unwrap_or("Task agent"),
            entry.host,
            entry.status,
            automation_ui::next_fire(entry.started_at.unwrap_or(entry.observed_at))
        );
        let mut meter =
            if let (Some(input), Some(output)) = (entry.input_tokens, entry.output_tokens) {
                format!("{input} input · {output} output tokens")
            } else if matches!(
                entry.kind,
                UsageLedgerKind::ModelCall | UsageLedgerKind::SoloModelCall
            ) {
                format!(
                    "Token usage unavailable · {} tokens reserved",
                    entry.reserved_tokens
                )
            } else {
                "Resource usage".into()
            };
        if let Some(ms) = entry.elapsed_milliseconds {
            meter.push_str(&format!(" · {:.3}s elapsed", ms as f64 / 1000.0));
        }
        let mut cost = entry
            .cost
            .as_ref()
            .map(cost_label)
            .unwrap_or_else(|| "Cost unavailable".into());
        if let Some(cap) = &entry.spending_allowance {
            cost.push_str(&format!(" · Approved allowance {}", money(cap)));
        }
        Self {
            entry,
            title,
            detail,
            meter,
            cost,
        }
    }
}
impl Waku {
    pub(super) fn reset_usage_ledger(&mut self, cx: &mut Context<Self>) {
        let ui = &mut self.control_ui.usage;
        ui.after = None;
        ui.previous.clear();
        ui.page = None;
        ui.rows = Rc::new(vec![]);
        ui.list.reset(0);
        ui.busy = false;
        ui.generation += 1;
        self.control_ui.activity_revision += 1;
        self.refresh_usage_ledger(cx);
        cx.notify();
    }
    pub(super) fn start_usage_poll(&mut self, cx: &mut Context<Self>) {
        if self.control_ui.usage.polling {
            return;
        }
        self.control_ui.usage.polling = true;
        cx.spawn(async move |this, cx| {
            loop {
                smol::Timer::after(Duration::from_secs(15)).await;
                let keep = this
                    .update(cx, |this, cx| {
                        if this.settings_page != Some(SettingsPage::Usage) {
                            this.control_ui.usage.polling = false;
                            return false;
                        }
                        if this.control_ui.budgets.open {
                            this.refresh_cloud_budgets(cx);
                        }
                        if !this.control_ui.usage.history && !this.usage_resource_open() {
                            this.refresh_usage_ledger(cx);
                        }
                        true
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        })
        .detach();
    }
    pub(super) fn refresh_usage_ledger(&mut self, cx: &mut Context<Self>) {
        if self.settings_page != Some(SettingsPage::Usage) || self.control_ui.usage.busy {
            return;
        }
        let query = self.control_ui.usage.query();
        self.control_ui.usage.busy = true;
        self.control_ui.usage.generation += 1;
        let generation = self.control_ui.usage.generation;
        let client = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let requested = query.clone();
            let response = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || -> anyhow::Result<UsageLedgerPage> {
                        let ResponsePayload::UsageLedger { page } = client.request(
                            Uuid::nil(),
                            Uuid::nil(),
                            Rpc::LoadUsageLedger { query: requested },
                        )?
                        else {
                            anyhow::bail!("Unexpected usage response")
                        };
                        Ok(page)
                    })
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if generation != this.control_ui.usage.generation {
                    return;
                }
                this.control_ui.usage.busy = false;
                if query != this.control_ui.usage.query() {
                    this.refresh_usage_ledger(cx);
                    return;
                }
                match response {
                    Ok(page) => {
                        this.control_ui.usage.rows = Rc::new(
                            page.entries
                                .iter()
                                .cloned()
                                .map(|e| Rc::new(UsageRow::new(e)))
                                .collect(),
                        );
                        this.control_ui.usage.page = Some(Rc::new(page));
                        this.control_ui
                            .usage
                            .list
                            .reset(this.control_ui.usage.rows.len());
                        this.control_ui.usage.error = None;
                        this.control_ui.activity_revision += 1;
                    }
                    Err(error) => {
                        this.control_ui.usage.error =
                            Some(format!("Could not load usage: {error}. Refresh to retry."))
                    }
                };
                cx.notify();
            });
        })
        .detach();
    }

    pub(in crate::app) fn open_usage_settings(&mut self, cx: &mut Context<Self>) {
        if self.control_ui.usage.history {
            self.ensure_usage_history(false, cx);
        }
        self.refresh_usage_ledger(cx);
        self.start_usage_poll(cx);
    }

    pub(in crate::app) fn usage_settings_fills_viewport(&self) -> bool {
        self.usage_resource_open()
            || !self.control_ui.usage.history
            || self.usage_view != UsageViewMode::Daily
    }

    fn usage_resource_open(&self) -> bool {
        self.control_ui.collector.request.is_some()
            || self.control_ui.resources.open
            || self.control_ui.remote.open
            || self.control_ui.billing.open
            || self.control_ui.budgets.open
    }

    fn close_usage_resources(&mut self) {
        self.control_ui.remote.close();
        self.control_ui.resources.close();
        self.control_ui.collector.close();
        self.control_ui.billing.close();
        self.control_ui.budgets.open = false;
    }

    pub(in crate::app) fn render_usage_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let mut tabs = div().flex().flex_wrap().items_center().gap(px(8.0));
        for (history, label) in [(false, "Ion activity"), (true, "CLI history")] {
            tabs = tabs.child(
                self.control_button(
                    format!("settings-usage-source-{history}"),
                    label,
                    cx,
                    move |this, _, cx| {
                        this.close_usage_resources();
                        this.control_ui.usage.history = history;
                        this.settings_scroll.set_offset(gpui::Point::default());
                        this.open_usage_settings(cx);
                        cx.notify();
                    },
                )
                .bg(
                    if self.control_ui.usage.history == history && !self.usage_resource_open() {
                        theme.overlay_strong
                    } else {
                        gpui::transparent_black()
                    },
                ),
            );
        }
        let weak = cx.entity().downgrade();
        let menu = self.menu_handle("usage-resource-menu", cx);
        tabs = tabs.child(div().flex_1()).child(dropdown_menu(
            self.control_button("usage-resources", "Resources", cx, |_, _, _| {})
                .child(icon("icons/chevron-down.svg", 12.0, theme.text_secondary)),
            "usage-resource-menu",
            &menu,
            MenuAlign::BelowRight,
            move |_| {
                [
                    "GPU budgets",
                    "Remote executions",
                    "Cloud billing",
                    "Command resources",
                ]
                .into_iter()
                .enumerate()
                .map(|(index, label)| {
                    let weak = weak.clone();
                    MenuItem::new(label, move |_, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.close_usage_resources();
                            match index {
                                0 => {
                                    this.control_ui.budgets.open = true;
                                    this.refresh_cloud_budgets(cx);
                                }
                                1 => this.open_remote_executions(cx),
                                2 => {
                                    this.control_ui.billing.open = true;
                                    this.refresh_cloud_billing(cx);
                                }
                                _ => this.open_command_resources(cx),
                            }
                            cx.notify();
                        });
                    })
                })
                .collect()
            },
        ));
        let content = if self.control_ui.collector.request.is_some() {
            self.render_gpu_collector(cx)
        } else if self.control_ui.resources.open {
            self.render_command_resources(cx)
        } else if self.control_ui.remote.open {
            self.render_remote_executions(cx)
        } else if self.control_ui.billing.open {
            self.render_cloud_billing(cx)
        } else if self.control_ui.budgets.open {
            self.render_cloud_budgets(cx)
        } else if self.control_ui.usage.history {
            self.render_usage_history_settings(cx)
        } else {
            self.render_harness_activity(cx)
        };
        div()
            .track_focus(&self.control_ui.focus)
            .tab_group()
            .mt(px(16.0))
            .when(self.usage_settings_fills_viewport(), |e| {
                e.flex_1().min_h_0()
            })
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(tabs)
            .child(content)
            .into_any_element()
    }

    fn render_harness_activity(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let ui = &self.control_ui.usage;
        let mut filters = div().flex().flex_wrap().items_center().gap(px(8.0));
        for which in 0..3 {
            let label = match which {
                0 => ui
                    .project
                    .and_then(|id| self.state.projects.iter().find(|p| p.id == id))
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| "All projects".into()),
                1 => ui
                    .provider
                    .map(|p| p.display_name())
                    .unwrap_or("All harnesses")
                    .into(),
                _ => ui.range.into(),
            };
            let id = format!("settings-usage-filter-{which}");
            let weak = cx.entity().downgrade();
            let menu = self.menu_handle(id.clone(), cx);
            filters = filters.child(dropdown_menu(
                self.control_button(id.clone(), label, cx, |_, _, _| {})
                    .child(icon("icons/chevron-down.svg", 12.0, theme.text_secondary)),
                SharedString::from(id),
                &menu,
                MenuAlign::BelowLeft,
                move |cx| {
                    let Some(entity) = weak.upgrade() else {
                        return vec![];
                    };
                    let this = entity.read(cx);
                    let ui = &this.control_ui.usage;
                    let mut options: Vec<(String, Option<String>, bool)> = vec![];
                    match which {
                        0 => {
                            options.push(("All projects".into(), None, ui.project.is_none()));
                            options.extend(this.state.projects.iter().map(|p| {
                                (
                                    p.name.clone(),
                                    Some(p.id.to_string()),
                                    ui.project == Some(p.id),
                                )
                            }));
                        }
                        1 => {
                            options.push(("All harnesses".into(), None, ui.provider.is_none()));
                            options.extend(ProviderKind::ALL.into_iter().map(|p| {
                                (
                                    p.display_name().into(),
                                    Some(p.id().to_owned()),
                                    ui.provider == Some(p),
                                )
                            }));
                        }
                        _ => {
                            for (days, label) in [
                                (None, "All time"),
                                (Some(7), "Last 7 days"),
                                (Some(30), "Last 30 days"),
                                (Some(90), "Last 90 days"),
                            ] {
                                options.push((
                                    label.into(),
                                    days.map(|d| d.to_string()),
                                    ui.range == label,
                                ));
                            }
                        }
                    }
                    options
                        .into_iter()
                        .map(|(label, value, selected)| {
                            let weak = weak.clone();
                            MenuItem::new(label.clone(), move |_, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    let ui = &mut this.control_ui.usage;
                                    match which {
                                        0 => {
                                            ui.project = value
                                                .as_deref()
                                                .and_then(|v| Uuid::parse_str(v).ok())
                                        }
                                        1 => {
                                            ui.provider = value.as_deref().and_then(|v| {
                                                ProviderKind::ALL.into_iter().find(|p| p.id() == v)
                                            })
                                        }
                                        _ => {
                                            ui.since = value
                                                .as_deref()
                                                .and_then(|v| v.parse::<u64>().ok())
                                                .map(|days| {
                                                    chrono::Utc::now().timestamp().max(0) as u64
                                                        - days * 86400
                                                });
                                            ui.range = match value.as_deref() {
                                                Some("7") => "Last 7 days",
                                                Some("30") => "Last 30 days",
                                                Some("90") => "Last 90 days",
                                                _ => "All time",
                                            };
                                        }
                                    }
                                    this.reset_usage_ledger(cx);
                                });
                            })
                            .selected(selected)
                            .into()
                        })
                        .collect()
                },
            ));
        }
        filters = filters.child(div().flex_1()).child(
            self.control_button("settings-usage-refresh", "", cx, |this, _, cx| {
                this.refresh_usage_ledger(cx)
            })
            .size(px(32.0))
            .p_0()
            .justify_center()
            .border_0()
            .tooltip(Tooltip::text(if ui.busy {
                "Refreshing usage"
            } else {
                "Refresh usage"
            }))
            .child(icon("icons/rotate-cw.svg", 16.0, theme.text_secondary)),
        );
        let mut body=div().flex_1().min_h_0().flex().flex_col().gap(px(12.0)).child(filters)
            .child(div().text_size(sp(12.5)).text_color(theme.text_secondary)
                .child("Usage recorded by Ion across tasks and teams. Missing token counts and costs are unavailable."));
        if let Some(page) = &ui.page {
            let s = &page.summary;
            body = body.child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(16.0))
                    .text_size(sp(13.0))
                    .text_color(theme.text)
                    .child(format!("{} requests", s.model_calls))
                    .child(format!(
                        "{} input / {} output tokens",
                        s.input_tokens, s.output_tokens
                    ))
                    .when(s.unknown_calls > 0, |e| {
                        e.child(format!("{} requests awaiting usage", s.unknown_calls))
                    }),
            );
            let costs = s
                .costs
                .iter()
                .map(|c| {
                    format!(
                        "{} · {} · {}",
                        money(&c.amount),
                        if c.quality == UsageCostQuality::Reported {
                            "reported"
                        } else {
                            "estimated"
                        },
                        if c.finality == UsageCostFinality::Final {
                            "final"
                        } else {
                            "provisional"
                        }
                    )
                })
                .collect::<Vec<_>>();
            body = body.child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(if costs.is_empty() {
                        "Cost unavailable".into()
                    } else {
                        costs.join(" · ")
                    })
                    .when(s.unknown_cost_records > 0, |e| {
                        e.child(format!(
                            " · {} records without cost",
                            s.unknown_cost_records
                        ))
                    }),
            );
        }
        if let Some(error) = &ui.error {
            body = body.child(
                div()
                    .text_size(sp(13.0))
                    .text_color(theme.danger)
                    .child(error.clone()),
            );
        }
        if ui.rows.is_empty() {
            body = body.child(
                div()
                    .flex_1()
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(12.0))
                    .text_color(theme.text_secondary)
                    .child(icon("icons/chart-column.svg", 28.0, theme.text_tertiary))
                    .child(if ui.busy {
                        "Loading harness activity…"
                    } else {
                        "No activity in this selection."
                    }),
            );
        } else {
            let rows = ui.rows.clone();
            let weak = cx.entity().downgrade();
            body = body.child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(
                        list(ui.list.clone(), move |index, _, cx| {
                            weak.upgrade()
                                .map(|entity| {
                                    entity.update(cx, |this, cx| {
                                        this.render_harness_usage_row(rows[index].clone(), cx)
                                    })
                                })
                                .unwrap_or_else(|| div().into_any_element())
                        })
                        .size_full(),
                    )
                    .child(scrollbar::vertical(&ui.list, &ui.scrollbar)),
            );
        }
        let mut paging = div().flex().gap(px(8.0));
        if !ui.previous.is_empty() {
            paging = paging.child(self.control_button(
                "settings-usage-previous",
                "Previous",
                cx,
                |this, _, cx| {
                    if this.control_ui.usage.busy {
                        return;
                    }
                    if let Some(after) = this.control_ui.usage.previous.pop() {
                        this.control_ui.usage.after = after;
                        this.refresh_usage_ledger(cx);
                    }
                },
            ));
        }
        if let Some(next) = ui.page.as_ref().and_then(|p| p.next_cursor) {
            paging = paging.child(self.control_button(
                "settings-usage-next",
                "Next",
                cx,
                move |this, _, cx| {
                    if this.control_ui.usage.busy {
                        return;
                    }
                    this.control_ui
                        .usage
                        .previous
                        .push(this.control_ui.usage.after);
                    this.control_ui.usage.after = Some(next);
                    this.refresh_usage_ledger(cx);
                },
            ));
        }
        body.child(paging).into_any_element()
    }

    fn render_harness_usage_row(&self, row: Rc<UsageRow>, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let task = row.entry.task_id;
        let provider = row.entry.provider;
        div()
            .w_full()
            .py(px(14.0))
            .border_b_1()
            .border_color(theme.border)
            .flex()
            .gap(px(12.0))
            .child(match provider {
                Some(provider) => {
                    crate::ui::provider_mark(&theme, provider, 20.0, theme.text_secondary)
                        .into_any_element()
                }
                None => icon("icons/server.svg", 20.0, theme.text_secondary).into_any_element(),
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(sp(13.5))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(row.title.clone()),
                    )
                    .child(
                        div()
                            .text_size(sp(12.5))
                            .text_color(theme.text_secondary)
                            .child(row.detail.clone()),
                    )
                    .child(div().text_size(sp(12.5)).child(row.meter.clone()))
                    .child(
                        div()
                            .text_size(sp(12.5))
                            .text_color(theme.text_secondary)
                            .child(row.cost.clone()),
                    ),
            )
            .child(self.control_button(
                format!("settings-usage-task-{}", row.entry.sequence),
                "Open task",
                cx,
                move |this, _, cx| {
                    this.settings_page = None;
                    this.open_team_activity_task(task, cx);
                },
            ))
            .into_any_element()
    }
}
