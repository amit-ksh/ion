//! Cached graph presentation for saved teams and the editable reporting hierarchy.
use super::team_graph_layout::{self as geometry, Layout};
use super::*;

#[derive(Default)]
pub(super) struct Graph {
    pub(super) team: Option<Team>,
    pub(super) members: Vec<TeamMember>,
    parents: HashMap<Uuid, Option<Uuid>>,
    pub(super) agents: Vec<Option<AgentProfile>>,
    pub(super) layout: Layout,
    pub(super) error: Option<String>,
}
impl Graph {
    pub(super) fn has_unavailable_agents(&self) -> bool {
        self.agents.iter().any(Option::is_none)
    }

    pub(super) fn new(
        team: Option<Team>,
        members: &[TeamMember],
        profiles: &HashMap<Uuid, &AgentProfile>,
    ) -> Self {
        let indices: HashMap<_, _> = members
            .iter()
            .enumerate()
            .map(|(i, m)| (m.agent_id, i))
            .collect();
        // Resolve missing parent IDs explicitly rather than drawing a false lead.
        let parents: Result<Vec<Option<usize>>, &str> = members
            .iter()
            .map(|m| match m.parent_id {
                None => Ok(None),
                Some(id) => indices
                    .get(&id)
                    .copied()
                    .map(Some)
                    .ok_or("Choose an existing agent for each reporting line."),
            })
            .collect();
        let result = if indices.len() == members.len() {
            parents.and_then(|parents| geometry::layout(&parents))
        } else {
            Err("An agent can appear only once in a team.")
        };
        let (layout, error) = match result {
            Ok(layout) => (layout, None),
            Err(error) => (Layout::default(), Some(error.into())),
        };
        Self {
            team,
            members: members.to_vec(),
            parents: members.iter().map(|m| (m.agent_id, m.parent_id)).collect(),
            layout,
            error,
            agents: members
                .iter()
                .map(|m| profiles.get(&m.agent_id).map(|p| (**p).clone()))
                .collect(),
        }
    }
}

impl Waku {
    pub(super) fn refresh_team_graph(&mut self) {
        let profiles = self
            .control_ui
            .snapshot
            .records
            .iter()
            .filter_map(|r| match r {
                ControlRecord::Agent(agent) => Some((agent.id, agent)),
                _ => None,
            })
            .collect();
        self.control_ui.team_graph = Rc::new(Graph::new(None, &self.control_ui.members, &profiles));
        if self
            .control_ui
            .team_graph_selected
            .is_some_and(|id| !self.control_ui.members.iter().any(|m| m.agent_id == id))
        {
            self.control_ui.team_graph_selected = None;
        }
    }

    pub(super) fn team_parent_allowed(&self, id: Uuid, parent: Uuid) -> bool {
        team_hierarchy::parent_allowed(&self.control_ui.team_graph.parents, id, parent)
    }

    pub(super) fn set_team_parent(
        &mut self,
        id: Uuid,
        parent: Option<Uuid>,
        cx: &mut Context<Self>,
    ) {
        let mut members: Vec<_> = self
            .control_ui
            .members
            .iter()
            .map(|m| (m.agent_id, m.parent_id))
            .collect();
        if let Err(error) = team_hierarchy::reparent(&mut members, id, parent) {
            self.control_ui.error = Some(error.into());
            cx.notify();
            return;
        }
        self.control_ui.members = members
            .into_iter()
            .map(|(agent_id, parent_id)| TeamMember {
                agent_id,
                parent_id,
            })
            .collect();
        self.control_ui.error = None;
        self.refresh_team_graph();
        cx.notify();
    }

    pub(super) fn toggle_team_member(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if self.control_ui.members.iter().any(|m| m.agent_id == id) {
            let mut members: Vec<_> = self
                .control_ui
                .members
                .iter()
                .map(|m| (m.agent_id, m.parent_id))
                .collect();
            team_hierarchy::remove(&mut members, id);
            self.control_ui.members = members
                .into_iter()
                .map(|(agent_id, parent_id)| TeamMember {
                    agent_id,
                    parent_id,
                })
                .collect();
        } else {
            if self.control_ui.members.len() >= 32 {
                self.control_ui.error = Some("A team can contain at most 32 agents.".into());
                cx.notify();
                return;
            }
            let parent_id = self
                .control_ui
                .team_graph_selected
                .filter(|id| self.control_ui.members.iter().any(|m| m.agent_id == *id))
                .or_else(|| {
                    self.control_ui
                        .members
                        .iter()
                        .find(|m| m.parent_id.is_none())
                        .map(|m| m.agent_id)
                });
            self.control_ui.members.push(TeamMember {
                agent_id: id,
                parent_id,
            });
            self.control_ui.team_graph_selected = Some(id);
        }
        self.refresh_team_graph();
        cx.notify();
    }

