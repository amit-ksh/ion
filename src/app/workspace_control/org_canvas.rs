//! Every team's reporting tree on one canvas, after Paperclip's org chart:
//! drag or scroll to move in any direction, Ctrl+scroll or the corner
//! controls to zoom, and a "+" under each agent to grow the tree.
use super::team_graph::Graph;
use super::team_graph_layout::{self as geometry, Layout, Node};
use super::*;
use gpui::{Bounds, Point, Size, canvas, point};
use std::cell::Cell;

const PAD: f32 = 40.0;
const CLUSTER_GAP: f32 = 88.0;
const ROW_GAP: f32 = 96.0;
const CHIP_W: f32 = 320.0;
const CHIP_H: f32 = 92.0;
const CHIP_GAP: f32 = 44.0;
const LOOSE_COLUMNS: usize = 4;
const LOOSE_GAP_X: f32 = 32.0;
const LOOSE_GAP_Y: f32 = 28.0;
const MIN_ZOOM: f32 = 0.4;
const MAX_ZOOM: f32 = 1.6;
const DOT_SPACING: f32 = 24.0;
/// Room kept clear on the right for the zoom controls.
const CONTROLS_W: f32 = 56.0;

pub(super) struct View {
    /// Top-left of the chart in viewport pixels; `None` until the user moves it.
    pan: Option<Point<f32>>,
    zoom: f32,
    /// Width the clusters wrap within, in chart units; fit sets it so the
    /// chart takes the viewport's shape. `None` wraps to the viewport.
    wrap: Option<f32>,
    drag: Option<(Point<Pixels>, Point<f32>)>,
    /// What the last frame drew, so input handlers move from what is on screen.
    drawn: Cell<Drawn>,
}
#[derive(Clone, Copy, Default)]
struct Drawn {
    pan: Point<f32>,
    viewport: Size<f32>,
}
impl Default for View {
    fn default() -> Self {
        Self {
            pan: None,
            zoom: 1.0,
            wrap: None,
            drag: None,
            drawn: Cell::new(Drawn::default()),
        }
    }
}

/// A subagent created from the chart: once `agent` saves, it joins `team`
/// under `parent`, or starts a new team led by `parent` when `team` is `None`.
pub(super) struct PendingReport {
    pub(super) agent: Uuid,
    pub(super) team: Option<Uuid>,
    pub(super) parent: Uuid,
}

/// One team's tree, or the agents on no team, placed on the canvas.
struct Cluster {
    graph: Graph,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    tree_x: f32,
}
impl Cluster {
    fn new(graph: Graph) -> Self {
        let tree_w = if graph.error.is_some() {
            geometry::NODE_WIDTH
        } else {
            graph.layout.width
        };
        let tree_h = if graph.error.is_some() {
            geometry::NODE_HEIGHT
        } else {
            graph.layout.height
        };
        let width = tree_w.max(CHIP_W);
        Self {
            x: 0.0,
            y: 0.0,
            width,
            height: CHIP_H + CHIP_GAP + tree_h,
            tree_x: (width - tree_w) / 2.0,
            graph,
        }
    }
}

/// Unattached agents sit in a grid; they have no reporting lines to draw.
fn loose_layout(count: usize) -> Layout {
    let columns = count.clamp(1, LOOSE_COLUMNS);
    let rows = count.div_ceil(columns);
    Layout {
        nodes: (0..count)
            .map(|index| Node {
                index,
                left: (index % columns) as f32 * (geometry::NODE_WIDTH + LOOSE_GAP_X),
                top: (index / columns) as f32 * (geometry::NODE_HEIGHT + LOOSE_GAP_Y),
            })
            .collect(),
        lines: Vec::new(),
        width: columns as f32 * geometry::NODE_WIDTH + (columns - 1) as f32 * LOOSE_GAP_X,
        height: rows as f32 * geometry::NODE_HEIGHT + rows.saturating_sub(1) as f32 * LOOSE_GAP_Y,
    }
}

/// Places clusters left to right, wrapping to a new row past `max_width`.
fn arrange(clusters: &mut [Cluster], max_width: f32) -> (f32, f32) {
    let (mut x, mut y, mut row_h, mut width) = (PAD, PAD, 0.0f32, 0.0f32);
    for cluster in clusters.iter_mut() {
        if x > PAD && x + cluster.width > max_width - PAD {
            x = PAD;
            y += row_h + ROW_GAP;
            row_h = 0.0;
        }
        cluster.x = x;
        cluster.y = y;
        x += cluster.width + CLUSTER_GAP;
        row_h = row_h.max(cluster.height);
        width = width.max(cluster.x + cluster.width);
    }
    (width + PAD, y + row_h + PAD)
}

