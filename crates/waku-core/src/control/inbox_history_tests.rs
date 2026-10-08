use super::*;
fn query() -> InboxQuery {
    InboxQuery {
        project_id: None,
        task_id: None,
        kind: None,
        view: InboxHistoryView::All,
        status: InboxHistoryStatus::All,
        after: None,
        limit: 32,
    }
}
struct Fixture {
    dir: PathBuf,
    store: ControlStore,
    project: Uuid,
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("ion-inbox-history-{}", Uuid::new_v4()));
        Self {
            store: ControlStore::new(dir.join("control.db")),
            dir,
            project: Uuid::new_v4(),
        }
    }
    fn action(&self, task: Uuid) -> WorkspaceAction {
        WorkspaceAction {
            id: Uuid::new_v4(),
            project_id: self.project,
            task_id: task,
            title: "Review proposed task".into(),
            detail: "Acceptance prepares a task without starting it".into(),
            action_kind: WorkspaceActionKind::ProposedTask,
            status: WorkspaceActionStatus::Pending,
            created_at: 100,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert_eq!(
            std::fs::canonicalize(&self.dir).unwrap().parent(),
            Some(
                std::fs::canonicalize(std::env::temp_dir())
                    .unwrap()
                    .as_path()
            )
        );
        std::fs::remove_dir_all(&self.dir).unwrap();
    }
}
#[test]
fn inbox_history_retains_resolved_deleted_sources_and_controls_never_approve() {
    let f = Fixture::new();
    let mut action = f.action(Uuid::new_v4());
    let defs = f.store.add_agent_action(action.clone()).unwrap();
    let page = f.store.inbox_history(query()).unwrap();
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.unread_count, 1);
    assert_eq!(page.needs_action_count, 1);
    assert_eq!(page.entries[0].source, InboxSource::WorkspaceAction);
    let key = page.entries[0].state.key.clone();
    f.store
        .update_inbox_item(page.revision, key.clone(), InboxAction::Acknowledge)
        .unwrap();
    assert_eq!(f.store.load().unwrap().revision, defs.revision);
    assert!(
        matches!(&f.store.load().unwrap().records[0],ControlRecord::Action(a) if a.status==WorkspaceActionStatus::Pending)
    );
    action.status = WorkspaceActionStatus::Accepted;
    let accepted = f
        .store
        .save(defs.revision, ControlRecord::Action(action.clone()))
        .unwrap();
    let page = f.store.inbox_history(query()).unwrap();
    assert_eq!(page.entries.len(), 2);
    assert_eq!(page.needs_action_count, 0);
    assert_eq!(page.unread_count, 1);
    assert_eq!(
        page.entries[0].detail,
        "accepted · Acceptance prepares a task without starting it"
    );
    assert!(page.entries[1].state.acknowledged);
    assert!(!page.entries[1].needs_action);
    f.store.remove(accepted.revision, action.id).unwrap();
    let reopened = ControlStore::new(f.store.path());
    assert_eq!(reopened.inbox_history(query()).unwrap().entries.len(), 2);
    reopened
        .update_inbox_item(page.revision, key.clone(), InboxAction::Archive)
        .unwrap();
    let page = reopened.inbox_history(query()).unwrap();
    let archived = page.entries.iter().find(|e| e.state.key == key).unwrap();
    assert!(archived.state.archived);
    // Exact replay with an old revision is allowed only for an already applied state.
    reopened
        .update_inbox_item(0, key.clone(), InboxAction::Archive)
        .unwrap();
    assert!(
        reopened
            .update_inbox_item(0, key, InboxAction::MarkUnread)
            .is_err()
    );
    assert!(
        reopened
            .update_inbox_item(page.revision, history_key(999), InboxAction::Archive)
            .is_err()
    );
}
#[test]
fn inbox_history_filters_pages_global_counts_and_repeated_save_is_not_a_new_fact() {
    let f = Fixture::new();
    let mut ids = Vec::new();
    for i in 0..40 {
        let mut action = f.action(Uuid::new_v4());
        if i % 2 == 0 {
            action.project_id = Uuid::nil();
        }
        ids.push(action.id);
        f.store.add_agent_action(action).unwrap();
    }
    let mut q = query();
    q.limit = 7;
    let first = f.store.inbox_history(q.clone()).unwrap();
    assert_eq!(first.entries.len(), 7);
    assert_eq!(first.matching_count, 40);
    assert_eq!(first.unread_count, 40);
    let mut seen = HashSet::new();
    let mut page = first;
    loop {
        for e in &page.entries {
            assert!(seen.insert(e.sequence));
        }
        let Some(next) = page.next_cursor else { break };
        q.after = Some(next);
        page = f.store.inbox_history(q.clone()).unwrap();
    }
    assert_eq!(seen.len(), 40);
    q.after = None;
    q.project_id = Some(f.project);
    q.kind = Some(InboxKind::Proposal);
    let page = f.store.inbox_history(q.clone()).unwrap();
    assert_eq!(page.matching_count, 20);
    assert_eq!(page.unread_count, 40);
    let entry = &page.entries[0];
    q.task_id = entry.task_id;
    assert_eq!(f.store.inbox_history(q.clone()).unwrap().matching_count, 1);
    let mut definitions = f.store.load().unwrap();
    let record = definitions.records.pop().unwrap();
    f.store.save(definitions.revision, record).unwrap();
    assert_eq!(f.store.inbox_history(query()).unwrap().matching_count, 40);
    f.store
        .update_inbox_item(page.revision, entry.state.key.clone(), InboxAction::Archive)
        .unwrap();
    q.view = InboxHistoryView::Archived;
    assert_eq!(f.store.inbox_history(q).unwrap().matching_count, 1);
    assert_eq!(f.store.inbox_history(query()).unwrap().unread_count, 39);
    let mut q = query();
    q.limit = 0;
    assert!(f.store.inbox_history(q).is_err());
    let mut q = query();
    q.after = Some(u64::MAX);
    assert!(f.store.inbox_history(q).is_err());
}
#[test]
fn inbox_history_task_outbox_preserves_lifecycle_after_deletion_and_reconciles_restart() {
    use crate::model::{AgentSession, Message, MessageRole, ProviderKind, SessionStatus};
    let f = Fixture::new();
    let tasks = crate::persistence::StateStore::daemon(f.dir.join("app.db"));
    let mut state = crate::persistence::PersistedState::empty();
    let mut session = AgentSession::new(f.project, ProviderKind::Nebius);
    session.title = "Desktop research".into();
    session.status = SessionStatus::Working;
    session
        .messages
        .push(Message::new(MessageRole::User, "Research this project"));
    let id = session.id;
    state.sessions.push(session);
    state.mark_session_dirty(id);
    tasks.save(&mut state).unwrap();
    for status in [
        SessionStatus::Waiting,
        SessionStatus::Working,
        SessionStatus::Failed,
        SessionStatus::Working,
        SessionStatus::Idle,
    ] {
        state.sessions[0].status = status;
        state.sessions[0].updated_at += 1;
        if status == SessionStatus::Idle {
            state.sessions[0].last_reply_at = Some(state.sessions[0].updated_at);
        }
        state.mark_session_dirty(id);
        tasks.save(&mut state).unwrap();
    }
    // Text/metadata-only saves do not append lifecycle notifications.
    state.sessions[0].title = "Edited title".into();
    state.mark_session_dirty(id);
    tasks.save(&mut state).unwrap();
    state.sessions.clear();
    tasks.save(&mut state).unwrap();
    f.store.sync_task_inbox(&tasks).unwrap();
    f.store.sync_task_inbox(&tasks).unwrap();
    let page = f.store.inbox_history(query()).unwrap();
    assert_eq!(page.entries.len(), 7);
    assert_eq!(page.needs_action_count, 0);
    assert_eq!(page.entries[0].detail, "Task removed");
    assert!(page.entries.iter().any(|e| e.kind == InboxKind::Completed));
    assert!(page.entries.iter().any(|e| e.kind == InboxKind::Issue));
    assert!(page.entries.iter().any(|e| e.kind == InboxKind::Approval));
    let reopened = ControlStore::new(f.store.path());
    reopened.sync_task_inbox(&tasks).unwrap();
    assert_eq!(reopened.inbox_history(query()).unwrap().entries.len(), 7);
}

