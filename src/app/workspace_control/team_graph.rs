//! Cached graph presentation for saved teams and the editable reporting hierarchy.
use super::team_graph_layout::{self as geometry, Layout};
use super::*;

#[derive(Default)]
pub(super) struct Graph {
    team: Option<Team>,
    members: Vec<TeamMember>,
    parents: HashMap<Uuid, Option<Uuid>>,
    agents: Vec<Option<AgentProfile>>,
    layout: Layout,
    error: Option<String>,
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

    pub(super) fn render_team_graph(&self, graph: Rc<Graph>, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::workspace(cx);
        if let Some(error) = &graph.error {
            return div()
                .p(px(16.0))
                .text_color(theme.danger)
                .child(error.clone())
                .into_any_element();
        }
        if graph.members.is_empty() {
            return div()
                .py(px(12.0))
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
            .w(px(graph.layout.width))
            .h(px(graph.layout.height));
        for line in &graph.layout.lines {
            canvas = canvas.child(
                div()
                    .absolute()
                    .left(px(line.left))
                    .top(px(line.top))
                    .w(px(line.width))
                    .h(px(line.height))
                    .bg(theme.border_strong),
            );
        }
        for node in &graph.layout.nodes {
            let member = &graph.members[node.index];
            let id = member.agent_id;
            let agent = graph.agents[node.index].as_ref();
            let name = agent
                .map(|a| a.name.clone())
                .unwrap_or_else(|| "Unavailable agent".into());
            let detail = agent
                .map(|a| {
                    format!(
                        "{} · {}",
                        a.provider.display_name(),
                        a.execution.environment.label()
                    )
                })
                .unwrap_or_else(|| "Choose an available agent".into());
            let team = graph.team.clone();
            let selected = team.is_none() && self.control_ui.team_graph_selected == Some(id);
            let lead = member.parent_id.is_none();
            // The avatar carries the agent's harness mark; the lead's tile is
            // the one lime object on the chart, so the hierarchy reads at a
            // glance without relying on position alone.
            let avatar = div()
                .size(px(34.0))
                .flex_none()
                .rounded(px(10.0))
                .flex()
                .items_center()
                .justify_center()
                .when(lead, |tile| tile.bg(theme.primary))
                .when(!lead, |tile| tile.bg(theme.inset))
                .child(match agent {
                    Some(agent) => provider_mark(
                        &theme,
                        agent.provider,
                        16.0,
                        if lead {
                            theme.on_primary
                        } else {
                            provider_color(&theme, agent.provider)
                        },
                    )
                    .into_any_element(),
                    None => icon("icons/alert.svg", 16.0, theme.warning).into_any_element(),
                });
            canvas = canvas.child(
                self.control_button(
                    format!("team-graph-{scope}-{id}"),
                    "",
                    cx,
                    move |this, _, cx| {
                        if let Some(team) = &team {
                            this.edit_control_record(ControlRecord::Team(team.clone()), cx);
                        }
                        this.control_ui.team_graph_selected = Some(id);
                        cx.notify();
                    },
                )
                .absolute()
                .left(px(node.left))
                .top(px(node.top))
                .w(px(geometry::NODE_WIDTH))
                .h(px(geometry::NODE_HEIGHT))
                .rounded(px(14.0))
                .pl(px(12.0))
                .pr(px(14.0))
                .gap(px(12.0))
                .bg(theme.ticket)
                .border_color(theme.ticket_border)
                .shadow(crate::ui::elevation(&theme, crate::ui::Elevation::Rest))
                .when(selected, |style| style.border_color(theme.accent))
                .child(avatar)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(name),
                                )
                                .when(lead, |row| {
                                    row.child(
                                        div()
                                            .flex_none()
                                            .h(px(18.0))
                                            .px(px(6.0))
                                            .rounded_full()
                                            .bg(theme.accent_soft)
                                            .flex()
                                            .items_center()
                                            .text_size(sp(10.5))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.accent)
                                            .child("Lead"),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(sp(12.0))
                                .text_color(theme.text_secondary)
                                .child(detail),
                        ),
                ),
            );
        }
        div()
            .id(SharedString::from(format!("team-graph-scroll-{scope}")))
            .w_full()
            .min_w_0()
            .overflow_x_scroll()
            .py(px(18.0))
            .child(canvas)
            .into_any_element()
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