impl Waku {
    fn org_clusters(&self, max_width: f32) -> (Vec<Cluster>, f32, f32) {
        let records = &self.control_ui.snapshot.records;
        let profiles: HashMap<Uuid, &AgentProfile> = records
            .iter()
            .filter_map(|r| match r {
                ControlRecord::Agent(agent) => Some((agent.id, agent)),
                _ => None,
            })
            .collect();
        let mut placed = HashSet::new();
        let mut clusters: Vec<Cluster> = records
            .iter()
            .filter_map(|r| match r {
                ControlRecord::Team(team) => Some(team),
                _ => None,
            })
            .map(|team| {
                placed.extend(team.members.iter().map(|m| m.agent_id));
                Cluster::new(Graph::new(Some(team.clone()), &team.members, &profiles))
            })
            .collect();
        let loose: Vec<TeamMember> = records
            .iter()
            .filter_map(|r| match r {
                ControlRecord::Agent(agent) if !placed.contains(&agent.id) => Some(TeamMember {
                    agent_id: agent.id,
                    parent_id: None,
                }),
                _ => None,
            })
            .collect();
        if !loose.is_empty() {
            let mut graph = Graph::new(None, &loose, &profiles);
            graph.layout = loose_layout(loose.len());
            graph.error = None;
            clusters.push(Cluster::new(graph));
        }
        let (width, height) = arrange(&mut clusters, max_width);
        (clusters, width, height)
    }

    /// The canvas's on-screen size, from the window minus the chrome around it.
    fn org_viewport(&self, window: &Window) -> Size<f32> {
        let size = window.viewport_size();
        let form = if self.control_ui.form.is_some() {
            444.0
        } else {
            0.0
        };
        Size {
            width: (f32::from(size.width)
                - self.sidebar_rendered_width
                - self.right_panel_rendered_width
                - 56.0
                - form)
                .max(320.0),
            height: (f32::from(size.height) - 170.0).max(240.0),
        }
    }