#[test]
fn inbox_history_task_import_is_bounded_and_deleted_pending_task_is_resolved() {
    use crate::model::{AgentSession, Message, MessageRole, ProviderKind, SessionStatus};
    let f = Fixture::new();
    let tasks = crate::persistence::StateStore::daemon(f.dir.join("app.db"));
    let mut state = crate::persistence::PersistedState::empty();
    let mut session = AgentSession::new(f.project, ProviderKind::Nebius);
    session
        .messages
        .push(Message::new(MessageRole::User, "Review"));
    session.status = SessionStatus::Working;
    let id = session.id;
    state.sessions.push(session);
    for i in 0..514 {
        state.sessions[0].status = if i % 2 == 0 {
            SessionStatus::Working
        } else {
            SessionStatus::Waiting
        };
        state.sessions[0].updated_at += 1;
        state.mark_session_dirty(id);
        tasks.save(&mut state).unwrap();
    }
    assert!(f.store.sync_task_inbox(&tasks).unwrap());
    assert_eq!(f.store.inbox_history(query()).unwrap().matching_count, 512);
    assert!(!f.store.sync_task_inbox(&tasks).unwrap());
    assert_eq!(
        f.store.inbox_history(query()).unwrap().needs_action_count,
        1
    );
    state.sessions.clear();
    tasks.save(&mut state).unwrap();
    assert!(!f.store.sync_task_inbox(&tasks).unwrap());
    let page = f.store.inbox_history(query()).unwrap();
    assert_eq!(page.matching_count, 515);
    assert_eq!(page.needs_action_count, 0);
    assert_eq!(page.entries[0].detail, "Task removed");
}
#[test]
fn inbox_history_racing_controls_have_one_winner_and_do_not_change_definitions() {
    let f = Fixture::new();
    for _ in 0..2 {
        f.store.add_agent_action(f.action(Uuid::new_v4())).unwrap();
    }
    let page = f.store.inbox_history(query()).unwrap();
    let definitions = f.store.load().unwrap().revision;
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles = page
        .entries
        .iter()
        .map(|entry| {
            let path = f.store.path();
            let key = entry.state.key.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                ControlStore::new(path).update_inbox_item(0, key, InboxAction::Archive)
            })
        })
        .collect::<Vec<_>>();
    let results = handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    let page = f.store.inbox_history(query()).unwrap();
    assert_eq!(page.revision, 1);
    assert_eq!(page.entries.iter().filter(|e| e.state.archived).count(), 1);
    assert_eq!(page.needs_action_count, 2);
    assert_eq!(f.store.load().unwrap().revision, definitions);
}
