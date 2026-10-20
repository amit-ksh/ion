//! Document library. Filters prepare one cached collection; only visible card
//! rows render. Document reads and exports use the existing background workers.
use super::*;
use std::hash::{Hash, Hasher};

pub(super) struct ArtifactsUi {
    search: Entity<TextInput>,
    project: Option<Uuid>,
    task: Option<Uuid>,
    format: Option<String>,
    key: u64,
    rows: Rc<Vec<Vec<Card>>>,
    list: ListState,
    scrollbar: Rc<ScrollbarState>,
    count: usize,
}
#[derive(Clone)]
struct Card {
    artifact: Artifact,
    project: String,
    task: String,
}
impl ArtifactsUi {
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        Self {
            search: cx.new(|cx| TextInput::new(window, cx).placeholder("Search documents")),
            project: None,
            task: None,
            format: None,
            key: 0,
            rows: Rc::new(vec![]),
            list: ListState::new(0, ListAlignment::Top, px(250.0)),
            scrollbar: ScrollbarState::new(),
            count: 0,
        }
    }
}
impl ControlUi {
    pub(in crate::app) fn artifact_search(&self) -> Entity<TextInput> {
        self.artifacts.search.clone()
    }
    pub(in crate::app) fn invalidate_artifact_labels(&mut self) {
        self.artifacts.key = 0;
    }
}
impl Waku {
    fn artifact_filter(&self, which: usize, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::workspace(cx);
        let ui = &self.control_ui.artifacts;
        let label = match which {
            0 => ui
                .project
                .and_then(|id| self.state.projects.iter().find(|p| p.id == id))
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "All projects".into()),
            1 => ui
                .task
                .and_then(|id| self.state.sessions.iter().find(|s| s.id == id))
                .map(|s| s.title.clone())
                .unwrap_or_else(|| "All tasks".into()),
            _ => ui
                .format
                .as_ref()
                .map(|f| f.to_uppercase())
                .unwrap_or_else(|| "All formats".into()),
        };
        let id = format!("artifact-filter-{which}");
        let menu = self.menu_handle(id.clone(), cx);
        let weak = cx.entity().downgrade();
        dropdown_menu(
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
                let ui = &this.control_ui.artifacts;
                let mut options: Vec<(String, Option<String>, bool)> = vec![(
                    match which {
                        0 => "All projects",
                        1 => "All tasks",
                        _ => "All formats",
                    }
                    .into(),
                    None,
                    match which {
                        0 => ui.project.is_none(),
                        1 => ui.task.is_none(),
                        _ => ui.format.is_none(),
                    },
                )];
                match which {
                    0 => options.extend(this.state.projects.iter().map(|p| {
                        (
                            p.name.clone(),
                            Some(p.id.to_string()),
                            ui.project == Some(p.id),
                        )
                    })),
                    1 => options.extend(
                        this.state
                            .sessions
                            .iter()
                            .filter(|s| ui.project.is_none_or(|id| s.project_id == id))
                            .map(|s| {
                                (
                                    s.title.clone(),
                                    Some(s.id.to_string()),
                                    ui.task == Some(s.id),
                                )
                            }),
                    ),
                    _ => {
                        let mut formats = this
                            .control_ui
                            .snapshot
                            .records
                            .iter()
                            .filter_map(|r| {
                                if let ControlRecord::Artifact(a) = r {
                                    Some(a.format.to_ascii_lowercase())
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>();
                        formats.sort();
                        formats.dedup();
                        options.extend(formats.into_iter().map(|f| {
                            (
                                f.to_uppercase(),
                                Some(f.clone()),
                                ui.format.as_ref() == Some(&f),
                            )
                        }));
                    }
                }
                options
                    .into_iter()
                    .map(|(label, value, selected)| {
                        let weak = weak.clone();
                        MenuItem::new(label, move |_, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                let ui = &mut this.control_ui.artifacts;
                                match which {
                                    0 => {
                                        ui.project =
                                            value.as_deref().and_then(|v| Uuid::parse_str(v).ok());
                                        ui.task = None;
                                    }
                                    1 => {
                                        ui.task =
                                            value.as_deref().and_then(|v| Uuid::parse_str(v).ok())
                                    }
                                    _ => ui.format = value.clone(),
                                }
                                cx.notify();
                            });
                        })
                        .selected(selected)
                        .into()
                    })
                    .collect()
            },
        )
        .into_any_element()
    }
    pub(super) fn render_artifacts_page(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::workspace(cx);
        let width = f32::from(window.viewport_size().width)
            - self.sidebar_rendered_width
            - self.right_panel_rendered_width
            - 56.0;
        let columns = if width >= 1040.0 {
            3
        } else if width >= 660.0 {
            2
        } else {
            1
        };
        let ui = &self.control_ui.artifacts;
        let query = ui.search.read(cx).content().trim().to_lowercase();
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        (
            self.control_ui.snapshot.revision,
            ui.project,
            ui.task,
            &ui.format,
            &query,
            columns,
        )
            .hash(&mut hash);
        self.state.projects.len().hash(&mut hash);
        self.state.sessions.len().hash(&mut hash);
        let key = hash.finish();
        if key != ui.key {
            let projects = self
                .state
                .projects
                .iter()
                .map(|p| (p.id, p.name.clone()))
                .collect::<HashMap<_, _>>();
            let tasks = self
                .state
                .sessions
                .iter()
                .map(|s| (s.id, s.title.clone()))
                .collect::<HashMap<_, _>>();
            let mut cards = self
                .control_ui
                .snapshot
                .records
                .iter()
                .filter_map(|r| {
                    if let ControlRecord::Artifact(a) = r {
                        Some(a)
                    } else {
                        None
                    }
                })
                .filter(|a| {
                    ui.project.is_none_or(|id| a.project_id == Some(id))
                        && ui.task.is_none_or(|id| a.task_id == Some(id))
                        && ui
                            .format
                            .as_ref()
                            .is_none_or(|f| a.format.eq_ignore_ascii_case(f))
                        && (query.is_empty() || a.name.to_lowercase().contains(&query))
                })
                .map(|a| Card {
                    artifact: a.clone(),
                    project: a
                        .project_id
                        .and_then(|id| projects.get(&id).cloned())
                        .unwrap_or_else(|| "Workspace".into()),
                    task: a
                        .task_id
                        .and_then(|id| tasks.get(&id).cloned())
                        .unwrap_or_else(|| "Agent document".into()),
                })
                .collect::<Vec<_>>();
            cards.sort_by_key(|a| std::cmp::Reverse((a.artifact.created_at, a.artifact.id)));
            let ui = &mut self.control_ui.artifacts;
            ui.count = cards.len();
            ui.rows = Rc::new(cards.chunks(columns).map(|row| row.to_vec()).collect());
            ui.list.reset(ui.rows.len());
            ui.key = key;
        }
        let ui = &self.control_ui.artifacts;
        let mut filters = div().flex().flex_wrap().items_center().gap(px(8.0)).child(
            div().min_w(px(200.0)).max_w(px(340.0)).flex_1().child(
                TextField::new("artifact-search", ui.search.clone())
                    .icon("icons/search.svg", 14.0)
                    .w_full(),
            ),
        );
        for which in 0..3 {
            filters = filters.child(self.artifact_filter(which, cx));
        }
        filters = filters
            .child(
                self.control_button("artifact-clear-filters", "Clear", cx, |this, _, cx| {
                    let ui = &mut this.control_ui.artifacts;
                    ui.project = None;
                    ui.task = None;
                    ui.format = None;
                    ui.search.update(cx, |input, cx| input.clear(cx));
                    cx.notify();
                }),
            )
            .child(
                div()
                    .ml_auto()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(format!("{} documents", ui.count)),
            );
        let weak = cx.entity().downgrade();
        let rows = ui.rows.clone();
        let content = if ui.count == 0 {
            div()
                .w_full()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(12.0))
                .text_color(theme.text_secondary)
                .child(icon("icons/file.svg", 32.0, theme.text_tertiary))
                .child(
                    if query.is_empty()
                        && ui.project.is_none()
                        && ui.task.is_none()
                        && ui.format.is_none()
                    {
                        "Documents created by your agents will appear here."
                    } else {
                        "No documents match these filters."
                    },
                )
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_h_0()
                .relative()
                .child(
                    list(ui.list.clone(), move |index, _, cx| {
                        weak.upgrade()
                            .map(|entity| {
                                entity.update(cx, |this, cx| {
                                    let mut row = div().flex().gap(px(16.0)).pb(px(16.0));
                                    for card in &rows[index] {
                                        row = row.child(this.render_artifact_card(card, cx));
                                    }
                                    for _ in rows[index].len()..columns {
                                        row = row.child(div().flex_1().min_w_0());
                                    }
                                    row.into_any_element()
                                })
                            })
                            .unwrap_or_else(|| div().into_any_element())
                    })
                    .size_full(),
                )
                .child(scrollbar::vertical(&ui.list, &ui.scrollbar))
                .into_any_element()
        };
        div()
            .id("artifact-library")
            .track_focus(&self.control_ui.focus)
            .tab_group()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .px(px(28.0))
            .py(px(16.0))
            .child(
                div()
                    .w_full()
                    .max_w(px(1280.0))
                    .mx_auto()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .gap(px(20.0))
                    .child(self.render_workspace_header(Page::Artifacts, cx))
                    .child(filters)
                    .child(content),
            )
            .into_any_element()
    }
    fn render_artifact_card(&self, card: &Card, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::workspace(cx);
        let a = &card.artifact;
        let id = a.id;
        let preview = a.clone();
        let export = ControlRecord::Artifact(a.clone());
        let publish = a.clone();
        div()
            .id(SharedString::from(format!("artifact-card-{id}")))
            .flex_1()
            .min_w_0()
            .min_h(px(220.0))
            .p(px(18.0))
            .rounded(px(12.0))
            .border_1()
            .border_color(theme.border_strong)
            .bg(theme.raised)
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(icon("icons/file.svg", 28.0, theme.text_secondary))
                    .child(
                        div()
                            .text_size(sp(11.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text_secondary)
                            .child(a.format.to_uppercase()),
                    ),
            )
            .child(
                div()
                    .text_size(sp(15.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .line_clamp(2)
                    .child(a.name.clone()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(div().truncate().child(card.project.clone()))
                    .child(div().truncate().child(card.task.clone()))
                    .child(
                        a.evidence
                            .as_ref()
                            .map(|e| {
                                format!(
                                    "{} bytes · {}",
                                    e.bytes,
                                    match e.producer.origin {
                                        ArtifactOrigin::TeamAssignment => "Generated by team",
                                        ArtifactOrigin::AgentEdit => "Captured agent edit",
                                        ArtifactOrigin::AgentGenerated => "Generated by agent",
                                    }
                                )
                            })
                            .unwrap_or_else(|| "Legacy document".into()),
                    ),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.0))
                    .when(a.format.eq_ignore_ascii_case("html") && a.evidence.is_some(), |row| row.child(self.control_button(format!("artifact-publish-{id}"), "Publish HTML", cx, move|this,_,cx|this.open_artifact_hosting(publish.clone(),cx))))
                    .child(self.control_button(
                        format!("artifact-open-{id}"),
                        "Open",
                        cx,
                        move |this, window, cx| {
                            this.open_document_preview(preview.clone(), window, cx)
                        },
                    ))
                    .child(
                        self.control_button(
                            format!("artifact-export-{id}"),
                            "Save copy",
                            cx,
                            move |this, _, cx| this.export_control_artifact(export.clone(), cx),
                        )
                        .child(icon(
                            "icons/download.svg",
                            14.0,
                            theme.text_secondary,
                        )),
                    ),
            )
            .into_any_element()
    }
}
