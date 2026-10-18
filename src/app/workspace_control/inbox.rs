//! Cached user notification state. No owner RPC or hashing occurs in row builders.
use super::*;
mod history;
use waku_client::inbox::{
    InboxAction, InboxItemState, InboxSnapshot, NotificationPolicy, event_key,
};

#[derive(Clone, Copy, Default, Eq, PartialEq, Hash)]
pub(super) enum InboxView {
    #[default]
    Inbox,
    Unread,
    Acknowledged,
    Archived,
    All,
}
impl InboxView {
    fn label(self) -> &'static str {
        match self {
            Self::Inbox => "Inbox",
            Self::Unread => "Unread",
            Self::Acknowledged => "Acknowledged",
            Self::Archived => "Archived",
            Self::All => "All notifications",
        }
    }
    fn includes(self, item: &InboxItemState) -> bool {
        match self {
            Self::Inbox => !item.archived,
            Self::Unread => !item.archived && !item.read,
            Self::Acknowledged => !item.archived && item.acknowledged,
            Self::Archived => item.archived,
            Self::All => true,
        }
    }
}
#[derive(Default)]
pub(super) struct InboxUi {
    pub snapshot: Rc<InboxSnapshot>,
    states: Rc<HashMap<String, InboxItemState>>,
    pub view: InboxView,
    pub loaded: bool,
    busy: bool,
    generation: u64,
    pub error: Option<String>,
    history: Option<Rc<waku_client::inbox::InboxPage>>,
    history_busy: bool,
    history_generation: u64,
    history_error: Option<String>,
    after: Option<u64>,
    previous: Vec<Option<u64>>,
}
pub(super) struct InboxRow {
    state: InboxItemState,
    row: Row,
}
fn row_key(row: &Row) -> Option<String> {
    let (source, id, event) = match row {
        Row::Notice(n) => {
            let id = n
                .record
                .as_ref()
                .map(ControlRecord::id)
                .or(n.run)
                .or(n.task)?;
            let action_status = n.record.as_ref().and_then(|r| match r {
                ControlRecord::Action(a) => Some(a.status),
                _ => None,
            });
            (
                "notice",
                id,
                serde_json::to_vec(&(
                    format!("{:?}", n.kind),
                    &n.title,
                    &n.detail,
                    n.pending,
                    n.timestamp,
                    action_status,
                ))
                .ok()?,
            )
        }
        Row::TeamDecision(d) => (
            "team-decision",
            d.id,
            serde_json::to_vec(&(d.status, d.revision)).ok()?,
        ),
        Row::Automation(r) => (
            "automation",
            r.id,
            serde_json::to_vec(&(r.status, r.current_stage, &r.reason)).ok()?,
        ),
        _ => return None,
    };
    Some(event_key(source, id, &event))
}
fn policy_label(policy: NotificationPolicy) -> &'static str {
    match policy {
        NotificationPolicy::All => "All task notifications",
        NotificationPolicy::IssuesOnly => "Issues only",
        NotificationPolicy::Off => "Notifications off",
    }
}
impl Waku {
    pub(super) fn refresh_inbox(&mut self, cx: &mut Context<Self>) {
        self.refresh_inbox_history(cx);
        if !self.control_ui.inbox.busy {
            self.inbox_request(Rpc::LoadInbox, cx);
        }
    }
    fn inbox_request(&mut self, command: Rpc, cx: &mut Context<Self>) {
        if self.control_ui.inbox.busy {
            self.show_toast("Inbox is updating. Try again.");
            return;
        }
        self.control_ui.inbox.busy = true;
        self.control_ui.inbox.generation += 1;
        let generation = self.control_ui.inbox.generation;
        let mutation = !matches!(command, Rpc::LoadInbox);
        let client = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || -> anyhow::Result<_> {
                        let ResponsePayload::Inbox { snapshot } =
                            client.request(Uuid::nil(), Uuid::nil(), command)?
                        else {
                            anyhow::bail!("Unexpected Inbox response");
                        };
                        let states = snapshot
                            .items
                            .iter()
                            .map(|item| (item.key.clone(), item.clone()))
                            .collect::<HashMap<_, _>>();
                        Ok((snapshot, states))
                    })
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.control_ui.inbox.generation != generation {
                    return;
                }
                this.control_ui.inbox.busy = false;
                match result {
                    Ok((snapshot, states)) => {
                        let changed = !this.control_ui.inbox.loaded
                            || snapshot.revision > this.control_ui.inbox.snapshot.revision
                            || this.control_ui.inbox.error.is_some();
                        if snapshot.revision >= this.control_ui.inbox.snapshot.revision {
                            this.control_ui.inbox.snapshot = Rc::new(snapshot);
                            this.control_ui.inbox.states = Rc::new(states);
                            this.control_ui.inbox.loaded = true;
                        }
                        this.control_ui.inbox.error = None;
                        if mutation {
                            this.refresh_inbox_history(cx);
                        }
                        if changed || mutation {
                            cx.notify();
                        }
                    }
                    Err(error) => {
                        let message =
                            format!("Could not update Inbox: {error}. Use Refresh to retry.");
                        if this.control_ui.inbox.error.as_ref() != Some(&message) {
                            this.control_ui.inbox.error = Some(message);
                            cx.notify();
                        }
                        if mutation {
                            this.show_toast(
                                "Inbox update was not confirmed. Refresh and retry the action.",
                            );
                            this.refresh_inbox(cx);
                        }
                    }
                }
            });
        })
        .detach();
        if mutation {
            cx.notify();
        }
    }
    pub(in crate::app) fn allows_task_notification(&self, needs_attention: bool) -> bool {
        self.control_ui.inbox.loaded
            && self
                .control_ui
                .inbox
                .snapshot
                .notification_policy
                .allows(needs_attention)
    }
    pub(super) fn inbox_rows(&self, rows: Vec<Row>) -> Vec<Row> {
        rows.into_iter()
            .filter_map(|row| {
                let Some(key) = row_key(&row) else {
                    return Some(row);
                };
                let state = self
                    .control_ui
                    .inbox
                    .states
                    .get(&key)
                    .cloned()
                    .unwrap_or_else(|| InboxItemState {
                        key,
                        ..Default::default()
                    });
                self.control_ui
                    .inbox
                    .view
                    .includes(&state)
                    .then(|| Row::Inbox(Rc::new(InboxRow { state, row })))
            })
            .collect()
    }
    fn update_inbox_item(&mut self, key: String, action: InboxAction, cx: &mut Context<Self>) {
        if !self.control_ui.inbox.loaded {
            self.show_toast("Wait for Inbox to load, then try again.");
            return;
        }
        self.inbox_request(
            Rpc::UpdateInboxItem {
                expected_revision: self.control_ui.inbox.snapshot.revision,
                key,
                action,
            },
            cx,
        );
    }
    pub(super) fn render_inbox_toolbar(&self, cx: &mut Context<Self>) -> Div {
        let theme = Theme::workspace(cx);
        let mut bar = div().flex().flex_wrap().items_center().gap(px(8.0));
        for policy_menu in [false, true] {
            let id = if policy_menu {
                "inbox-notifications"
            } else {
                "inbox-view"
            };
            let label = if policy_menu {
                policy_label(self.control_ui.inbox.snapshot.notification_policy)
            } else {
                self.control_ui.inbox.view.label()
            };
            let menu = self.menu_handle(id, cx);
            let weak = cx.entity().downgrade();
            bar = bar.child(dropdown_menu(
                self.control_button(id, label, cx, |_, _, _| {}).child(icon(
                    "icons/chevron-down.svg",
                    12.0,
                    theme.text_secondary,
                )),
                id,
                &menu,
                MenuAlign::BelowLeft,
                move |cx| {
                    let Some(entity) = weak.upgrade() else {
                        return Vec::new();
                    };
                    let this = entity.read(cx);
                    if policy_menu {
                        [
                            NotificationPolicy::All,
                            NotificationPolicy::IssuesOnly,
                            NotificationPolicy::Off,
                        ]
                        .into_iter()
                        .map(|policy| {
                            let weak = weak.clone();
                            MenuItem::new(policy_label(policy), move |_, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    if !this.control_ui.inbox.loaded {
                                        this.show_toast("Wait for Inbox to load, then try again.");
                                        return;
                                    }
                                    this.inbox_request(
                                        Rpc::SetNotificationPolicy {
                                            expected_revision: this
                                                .control_ui
                                                .inbox
                                                .snapshot
                                                .revision,
                                            policy,
                                        },
                                        cx,
                                    );
                                });
                            })
                            .selected(this.control_ui.inbox.snapshot.notification_policy == policy)
                        })
                        .collect()
                    } else {
                        [
                            InboxView::Inbox,
                            InboxView::Unread,
                            InboxView::Acknowledged,
                            InboxView::Archived,
                            InboxView::All,
                        ]
                        .into_iter()
                        .map(|view| {
                            let weak = weak.clone();
                            MenuItem::new(view.label(), move |_, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    this.control_ui.inbox.view = view;
                                    this.reset_inbox_history(cx);
                                    cx.notify();
                                });
                            })
                            .selected(this.control_ui.inbox.view == view)
                        })
                        .collect()
                    }
                },
            ));
        }
        if self.control_ui.inbox.busy {
            bar = bar.child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child("Updating…"),
            );
        }
        bar.child(self.history_toolbar(cx))
    }
    pub(super) fn render_inbox_row(
        &self,
        row: Rc<InboxRow>,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::workspace(cx);
        let state = &row.state;
        let label = if state.archived {
            "Archived"
        } else if state.acknowledged {
            "Acknowledged"
        } else if state.read {
            "Read"
        } else {
            "Unread"
        };
        let mut footer = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.0))
            .pb(px(12.0))
            .child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(label),
            );
        let actions = [
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
        ];
        for (n, (label, action)) in actions.into_iter().enumerate() {
            if action == InboxAction::Acknowledge && state.acknowledged {
                continue;
            }
            let key = state.key.clone();
            footer = footer.child(
                self.control_button(
                    format!("inbox-action-{index}-{n}"),
                    label,
                    cx,
                    move |this, _, cx| this.update_inbox_item(key.clone(), action, cx),
                )
                .border_0()
                .tooltip(Tooltip::text(match action {
                    InboxAction::Acknowledge => {
                        "Acknowledge this notification; pending work still needs its own approval."
                    }
                    InboxAction::Archive => {
                        "Archive this notification; pending decisions remain in Decisions."
                    }
                    _ => label,
                })),
            );
        }
        div()
            .min_w_0()
            .flex()
            .flex_col()
            .child(self.render_control_row(row.row.clone(), index, cx))
            .child(footer)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inbox_event_identity_is_stable_but_new_task_updates_are_unread() {
        let notice = Notice {
            project: Some(Uuid::new_v4()),
            task: Some(Uuid::new_v4()),
            title: "Task completed".into(),
            detail: "Report saved".into(),
            kind: NoticeKind::Completed,
            pending: false,
            timestamp: 1,
            record: None,
            run: None,
            decision: None,
        };
        let original = row_key(&Row::Notice(notice.clone())).unwrap();
        assert_eq!(original, row_key(&Row::Notice(notice.clone())).unwrap());
        assert!(original.starts_with("ion:v1") && !original.contains("Report"));
        let mut next = notice;
        next.timestamp = 2;
        assert_ne!(original, row_key(&Row::Notice(next)).unwrap());
        let archived = InboxItemState {
            key: original,
            read: true,
            acknowledged: false,
            archived: true,
        };
        assert!(!InboxView::Inbox.includes(&archived));
        assert!(InboxView::Archived.includes(&archived));
        assert!(InboxView::All.includes(&archived));
        assert!(!InboxView::Unread.includes(&archived));
        assert!(InboxView::Unread.includes(&InboxItemState::default()));
    }
}
