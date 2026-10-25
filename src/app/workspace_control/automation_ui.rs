//! Cached controls over durable workflow/routine projections.
use super::*;

fn document_format_label(format: &str) -> String {
    match format {
        "md" => "Markdown".into(),
        "txt" => "Text".into(),
        "yml" => "YAML (.yml)".into(),
        _ => format.to_uppercase(),
    }
}
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
/// The run's state as an icon and color, the way Paperclip leads each run row.
fn status_glyph(status: AutomationStatus, theme: &Theme) -> (&'static str, Hsla) {
    match status {
        AutomationStatus::Running => ("icons/loader-circle.svg", theme.accent),
        AutomationStatus::WaitingDecision => ("icons/shield-check.svg", theme.accent),
        AutomationStatus::Blocked => ("icons/alert.svg", theme.danger),
        AutomationStatus::Completed => ("icons/check.svg", theme.success),
        AutomationStatus::Canceled => ("icons/stop.svg", theme.text_tertiary),
        AutomationStatus::Skipped => ("icons/arrow-right.svg", theme.text_tertiary),
    }
}
fn ago(at: u64) -> String {
    let seconds = unix_time().saturating_sub(at);
    if seconds < 60 {
        super::super::sidebar::format_time_ago(seconds)
    } else {
        format!("{} ago", super::super::sidebar::format_time_ago(seconds))
    }
}
/// Daemon reasons can end in a raw Unix time ("Stopped by user at
/// 1792396687"); say how long ago instead.
fn readable_reason(reason: &str) -> String {
    reason
        .rsplit_once(" at ")
        .and_then(|(head, tail)| {
            tail.trim()
                .parse::<u64>()
                .ok()
                .filter(|at| *at > 1_000_000_000)
                .map(|at| format!("{head} {}", ago(at)))
        })
        .unwrap_or_else(|| reason.to_owned())
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
            form_select(format!("stage-team-{index}"), label, &Theme::workspace(cx)),
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
            form_select(
                format!("stage-documents-{index}"),
                label,
                &Theme::workspace(cx),
            ),
            menu_id,
            &menu,
            MenuAlign::BelowLeft,
            move |_| {
                WORKFLOW_DOCUMENT_FORMATS
                    .iter()
                    .map(|format| {
                        let weak = weak.clone();
                        let format = *format;
                        MenuItem::new(document_format_label(format), move |_, cx| {
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
                        })
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
            form_select("routine-target", label, &Theme::workspace(cx)),
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
        let mut header = div().flex().gap(px(8.0));
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
        let theme = Theme::workspace(cx);
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
                        "{} · {}{}",
                        workflow_ui::transition_label(&stage.transition),
                        workflow_ui::retry_label(stage.retry),
                        if stage.inherit_workspace {
                            stage
                                .workspace_from
                                .and_then(|id| run.stages.iter().position(|s| s.team_run_id == id))
                                .map(|source| format!(" · Code from stage {}", source + 1))
                                .unwrap_or_else(|| " · Inherit code: on".into())
                        } else {
                            String::new()
                        }
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
                actions = actions.child(self.control_button_tone(
                    format!("automation-open-{id}"),
                    "Open",
                    ControlTone::Ghost,
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
                actions = actions.child(self.control_button_tone(
                    format!("automation-stop-{id}"),
                    "Stop",
                    ControlTone::Ghost,
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
            let done = run.stages.iter().filter(|s| s.completed).count();
            return self.render_run_summary(&run, done, actions, &theme);
        };
        div()
            .id(SharedString::from(format!("automation-row-{id}-{stage:?}")))
            .mx(px(2.0))
            .mb(px(8.0))
            .px(px(16.0))
            .py(px(12.0))
            .rounded(px(12.0))
            .border_1()
            .border_color(theme.ticket_border)
            .bg(theme.ticket)
            .flex()
            .items_center()
            .gap(px(14.0))
            .child(
                avatar_tile(32.0, theme.accent_soft, gpui::transparent_black()).child(icon(
                    icon_path,
                    15.0,
                    theme.accent,
                )),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(5.0))
                    .child(
                        div()
                            .text_size(sp(13.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(sp(12.5))
                            .line_height(sp(18.0))
                            .text_color(theme.text_secondary)
                            .child(detail),
                    ),
            )
            .child(actions)
            .into_any_element()
    }

    /// One run in the history: state glyph, name and short id, a stage
    /// progress meter, when it started and why it ended, then its actions.
    fn render_run_summary(
        &self,
        run: &AutomationRun,
        done: usize,
        actions: Div,
        theme: &Theme,
    ) -> AnyElement {
        let theme = *theme;
        let id = run.id;
        let (glyph, tone) = status_glyph(run.status, &theme);
        let total = run.stages.len().max(1);
        let selected = self.control_ui.automation_selected == Some(id);
        let mut facts = vec![
            format!("{done} of {} stages", run.stages.len()),
            format!("started {}", ago(run.created_at)),
        ];
        if selected {
            if let Some(trigger) = run.trigger.as_ref() {
                facts.push(routine_ui::trigger_label(trigger));
            }
        }
        if let Some(reason) = run.reason.as_ref() {
            facts.push(readable_reason(reason));
        }
        let short_id = format!("#{}", &id.simple().to_string()[..6].to_uppercase());
        let pill_bg = match run.status {
            AutomationStatus::Blocked => theme.danger_soft,
            AutomationStatus::Running | AutomationStatus::WaitingDecision => theme.accent_soft,
            _ => theme.overlay,
        };
        let pill_fg = match run.status {
            AutomationStatus::Blocked => theme.danger,
            AutomationStatus::Running | AutomationStatus::WaitingDecision => theme.accent,
            _ => theme.text_secondary,
        };
        div()
            .id(SharedString::from(format!("automation-row-{id}")))
            .mx(px(2.0))
            .mb(px(8.0))
            .px(px(16.0))
            .py(px(12.0))
            .rounded(px(12.0))
            .border_1()
            .border_color(theme.ticket_border)
            .bg(theme.ticket)
            .hover(move |style| style.border_color(theme.border_strong))
            .flex()
            .items_center()
            .gap(px(14.0))
            .child(
                div()
                    .flex_none()
                    .size(px(32.0))
                    .rounded_full()
                    .bg(theme.inset)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(glyph, 15.0, tone)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(5.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .min_w_0()
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(sp(13.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text)
                                    .child(run.name.clone()),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .font_family(crate::md::render::MONO_FAMILY)
                                    .text_size(sp(11.0))
                                    .text_color(theme.text_tertiary)
                                    .child(short_id),
                            )
                            .child(status_pill(glyph, status(run.status), pill_fg, pill_bg)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .min_w_0()
                            .child(
                                div()
                                    .flex_none()
                                    .w(px(64.0))
                                    .h(px(4.0))
                                    .rounded_full()
                                    .bg(theme.overlay_strong)
                                    .child(
                                        div()
                                            .h_full()
                                            .rounded_full()
                                            .bg(if run.status == AutomationStatus::Completed {
                                                theme.success
                                            } else {
                                                theme.primary
                                            })
                                            .w(px(64.0 * done.min(total) as f32 / total as f32)),
                                    ),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(sp(12.0))
                                    .text_color(theme.text_secondary)
                                    .child(facts.join(" · ")),
                            ),
                    ),
            )
            .child(actions)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_supported_document_format_has_a_distinct_menu_label() {
        let labels: Vec<_> = WORKFLOW_DOCUMENT_FORMATS
            .iter()
            .map(|format| document_format_label(format))
            .collect();
        assert_eq!(
            labels
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            labels.len()
        );
        for format in ["pdf", "svg", "xml", "yaml", "docx", "xlsx", "pptx", "png"] {
            assert_eq!(document_format_label(format), format.to_uppercase());
        }
    }
}
