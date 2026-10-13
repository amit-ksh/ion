//! Remote inventory is loaded once per user action; render reads cached rows only.
use super::*;
use waku_client::remote_control::*;
pub(super) struct RemoteUi {
    pub open: bool,
    busy: bool,
    generation: u64,
    error: Option<String>,
    project: Option<Uuid>,
    kind: Option<RemoteExecutionKind>,
    unresolved: bool,
    after: Option<RemoteExecutionCursor>,
    previous: Vec<Option<RemoteExecutionCursor>>,
    next: Option<RemoteExecutionCursor>,
    rows: Rc<Vec<Rc<RemoteRow>>>,
    list: ListState,
    focus: FocusHandle,
    selected: usize,
}
struct RemoteRow {
    entry: RemoteExecution,
    title: String,
    detail: String,
    handle: String,
}
impl RemoteUi {
    pub fn new(cx: &mut App) -> Self {
        Self {
            open: false,
            busy: false,
            generation: 0,
            error: None,
            project: None,
            kind: None,
            unresolved: true,
            after: None,
            previous: vec![],
            next: None,
            rows: Rc::new(vec![]),
            list: ListState::new(0, ListAlignment::Top, px(145.0)),
            focus: cx.focus_handle(),
            selected: 0,
        }
    }
    pub fn close(&mut self) {
        self.open = false;
        self.busy = false;
        self.generation += 1;
    }
    fn query(&self) -> RemoteExecutionQuery {
        RemoteExecutionQuery {
            project_id: self.project,
            kind: self.kind,
            unresolved_only: self.unresolved,
            after: self.after.clone(),
            limit: 32,
        }
    }
}
fn label(kind: RemoteExecutionKind) -> &'static str {
    match kind {
        RemoteExecutionKind::Model => "Sandbox model call",
        RemoteExecutionKind::Command => "Sandbox command",
        RemoteExecutionKind::Gpu => "GPU job",
    }
}
impl Waku {
    fn reset_remote_executions(&mut self, cx: &mut Context<Self>) {
        let ui = &mut self.control_ui.remote;
        ui.generation += 1;
        ui.busy = false;
        ui.after = None;
        ui.previous.clear();
        ui.next = None;
        ui.rows = Rc::new(vec![]);
        ui.list.reset(0);
        self.refresh_remote_executions(cx);
    }
    pub(super) fn open_remote_executions(&mut self, cx: &mut Context<Self>) {
        self.control_ui.remote.open = true;
        self.reset_remote_executions(cx);
    }
    fn refresh_remote_executions(&mut self, cx: &mut Context<Self>) {
        let ui = &mut self.control_ui.remote;
        if !ui.open || ui.busy || self.control_ui.page != Some(Page::Usage) {
            return;
        }
        ui.busy = true;
        ui.generation += 1;
        ui.error = None;
        let generation = ui.generation;
        let query = ui.query();
        let requested = query.clone();
        let client = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || -> anyhow::Result<RemoteExecutionPage> {
                        let ResponsePayload::RemoteExecutions { page } = client.request(
                            Uuid::nil(),
                            Uuid::nil(),
                            Rpc::ListRemoteExecutions { query },
                        )?
                        else {
                            anyhow::bail!("Unexpected remote execution response");
                        };
                        Ok(page)
                    })
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let ui = &mut this.control_ui.remote;
                if ui.generation != generation
                    || !ui.open
                    || ui.query() != requested
                    || this.control_ui.page != Some(Page::Usage)
                {
                    return;
                }
                ui.busy = false;
                match result {
                    Ok(page) => {
                        ui.next = page.next_cursor;
                        ui.rows = Rc::new(
                            page.entries
                                .into_iter()
                                .map(|entry| {
                                    let title =
                                        format!("{} · {}", label(entry.kind), entry.objective);
                                    let detail = format!(
                                        "{} · {} · {}",
                                        entry.team_name,
                                        entry.state,
                                        if entry.settled {
                                            "Terminal observation recorded"
                                        } else if entry.stop_requested {
                                            "Stop requested · awaiting terminal receipt"
                                        } else {
                                            "Unresolved"
                                        }
                                    );
                                    let mut handle = format!(
                                        "{} · Request {}",
                                        entry.remote_project, entry.request_id
                                    );
                                    if let Some(id) = &entry.operation_id {
                                        handle.push_str(&format!(" · Operation {id}"));
                                    } else if !entry.settled {
                                        handle.push_str(" · Operation handle unavailable");
                                    }
                                    if let Some(id) = &entry.job_id {
                                        handle.push_str(&format!(" · Job {id}"));
                                    }
                                    if let Some(id) = &entry.cancellation_operation_id {
                                        handle.push_str(&format!(" · Cancellation operation {id}"));
                                    }
                                    if let Some(state) = &entry.cleanup_state {
                                        handle.push_str(&format!(" · Cleanup {state}"));
                                    }
                                    Rc::new(RemoteRow {
                                        entry,
                                        title,
                                        detail,
                                        handle,
                                    })
                                })
                                .collect(),
                        );
                        ui.selected = 0;
                        ui.list.reset(ui.rows.len());
                    }
                    Err(error) => {
                        ui.error = Some(format!("Could not load remote executions: {error}"));
                        ui.rows = Rc::new(vec![]);
                        ui.next = None;
                        ui.list.reset(0);
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn remote_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let ui = &mut self.control_ui.remote;
        if event.keystroke.key == "escape" {
            ui.close();
            cx.stop_propagation();
            cx.notify();
            return;
        }
        if ui.busy || ui.rows.is_empty() {
            return;
        }
        let count = ui.rows.len();
        let index = match event.keystroke.key.as_str() {
            "down" => (ui.selected + 1).min(count - 1),
            "up" => ui.selected.saturating_sub(1),
            "home" => 0,
            "end" => count - 1,
            "pagedown" => (ui.selected + 4).min(count - 1),
            "pageup" => ui.selected.saturating_sub(4),
            "enter" | "space" => {
                let run = ui.rows[ui.selected].entry.run_id;
                self.open_workspace_run(run, cx);
                cx.stop_propagation();
                return;
            }
            _ => return,
        };
        ui.selected = index;
        ui.list.scroll_to_reveal_item(index);
        cx.stop_propagation();
        cx.notify();
    }
    fn render_remote_row(
        &self,
        row: Rc<RemoteRow>,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        let run = row.entry.run_id;
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .p(px(14.0))
            .border_b_1()
            .border_color(theme.border)
            .when(self.control_ui.remote.selected == index, |body| {
                body.bg(theme.overlay)
            })
            .child(
                div()
                    .text_size(sp(14.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text)
                    .child(row.title.clone()),
            )
            .child(
                div()
                    .text_size(sp(13.0))
                    .text_color(theme.text)
                    .child(row.detail.clone()),
            )
            .child(
                div()
                    .text_size(sp(12.0))
                    .text_color(theme.text_secondary)
                    .child(row.handle.clone()),
            )
            .child(self.control_button(
                format!("remote-run-{index}"),
                "Open run controls",
                cx,
                move |this, _, cx| this.open_workspace_run(run, cx),
            ))
            .when(row.entry.kind == RemoteExecutionKind::Gpu, |body| {
                let request = row.entry.request_id;
                body.child(self.control_button(
                    format!("collector-{request}"),
                    "Collector storage",
                    cx,
                    move |this, _, cx| this.open_gpu_collector(request, cx),
                ))
            })
            .into_any_element()
    }
    pub(super) fn render_remote_executions(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let ui = &self.control_ui.remote;
        let mut bar = div()
            .flex()
            .flex_wrap()
            .gap(px(8.0))
            .child(
                self.control_button("remote-usage", "Usage", cx, |this, _, cx| {
                    this.control_ui.remote.close();
                    cx.notify();
                }),
            )
            .child(
                self.control_button("remote-refresh", "", cx, |this, _, cx| {
                    this.refresh_remote_executions(cx)
                })
                .tooltip(Tooltip::text("Refresh remote execution records"))
                .child(icon("icons/rotate-cw.svg", 16.0, theme.text_secondary)),
            );
        let weak = cx.entity().downgrade();
        let menu = self.menu_handle("remote-project", cx);
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
        bar = bar.child(dropdown_menu(
            self.control_button("remote-project", project, cx, |_, _, _| {})
                .child(icon("icons/chevron-down.svg", 12.0, theme.text_secondary)),
            "remote-project",
            &menu,
            MenuAlign::BelowLeft,
            move |cx| {
                let Some(entity) = weak.upgrade() else {
                    return vec![];
                };
                let this = entity.read(cx);
                let mut options = vec![("All projects".to_string(), None)];
                options.extend(
                    this.state
                        .projects
                        .iter()
                        .map(|p| (p.name.clone(), Some(p.id))),
                );
                options
                    .into_iter()
                    .map(|(name, project)| {
                        let weak = weak.clone();
                        MenuItem::new(name, move |_, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.control_ui.remote.project = project;
                                this.reset_remote_executions(cx);
                            });
                        })
                        .selected(this.control_ui.remote.project == project)
                        .into()
                    })
                    .collect()
            },
        ));
        for (id, name, kind) in [
            ("all", "All types", None),
            ("model", "Model calls", Some(RemoteExecutionKind::Model)),
            ("command", "Commands", Some(RemoteExecutionKind::Command)),
            ("gpu", "GPU jobs", Some(RemoteExecutionKind::Gpu)),
        ] {
            bar = bar.child(
                self.control_button(format!("remote-kind-{id}"), name, cx, move |this, _, cx| {
                    this.control_ui.remote.kind = kind;
                    this.reset_remote_executions(cx);
                })
                .when(ui.kind == kind, |b| b.bg(theme.overlay)),
            );
        }
        bar = bar.child(self.control_button(
            "remote-state",
            if ui.unresolved {
                "Unresolved"
            } else {
                "All records"
            },
            cx,
            |this, _, cx| {
                this.control_ui.remote.unresolved = !this.control_ui.remote.unresolved;
                this.reset_remote_executions(cx);
            },
        ));
        let mut body=div().track_focus(&self.control_ui.focus).tab_group().flex_1().min_h_0().flex().flex_col().gap(px(12.0)).p(px(24.0)).child(bar)
   .child(div().text_size(sp(18.0)).text_color(theme.text).child("Remote executions"))
   .child(div().text_size(sp(13.0)).text_color(theme.text_secondary).child("Saved sandbox and GPU records. Open a run to stop work or verify its original operation. Terminal receipts do not settle billing."));
        if ui.busy {
            body = body.child(
                div()
                    .text_color(theme.text_secondary)
                    .child("Loading remote records…"),
            );
        }
        if let Some(error) = &ui.error {
            body = body.child(div().text_color(theme.text).child(error.clone()));
        }
        if ui.rows.is_empty() && !ui.busy && ui.error.is_none() {
            body = body.child(
                div()
                    .text_color(theme.text_secondary)
                    .child("No remote executions match these filters."),
            );
        }
        let weak = cx.entity().downgrade();
        let rows = ui.rows.clone();
        body = body.child(
            div()
                .id("remote-list")
                .track_focus(&ui.focus)
                .tab_index(0)
                .tab_stop(true)
                .focus_visible(|style| style.border_1().border_color(theme.accent))
                .on_key_down(cx.listener(|this, event, _, cx| this.remote_key(event, cx)))
                .flex_1()
                .min_h_0()
                .child(
                    list(ui.list.clone(), move |index, _, cx| {
                        weak.upgrade()
                            .map(|entity| {
                                entity.update(cx, |this, cx| {
                                    rows.get(index)
                                        .cloned()
                                        .map(|row| this.render_remote_row(row, index, cx))
                                        .unwrap_or_else(|| div().into_any_element())
                                })
                            })
                            .unwrap_or_else(|| div().into_any_element())
                    })
                    .size_full(),
                ),
        );
        let mut paging = div().flex().gap(px(8.0));
        if !ui.previous.is_empty() {
            paging = paging.child(self.control_button(
                "remote-previous",
                "Previous",
                cx,
                |this, _, cx| {
                    let ui = &mut this.control_ui.remote;
                    if ui.busy {
                        return;
                    }
                    if let Some(after) = ui.previous.pop() {
                        ui.after = after;
                        this.refresh_remote_executions(cx);
                    }
                },
            ));
        }
        if let Some(next) = &ui.next {
            let next = next.clone();
            paging =
                paging.child(
                    self.control_button("remote-next", "Next", cx, move |this, _, cx| {
                        let ui = &mut this.control_ui.remote;
                        if ui.busy {
                            return;
                        }
                        ui.previous.push(ui.after.clone());
                        ui.after = Some(next.clone());
                        this.refresh_remote_executions(cx);
                    }),
                );
        }
        body.child(paging).into_any_element()
    }
}
