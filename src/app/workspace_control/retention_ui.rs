//! Explicit owner cleanup; render reads bounded, prepared RPC snapshots only.
use super::*;
use waku_client::artifact_retention::*;

pub(super) struct RetentionUi {
    pub open: bool,
    busy: bool,
    generation: u64,
    project: Option<Uuid>,
    task: Option<Uuid>,
    after: Option<Uuid>,
    previous: Vec<Option<Uuid>>,
    next: Option<Uuid>,
    items: Rc<Vec<ArtifactRetentionItem>>,
    selected: Option<ArtifactCleanupRequest>,
    error: Option<String>,
    note: Option<String>,
    focus: FocusHandle,
    scroll: gpui::ScrollHandle,
}
impl RetentionUi {
    pub fn new(cx: &mut App) -> Self {
        Self {
            open: false,
            busy: false,
            generation: 0,
            project: None,
            task: None,
            after: None,
            previous: vec![],
            next: None,
            items: Rc::new(vec![]),
            selected: None,
            error: None,
            note: None,
            focus: cx.focus_handle(),
            scroll: gpui::ScrollHandle::new(),
        }
    }
    pub(super) fn close(&mut self) {
        self.open = false;
        self.generation += 1;
        self.busy = false;
        self.selected = None;
        self.items = Rc::new(vec![]);
        self.error = None;
        self.note = None;
    }
    fn reset_scope(&mut self) {
        self.generation += 1;
        self.busy = false;
        self.after = None;
        self.previous.clear();
        self.next = None;
        self.selected = None;
        self.items = Rc::new(vec![]);
    }
}
impl Waku {
    pub(super) fn open_artifact_retention(&mut self, cx: &mut Context<Self>) {
        let ui = &mut self.control_ui.retention;
        ui.open = true;
        ui.project = self.control_ui.filter_project;
        ui.task = self.control_ui.filter_task;
        ui.reset_scope();
        self.refresh_artifact_retention(cx);
    }
    fn refresh_artifact_retention(&mut self, cx: &mut Context<Self>) {
        let ui = &mut self.control_ui.retention;
        if !ui.open || ui.busy || self.control_ui.page != Some(Page::Artifacts) {
            return;
        }
        ui.generation += 1;
        ui.busy = true;
        ui.error = None;
        ui.selected = None;
        ui.items = Rc::new(vec![]);
        let generation = ui.generation;
        let query = ArtifactRetentionQuery {
            project_id: ui.project,
            task_id: ui.task,
            after: ui.after,
            limit: 32,
        };
        let client = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || -> anyhow::Result<_> {
                        let ResponsePayload::ArtifactRetention { page } = client.request(
                            Uuid::nil(),
                            Uuid::nil(),
                            Rpc::ListArtifactRetention { query },
                        )?
                        else {
                            anyhow::bail!("Unexpected artifact storage response");
                        };
                        anyhow::ensure!(
                            page.items.len() <= 32,
                            "Artifact storage page exceeds its bound"
                        );
                        Ok(page)
                    })
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let ui = &mut this.control_ui.retention;
                if !ui.open
                    || ui.generation != generation
                    || this.control_ui.page != Some(Page::Artifacts)
                {
                    return;
                }
                ui.busy = false;
                match result {
                    Ok(page) => {
                        ui.items = Rc::new(page.items);
                        ui.next = page.next;
                    }
                    Err(e) => {
                        ui.error = Some(format!(
                            "Could not load artifact storage: {e}. Refresh to retry."
                        ))
                    }
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn clean_selected_artifact(&mut self, cx: &mut Context<Self>) {
        let ui = &mut self.control_ui.retention;
        if !ui.open || ui.busy {
            return;
        }
        let Some(request) = ui.selected.clone() else {
            return;
        };
        ui.generation += 1;
        ui.busy = true;
        ui.error = None;
        let generation = ui.generation;
        let client = self.daemon.client();
        let expected = request.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || -> anyhow::Result<_> {
                        let ResponsePayload::ArtifactCleaned { receipt } = client.request(
                            Uuid::nil(),
                            Uuid::nil(),
                            Rpc::CleanupArtifact { request },
                        )?
                        else {
                            anyhow::bail!("Unexpected artifact cleanup response");
                        };
                        anyhow::ensure!(
                            receipt.request_id == expected.request_id
                                && receipt.artifact_id == expected.expected.id,
                            "Cleanup receipt identity changed"
                        );
                        Ok(receipt)
                    })
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let ui = &mut this.control_ui.retention;
                if !ui.open
                    || ui.generation != generation
                    || this.control_ui.page != Some(Page::Artifacts)
                {
                    return;
                }
                ui.busy = false;
                match result {
                    Ok(_) => {
                        ui.note =
                            Some("Document content cleaned. Publication history retained.".into());
                        this.refresh_artifact_retention(cx);
                    }
                    Err(e) => {
                        ui.error = Some(format!(
                            "Cleanup did not finish: {e}. Retry keeps the same cleanup request."
                        ))
                    }
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn retention_filter(&self, task_filter: bool, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let ui = &self.control_ui.retention;
        let key = if task_filter {
            "retention-task"
        } else {
            "retention-project"
        };
        let label = if task_filter {
            ui.task
                .and_then(|id| self.state.sessions.iter().find(|s| s.id == id))
                .map(|s| s.title.clone())
                .unwrap_or_else(|| "All tasks".into())
        } else {
            ui.project
                .and_then(|id| self.state.projects.iter().find(|p| p.id == id))
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "All projects".into())
        };
        let weak = cx.entity().downgrade();
        let menu = self.menu_handle(key, cx);
        dropdown_menu(
            self.control_button(key, label, cx, |_, _, _| {})
                .child(icon("icons/chevron-down.svg", 12.0, theme.text_secondary)),
            key,
            &menu,
            MenuAlign::BelowLeft,
            move |cx| {
                let Some(entity) = weak.upgrade() else {
                    return vec![];
                };
                let this = entity.read(cx);
                let mut options = vec![(
                    if task_filter {
                        "All tasks"
                    } else {
                        "All projects"
                    }
                    .to_string(),
                    None,
                )];
                if task_filter {
                    options.extend(
                        this.state
                            .sessions
                            .iter()
                            .filter(|s| {
                                this.control_ui
                                    .retention
                                    .project
                                    .is_none_or(|p| p == s.project_id)
                            })
                            .map(|s| (s.title.clone(), Some(s.id))),
                    );
                } else {
                    options.extend(
                        this.state
                            .projects
                            .iter()
                            .map(|p| (p.name.clone(), Some(p.id))),
                    );
                }
                options
                    .into_iter()
                    .map(|(label, id)| {
                        let weak = weak.clone();
                        MenuItem::new(label, move |_, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                let ui = &mut this.control_ui.retention;
                                if task_filter {
                                    ui.task = id;
                                } else {
                                    ui.project = id;
                                    ui.task = None;
                                }
                                ui.reset_scope();
                                this.refresh_artifact_retention(cx);
                            });
                        })
                        .selected(if task_filter {
                            this.control_ui.retention.task == id
                        } else {
                            this.control_ui.retention.project == id
                        })
                        .into()
                    })
                    .collect()
            },
        )
        .into_any_element()
    }
    fn retention_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let modifiers = &event.keystroke.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform {
            return;
        }
        if event.keystroke.key == "escape" {
            let ui = &mut self.control_ui.retention;
            if ui.busy {
                return;
            }
            if ui.selected.is_some() {
                ui.selected = None;
            } else {
                ui.close();
            }
            cx.stop_propagation();
            cx.notify();
            return;
        }
        let scroll = &self.control_ui.retention.scroll;
        let offset = scroll.offset();
        let max = scroll.max_offset().y;
        let page = scroll.bounds().size.height * 0.9;
        let next = match event.keystroke.key.as_str() {
            "up" => offset.y + px(40.0),
            "down" => offset.y - px(40.0),
            "pageup" => offset.y + page,
            "pagedown" => offset.y - page,
            "home" => Pixels::ZERO,
            "end" => -max,
            _ => return,
        }
        .clamp(-max, Pixels::ZERO);
        scroll.set_offset(point(offset.x, next));
        cx.stop_propagation();
        cx.notify();
    }
    pub(super) fn render_artifact_retention(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let ui = &self.control_ui.retention;
        let toolbar = div()
            .flex()
            .flex_wrap()
            .gap(px(8.0))
            .child(
                self.control_button("retention-back", "Artifacts", cx, |this, _, cx| {
                    this.control_ui.retention.close();
                    cx.notify();
                }),
            )
            .child(self.retention_filter(false, cx))
            .child(self.retention_filter(true, cx))
            .child(
                self.control_button("retention-refresh", "", cx, |this, _, cx| {
                    this.refresh_artifact_retention(cx)
                })
                .tooltip(Tooltip::text("Refresh artifact storage"))
                .child(icon("icons/rotate-cw.svg", 16.0, theme.text_secondary)),
            );
        let mut body=div().id("artifact-retention-scroll").track_focus(&ui.focus).tab_index(0).tab_stop(true)
            .focus_visible(|s|s.border_1().border_color(theme.accent)).track_scroll(&ui.scroll)
            .flex_1().min_h_0().overflow_y_scroll().flex().flex_col().gap(px(12.0)).p(px(20.0))
            .child(div().text_size(sp(18.0)).font_weight(FontWeight::SEMIBOLD).child("Artifact storage"))
            .child(div().text_size(sp(13.0)).text_color(theme.text_secondary).child("Keep until explicitly cleaned. Review a document before removing its stored content."));
        if let Some(note) = &ui.note {
            body = body.child(div().text_size(sp(13.0)).child(note.clone()));
        }
        if let Some(error) = &ui.error {
            body = body.child(
                div()
                    .text_size(sp(13.0))
                    .text_color(theme.danger)
                    .child(error.clone()),
            );
        }
        if ui.busy {
            body = body.child(div().child("Workingâ€¦"));
        }
        if let Some(request) = &ui.selected {
            let a = &request.expected;
            body = body.child(
                div()
                    .text_size(sp(16.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(a.name.clone()),
            );
            if let Some(e) = &a.evidence {
                body = body
                    .child(div().text_size(sp(13.0)).child(format!(
                        "{} Â· {} bytes",
                        a.format.to_uppercase(),
                        e.bytes
                    )))
                    .child(
                        div()
                            .text_size(sp(12.0))
                            .text_color(theme.text_secondary)
                            .child(format!("SHA-256 {}", e.sha256)),
                    );
            }
            body=body.child(div().text_size(sp(13.0)).child("This removes Ionâ€™s stored document content. Its publication and provenance remain in history. Project source files are kept. Cleaned documents cannot be previewed or passed to later tasks."));
            if !ui.busy {
                body = body.child(
                    div()
                        .flex()
                        .gap(px(8.0))
                        .child(self.control_button(
                            "retention-confirm",
                            "Clean document content",
                            cx,
                            |this, _, cx| this.clean_selected_artifact(cx),
                        ))
                        .child(self.control_button(
                            "retention-cancel",
                            "Cancel",
                            cx,
                            |this, _, cx| {
                                this.control_ui.retention.selected = None;
                                cx.notify();
                            },
                        )),
                );
            }
        } else {
            if !ui.busy && ui.items.is_empty() {
                body = body.child(
                    div()
                        .text_color(theme.text_secondary)
                        .child("No documents match these filters."),
                );
            }
            for item in ui.items.iter() {
                let state = match item.state {
                    ArtifactRetentionState::Retained => "Retained",
                    ArtifactRetentionState::Protected => "Protected",
                    ArtifactRetentionState::CleanupPending => "Cleanup pending",
                    ArtifactRetentionState::Cleaned => "Cleaned",
                };
                let mut row = div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(12.0))
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(160.0))
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .child(div().text_size(sp(14.0)).child(item.artifact.name.clone()))
                            .child(
                                div()
                                    .text_size(sp(12.0))
                                    .text_color(theme.text_secondary)
                                    .child(item.reason.clone().unwrap_or_else(|| {
                                        item.artifact
                                            .evidence
                                            .as_ref()
                                            .map(|e| {
                                                format!(
                                                    "{} Â· {} bytes",
                                                    item.artifact.format.to_uppercase(),
                                                    e.bytes
                                                )
                                            })
                                            .unwrap_or_else(|| {
                                                "Integrity evidence unavailable".into()
                                            })
                                    })),
                            ),
                    )
                    .child(div().text_size(sp(12.0)).child(state));
                if !ui.busy
                    && matches!(
                        item.state,
                        ArtifactRetentionState::Retained | ArtifactRetentionState::CleanupPending
                    )
                {
                    let expected = item.artifact.clone();
                    let cleanup_request = item.cleanup_request;
                    row = row.child(self.control_button(
                        format!("retention-review-{}", expected.id),
                        if item.state == ArtifactRetentionState::CleanupPending {
                            "Review retry"
                        } else {
                            "Review cleanup"
                        },
                        cx,
                        move |this, _, cx| {
                            if this.control_ui.retention.busy {
                                return;
                            }
                            this.control_ui.retention.selected = Some(ArtifactCleanupRequest {
                                request_id: cleanup_request.unwrap_or_else(Uuid::new_v4),
                                expected: expected.clone(),
                            });
                            cx.notify();
                        },
                    ));
                }
                body = body.child(row);
            }
            if !ui.busy {
                let mut pages = div().flex().gap(px(8.0));
                if !ui.previous.is_empty() {
                    pages = pages.child(self.control_button(
                        "retention-previous",
                        "Previous",
                        cx,
                        |this, _, cx| {
                            let ui = &mut this.control_ui.retention;
                            if ui.busy {
                                return;
                            }
                            ui.after = ui.previous.pop().flatten();
                            this.refresh_artifact_retention(cx);
                        },
                    ));
                }
                if let Some(next) = ui.next {
                    pages = pages.child(self.control_button(
                        "retention-next",
                        "Next",
                        cx,
                        move |this, _, cx| {
                            let ui = &mut this.control_ui.retention;
                            if ui.busy {
                                return;
                            }
                            ui.previous.push(ui.after);
                            ui.after = Some(next);
                            this.refresh_artifact_retention(cx);
                        },
                    ));
                }
                body = body.child(pages);
            }
        }
        div()
            .track_focus(&self.control_ui.focus)
            .tab_group()
            .flex()
            .flex_col()
            .size_full()
            .text_color(theme.text)
            .on_key_down(cx.listener(|this, event, _, cx| this.retention_key(event, cx)))
            .child(
                div()
                    .p(px(16.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(toolbar),
            )
            .child(body)
            .into_any_element()
    }
}
