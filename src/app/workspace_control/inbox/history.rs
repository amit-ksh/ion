//! Historical rows are bounded owner projections; actions use current source state.
use super::*;
use waku_client::inbox::{
    InboxEntry, InboxHistoryStatus, InboxHistoryView, InboxKind, InboxPage, InboxQuery, InboxSource,
};
impl Waku {
    fn inbox_history_query(&self) -> InboxQuery {
        let kind = match self.control_ui.filter_kind {
            NoticeKind::All => None,
            NoticeKind::Proposal => Some(InboxKind::Proposal),
            NoticeKind::Decision => Some(InboxKind::Decision),
            NoticeKind::Approval => Some(InboxKind::Approval),
            NoticeKind::Question => Some(InboxKind::Question),
            NoticeKind::Issue => Some(InboxKind::Issue),
            NoticeKind::Completed => Some(InboxKind::Completed),
            NoticeKind::Artifact => Some(InboxKind::Artifact),
        };
        InboxQuery {
            project_id: self.control_ui.filter_project,
            task_id: self.control_ui.filter_task,
            kind,
            view: match self.control_ui.inbox.view {
                InboxView::Inbox => InboxHistoryView::Inbox,
                InboxView::Unread => InboxHistoryView::Unread,
                InboxView::Acknowledged => InboxHistoryView::Acknowledged,
                InboxView::Archived => InboxHistoryView::Archived,
                InboxView::All => InboxHistoryView::All,
            },
            status: match self.control_ui.filter_status {
                NoticeStatus::All => InboxHistoryStatus::All,
                NoticeStatus::Pending => InboxHistoryStatus::NeedsAction,
                NoticeStatus::Resolved => InboxHistoryStatus::Resolved,
            },
            after: self.control_ui.inbox.after,
            limit: 32,
        }
    }
    pub(in crate::app::workspace_control) fn reset_inbox_history(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        self.control_ui.inbox.after = None;
        self.control_ui.inbox.previous.clear();
        self.control_ui.inbox.history = None;
        self.control_ui.inbox.history_generation += 1;
        self.control_ui.inbox.history_busy = false;
        self.refresh_inbox_history(cx);
    }
    pub(super) fn refresh_inbox_history(&mut self, cx: &mut Context<Self>) {
        if self.control_ui.inbox.history_busy {
            return;
        }
        let query = self.inbox_history_query();
        self.control_ui.inbox.history_busy = true;
        self.control_ui.inbox.history_generation += 1;
        let generation = self.control_ui.inbox.history_generation;
        let client = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let requested = query.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || -> anyhow::Result<InboxPage> {
                        let ResponsePayload::InboxHistory { page } = client.request(
                            Uuid::nil(),
                            Uuid::nil(),
                            Rpc::ListInboxHistory { query: requested },
                        )?
                        else {
                            anyhow::bail!("Unexpected Inbox history response")
                        };
                        Ok(page)
                    })
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if generation != this.control_ui.inbox.history_generation {
                    return;
                }
                this.control_ui.inbox.history_busy = false;
                if query != this.inbox_history_query() {
                    this.refresh_inbox_history(cx);
                    return;
                }
                match result {
                    Ok(page) => {
                        this.control_ui.inbox.history_error = None;
                        // Every 3 s poll lands here; an unchanged page must not
                        // rebuild (and so rescroll to the top) every workspace list.
                        if this.control_ui.inbox.history.as_deref() != Some(&page) {
                            this.control_ui.inbox.history = Some(Rc::new(page));
                            this.control_ui.activity_revision += 1;
                        }
                    }
                    Err(error) => {
                        this.control_ui.inbox.history_error = Some(format!(
                            "Could not load Inbox history: {error}. Refresh to retry."
                        ))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(in crate::app::workspace_control) fn inbox_history_rows(&self) -> Vec<Row> {
        self.control_ui
            .inbox
            .history
            .as_ref()
            .map(|p| {
                p.entries
                    .iter()
                    .cloned()
                    .map(|e| Row::InboxEvent(Rc::new(e)))
                    .collect()
            })
            .unwrap_or_default()
    }
    pub(in crate::app::workspace_control) fn inbox_unread_count(&self) -> u64 {
        self.control_ui
            .inbox
            .history
            .as_ref()
            .map(|p| p.unread_count)
            .unwrap_or(0)
    }
    pub(super) fn history_toolbar(&self, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        let mut row = div().flex().flex_wrap().items_center().gap(px(8.0));
        if let Some(page) = &self.control_ui.inbox.history {
            row = row.child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(format!(
                        "{} notifications · {} unread across projects",
                        page.matching_count, page.unread_count
                    )),
            );
            if !self.control_ui.inbox.previous.is_empty() {
                row = row.child(self.control_button(
                    "inbox-history-previous",
                    "Previous",
                    cx,
                    |this, _, cx| {
                        if this.control_ui.inbox.history_busy {
                            return;
                        }
                        if let Some(after) = this.control_ui.inbox.previous.pop() {
                            this.control_ui.inbox.after = after;
                            this.refresh_inbox_history(cx);
                        }
                    },
                ));
            }
            if let Some(next) = page.next_cursor {
                row = row.child(self.control_button(
                    "inbox-history-next",
                    "Next",
                    cx,
                    move |this, _, cx| {
                        if this.control_ui.inbox.history_busy {
                            return;
                        }
                        this.control_ui
                            .inbox
                            .previous
                            .push(this.control_ui.inbox.after);
                        this.control_ui.inbox.after = Some(next);
                        this.refresh_inbox_history(cx);
                    },
                ));
            }
        }
        if self
            .control_ui
            .inbox
            .history
            .as_ref()
            .is_some_and(|p| p.task_history_syncing)
        {
            row = row.child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child("Importing task history…"),
            );
        }
        if self.control_ui.inbox.history_busy {
            row = row.child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child("Updating…"),
            );
        }
        if let Some(error) = &self.control_ui.inbox.history_error {
            row = row.child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text)
                    .child(error.clone()),
            );
        }
        row
    }
    pub(in crate::app::workspace_control) fn render_inbox_history_row(
        &self,
        entry: Rc<InboxEntry>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        let mut actions = div().flex().flex_wrap().gap(px(8.0));
        let open = entry.clone();
        actions = actions.child(self.control_button(
            format!("inbox-source-{}", entry.sequence),
            if entry.needs_action {
                "Review source"
            } else {
                "Open source"
            },
            cx,
            move |this, _, cx| {
                if open.source == InboxSource::Automation {
                    this.control_request(Rpc::GetAutomationRun { id: open.source_id }, cx);
                } else if open.needs_action
                    && matches!(
                        open.source,
                        InboxSource::WorkspaceAction | InboxSource::TeamDecision
                    )
                {
                    this.open_workspace_page(Page::Decisions, cx);
                    this.control_ui.filter_project = open.project_id;
                    this.control_ui.filter_task = open.task_id;
                } else if let Some(task) = open.task_id {
                    this.open_team_activity_task(task, cx);
                } else {
                    this.open_workspace_page(Page::Artifacts, cx);
                    this.control_ui.filter_project = open.project_id;
                }
                cx.notify();
            },
        ));
        let state = &entry.state;
        for (i, (label, action)) in [
            (
                if state.read {
                    "Mark unread"
                } else {
                    "Mark read"
                },
                if state.read {
                    InboxAction::MarkUnread
                } else {
                    InboxAction::MarkRead
                },
            ),
            ("Acknowledge", InboxAction::Acknowledge),
            (
                if state.archived { "Restore" } else { "Archive" },
                if state.archived {
                    InboxAction::Restore
                } else {
                    InboxAction::Archive
                },
            ),
        ]
        .into_iter()
        .enumerate()
        {
            if action == InboxAction::Acknowledge && state.acknowledged {
                continue;
            }
            let key = state.key.clone();
            let revision = self
                .control_ui
                .inbox
                .history
                .as_ref()
                .map(|p| p.revision)
                .unwrap_or(0);
            actions = actions.child(
                self.control_button(
                    format!("inbox-history-action-{}-{i}", entry.sequence),
                    label,
                    cx,
                    move |this, _, cx| {
                        this.inbox_request(
                            Rpc::UpdateInboxItem {
                                expected_revision: revision,
                                key: key.clone(),
                                action,
                            },
                            cx,
                        );
                    },
                )
                .tooltip(Tooltip::text(if action == InboxAction::Acknowledge {
                    "Acknowledge the notification. Pending work still needs its own decision."
                } else {
                    label
                })),
            );
        }
        let label = if state.archived {
            "Archived"
        } else if state.acknowledged {
            "Acknowledged"
        } else if state.read {
            "Read"
        } else {
            "Unread"
        };
        let project = entry
            .project_id
            .and_then(|id| self.state.projects.iter().find(|p| p.id == id))
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "Project".into());
        div()
            .id(SharedString::from(format!(
                "inbox-history-{}",
                entry.sequence
            )))
            .px(px(24.0))
            .py(px(14.0))
            .border_b_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(sp(14.0))
                    .text_color(theme.text)
                    .font_weight(FontWeight::MEDIUM)
                    .child(entry.title.clone()),
            )
            .child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(format!(
                        "{project} · {} · {label}{}",
                        automation_ui::next_fire(entry.occurred_at),
                        if entry.needs_action {
                            " · Needs action"
                        } else {
                            ""
                        }
                    )),
            )
            .child(
                div()
                    .text_size(sp(13.0))
                    .text_color(theme.text)
                    .child(entry.detail.clone()),
            )
            .child(actions)
            .into_any_element()
    }
}
