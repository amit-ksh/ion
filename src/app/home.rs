use super::sidebar::{format_time_ago, session_time_label};
use super::*;
use crate::ui::{ActivationExt, Elevation, elevation};
use gpui::{PathBuilder, relative};

/// Most tickets the home rail shows. The sidebar keeps the full history;
/// the rail is the glance at what is live.
const HOME_RAIL_LIMIT: usize = 8;
const TICKET_MIN_WIDTH: f32 = 220.0;
const TICKET_GAP: f32 = 12.0;
const RAIL_MAX_WIDTH: f32 = 1040.0;

/// What a ticket on the rail is waiting for. Every state carries a label and
/// an icon, so meaning never rests on color alone. The derived order is the
/// rail order: work that needs the user leads, settled work trails.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum TicketState {
    NeedsYou,
    Working,
    Background,
    Failed,
    Ready,
}

impl TicketState {
    fn of(status: SessionStatus) -> Self {
        match status {
            SessionStatus::Waiting => Self::NeedsYou,
            SessionStatus::Connecting | SessionStatus::Working => Self::Working,
            SessionStatus::Background => Self::Background,
            SessionStatus::Failed => Self::Failed,
            SessionStatus::Idle => Self::Ready,
        }
    }

    fn label(self) -> String {
        match self {
            Self::NeedsYou => tr!("home.ticket_needs_you"),
            Self::Working => tr!("home.ticket_working"),
            Self::Background => tr!("home.ticket_background"),
            Self::Failed => tr!("home.ticket_failed"),
            Self::Ready => tr!("home.ticket_ready"),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::NeedsYou => "icons/alert.svg",
            Self::Working => "icons/loader-circle.svg",
            Self::Background => "icons/hourglass.svg",
            Self::Failed => "icons/x.svg",
            Self::Ready => "icons/check.svg",
        }
    }

    fn color(self, theme: &Theme) -> Hsla {
        match self {
            Self::NeedsYou | Self::Working => theme.accent,
            Self::Background => theme.text_secondary,
            Self::Failed => theme.danger,
            Self::Ready => theme.success,
        }
    }
}

/// The rail's order: tickets waiting on you, then working ones, then settled
/// tickets by most recent activity, so a fresh result is not buried under
/// week-old failures.
fn rail_sessions(sessions: &[AgentSession]) -> Vec<&AgentSession> {
    let mut started = sessions
        .iter()
        .filter(|session| session.has_started())
        .collect::<Vec<_>>();
    started.sort_by_key(|session| {
        (
            match TicketState::of(session.status) {
                TicketState::NeedsYou => 0,
                TicketState::Working => 1,
                _ => 2,
            },
            std::cmp::Reverse(session.last_reply_at.unwrap_or(session.created_at)),
        )
    });
    started.truncate(HOME_RAIL_LIMIT);
    started
}

/// A ticket's short order number: the first four hex digits of its id, the
/// same on every render so a ticket keeps its identity as it changes state.
fn ticket_number(id: Uuid) -> String {
    let hex = id.simple().to_string();
    format!("#{}", hex[..4].to_uppercase())
}

/// The tear line between a ticket's stub and its body.
pub(super) fn perforation(color: Hsla) -> impl IntoElement {
    div().mx(px(12.0)).h(px(1.0)).child(
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let y = bounds.origin.y + px(0.5);
                let mut builder = PathBuilder::stroke(px(1.0)).dash_array(&[px(3.0), px(3.0)]);
                builder.move_to(point(bounds.origin.x, y));
                builder.line_to(point(bounds.origin.x + bounds.size.width, y));
                if let Ok(line) = builder.build() {
                    window.paint_path(line, color);
                }
            },
        )
        .size_full(),
    )
}

/// The home glow field: one mint-to-lime wash rising from the bottom edge.
/// Both layers fade vertically to zero alpha at their top edge; GPUI
/// gradients have two stops and no mask, so a diagonal layer would end on a
/// visible seam. Place it first inside a `relative()` container.
pub(super) fn glow_field(theme: &Theme) -> [Div; 2] {
    let clear_mint = Hsla {
        a: 0.0,
        ..theme.glow_mint
    };
    let clear_lime = Hsla {
        a: 0.0,
        ..theme.glow_lime
    };
    [
        div()
            .absolute()
            .left_0()
            .bottom_0()
            .w_full()
            .h(relative(0.62))
            .bg(linear_gradient(
                180.0,
                linear_color_stop(clear_mint, 0.0),
                linear_color_stop(theme.glow_mint, 1.0),
            )),
        div()
            .absolute()
            .left_0()
            .bottom_0()
            .w_full()
            .h(relative(0.30))
            .bg(linear_gradient(
                180.0,
                linear_color_stop(clear_lime, 0.0),
                linear_color_stop(theme.glow_lime, 1.0),
            )),
    ]
}

