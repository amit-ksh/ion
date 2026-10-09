//! Cached controls over durable workflow/routine projections.
use super::*;
fn status(status: AutomationStatus) -> &'static str {
    match status {
        AutomationStatus::Running => "Running",
        AutomationStatus::WaitingDecision => "Decision required",
        AutomationStatus::Blocked => "Needs attention",
        AutomationStatus::Completed => "Completed",
        AutomationStatus::Canceled => "Stopped",
        AutomationStatus::Skipped => "Skipped",
    }
}
pub(super) fn next_fire(at: u64) -> String {
    i64::try_from(at)
        .ok()
        .and_then(|at| chrono::DateTime::from_timestamp(at, 0))
        .map(|time| time.format("%b %d, %H:%M UTC").to_string())
        .unwrap_or_else(|| "Unknown time".into())
}
impl Waku {
    pub(super) fn start_control_automation(&mut self, definition: Uuid, cx: &mut Context<Self>) {
        if self.control_ui.busy {
            return;
        }
        let revision = self.control_ui.snapshot.revision;
        let (id, revision) = *self
            .control_ui
            .automation_requests
            .entry(definition)
            .or_insert_with(|| (Uuid::new_v4(), revision));
        self.control_request(
            Rpc::StartAutomation {
                id,
                definition_id: definition,
                expected_definition_revision: revision,
            },
            cx,
        );
    }
    pub(super) fn render_stage_team(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let teams: Vec<_> = self.control_ui.team_index.iter().cloned().collect();
        let selected = self.control_ui.stages[index].3;
        let label = teams
            .iter()
            .find(|t| Some(t.id) == selected)
            .map(|t| format!("Team: {}", t.name))
            .unwrap_or_else(|| "Choose stage team".into());
        let weak = cx.entity().downgrade();
        let menu_id = SharedString::from(format!("stage-team-menu-{index}"));
        let menu = self.menu_handle(menu_id.clone(), cx);
        dropdown_menu(
            self.control_button(format!("stage-team-{index}"), label, cx, |_, _, _| {}),
            menu_id,
            &menu,
            MenuAlign::BelowLeft,
            move |_| {
                teams
                    .iter()
                    .map(|t| {
                        let weak = weak.clone();
                        let id = t.id;
                        MenuItem::new(t.name.clone(), move |_, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                if let Some(stage) = this.control_ui.stages.get_mut(index) {
                                    stage.3 = Some(id);
                                }
                                cx.notify();
                            });
                        })
                        .selected(Some(id) == selected)
                    })
                    .collect()
            },
        )
        .into_any_element()
    }
    pub(super) fn render_stage_documents(
        &self,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.control_ui.stages[index].4.clone();
        let label = if selected.is_empty() {
            "No required documents".to_owned()
        } else {
            format!(
                "Required: {}",
                selected
                    .iter()
                    .map(|f| f.to_uppercase())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        let weak = cx.entity().downgrade();
        let menu_id = SharedString::from(format!("stage-documents-menu-{index}"));
        let menu = self.menu_handle(menu_id.clone(), cx);
        dropdown_menu(
            self.control_button(format!("stage-documents-{index}"), label, cx, |_, _, _| {}),
            menu_id,
            &menu,
            MenuAlign::BelowLeft,
            move |_| {
                WORKFLOW_DOCUMENT_FORMATS
                    .iter()
                    .map(|format| {
                        let weak = weak.clone();
                        let format = *format;
                        MenuItem::new(
                            match format {
                                "md" => "Markdown",
                                "txt" => "Text",
                                "html" => "HTML",
                                "csv" => "CSV",
                                _ => "JSON",
                            },
                            move |_, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    if let Some(stage) = this.control_ui.stages.get_mut(index) {
                                        if stage.4.iter().any(|f| f == format) {
                                            stage.4.retain(|f| f != format);
                                        } else {
                                            stage.4.push(format.to_owned());
                                        }
                                        stage.4.sort();
                                    }
                                    cx.notify();
                                });
                            },
                        )
                        .selected(selected.iter().any(|f| f == format))
                    })
                    .collect()
            },
        )
        .into_any_element()
    }
    pub(super) fn render_routine_target(&self, cx: &mut Context<Self>) -> AnyElement {
        let selected = self
            .control_ui
            .routine_team
            .or(self.control_ui.routine_workflow);
        let targets: Vec<_> = self
            .control_ui
            .snapshot
            .records
            .iter()
            .filter_map(|r| match r {
                ControlRecord::Team(t) => Some((t.id, format!("Team: {}", t.name), false)),
                ControlRecord::Workflow(w) if Some(w.project_id) == self.control_ui.project => {
                    Some((w.id, format!("Workflow: {}", w.name), true))
                }
                _ => None,
            })
            .collect();
        let label = targets
            .iter()
            .find(|t| Some(t.0) == selected)
            .map(|t| t.1.clone())
            .unwrap_or_else(|| "Choose team or workflow".into());
        let weak = cx.entity().downgrade();
        let menu = self.menu_handle("routine-target-menu", cx);
        dropdown_menu(
            self.control_button("routine-target", label, cx, |_, _, _| {}),
            "routine-target-menu",
            &menu,
            MenuAlign::BelowLeft,
            move |_| {
                targets
                    .iter()
                    .map(|(id, label, workflow)| {
                        let weak = weak.clone();
                        let id = *id;
                        let workflow = *workflow;
                        MenuItem::new(label.clone(), move |_, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.control_ui.routine_team = (!workflow).then_some(id);
                                this.control_ui.routine_workflow = workflow.then_some(id);
                                cx.notify();
                            });
                        })
                        .selected(Some(id) == selected)
                    })
                    .collect()
            },
        )
        .into_any_element()
    }
    pub(super) fn automation_rows(&self) -> Vec<Row> {
        if let Some(run) = self
            .control_ui
            .automation_detail
            .as_ref()
            .filter(|r| Some(r.id) == self.control_ui.automation_selected)
        {
            let mut rows = vec![Row::Automation(run.clone())];
            rows.extend((0..run.stages.len()).map(|i| Row::AutomationStage(run.clone(), i)));
            return rows;
        }
        let rows: Vec<_> = self
            .control_ui
            .automation_runs
            .iter()
            .filter(|r| match self.control_ui.page {
                Some(Page::Routines) => r.routine_id.is_some(),
                Some(Page::Workflows) => r.workflow_id.is_some(),
                _ => true,
            })
            .cloned()
            .map(|r| Row::Automation(Rc::new(r)))
            .collect();
        if rows.is_empty() {
            vec![Row::Empty("No runs yet.")]
        } else {
            rows
        }
    }
    pub(super) fn render_automation_header(&self, cx: &mut Context<Self>) -> Div {
        let mut header = div().flex().gap(px(8.0)).px(px(24.0)).py(px(10.0));
        if self.control_ui.automation_selected.is_some() {
            return header.child(self.control_button(
                "automation-back",
                "All runs",
                cx,
                |this, _, cx| {
                    this.control_ui.automation_selected = None;
                    this.control_ui.automation_detail = None;
                    this.control_ui.activity_revision += 1;
                    this.refresh_team_activity(true, cx);
                    cx.notify();
                },
            ));
        }
        if !self.control_ui.automation_previous.is_empty() {
            header = header.child(self.control_button(
                "automation-previous",
                "Previous",
                cx,
                |this, _, cx| {
                    if let Some(after) = this.control_ui.automation_previous.pop() {
                        this.control_ui.automation_after = after;
                        this.refresh_team_activity(true, cx);
                    }
                },
            ));
        }
        if let Some(cursor) = self.control_ui.automation_cursor {
            header = header.child(self.control_button(
                "automation-next",
                "Next",
                cx,
                move |this, _, cx| {
                    this.control_ui
                        .automation_previous
                        .push(this.control_ui.automation_after);
                    this.control_ui.automation_after = Some(cursor);
                    this.refresh_team_activity(true, cx);
                },
            ));
        }
        header
    }
    pub(super) fn open_control_automation(
        &mut self,
        run: Rc<AutomationRun>,
        cx: &mut Context<Self>,
    ) {
        let page = if run.routine_id.is_some() {
            Page::Routines
        } else {
            Page::Workflows
        };
        if self.control_ui.page != Some(page) {
            self.open_workspace_page(page, cx);
        }
        self.control_ui.automation_history = true;
        self.control_ui.automation_selected = Some(run.id);
        self.control_ui.automation_detail = Some(run);
        self.control_ui.activity_revision += 1;
        self.refresh_team_activity(true, cx);
        cx.notify();
    }
    pub(super) fn render_automation_row(
        &self,
        run: Rc<AutomationRun>,
        stage: Option<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        let id = run.id;
        let revision = run.revision;
        let mut actions = div().flex().items_center().gap(px(8.0));
        let (title, detail, icon_path) = if let Some(index) = stage {
            let stage = &run.stages[index];
            let label = if stage.skipped {
                "Skipped by workflow route"
            } else if stage.completed {
                "Completed"
            } else if stage.started {
                "Started"
            } else if usize::from(run.current_stage) == index
                && run.status == AutomationStatus::WaitingDecision
            {
                "Awaiting your decision"
            } else {
                "Not started"
            };
            if stage.started {
                let task = stage.task_id;
                actions = actions.child(self.control_button(
                    format!("automation-task-{id}-{index}"),
                    "Open task",
                    cx,
                    move |this, _, cx| {
                        this.open_team_activity_task(task, cx);
                    },
                ));
            }
            let team = self
                .control_ui
                .snapshot
                .records
                .iter()
                .find_map(|r| match r {
                    ControlRecord::Team(t) if t.id == stage.team_id => Some(t.name.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| stage.team_id.to_string());
            (
                format!("{}. {}", index + 1, stage.title),
                format!(
                    "{label} · {team}{}\n{}\n{}{}",
                    if stage.required_formats.is_empty() {
                        String::new()
                    } else {
                        format!(
                            " · Required: {}",
                            stage.required_formats.join(", ").to_uppercase()
                        )
                    },
                    format!(
                        "{}{}",
                        stage.instructions,
                        stage
                            .route
                            .as_ref()
                            .and_then(|r| r.artifact_id.zip(r.sha256.as_ref()))
                            .map(|(id, hash)| format!("\nDocument {id} · SHA-256 {hash}"))
                            .unwrap_or_default()
                    ),
                    format!(
                        "{} · {}",
                        workflow_ui::transition_label(&stage.transition),
                        workflow_ui::retry_label(stage.retry)
                    ),
                    stage
                        .route
                        .as_ref()
                        .map(|r| format!(
                            " · Chosen: {}{}",
                            workflow_ui::target_label(r.target),
                            r.matched
                                .map(|m| format!(
                                    " ({})",
                                    if m { "matched" } else { "did not match" }
                                ))
                                .unwrap_or_default()
                        ))
                        .unwrap_or_default()
                ),
                "icons/git-branch.svg",
            )
        } else {
            if self.control_ui.automation_selected != Some(id) {
                let value = run.clone();
                actions = actions.child(self.control_button(
                    format!("automation-open-{id}"),
                    "Open",
                    cx,
                    move |this, _, cx| this.open_control_automation(value.clone(), cx),
                ));
            }
            if run.status == AutomationStatus::WaitingDecision
                && self.control_ui.automation_selected == Some(id)
            {
                let current = run.current_stage;
                actions = actions
                    .child(self.control_button(
                        format!("automation-approve-{id}"),
                        "Approve stage",
                        cx,
                        move |this, _, cx| {
                            this.control_request(
                                Rpc::ResolveAutomationGate {
                                    id,
                                    expected_revision: revision,
                                    stage: current,
                                    approved: true,
                                },
                                cx,
                            )
                        },
                    ))
                    .child(self.control_button(
                        format!("automation-decline-{id}"),
                        "Decline",
                        cx,
                        move |this, _, cx| {
                            this.control_request(
                                Rpc::ResolveAutomationGate {
                                    id,
                                    expected_revision: revision,
                                    stage: current,
                                    approved: false,
                                },
                                cx,
                            )
                        },
                    ));
            } else if matches!(
                run.status,
                AutomationStatus::Running | AutomationStatus::Blocked
            ) {
                actions = actions.child(self.control_button(
                    format!("automation-stop-{id}"),
                    "Stop",
                    cx,
                    move |this, _, cx| {
                        this.control_request(
                            Rpc::StopAutomation {
                                id,
                                expected_revision: revision,
                            },
                            cx,
                        )
                    },
                ));
            }
            (
                run.name.clone(),
                format!(
                    "{} · {} of {} stages{}{}",
                    status(run.status),
                    run.stages.iter().filter(|s| s.completed).count(),
                    run.stages.len(),
                    if self.control_ui.automation_selected == Some(id) {
                        run.trigger
                            .as_ref()
                            .map(|t| format!("\n{}", routine_ui::trigger_label(t)))
                            .unwrap_or_default()
                    } else {
                        String::new()
                    },
                    run.reason
                        .as_ref()
                        .map(|r| format!(" · {r}"))
                        .unwrap_or_default()
                ),
                "icons/rotate-cw.svg",
            )
        };
        div()
            .id(SharedString::from(format!("automation-row-{id}-{stage:?}")))
            .flex()
            .items_center()
            .gap(px(14.0))
            .px(px(24.0))
            .py(px(14.0))
            .border_b_1()
            .border_color(theme.border)
            .child(icon(icon_path, 18.0, theme.accent))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(5.0))
                    .child(
                        div()
                            .text_size(sp(14.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(sp(12.5))
                            .text_color(theme.text_secondary)
                            .child(detail),
                    ),
            )
            .child(actions)
            .into_any_element()
    }
}
