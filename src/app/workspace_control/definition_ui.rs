//! Workflow and routine definitions in Paperclip's shape: a workflow is a
//! board over its stage pipeline, a routine is a row you can run or switch off.
use super::*;

impl Waku {
    fn definition_team_name(&self, id: Option<Uuid>) -> Option<String> {
        let id = id?;
        self.control_ui.snapshot.records.iter().find_map(|r| match r {
            ControlRecord::Team(team) if team.id == id => Some(team.name.clone()),
            _ => None,
        })
    }

    fn run_definition_button(&self, id: Uuid, cx: &mut Context<Self>) -> Stateful<Div> {
        let retry = self.control_ui.automation_requests.contains_key(&id);
        self.control_button_icon(
            format!("automation-start-{id}"),
            "icons/play.svg",
            if retry { "Retry run" } else { "Run now" },
            ControlTone::Secondary,
            cx,
            move |this, _, cx| this.start_control_automation(id, cx),
        )
    }

    fn definition_history_button(&self, id: Uuid, cx: &mut Context<Self>) -> Stateful<Div> {
        self.control_button_tone(
            format!("automation-history-{id}"),
            "Runs",
            ControlTone::Ghost,
            cx,
            move |this, _, cx| {
                this.control_ui.automation_history = true;
                this.control_ui.automation_selected = None;
                this.control_ui.automation_definition = Some(id);
                this.control_ui.automation_after = None;
                this.control_ui.automation_previous.clear();
                this.refresh_team_activity(true, cx);
                cx.notify();
            },
        )
    }

    fn definition_edit_button(&self, record: ControlRecord, cx: &mut Context<Self>) -> Stateful<Div> {
        self.control_button_tone(
            format!("record-edit-{}", record.id()),
            "Edit",
            ControlTone::Ghost,
            cx,
            move |this, window, cx| {
                this.edit_control_record(record.clone(), cx);
                this.control_ui.name.read(cx).focus().focus(window, cx);
            },
        )
    }