impl Waku {
    /// Home: the question, the composer under it, and the live rail of
    /// tickets, over one mint-to-lime glow field rising from the bottom edge.
    pub(super) fn render_home(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        let headline = self.render_home_headline(cx);
        let queued = self.render_queued_messages(cx);
        let composer = self.render_composer(window, cx);
        let footer = self.render_workspace_footer(cx);
        let rail = self.render_home_rail(window, cx);

        div()
            .flex_1()
            .min_h_0()
            .relative()
            .overflow_hidden()
            .children(glow_field(&theme))
            .child(
                div()
                    .id("home-scroll")
                    .absolute()
                    .inset_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(div().flex_none().w_full().h(relative(0.16)).min_h(px(28.0)))
                    .child(div().px(px(24.0)).flex().justify_center().child(headline))
                    .child(div().h(px(26.0)).flex_none())
                    .child(div().w_full().children(queued))
                    .child(div().w_full().child(composer))
                    .child(div().w_full().child(footer))
                    .child(div().h(px(44.0)).flex_none())
                    .child(rail)
                    .child(div().h(px(40.0)).flex_none()),
            )
            .into_any_element()
    }

    fn render_home_rail(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let now = unix_time();
        let sessions = rail_sessions(&self.state.sessions);
        // Whole tickets only: as many columns as fit the content column,
        // never a ticket cut at the edge.
        let available = (f32::from(window.viewport_size().width)
            - self.sidebar_rendered_width
            - self.right_panel_rendered_width
            - 48.0)
            .min(RAIL_MAX_WIDTH - 40.0);
        let columns = (((available + TICKET_GAP) / (TICKET_MIN_WIDTH + TICKET_GAP)).floor()
            as usize)
            .clamp(1, 4);
        let needs_you = sessions
            .iter()
            .filter(|session| TicketState::of(session.status) == TicketState::NeedsYou)
            .count();
        // "Live" only while something is in flight or waiting on you; a rail
        // of settled tickets is recent history.
        let live = sessions.iter().any(|session| {
            matches!(
                TicketState::of(session.status),
                TicketState::NeedsYou | TicketState::Working
            )
        });

        let heading = div()
            .w_full()
            .flex()
            .items_center()
            .gap(px(10.0))
            .child(
                div()
                    .text_size(sp(13.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text)
                    .child(if live {
                        tr!("home.rail_title")
                    } else {
                        tr!("home.rail_title_recent")
                    }),
            )
            .when(needs_you > 0, |row| {
                row.child(
                    div()
                        .h(px(20.0))
                        .px(px(8.0))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .gap(px(5.0))
                        .bg(theme.accent_soft)
                        .text_size(sp(11.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.accent)
                        .child(icon("icons/alert.svg", 11.0, theme.accent))
                        .child(format!("{needs_you} {}", tr!("home.ticket_needs_you"))),
                )
            })
            .child(div().flex_1())
            .child(
                div()
                    .id("home-all-tasks")
                    .tab_index(0)
                    .h(px(28.0))
                    .px(px(10.0))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .cursor_default()
                    .text_size(sp(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text_secondary)
                    .border_1()
                    .border_color(gpui::transparent_black())
                    .focus_visible(|style| style.border_color(theme.accent))
                    .hover(|style| style.bg(theme.overlay))
                    .active(|style| style.bg(theme.overlay_strong))
                    .child(tr!("home.all_tasks"))
                    .child(icon("icons/chevron-right.svg", 10.0, theme.text_tertiary))
                    .on_activation(cx, |this, _, cx| {
                        this.open_workspace_page(super::workspace_control::Page::Tasks, cx)
                    }),
            );

        let body = if sessions.is_empty() {
            div()
                .w_full()
                .min_h(px(96.0))
                .px(px(20.0))
                .rounded(px(14.0))
                .border_1()
                .border_color(theme.ticket_border)
                .bg(theme.ticket.opacity(0.6))
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(icon("icons/list.svg", 16.0, theme.text_tertiary))
                .child(
                    div()
                        .max_w(px(440.0))
                        .text_size(sp(13.0))
                        .line_height(sp(19.0))
                        .text_color(theme.text_secondary)
                        .child(tr!("home.rail_empty")),
                )
                .into_any_element()
        } else {
            div()
                .id("home-rail")
                .w_full()
                .grid()
                .grid_cols(columns as u16)
                .gap(px(TICKET_GAP))
                .children(
                    sessions
                        .iter()
                        .map(|session| self.render_home_ticket(session, now, cx)),
                )
                .into_any_element()
        };

        div()
            .w_full()
            .max_w(px(RAIL_MAX_WIDTH))
            .px(px(20.0))
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(div().px(px(4.0)).child(heading))
            .child(body)
            .into_any_element()
    }

    fn render_home_ticket(
        &self,
        session: &AgentSession,
        now: u64,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        let state = TicketState::of(session.status);
        let needs_you = state == TicketState::NeedsYou;
        let session_id = session.id;
        let state_color = state.color(&theme);
        // A ticket waiting on you is not working, so it shows an age, not a running clock.
        let time = if needs_you {
            session
                .turns
                .last()
                .map(|turn| format_time_ago(now.saturating_sub(turn.started_at)))
        } else {
            session_time_label(session, now)
        }
        .unwrap_or_default();
        let station = self
            .state
            .projects
            .iter()
            .find(|project| project.id == session.project_id)
            .map(|project| {
                if project.is_projectless() {
                    tr!("project.no_project")
                } else {
                    project.display_name()
                }
            })
            .unwrap_or_default();
        let lifted = elevation(&theme, Elevation::Lifted);
        let state_icon = icon(state.icon(), 11.0, state_color);

        div()
            .id(SharedString::from(format!("home-ticket-{session_id}")))
            .tab_index(0)
            .w_full()
            .min_w_0()
            .rounded(px(12.0))
            .border_1()
            .border_color(if needs_you {
                theme.primary
            } else {
                theme.ticket_border
            })
            .bg(theme.ticket)
            .shadow(elevation(&theme, Elevation::Rest))
            .cursor_default()
            .flex()
            .flex_col()
            .hover(move |style| style.shadow(lifted.clone()))
            .focus_visible(|style| style.border_color(theme.accent))
            .tooltip(Tooltip::text(SharedString::from(localized_title(session))))
            .on_activation(cx, move |this, _, cx| this.select_session(session_id, cx))
            .child(
                div()
                    .px(px(12.0))
                    .pt(px(9.0))
                    .pb(px(8.0))
                    // Inner radius: the card's 12px minus its 1px border.
                    .rounded_t(px(11.0))
                    .when(needs_you, |stub| stub.bg(theme.accent_soft))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .font_family(crate::md::render::MONO_FAMILY)
                    .text_size(sp(11.5))
                    .line_height(sp(14.0))
                    .text_color(theme.text_tertiary)
                    .child(ticket_number(session_id))
                    .child(div().flex_1())
                    .child(time),
            )
            .child(perforation(theme.ticket_border))
            .child(
                div()
                    .px(px(12.0))
                    .pt(px(9.0))
                    .pb(px(12.0))
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(
                        div()
                            .h(sp(38.0))
                            .whitespace_normal()
                            .line_clamp(2)
                            .text_overflow(gpui::TextOverflow::Truncate("…".into()))
                            .text_size(sp(13.5))
                            .line_height(sp(19.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(SharedString::from(localized_title(session))),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(provider_mark(
                                &theme,
                                session.provider,
                                12.0,
                                provider_color(&theme, session.provider),
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(sp(12.0))
                                    .text_color(theme.text_secondary)
                                    .child(station),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .h(px(20.0))
                                    .px(px(7.0))
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .bg(state_color.opacity(0.12))
                                    .text_size(sp(11.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(state_color)
                                    .child(if state == TicketState::Working {
                                        motion::spin_slow(state_icon).into_any_element()
                                    } else {
                                        state_icon.into_any_element()
                                    })
                                    .child(state.label()),
                            ),
                    )
                    .when_some(
                        needs_you
                            .then(|| self.render_ticket_pass(session_id, &theme, cx))
                            .flatten(),
                        |body, actions| body.child(actions),
                    ),
            )
            .into_any_element()
    }

    /// The pass on a ticket: answer its pending permission from the rail.
    /// The provider's first allow option leads; its first deny option sends
    /// the work back.
    fn render_ticket_pass(
        &self,
        session_id: Uuid,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let permission = self
            .runtimes
            .get(&session_id)?
            .pending_permission
            .as_ref()?;
        let allow = permission.options.iter().find(|option| option.allow)?;
        let deny = permission.options.iter().find(|option| !option.allow);
        let button = |id: String, label: String, primary: bool, option_id: String| {
            let request_id = permission.request_id.clone();
            div()
                .id(SharedString::from(id))
                .flex_1()
                .h(px(30.0))
                .rounded(px(8.0))
                .flex()
                .items_center()
                .justify_center()
                .gap(px(6.0))
                .cursor_default()
                .text_size(sp(12.5))
                .font_weight(FontWeight::SEMIBOLD)
                .when(primary, |button| {
                    button
                        .bg(theme.primary)
                        .text_color(theme.on_primary)
                        .hover(|button| button.opacity(0.9))
                        .child(icon("icons/check.svg", 12.0, theme.on_primary))
                })
                .when(!primary, |button| {
                    button
                        .border_1()
                        .border_color(theme.border_strong)
                        .text_color(theme.text_secondary)
                        .hover(|button| button.bg(theme.overlay))
                        .child(icon("icons/x.svg", 12.0, theme.text_secondary))
                })
                .child(SharedString::from(label))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.respond_permission_for(
                        session_id,
                        request_id.clone(),
                        option_id.clone(),
                        cx,
                    );
                }))
        };
        Some(
            div()
                .flex()
                .gap(px(8.0))
                .child(button(
                    format!("ticket-allow-{session_id}"),
                    allow.label.clone(),
                    true,
                    allow.id.clone(),
                ))
                .when_some(deny, |row, deny| {
                    row.child(button(
                        format!("ticket-deny-{session_id}"),
                        deny.label.clone(),
                        false,
                        deny.id.clone(),
                    ))
                }),
        )
    }
}

fn localized_title(session: &AgentSession) -> String {
    let title = session.display_title();
    if title == AgentSession::DEFAULT_TITLE {
        tr!("session.new_task")
    } else {
        title.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rail_leads_with_work_that_needs_the_user() {
        let project = Uuid::new_v4();
        let mut ready = AgentSession::new(project, ProviderKind::Codex);
        ready.last_reply_at = Some(300);
        ready.messages.push(Message::new(MessageRole::User, "done"));
        let mut waiting = AgentSession::new(project, ProviderKind::Codex);
        waiting.status = SessionStatus::Waiting;
        waiting.last_reply_at = Some(100);
        waiting
            .messages
            .push(Message::new(MessageRole::User, "approve?"));
        let mut working = AgentSession::new(project, ProviderKind::Codex);
        working.status = SessionStatus::Working;
        working.last_reply_at = Some(200);
        working
            .messages
            .push(Message::new(MessageRole::User, "build"));

        let sessions = vec![ready.clone(), working.clone(), waiting.clone()];
        let order = rail_sessions(&sessions)
            .into_iter()
            .map(|session| session.id)
            .collect::<Vec<_>>();
        assert_eq!(order, vec![waiting.id, working.id, ready.id]);
    }

    #[test]
    fn settled_tickets_follow_recency_not_state() {
        let project = Uuid::new_v4();
        let mut old_failure = AgentSession::new(project, ProviderKind::Codex);
        old_failure.status = SessionStatus::Failed;
        old_failure.last_reply_at = Some(100);
        old_failure
            .messages
            .push(Message::new(MessageRole::User, "broke"));
        let mut fresh_result = AgentSession::new(project, ProviderKind::Codex);
        fresh_result.last_reply_at = Some(900);
        fresh_result
            .messages
            .push(Message::new(MessageRole::User, "done"));

        let sessions = vec![old_failure.clone(), fresh_result.clone()];
        let order = rail_sessions(&sessions)
            .into_iter()
            .map(|session| session.id)
            .collect::<Vec<_>>();
        assert_eq!(order, vec![fresh_result.id, old_failure.id]);
    }

    #[test]
    fn ticket_numbers_are_stable_short_ids() {
        let id = Uuid::from_u128(0xabcd_ef01_0000_0000_0000_0000_0000_0000);
        assert_eq!(ticket_number(id), "#ABCD");
        assert_eq!(ticket_number(id), ticket_number(id));
    }
}