    fn org_zoom_by(&mut self, factor: f32, cx: &mut Context<Self>) {
        let drawn = self.control_ui.org.drawn.get();
        let zoom = self.control_ui.org.zoom;
        let next = (zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        // Keep the viewport's center fixed while the scale changes.
        let center = point(drawn.viewport.width / 2.0, drawn.viewport.height / 2.0);
        let scale = next / zoom;
        self.control_ui.org.pan = Some(point(
            center.x - (center.x - drawn.pan.x) * scale,
            center.y - (center.y - drawn.pan.y) * scale,
        ));
        self.control_ui.org.zoom = next;
        cx.notify();
    }

    /// Fits the whole chart on screen at the largest zoom that works,
    /// re-wrapping clusters to the viewport's shape on the way down.
    fn org_fit(&mut self, cx: &mut Context<Self>) {
        let viewport = self.control_ui.org.drawn.get().viewport;
        if viewport.width <= 0.0 || viewport.height <= 0.0 {
            return;
        }
        let usable = viewport.width - CONTROLS_W;
        let mut zoom = 1.0f32;
        let (wrap, width, height) = loop {
            let wrap = usable / zoom;
            let (_, width, height) = self.org_clusters(wrap);
            if (width * zoom <= usable && height * zoom <= viewport.height) || zoom <= MIN_ZOOM {
                break (wrap, width, height);
            }
            zoom = (zoom - 0.05).max(MIN_ZOOM);
        };
        self.control_ui.org.zoom = zoom;
        self.control_ui.org.wrap = Some(wrap);
        self.control_ui.org.pan = Some(point(
            ((usable - width * zoom) / 2.0).max(0.0),
            ((viewport.height - height * zoom) / 2.0).max(0.0),
        ));
        cx.notify();
    }

    /// Opens the agent editor for a new agent that will report to `parent`.
    fn begin_subagent(
        &mut self,
        team: Option<Uuid>,
        parent: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let template = self
            .control_ui
            .snapshot
            .records
            .iter()
            .find_map(|r| match r {
                ControlRecord::Agent(agent) if agent.id == parent => Some(agent.clone()),
                _ => None,
            });
        self.begin_control_form(FormKind::Agent, cx);
        // A subagent starts on its manager's harness and environment.
        if let Some(manager) = template {
            self.control_ui.provider = manager.provider;
            self.control_ui.execution = manager.execution.clone();
            self.control_ui.access = manager.access.clone();
            self.control_ui.model.update(cx, |input, cx| {
                input.set_content(manager.model.clone().unwrap_or_default(), cx)
            });
            self.request_provider_model_discovery(manager.provider);
        }
        self.control_ui.pending_report = Some(PendingReport {
            agent: Uuid::new_v4(),
            team,
            parent,
        });
        self.control_ui.name.read(cx).focus().focus(window, cx);
        cx.notify();
    }

    /// Saves `agent` into `team` under `parent`; without a team, `parent`
    /// becomes the lead of a new team with `agent` reporting to it.
    pub(super) fn attach_report(
        &mut self,
        team: Option<Uuid>,
        parent: Uuid,
        agent: Uuid,
        cx: &mut Context<Self>,
    ) {
        let records = &self.control_ui.snapshot.records;
        let name_of = |id: Uuid| {
            records.iter().find_map(|r| match r {
                ControlRecord::Agent(a) if a.id == id => Some(a.name.clone()),
                _ => None,
            })
        };
        let (Some(parent_name), Some(agent_name)) = (name_of(parent), name_of(agent)) else {
            return;
        };
        let existing = team.and_then(|id| {
            records.iter().find_map(|r| match r {
                ControlRecord::Team(t) if t.id == id => Some(t.clone()),
                _ => None,
            })
        });
        let record = match existing {
            Some(mut team) => {
                if team.members.iter().any(|m| m.agent_id == agent) {
                    return;
                }
                if team.members.len() >= 32 {
                    self.show_toast("A team can contain at most 32 agents.");
                    cx.notify();
                    return;
                }
                team.members.push(TeamMember {
                    agent_id: agent,
                    parent_id: Some(parent),
                });
                self.show_success_toast(format!("{agent_name} now reports to {parent_name}"));
                ControlRecord::Team(team)
            }
            None => {
                self.show_success_toast(format!("Started {parent_name}'s team with {agent_name}"));
                ControlRecord::Team(Team {
                    id: Uuid::new_v4(),
                    name: format!("{parent_name}'s team"),
                    objective: format!("Work led by {parent_name}."),
                    limits: Some(waku_client::teams::TeamRunLimits::default()),
                    members: vec![
                        TeamMember {
                            agent_id: parent,
                            parent_id: None,
                        },
                        TeamMember {
                            agent_id: agent,
                            parent_id: Some(parent),
                        },
                    ],
                })
            }
        };
        self.control_request(
            Rpc::SaveWorkspaceRecord {
                expected_revision: self.control_ui.snapshot.revision,
                record,
            },
            cx,
        );
    }

    /// After a subagent saves, attach it where it was created.
    pub(super) fn attach_pending_report(&mut self, cx: &mut Context<Self>) {
        let Some(pending) = self.control_ui.pending_report.take() else {
            return;
        };
        let saved = self
            .control_ui
            .snapshot
            .records
            .iter()
            .any(|r| matches!(r, ControlRecord::Agent(a) if a.id == pending.agent));
        if saved {
            self.attach_report(pending.team, pending.parent, pending.agent, cx);
        }
    }

    fn detach_report(&mut self, team: Uuid, agent: Uuid, cx: &mut Context<Self>) {
        let Some(mut team) = self
            .control_ui
            .snapshot
            .records
            .iter()
            .find_map(|r| match r {
                ControlRecord::Team(t) if t.id == team => Some(t.clone()),
                _ => None,
            })
        else {
            return;
        };
        let mut members: Vec<_> = team
            .members
            .iter()
            .map(|m| (m.agent_id, m.parent_id))
            .collect();
        team_hierarchy::remove(&mut members, agent);
        team.members = members
            .into_iter()
            .map(|(agent_id, parent_id)| TeamMember {
                agent_id,
                parent_id,
            })
            .collect();
        self.control_request(
            Rpc::SaveWorkspaceRecord {
                expected_revision: self.control_ui.snapshot.revision,
                record: ControlRecord::Team(team),
            },
            cx,
        );
    }

    pub(super) fn render_org_canvas(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::workspace(cx);
        let viewport = self.org_viewport(window);
        let view = &self.control_ui.org;
        let z = view.zoom;
        let wrap = view.wrap.unwrap_or(viewport.width - CONTROLS_W);
        let (clusters, width, height) = self.org_clusters(wrap);
        let pan = view
            .pan
            .unwrap_or_else(|| point(((viewport.width - width * z) / 2.0).max(0.0), 0.0));
        view.drawn.set(Drawn { pan, viewport });
        let mut layer = div()
            .absolute()
            .left(px(pan.x))
            .top(px(pan.y))
            .w(px(width * z))
            .h(px(height * z));
        for cluster in &clusters {
            layer = self.render_org_cluster(layer, cluster, z, &theme, cx);
        }
        let grid_color = theme.border_strong.opacity(0.55);
        let dots = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let spacing = DOT_SPACING * z.max(0.6);
                let offset_x = pan.x.rem_euclid(spacing);
                let offset_y = pan.y.rem_euclid(spacing);
                let size = gpui::size(px(1.6), px(1.6));
                let mut y = offset_y;
                while y < f32::from(bounds.size.height) {
                    let mut x = offset_x;
                    while x < f32::from(bounds.size.width) {
                        window.paint_quad(
                            gpui::fill(
                                Bounds::new(bounds.origin + point(px(x), px(y)), size),
                                grid_color,
                            )
                            .corner_radii(px(0.8)),
                        );
                        x += spacing;
                    }
                    y += spacing;
                }
            },
        )
        .absolute()
        .size_full();
        let dragging = view.drag.is_some();
        div()
            .id("org-canvas")
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .rounded(px(16.0))
            .border_1()
            .border_color(theme.border)
            .bg(theme.canvas)
            .overflow_hidden()
            .cursor(if dragging {
                gpui::CursorStyle::ClosedHand
            } else {
                gpui::CursorStyle::OpenHand
            })
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                    let pan = this.control_ui.org.drawn.get().pan;
                    this.control_ui.org.drag = Some((event.position, pan));
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                let Some((start, origin)) = this.control_ui.org.drag else {
                    return;
                };
                if event.pressed_button != Some(gpui::MouseButton::Left) {
                    this.control_ui.org.drag = None;
                } else {
                    let moved = event.position - start;
                    this.control_ui.org.pan = Some(point(
                        origin.x + f32::from(moved.x),
                        origin.y + f32::from(moved.y),
                    ));
                }
                cx.notify();
            }))
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, _: &gpui::MouseUpEvent, _, cx| {
                    this.control_ui.org.drag = None;
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                gpui::MouseButton::Left,
                cx.listener(|this, _: &gpui::MouseUpEvent, _, cx| {
                    this.control_ui.org.drag = None;
                    cx.notify();
                }),
            )
            .on_scroll_wheel(cx.listener(|this, event: &gpui::ScrollWheelEvent, _, cx| {
                let delta = event.delta.pixel_delta(px(24.0));
                let (dx, dy) = (f32::from(delta.x), f32::from(delta.y));
                if event.modifiers.control || event.modifiers.platform {
                    this.org_zoom_by(if dy > 0.0 { 1.1 } else { 1.0 / 1.1 }, cx);
                } else {
                    // Shift turns a vertical wheel into a sideways move.
                    let (dx, dy) = if event.modifiers.shift && dx == 0.0 {
                        (dy, 0.0)
                    } else {
                        (dx, dy)
                    };
                    let pan = this.control_ui.org.drawn.get().pan;
                    this.control_ui.org.pan = Some(point(pan.x + dx, pan.y + dy));
                    cx.notify();
                }
                cx.stop_propagation();
            }))
            .child(dots)
            .child(layer)
            .child(self.render_org_controls(z, &theme, cx))
            .child(
                div()
                    .absolute()
                    .left(px(12.0))
                    .bottom(px(12.0))
                    .px(px(12.0))
                    .h(px(30.0))
                    .rounded_full()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.raised)
                    .shadow(crate::ui::elevation(&theme, crate::ui::Elevation::Rest))
                    .flex()
                    .items_center()
                    .gap(px(14.0))
                    .text_size(sp(11.5))
                    .text_color(theme.text_tertiary)
                    .child("Drag or scroll to move · Ctrl + scroll to zoom")
                    .child(legend_dot("Ready", theme.success))
                    .child(legend_dot("Harness missing", theme.warning)),
            )
            .into_any_element()
    }

    fn render_org_controls(&self, z: f32, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let theme = *theme;
        let button = |id: &'static str, path: &'static str, tip: &'static str| {
            div()
                .id(id)
                .size(px(30.0))
                .rounded(px(8.0))
                .flex()
                .items_center()
                .justify_center()
                .cursor_default()
                .hover(move |style| style.bg(theme.overlay_strong))
                .tooltip(Tooltip::text(tip))
                .child(icon(path, 14.0, theme.text_secondary))
        };
        // One floating panel, so the controls read as a toolbar over the chart.
        div()
            .absolute()
            .top(px(12.0))
            .right(px(12.0))
            .p(px(4.0))
            .rounded(px(12.0))
            .border_1()
            .border_color(theme.border)
            .bg(theme.raised)
            .shadow(crate::ui::elevation(&theme, crate::ui::Elevation::Float))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(2.0))
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                button("org-zoom-in", "icons/plus.svg", "Zoom in")
                    .on_click(cx.listener(|this, _, _, cx| this.org_zoom_by(1.2, cx))),
            )
            .child(
                button("org-zoom-out", "icons/minus.svg", "Zoom out")
                    .on_click(cx.listener(|this, _, _, cx| this.org_zoom_by(1.0 / 1.2, cx))),
            )
            .child(
                button("org-fit", "icons/target.svg", "Fit to screen")
                    .on_click(cx.listener(|this, _, _, cx| this.org_fit(cx))),
            )
            .child(
                div()
                    .pt(px(2.0))
                    .pb(px(4.0))
                    .border_t_1()
                    .border_color(theme.border)
                    .w_full()
                    .text_center()
                    .font_family(crate::md::render::MONO_FAMILY)
                    .text_size(sp(10.5))
                    .text_color(theme.text_tertiary)
                    .child(format!("{}%", (z * 100.0).round())),
            )
    }

    fn render_org_cluster(
        &self,
        mut layer: Div,
        cluster: &Cluster,
        z: f32,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = *theme;
        let graph = &cluster.graph;
        let tree_top = cluster.y + CHIP_H + CHIP_GAP;
        let tree_left = cluster.x + cluster.tree_x;
        let chip_x = cluster.x + (cluster.width - CHIP_W) / 2.0;
        layer = layer.child(
            div()
                .absolute()
                .left(px(chip_x * z))
                .top(px(cluster.y * z))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(self.render_org_chip(graph, z, &theme, cx)),
        );
        if graph.team.is_some() && graph.error.is_none() {
            // The team chip hangs over its lead.
            let lead = graph
                .layout
                .nodes
                .iter()
                .find(|n| graph.members[n.index].parent_id.is_none());
            if let Some(lead) = lead {
                let x = tree_left + lead.left + geometry::NODE_WIDTH / 2.0;
                layer = layer.child(
                    div()
                        .absolute()
                        .left(px(x * z - 0.75))
                        .top(px((cluster.y + CHIP_H) * z))
                        .w(px(1.5))
                        .h(px(CHIP_GAP * z))
                        .bg(theme.border_strong),
                );
            }
        }
        if let Some(error) = &graph.error {
            return layer.child(
                div()
                    .absolute()
                    .left(px(tree_left * z))
                    .top(px(tree_top * z))
                    .w(px(geometry::NODE_WIDTH * z))
                    .text_size(sp(12.5 * z))
                    .text_color(theme.danger)
                    .child(error.clone()),
            );
        }
        for line in &graph.layout.lines {
            layer = layer.child(
                div()
                    .absolute()
                    .left(px((tree_left + line.left) * z - 0.25))
                    .top(px((tree_top + line.top) * z))
                    .w(px((line.width * z).max(1.5)))
                    .h(px((line.height * z).max(1.5)))
                    .bg(theme.border_strong),
            );
        }
        let team_id = graph.team.as_ref().map(|t| t.id);
        let scope = team_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "loose".into());
        let editing = (self.control_ui.form == Some(FormKind::Agent))
            .then_some(self.control_ui.edit_id)
            .flatten();
        for node in &graph.layout.nodes {
            let member = &graph.members[node.index];
            let agent = graph.agents[node.index].as_ref();
            let id = member.agent_id;
            let left = (tree_left + node.left) * z;
            let top = (tree_top + node.top) * z;
            let card = self
                .render_team_node(&scope, member, agent, editing == Some(id), z, cx)
                .when_some(agent.cloned(), |card, agent| {
                    card.on_activation(cx, move |this, window, cx| {
                        this.edit_control_record(ControlRecord::Agent(agent.clone()), cx);
                        this.control_ui.name.read(cx).focus().focus(window, cx);
                    })
                });
            layer = layer.child(
                div()
                    .absolute()
                    .left(px(left))
                    .top(px(top))
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(card),
            );
            if agent.is_some() {
                let add = 24.0 * z;
                layer = layer.child(
                    div()
                        .absolute()
                        .left(px(left + geometry::NODE_WIDTH * z / 2.0 - add / 2.0))
                        .top(px(top + geometry::NODE_HEIGHT * z - add / 2.0))
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(self.render_org_add(&scope, team_id, member, add, &theme, cx)),
                );
            }
        }
        layer
    }

    /// The team's header on the canvas: who it is and what it can do.
    fn render_org_chip(
        &self,
        graph: &Graph,
        z: f32,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = *theme;
        let (id, title, summary, icon_path) = match &graph.team {
            Some(team) => (
                team.id.to_string(),
                team.name.clone(),
                match team.members.len().saturating_sub(1) {
                    0 => "Lead only".to_owned(),
                    1 => "Lead + 1 agent".to_owned(),
                    n => format!("Lead + {n} agents"),
                },
                "icons/fork.svg",
            ),
            None => (
                "loose".to_owned(),
                "Not on a team".to_owned(),
                match graph.members.len() {
                    1 => "1 agent".to_owned(),
                    n => format!("{n} agents"),
                },
                "icons/bot.svg",
            ),
        };
        // Labelled pills, as on Android: icon-only buttons here read as two
        // near-identical pencils.
        let mini = |key: String, path: &'static str, label: &'static str| {
            div()
                .id(SharedString::from(key))
                .flex_none()
                .h(px(26.0 * z))
                .pl(px(8.0 * z))
                .pr(px(10.0 * z))
                .rounded_full()
                .border_1()
                .border_color(theme.border)
                .bg(theme.overlay)
                .flex()
                .items_center()
                .gap(px(5.0 * z))
                .cursor_default()
                .hover(move |style| style.bg(theme.overlay_strong))
                .child(icon(path, 12.0 * z, theme.text_secondary))
                .child(
                    div()
                        .text_size(sp(11.5 * z))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_secondary)
                        .child(label),
                )
        };
        let mut actions = div().flex().items_center().gap(px(6.0 * z));
        match graph.team.clone() {
            Some(team) => {
                let team_id = team.id;
                let task = ControlRecord::Team(team.clone());
                let edit = ControlRecord::Team(team);
                actions = actions
                    .child(
                        mini(
                            format!("org-task-{team_id}"),
                            "icons/compose.svg",
                            "New task",
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.prepare_record_task(task.clone(), cx)
                        })),
                    )
                    .child(
                        mini(format!("org-runs-{team_id}"), "icons/list.svg", "Runs").on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.control_ui.runs_team = Some(team_id);
                                this.control_ui.runs_tab = true;
                                this.clear_team_recovery();
                                this.control_ui.selected_run = None;
                                this.control_ui.team_run = None;
                                this.control_ui.runs_after = None;
                                this.control_ui.runs_previous.clear();
                                this.refresh_team_activity(true, cx);
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        mini(format!("org-edit-{team_id}"), "icons/pencil.svg", "Edit").on_click(
                            cx.listener(move |this, _, window, cx| {
                                this.edit_control_record(edit.clone(), cx);
                                this.control_ui.name.read(cx).focus().focus(window, cx);
                            }),
                        ),
                    );
            }
            None => {
                actions = actions.child(
                    mini("org-new-team".to_owned(), "icons/plus.svg", "New team").on_click(
                        cx.listener(|this, _, window, cx| {
                            this.begin_control_form(FormKind::Team, cx);
                            this.control_ui.name.read(cx).focus().focus(window, cx);
                        }),
                    ),
                );
            }
        }
        div()
            .id(SharedString::from(format!("org-chip-{id}")))
            .w(px(CHIP_W * z))
            .h(px(CHIP_H * z))
            .px(px(12.0 * z))
            .py(px(10.0 * z))
            .rounded(px(14.0 * z))
            .border_1()
            .border_color(theme.ticket_border)
            .bg(theme.raised)
            .shadow(crate::ui::elevation(&theme, crate::ui::Elevation::Lifted))
            .flex()
            .flex_col()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0 * z))
                    .child(
                        avatar_tile(34.0 * z, theme.accent_soft, gpui::transparent_black())
                            .child(icon(icon_path, 15.0 * z, theme.accent)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.0 * z))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(sp(13.5 * z))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text)
                                    .child(title),
                            )
                            .child(
                                div()
                                    .font_family(crate::md::render::MONO_FAMILY)
                                    .text_size(sp(10.5 * z))
                                    .text_color(theme.text_tertiary)
                                    .child(summary),
                            ),
                    ),
            )
            .child(actions)
    }

    /// The "+" under an agent: create a subagent, attach an existing agent,
    /// edit it, or take it off the team.
    fn render_org_add(
        &self,
        scope: &str,
        team: Option<Uuid>,
        member: &TeamMember,
        size: f32,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = *theme;
        let parent = member.agent_id;
        let lead = member.parent_id.is_none();
        let records = &self.control_ui.snapshot.records;
        let in_team: HashSet<Uuid> = team
            .and_then(|id| {
                records.iter().find_map(|r| match r {
                    ControlRecord::Team(t) if t.id == id => {
                        Some(t.members.iter().map(|m| m.agent_id).collect())
                    }
                    _ => None,
                })
            })
            .unwrap_or_else(|| HashSet::from([parent]));
        let candidates: Vec<(Uuid, String)> = records
            .iter()
            .filter_map(|r| match r {
                ControlRecord::Agent(a) if !in_team.contains(&a.id) => Some((a.id, a.name.clone())),
                _ => None,
            })
            .collect();
        let agent = records.iter().find_map(|r| match r {
            ControlRecord::Agent(a) if a.id == parent => Some(a.clone()),
            _ => None,
        });
        let header = agent.as_ref().map(|a| a.name.clone()).unwrap_or_default();
        let menu_id = SharedString::from(format!("org-add-menu-{scope}-{parent}"));
        let menu = self.menu_handle(menu_id.clone(), cx);
        let weak = cx.entity().downgrade();
        let trigger = div()
            .id(SharedString::from(format!("org-add-{scope}-{parent}")))
            .size(px(size))
            .rounded_full()
            .border_1()
            .border_color(theme.border_strong)
            .bg(theme.raised)
            .shadow(crate::ui::elevation(&theme, crate::ui::Elevation::Rest))
            .flex()
            .items_center()
            .justify_center()
            .cursor_default()
            .hover(move |style| style.bg(theme.primary).border_color(theme.primary))
            .tooltip(Tooltip::text("Add a report"))
            .child(icon("icons/plus.svg", size * 0.5, theme.text_secondary));
        dropdown_menu(trigger, menu_id, &menu, MenuAlign::BelowLeft, move |_| {
            let mut items = vec![MenuItem::Header(header.clone().into())];
            let subagent = weak.clone();
            items.push(MenuItem::new("New subagent", move |window, cx| {
                let _ =
                    subagent.update(cx, |this, cx| this.begin_subagent(team, parent, window, cx));
            }));
            if !candidates.is_empty() {
                let attach = weak.clone();
                let candidates = candidates.clone();
                items.push(MenuItem::Submenu {
                    label: "Add existing agent".into(),
                    value: None,
                    items: Rc::new(move |_| {
                        candidates
                            .iter()
                            .map(|(id, name)| {
                                let id = *id;
                                let attach = attach.clone();
                                MenuItem::new(name.clone(), move |_, cx| {
                                    let _ = attach.update(cx, |this, cx| {
                                        this.attach_report(team, parent, id, cx)
                                    });
                                })
                            })
                            .collect()
                    }),
                });
            }
            if let Some(agent) = agent.clone() {
                let edit = weak.clone();
                items.push(MenuItem::new("Edit agent", move |window, cx| {
                    let _ = edit.update(cx, |this, cx| {
                        this.edit_control_record(ControlRecord::Agent(agent.clone()), cx);
                        this.control_ui.name.read(cx).focus().focus(window, cx);
                    });
                }));
            }
            if let Some(team) = team.filter(|_| !lead) {
                let detach = weak.clone();
                items.push(MenuItem::Separator);
                items.push(MenuItem::new("Remove from team", move |_, cx| {
                    let _ = detach.update(cx, |this, cx| this.detach_report(team, parent, cx));
                }));
            }
            items
        })
    }
}

fn legend_dot(label: &'static str, color: Hsla) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(div().size(px(8.0)).rounded_full().bg(color))
        .child(label)
}
