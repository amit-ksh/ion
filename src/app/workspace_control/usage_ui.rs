//! Cached bounded usage pages. Queries and polling never run from render.
use super::*;
use waku_client::usage_ledger::*;
pub(super) struct UsageUi {
    page: Option<Rc<UsageLedgerPage>>,
    rows: Rc<Vec<Rc<UsageRow>>>,
    busy: bool,
    generation: u64,
    polling: bool,
    error: Option<String>,
    project: Option<Uuid>,
    task: Option<Uuid>,
    agent: Option<Uuid>,
    provider: Option<ProviderKind>,
    environment: Option<ExecutionEnvironment>,
    kind: Option<UsageLedgerKind>,
    model: String,
    since: Option<u64>,
    range: &'static str,
    after: Option<u64>,
    previous: Vec<Option<u64>>,
    model_input: Entity<TextInput>,
}
pub(super) struct UsageRow {
    entry: UsageLedgerEntry,
    title: String,
    detail: String,
    meter: String,
    cost: String,
}
impl UsageUi {
    pub(super) fn new(window: &mut Window, cx: &mut App) -> Self {
        Self {
            page: None,
            rows: Rc::new(vec![]),
            busy: false,
            generation: 0,
            polling: false,
            error: None,
            project: None,
            task: None,
            agent: None,
            provider: None,
            environment: None,
            kind: None,
            model: String::new(),
            since: None,
            range: "All time",
            after: None,
            previous: vec![],
            model_input: cx.new(|cx| TextInput::new(window, cx).placeholder("All models")),
        }
    }
    fn query(&self) -> UsageLedgerQuery {
        UsageLedgerQuery {
            project_id: self.project,
            task_id: self.task,
            agent_id: self.agent,
            provider: self.provider,
            environment: self.environment,
            kind: self.kind,
            model: (!self.model.is_empty()).then(|| self.model.clone()),
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
#[derive(Clone, Copy)]
enum Filter {
    Project(Option<Uuid>),
    Task(Option<Uuid>),
    Agent(Option<Uuid>),
    Provider(Option<ProviderKind>),
    Environment(Option<ExecutionEnvironment>),
    Kind(Option<UsageLedgerKind>),
    Range(Option<u64>, &'static str),
}
impl Waku {
    pub(super) fn reset_usage_ledger(&mut self, cx: &mut Context<Self>) {
        let ui = &mut self.control_ui.usage;
        ui.after = None;
        ui.previous.clear();
        ui.page = None;
        ui.rows = Rc::new(vec![]);
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
                        if this.control_ui.page != Some(Page::Usage) {
                            this.control_ui.usage.polling = false;
                            return false;
                        }
                        this.refresh_usage_ledger(cx);
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
        if self.control_ui.page != Some(Page::Usage) || self.control_ui.usage.busy {
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
    pub(super) fn usage_rows(&self) -> Vec<Row> {
        self.control_ui
            .usage
            .rows
            .iter()
            .cloned()
            .map(Row::Usage)
            .collect()
    }
    pub(super) fn usage_toolbar(&self, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        let ui = &self.control_ui.usage;
        let mut bar = div().flex().flex_col().gap(px(12.0));
        let mut filters = div().flex().flex_wrap().gap(px(8.0));
        let project = ui
            .project
            .and_then(|id| self.state.projects.iter().find(|p| p.id == id))
            .map(|p| p.name.clone())
            .unwrap_or_else(|| {
                if ui.project.is_some() {
                    "Selected project".into()
                } else {
                    "All projects".into()
                }
            });
        let agent = ui
            .agent
            .and_then(|id| {
                self.control_ui
                    .snapshot
                    .records
                    .iter()
                    .find_map(|r| match r {
                        ControlRecord::Agent(a) if a.id == id => Some(a.name.clone()),
                        _ => None,
                    })
            })
            .unwrap_or_else(|| {
                if ui.agent.is_some() {
                    "Selected agent".into()
                } else {
                    "All agents".into()
                }
            });
        let labels = [
            project,
            if ui.task.is_some() {
                "Selected task"
            } else {
                "All tasks"
            }
            .into(),
            agent,
            ui.provider
                .map(|p| p.display_name())
                .unwrap_or("All harnesses")
                .into(),
            ui.environment
                .map(|e| {
                    if e == ExecutionEnvironment::Local {
                        "Local"
                    } else {
                        "Nebius"
                    }
                })
                .unwrap_or("All environments")
                .into(),
            ui.kind
                .map(|k| match k {
                    UsageLedgerKind::ModelCall => "Team model calls",
                    UsageLedgerKind::SoloModelCall => "Task model calls",
                    UsageLedgerKind::RemoteCommand => "Remote commands",
                    UsageLedgerKind::GpuJob => "GPU jobs",
                })
                .unwrap_or("All usage")
                .into(),
            ui.range.into(),
        ];
        for (index, label) in labels.into_iter().enumerate() {
            let weak = cx.entity().downgrade();
            let id = format!("usage-filter-{index}");
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
                    let mut options: Vec<(String, Filter, bool)> = vec![];
                    match index {
                        0 => {
                            options.push((
                                "All projects".into(),
                                Filter::Project(None),
                                ui.project.is_none(),
                            ));
                            options.extend(this.state.projects.iter().map(|p| {
                                (
                                    p.name.clone(),
                                    Filter::Project(Some(p.id)),
                                    ui.project == Some(p.id),
                                )
                            }));
                        }
                        1 => {
                            options.push((
                                "All tasks".into(),
                                Filter::Task(None),
                                ui.task.is_none(),
                            ));
                            options.extend(
                                this.state
                                    .sessions
                                    .iter()
                                    .filter(|s| ui.project.is_none_or(|p| p == s.project_id))
                                    .map(|s| {
                                        (
                                            s.display_title().into(),
                                            Filter::Task(Some(s.id)),
                                            ui.task == Some(s.id),
                                        )
                                    }),
                            );
                        }
                        2 => {
                            options.push((
                                "All agents".into(),
                                Filter::Agent(None),
                                ui.agent.is_none(),
                            ));
                            options.extend(this.control_ui.snapshot.records.iter().filter_map(
                                |r| {
                                    if let ControlRecord::Agent(a) = r {
                                        Some((
                                            a.name.clone(),
                                            Filter::Agent(Some(a.id)),
                                            ui.agent == Some(a.id),
                                        ))
                                    } else {
                                        None
                                    }
                                },
                            ));
                        }
                        3 => {
                            options.push((
                                "All harnesses".into(),
                                Filter::Provider(None),
                                ui.provider.is_none(),
                            ));
                            options.extend(ProviderKind::ALL.into_iter().map(|p| {
                                (
                                    p.display_name().into(),
                                    Filter::Provider(Some(p)),
                                    ui.provider == Some(p),
                                )
                            }));
                        }
                        4 => {
                            options.push((
                                "All environments".into(),
                                Filter::Environment(None),
                                ui.environment.is_none(),
                            ));
                            for e in [
                                ExecutionEnvironment::Local,
                                ExecutionEnvironment::NebiusSandbox,
                            ] {
                                options.push((
                                    if e == ExecutionEnvironment::Local {
                                        "Local"
                                    } else {
                                        "Nebius"
                                    }
                                    .into(),
                                    Filter::Environment(Some(e)),
                                    ui.environment == Some(e),
                                ));
                            }
                        }
                        5 => {
                            options.push((
                                "All usage".into(),
                                Filter::Kind(None),
                                ui.kind.is_none(),
                            ));
                            for (k, name) in [
                                (UsageLedgerKind::ModelCall, "Team model calls"),
                                (UsageLedgerKind::SoloModelCall, "Task model calls"),
                                (UsageLedgerKind::RemoteCommand, "Remote commands"),
                                (UsageLedgerKind::GpuJob, "GPU jobs"),
                            ] {
                                options.push((
                                    name.into(),
                                    Filter::Kind(Some(k)),
                                    ui.kind == Some(k),
                                ));
                            }
                        }
                        _ => {
                            for (hours, name) in [
                                (None, "All time"),
                                (Some(24), "Last 24 hours"),
                                (Some(168), "Last 7 days"),
                                (Some(720), "Last 30 days"),
                            ] {
                                let since = hours
                                    .map(|h| crate::model::unix_time().saturating_sub(h * 3600));
                                options.push((
                                    name.into(),
                                    Filter::Range(since, name),
                                    ui.range == name,
                                ));
                            }
                        }
                    }
                    options
                        .into_iter()
                        .map(|(name, filter, selected)| {
                            let weak = weak.clone();
                            MenuItem::new(name, move |_, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    let ui = &mut this.control_ui.usage;
                                    match filter {
                                        Filter::Project(v) => {
                                            ui.project = v;
                                            ui.task = None;
                                        }
                                        Filter::Task(v) => ui.task = v,
                                        Filter::Agent(v) => ui.agent = v,
                                        Filter::Provider(v) => ui.provider = v,
                                        Filter::Environment(v) => ui.environment = v,
                                        Filter::Kind(v) => ui.kind = v,
                                        Filter::Range(v, name) => {
                                            ui.since = v;
                                            ui.range = name;
                                        }
                                    }
                                    this.reset_usage_ledger(cx);
                                });
                            })
                            .selected(selected)
                        })
                        .collect()
                },
            ));
        }
        bar = bar.child(filters).child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(TextField::new("usage-model", ui.model_input.clone()).w(px(260.0)))
                .child(self.control_button(
                    "usage-model-apply",
                    "Apply model",
                    cx,
                    |this, _, cx| {
                        let model = this
                            .control_ui
                            .usage
                            .model_input
                            .read(cx)
                            .content()
                            .trim()
                            .to_owned();
                        if model.len() > 512 {
                            this.control_ui.usage.error =
                                Some("Model name exceeds 512 bytes".into());
                            cx.notify();
                            return;
                        }
                        this.control_ui.usage.model = model;
                        this.reset_usage_ledger(cx);
                    },
                ))
                .child(self.control_button(
                    "usage-provider-history",
                    "Provider history",
                    cx,
                    |this, window, cx| {
                        this.open_settings_action(&OpenSettings, window, cx);
                        this.open_settings_page(SettingsPage::Usage, cx);
                    },
                )),
        );
        if let Some(page) = &ui.page {
            let s = &page.summary;
            bar=bar.child(div().text_size(sp(13.0)).text_color(theme.text).child(format!("{} records · {} model requests · {} input / {} output tokens · {} tokens reserved · {} requests awaiting usage",s.records,s.model_calls,s.input_tokens,s.output_tokens,s.reserved_tokens,s.unknown_calls)));
            let mut costs = s
                .costs
                .iter()
                .map(|c| {
                    format!(
                        "{} {} ({})",
                        money(&c.amount),
                        if c.quality == UsageCostQuality::Estimated {
                            "estimated"
                        } else {
                            "reported"
                        },
                        if c.finality == UsageCostFinality::Final {
                            "final"
                        } else {
                            "provisional"
                        }
                    )
                })
                .collect::<Vec<_>>();
            costs.push(format!("{} costs unavailable", s.unknown_cost_records));
            bar = bar.child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(costs.join(" · ")),
            );
            let mut paging = div().flex().gap(px(8.0));
            if !ui.previous.is_empty() {
                paging = paging.child(self.control_button(
                    "usage-previous",
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
            if let Some(next) = page.next_cursor {
                paging = paging.child(self.control_button(
                    "usage-next",
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
            bar = bar.child(paging);
        }
        if ui.busy {
            bar = bar.child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child("Updating usage…"),
            );
        }
        if let Some(error) = &ui.error {
            bar = bar.child(
                div()
                    .text_size(sp(13.0))
                    .text_color(theme.text)
                    .child(error.clone()),
            );
        }
        bar
    }
    pub(super) fn render_usage_row(&self, row: Rc<UsageRow>, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let task = row.entry.task_id;
        let model = row.entry.model.clone();
        let agent = row.entry.agent_id;
        let mut actions = div().flex().gap(px(8.0)).child(self.control_button(
            format!("usage-task-{}", row.entry.sequence),
            "Open task",
            cx,
            move |this, _, cx| this.open_team_activity_task(task, cx),
        ));
        if let Some(model) = model {
            actions = actions.child(self.control_button(
                format!("usage-model-{}", row.entry.sequence),
                "Filter model",
                cx,
                move |this, _, cx| {
                    this.control_ui.usage.model = model.clone();
                    this.control_ui
                        .usage
                        .model_input
                        .update(cx, |input, cx| input.set_content(model.clone(), cx));
                    this.reset_usage_ledger(cx);
                },
            ));
        }
        if let Some(agent) = agent {
            actions = actions.child(self.control_button(
                format!("usage-agent-{}", row.entry.sequence),
                "Filter agent",
                cx,
                move |this, _, cx| {
                    this.control_ui.usage.agent = Some(agent);
                    this.reset_usage_ledger(cx);
                },
            ));
        }
        div()
            .px(px(24.0))
            .py(px(14.0))
            .border_b_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(sp(14.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child(row.title.clone()),
            )
            .child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(row.detail.clone()),
            )
            .child(
                div()
                    .text_size(sp(13.0))
                    .text_color(theme.text)
                    .child(row.meter.clone()),
            )
            .child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(row.cost.clone()),
            )
            .child(actions)
            .into_any_element()
    }
}