    /// A saved team renders as a Paperclip-style board: the team's identity
    /// and actions over its org chart. The editor shows the chart alone.
    pub(super) fn render_team_graph(&self, graph: Rc<Graph>, cx: &mut Context<Self>) -> AnyElement {
        let chart = self.render_team_chart(&graph, cx);
        match graph.team.clone() {
            Some(team) => self.render_team_board(team, chart, cx),
            None => div()
                .w_full()
                .min_w_0()
                .py(px(18.0))
                .child(chart)
                .into_any_element(),
        }
    }

    fn render_team_board(
        &self,
        team: Team,
        chart: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::workspace(cx);
        let id = team.id;
        let reports = team.members.len().saturating_sub(1);
        let summary = match reports {
            0 => "Lead only".to_owned(),
            1 => "Lead + 1 agent".to_owned(),
            n => format!("Lead + {n} agents"),
        };
        let budget = team
            .limits
            .as_ref()
            .and_then(|limits| limits.max_tokens)
            .map(|tokens| {
                status_pill(
                    "icons/gauge.svg",
                    format!(
                        "{} token budget",
                        super::super::usage_page::format_tokens_compact(tokens as f64)
                    ),
                    theme.text_secondary,
                    theme.overlay,
                )
            });
        let record = ControlRecord::Team(team.clone());
        let edit = record.clone();
        let actions = div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(self.control_button_icon(
                format!("record-run-{id}"),
                "icons/compose.svg",
                "New task",
                ControlTone::Secondary,
                cx,
                move |this, _, cx| this.prepare_record_task(record.clone(), cx),
            ))
            .child(self.control_button_tone(
                format!("team-runs-{id}"),
                "Runs",
                ControlTone::Ghost,
                cx,
                move |this, _, cx| {
                    this.control_ui.runs_team = Some(id);
                    this.control_ui.runs_tab = true;
                    this.clear_team_recovery();
                    this.control_ui.selected_run = None;
                    this.control_ui.team_run = None;
                    this.control_ui.runs_after = None;
                    this.control_ui.runs_previous.clear();
                    this.refresh_team_activity(true, cx);
                    cx.notify();
                },
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
        let header = div()
            .px(px(18.0))
            .py(px(14.0))
            .flex()
            .items_center()
            .gap(px(14.0))
            .child(
                avatar_tile(38.0, theme.accent_soft, gpui::transparent_black()).child(icon(
                    "icons/fork.svg",
                    17.0,
                    theme.accent,
                )),
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
                                    .child(team.name.clone()),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .font_family(crate::md::render::MONO_FAMILY)
                                    .text_size(sp(11.5))
                                    .text_color(theme.text_tertiary)
                                    .child(summary),
                            )
                            .children(budget),
                    )
                    .when(!team.objective.trim().is_empty(), |column| {
                        column.child(
                            div()
                                .truncate()
                                .text_size(sp(12.5))
                                .text_color(theme.text_secondary)
                                .child(team.objective.clone()),
                        )
                    }),
            )
            .child(actions);
        div()
            .id(SharedString::from(format!("team-board-{id}")))
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
                    .border_t_1()
                    .border_color(theme.border)
                    .bg(theme.canvas)
                    .py(px(26.0))
                    .child(chart),
            )
            .into_any_element()
    }

    /// The org chart itself: elbow connectors from each manager's bottom
    /// edge to its reports, and a card per agent carrying its harness mark,
    /// readiness dot, role and adapter, after Paperclip's org chart.
    fn render_team_chart(&self, graph: &Graph, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::workspace(cx);
        if let Some(error) = &graph.error {
            return div()
                .px(px(16.0))
                .text_color(theme.danger)
                .child(error.clone())
                .into_any_element();
        }
        if graph.members.is_empty() {
            return div()
                .px(px(16.0))
                .text_color(theme.text_secondary)
                .child("Add agents below to build your team.")
                .into_any_element();
        }
        let scope = graph
            .team
            .as_ref()
            .map(|t| t.id.to_string())
            .unwrap_or_else(|| "editor".into());
        let mut canvas = div()
            .relative()
            .flex_none()
            .w(px(graph.layout.width))
            .h(px(graph.layout.height));
        for line in &graph.layout.lines {
            canvas = canvas.child(
                div()
                    .absolute()
                    .left(px(line.left))
                    .top(px(line.top))
                    .w(px(line.width.max(1.5)))
                    .h(px(line.height.max(1.5)))
                    .rounded_full()
                    .bg(theme.border_strong),
            );
        }
        for node in &graph.layout.nodes {
            let member = &graph.members[node.index];
            let agent = graph.agents[node.index].as_ref();
            let team = graph.team.clone();
            let id = member.agent_id;
            let selected = team.is_none() && self.control_ui.team_graph_selected == Some(id);
            canvas = canvas.child(
                self.render_team_node(&scope, member, agent, selected, 1.0, cx)
                    .absolute()
                    .left(px(node.left))
                    .top(px(node.top))
                    .on_activation(cx, move |this, _, cx| {
                        if let Some(team) = &team {
                            this.edit_control_record(ControlRecord::Team(team.clone()), cx);
                        }
                        this.control_ui.team_graph_selected = Some(id);
                        cx.notify();
                    }),
            );
        }
        div()
            .id(SharedString::from(format!("team-graph-scroll-{scope}")))
            .w_full()
            .min_w_0()
            .overflow_x_scroll()
            // A vertical wheel belongs to the page list, not this sideways chart.
            .restrict_scroll_to_axis()
            .child(
                div()
                    .min_w(gpui::relative(1.0))
                    .px(px(24.0))
                    .flex()
                    .justify_center()
                    .child(canvas),
            )
            .into_any_element()
    }

    /// One agent card at zoom `z`, sized from the layout grid.
    pub(super) fn render_team_node(
        &self,
        scope: &str,
        member: &TeamMember,
        agent: Option<&AgentProfile>,
        selected: bool,
        z: f32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = Theme::workspace(cx);
        let lead = member.parent_id.is_none();
        let (name, role, adapter) = match agent {
            Some(agent) => (
                agent.name.clone(),
                agent.expertise.clone(),
                format!(
                    "{} · {}",
                    agent.provider.display_name(),
                    agent.execution.environment.label()
                ),
            ),
            None => (
                "Unavailable agent".to_owned(),
                "Choose an available agent".to_owned(),
                "Missing".to_owned(),
            ),
        };
        div()
            .id(SharedString::from(format!(
                "team-graph-{scope}-{}",
                member.agent_id
            )))
            .tab_index(0)
            .w(px(geometry::NODE_WIDTH * z))
            .h(px(geometry::NODE_HEIGHT * z))
            .px(px(14.0 * z))
            .rounded(px(14.0 * z))
            .border_1()
            .border_color(if selected {
                theme.accent
            } else {
                theme.ticket_border
            })
            .bg(theme.ticket)
            .shadow(crate::ui::elevation(&theme, crate::ui::Elevation::Rest))
            .cursor_default()
            .hover(move |style| {
                style
                    .border_color(theme.border_strong)
                    .shadow(crate::ui::elevation(&theme, crate::ui::Elevation::Lifted))
            })
            .focus_visible(move |style| style.border_color(theme.accent))
            .flex()
            .items_center()
            .gap(px(12.0 * z))
            .child(self.agent_avatar(agent, 38.0 * z, lead, &theme))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(3.0 * z))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0 * z))
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(sp(13.5 * z))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text)
                                    .child(name),
                            )
                            .when(lead, |row| {
                                row.child(
                                    div()
                                        .flex_none()
                                        .h(px(18.0 * z))
                                        .px(px(6.0 * z))
                                        .rounded_full()
                                        .bg(theme.accent_soft)
                                        .flex()
                                        .items_center()
                                        .text_size(sp(10.5 * z))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.accent)
                                        .child("Lead"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(sp(12.0 * z))
                            .text_color(theme.text_secondary)
                            .child(role),
                    )
                    .child(
                        div()
                            .truncate()
                            .font_family(crate::md::render::MONO_FAMILY)
                            .text_size(sp(10.5 * z))
                            .text_color(theme.text_tertiary)
                            .child(adapter),
                    ),
            )
    }

    /// A round harness avatar with a readiness dot: green when the agent's
    /// harness is installed, amber when it is missing, red when the agent
    /// itself no longer exists. The lead's avatar is the chart's lime object.
    pub(super) fn agent_avatar(
        &self,
        agent: Option<&AgentProfile>,
        size: f32,
        lead: bool,
        theme: &Theme,
    ) -> Div {
        let theme = *theme;
        let dot = match agent {
            None => theme.danger,
            Some(agent) => match self.provider_probe(agent.provider) {
                Some(probe) if probe.installed => theme.success,
                Some(_) => theme.warning,
                None => theme.text_tertiary,
            },
        };
        let mark = match agent {
            Some(agent) => provider_mark(
                &theme,
                agent.provider,
                (size * 0.44).round(),
                if lead {
                    theme.on_primary
                } else {
                    provider_color(&theme, agent.provider)
                },
            )
            .into_any_element(),
            None => {
                icon("icons/alert.svg", (size * 0.42).round(), theme.warning).into_any_element()
            }
        };
        let dot_size = (size * 0.3).round().max(9.0);
        div()
            .relative()
            .flex_none()
            .size(px(size))
            .child(
                div()
                    .size_full()
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(if lead { theme.primary } else { theme.inset })
                    .child(mark),
            )
            .child(
                div()
                    .absolute()
                    .right(px(-1.0))
                    .bottom(px(-1.0))
                    .size(px(dot_size))
                    .rounded_full()
                    .border_2()
                    .border_color(theme.ticket)
                    .bg(dot),
            )
    }

    pub(super) fn render_team_graph_selection(&self, cx: &mut Context<Self>) -> Div {
        let theme = Theme::workspace(cx);
        let Some(id) = self.control_ui.team_graph_selected else {
            return div();
        };
        let profiles: Vec<_> = self
            .control_ui
            .snapshot
            .records
            .iter()
            .filter_map(|r| match r {
                ControlRecord::Agent(agent) => Some(agent.clone()),
                _ => None,
            })
            .collect();
        if !self.control_ui.members.iter().any(|m| m.agent_id == id) {
            return div();
        }
        let context = profiles
            .iter()
            .find(|a| a.id == id)
            .map(|agent| format!("{} · {}", agent.name, agent.expertise))
            .unwrap_or_else(|| "Unavailable agent".into());
        let selected_parent = self
            .control_ui
            .members
            .iter()
            .find(|m| m.agent_id == id)
            .and_then(|m| m.parent_id);
        let mut options = vec![(None, "Team lead".to_string())];
        options.extend(
            profiles
                .iter()
                .filter(|a| self.team_parent_allowed(id, a.id))
                .map(|a| (Some(a.id), a.name.clone())),
        );
        let label = selected_parent
            .map(|id| {
                format!(
                    "Reports to {}",
                    profiles
                        .iter()
                        .find(|a| a.id == id)
                        .map(|a| a.name.as_str())
                        .unwrap_or("unavailable agent")
                )
            })
            .unwrap_or_else(|| "Team lead".into());
        let weak = cx.entity().downgrade();
        let menu_id = SharedString::from(format!("graph-parent-menu-{id}"));
        let menu = self.menu_handle(menu_id.clone(), cx);
        let candidates: Vec<_> = profiles
            .iter()
            .filter(|a| !self.control_ui.members.iter().any(|m| m.agent_id == a.id))
            .map(|a| (a.id, a.name.clone()))
            .collect();
        let mut panel = div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(div().text_color(theme.text_secondary).child(context))
            .child(dropdown_menu(
                self.control_button(format!("graph-parent-{id}"), label, cx, |_, _, _| {}),
                menu_id,
                &menu,
                MenuAlign::BelowLeft,
                move |_| {
                    options
                        .iter()
                        .map(|(parent, label)| {
                            let parent = *parent;
                            let weak = weak.clone();
                            MenuItem::new(label.clone(), move |_, cx| {
                                let _ = weak
                                    .update(cx, |this, cx| this.set_team_parent(id, parent, cx));
                            })
                            .selected(parent == selected_parent)
                        })
                        .collect()
                },
            ));
        if !candidates.is_empty() {
            let weak = cx.entity().downgrade();
            let menu_id = SharedString::from(format!("graph-report-menu-{id}"));
            let menu = self.menu_handle(menu_id.clone(), cx);
            panel = panel.child(dropdown_menu(
                self.control_button(
                    format!("graph-add-report-{id}"),
                    "Add direct report",
                    cx,
                    |_, _, _| {},
                ),
                menu_id,
                &menu,
                MenuAlign::BelowLeft,
                move |_| {
                    candidates
                        .iter()
                        .map(|(agent, label)| {
                            let agent = *agent;
                            let weak = weak.clone();
                            MenuItem::new(label.clone(), move |_, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    // A stale menu cannot add under a manager removed since it opened.
                                    if this.control_ui.members.iter().any(|m| m.agent_id == id)
                                        && !this
                                            .control_ui
                                            .members
                                            .iter()
                                            .any(|m| m.agent_id == agent)
                                    {
                                        this.control_ui.team_graph_selected = Some(id);
                                        this.toggle_team_member(agent, cx);
                                    }
                                });
                            })
                        })
                        .collect()
                },
            ));
        }
        panel.child(self.control_button(
            format!("graph-remove-{id}"),
            "Remove from team",
            cx,
            move |this, _, cx| {
                if this.control_ui.members.iter().any(|m| m.agent_id == id) {
                    this.toggle_team_member(id, cx);
                }
            },
        ))
    }
}