    /// A workflow as a board: identity and actions over its stages, drawn
    /// left to right as a pipeline so the order and review gates read at a glance.
    pub(super) fn render_workflow_board(&self, workflow: &Workflow, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::workspace(cx);
        let id = workflow.id;
        let gates = workflow
            .stages
            .iter()
            .filter(|stage| stage.requires_decision)
            .count();
        let summary = format!(
            "{} stage{}{}",
            workflow.stages.len(),
            if workflow.stages.len() == 1 { "" } else { "s" },
            match gates {
                0 => String::new(),
                1 => " · 1 review gate".to_owned(),
                n => format!(" · {n} review gates"),
            }
        );
        let header = div()
            .px(px(18.0))
            .py(px(14.0))
            .flex()
            .items_center()
            .gap(px(14.0))
            .child(
                avatar_tile(38.0, theme.accent_soft, gpui::transparent_black())
                    .child(icon("icons/git-branch.svg", 17.0, theme.accent)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .min_w_0()
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(sp(15.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text)
                                    .child(workflow.name.clone()),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .font_family(crate::md::render::MONO_FAMILY)
                                    .text_size(sp(11.5))
                                    .text_color(theme.text_tertiary)
                                    .child(summary),
                            ),
                    )
                    .when(!workflow.prompt.trim().is_empty(), |column| {
                        column.child(
                            div()
                                .truncate()
                                .text_size(sp(12.5))
                                .text_color(theme.text_secondary)
                                .child(workflow.prompt.clone()),
                        )
                    }),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(self.run_definition_button(id, cx))
                    .child(self.definition_history_button(id, cx))
                    .child(self.definition_edit_button(ControlRecord::Workflow(workflow.clone()), cx)),
            );
        let mut pipeline = div().flex().items_center().flex_none();
        for (index, stage) in workflow.stages.iter().enumerate() {
            if index > 0 {
                pipeline = pipeline.child(
                    div()
                        .flex_none()
                        .flex()
                        .items_center()
                        .child(div().w(px(18.0)).h(px(1.5)).bg(theme.border_strong))
                        .child(icon("icons/chevron-right.svg", 12.0, theme.text_tertiary)),
                );
            }
            let team = self
                .definition_team_name(stage.team_id)
                .unwrap_or_else(|| "No team yet".to_owned());
            pipeline = pipeline.child(
                div()
                    .flex_none()
                    .w(px(260.0))
                    .h(px(64.0))
                    .px(px(12.0))
                    .rounded(px(12.0))
                    .border_1()
                    .border_color(if stage.requires_decision {
                        theme.primary
                    } else {
                        theme.ticket_border
                    })
                    .bg(theme.ticket)
                    .shadow(crate::ui::elevation(&theme, crate::ui::Elevation::Rest))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        div()
                            .flex_none()
                            .size(px(26.0))
                            .rounded_full()
                            .bg(theme.inset)
                            .flex()
                            .items_center()
                            .justify_center()
                            .font_family(crate::md::render::MONO_FAMILY)
                            .text_size(sp(11.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_secondary)
                            .child((index + 1).to_string()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(3.0))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(sp(12.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text)
                                    .child(stage.title.clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    .min_w_0()
                                    .when(stage.requires_decision, |line| {
                                        line.child(
                                            div()
                                                .flex_none()
                                                .flex()
                                                .items_center()
                                                .gap(px(3.0))
                                                .text_size(sp(10.5))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(theme.accent)
                                                .child(icon(
                                                    "icons/shield-check.svg",
                                                    11.0,
                                                    theme.accent,
                                                ))
                                                .child("Review"),
                                        )
                                    })
                                    .child(
                                        div()
                                            .min_w_0()
                                            .truncate()
                                            .text_size(sp(11.0))
                                            .text_color(theme.text_tertiary)
                                            .child(team),
                                    ),
                            ),
                    ),
            );
        }
        div()
            .id(SharedString::from(format!("workflow-board-{id}")))
            .mx(px(2.0))
            .mb(px(14.0))
            .rounded(px(16.0))
            .border_1()
            .border_color(theme.ticket_border)
            .bg(theme.ticket)
            .shadow(crate::ui::elevation(&theme, crate::ui::Elevation::Rest))
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(header)
            .child(
                div()
                    .id(SharedString::from(format!("workflow-pipeline-{id}")))
                    .border_t_1()
                    .border_color(theme.border)
                    .bg(theme.canvas)
                    .px(px(18.0))
                    .py(px(16.0))
                    .overflow_x_scroll()
                    .child(pipeline),
            )
            .into_any_element()
    }

    /// A routine row after Paperclip's: what it is and when it fires, then
    /// run it now, switch its schedule on or off, see its runs, or edit it.
    pub(super) fn render_routine_row(&self, routine: &Routine, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::workspace(cx);
        let id = routine.id;
        let paused = routine.paused;
        let target = self
            .definition_team_name(routine.team_id)
            .map(|name| ("icons/fork.svg", name))
            .or_else(|| {
                routine.workflow_id.and_then(|workflow| {
                    self.control_ui.snapshot.records.iter().find_map(|r| match r {
                        ControlRecord::Workflow(w) if w.id == workflow => {
                            Some(("icons/git-branch.svg", w.name.clone()))
                        }
                        _ => None,
                    })
                })
            })
            .unwrap_or(("icons/bot.svg", routine.provider.display_name().to_owned()));
        let pill = if routine.last_error.is_some() {
            status_pill("icons/alert.svg", "Needs attention", theme.danger, theme.danger_soft)
        } else if paused {
            status_pill("icons/stop.svg", "Paused", theme.text_secondary, theme.overlay)
        } else {
            status_pill("icons/rotate-cw.svg", "Scheduled", theme.accent, theme.accent_soft)
        };
        let next = match &routine.last_error {
            Some(error) => error.clone(),
            None if paused => "Schedule off".to_owned(),
            None if routine.next_run > 0 => {
                format!("Next {}", automation_ui::next_fire(routine.next_run))
            }
            None => "Not scheduled yet".to_owned(),
        };
        let meta_item = |path: &'static str, text: String, color: Hsla| {
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .min_w_0()
                .child(icon(path, 12.5, theme.text_tertiary))
                .child(div().truncate().text_color(color).child(text))
        };
        let toggle = div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.0))
            .px(px(6.0))
            .child(crate::ui::toggle_switch(
                SharedString::from(format!("routine-schedule-{id}")),
                !paused,
                self.control_ui.busy,
                theme,
                cx,
                move |this, _, cx| {
                    this.control_request(
                        Rpc::SetRoutinePaused {
                            id,
                            expected_revision: this.control_ui.snapshot.revision,
                            paused: !paused,
                        },
                        cx,
                    )
                },
            ))
            .child(
                div()
                    .w(px(24.0))
                    .text_size(sp(12.0))
                    .text_color(theme.text_secondary)
                    .child(if paused { "Off" } else { "On" }),
            );
        div()
            .id(SharedString::from(format!("routine-row-{id}")))
            .mx(px(2.0))
            .mb(px(10.0))
            .px(px(16.0))
            .py(px(14.0))
            .rounded(px(14.0))
            .border_1()
            .border_color(theme.ticket_border)
            .bg(theme.ticket)
            .shadow(crate::ui::elevation(&theme, crate::ui::Elevation::Rest))
            .hover(move |style| style.border_color(theme.border_strong))
            .flex()
            .items_center()
            .gap(px(14.0))
            .child(
                avatar_tile(38.0, theme.accent_soft, gpui::transparent_black())
                    .child(icon("icons/calendar-clock.svg", 17.0, theme.accent)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
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
                                    .text_size(sp(14.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text)
                                    .child(routine.name.clone()),
                            )
                            .child(pill),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(16.0))
                            .min_w_0()
                            .text_size(sp(12.0))
                            .child(meta_item(
                                "icons/rotate-cw.svg",
                                routine_ui::schedule_label(
                                    routine.interval_minutes,
                                    routine.calendar.as_ref(),
                                ),
                                theme.text_secondary,
                            ))
                            .child(meta_item(target.0, target.1, theme.text_secondary))
                            .child(meta_item(
                                "icons/hourglass.svg",
                                next,
                                if routine.last_error.is_some() {
                                    theme.danger
                                } else {
                                    theme.text_secondary
                                },
                            )),
                    ),
            )
            .child(self.run_definition_button(id, cx))
            .child(toggle)
            .child(self.definition_history_button(id, cx))
            .child(self.definition_edit_button(ControlRecord::Routine(routine.clone()), cx))
            .into_any_element()
    }
}
