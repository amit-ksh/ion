//! Paperclip-style dense rows: group headings and the Teams page agent roster.
use super::*;

/// A group heading with its count, as Paperclip heads its agent and routine groups.
pub(super) fn render_section_heading(
    label: &'static str,
    count: usize,
    theme: &Theme,
) -> AnyElement {
    div()
        .px(px(4.0))
        .pt(px(6.0))
        .pb(px(10.0))
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .text_size(sp(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text)
                .child(label),
        )
        .child(
            div()
                .h(px(18.0))
                .px(px(7.0))
                .rounded_full()
                .bg(theme.overlay)
                .flex()
                .items_center()
                .font_family(crate::md::render::MONO_FAMILY)
                .text_size(sp(11.0))
                .text_color(theme.text_secondary)
                .child(count.to_string()),
        )
        .into_any_element()
}

impl Waku {
    /// One agent in a single bordered roster: identity, model and adapter,
    /// where it runs, which teams it serves, then its quiet actions.
    pub(super) fn render_roster_row(
        &self,
        row: Rc<RosterRow>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::workspace(cx);
        let agent = &row.agent;
        let id = agent.id;
        let record = ControlRecord::Agent(agent.clone());
        let edit = record.clone();
        let model = agent
            .model
            .as_ref()
            .filter(|model| !model.trim().is_empty())
            .map(|model| {
                self.provider_probe(agent.provider)
                    .and_then(|probe| probe.models.iter().find(|m| &m.id == model))
                    .map(|m| m.name.clone())
                    .unwrap_or_else(|| model.clone())
            })
            .unwrap_or_else(|| "Harness default".to_owned());
        let (membership, role) = match row.teams.as_slice() {
            [] => ("No team".to_owned(), "Available"),
            [(name, true)] => (name.clone(), "Lead"),
            [(name, false)] => (name.clone(), "Member"),
            teams => (
                format!("{} teams", teams.len()),
                if teams.iter().any(|(_, lead)| *lead) {
                    "Lead and member"
                } else {
                    "Member"
                },
            ),
        };
        let concurrency = match agent.max_parallel_tasks {
            1 => "1 task at a time".to_owned(),
            n => format!("{n} tasks at a time"),
        };
        let column = |top: String, bottom: String, width: f32| {
            div()
                .w(px(width))
                .flex_none()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .truncate()
                        .text_size(sp(12.0))
                        .text_color(theme.text_secondary)
                        .child(top),
                )
                .child(
                    div()
                        .truncate()
                        .font_family(crate::md::render::MONO_FAMILY)
                        .text_size(sp(10.5))
                        .text_color(theme.text_tertiary)
                        .child(bottom),
                )
        };
        let actions = div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(2.0))
            .child(self.control_button_tone(
                format!("record-run-{id}"),
                "New task",
                ControlTone::Ghost,
                cx,
                move |this, _, cx| this.prepare_record_task(record.clone(), cx),
            ))
            .child(self.control_button_tone(
                format!("record-edit-{id}"),
                "Edit",
                ControlTone::Ghost,
                cx,
                move |this, window, cx| {
                    this.edit_control_record(edit.clone(), cx);
                    this.control_ui.name.read(cx).focus().focus(window, cx);
                },
            ));
        let line = div()
            .id(SharedString::from(format!("roster-{id}")))
            .px(px(16.0))
            .py(px(11.0))
            .flex()
            .items_center()
            .gap(px(14.0))
            .when(!row.first, |line| {
                line.border_t_1().border_color(theme.border)
            })
            .when(row.first, |line| line.rounded_t(px(13.0)))
            .when(row.last, |line| line.rounded_b(px(13.0)))
            .hover(move |style| style.bg(theme.overlay))
            .child(self.agent_avatar(Some(agent), 32.0, false, &theme))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .truncate()
                            .text_size(sp(13.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(agent.name.clone()),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(sp(12.0))
                            .text_color(theme.text_secondary)
                            .child(agent.expertise.clone()),
                    ),
            )
            .child(column(
                model,
                agent.provider.display_name().to_owned(),
                180.0,
            ))
            .child(column(
                agent.execution.environment.label().to_owned(),
                concurrency,
                128.0,
            ))
            .child(column(membership, role.to_owned(), 150.0))
            .child(actions);
        div()
            .mx(px(2.0))
            .bg(theme.ticket)
            .border_l_1()
            .border_r_1()
            .border_color(theme.ticket_border)
            .when(row.first, |group| group.border_t_1().rounded_t(px(14.0)))
            .when(row.last, |group| {
                group.border_b_1().rounded_b(px(14.0)).mb(px(14.0))
            })
            .child(line)
            .into_any_element()
    }
}
