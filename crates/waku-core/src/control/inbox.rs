//! Bounded daemon-owned notification state, separate from all execution revisions.
use super::*;
use waku_protocol::inbox::*;
const MAX_ITEMS: usize = 10_000;

fn open(store: &ControlStore) -> Result<Connection> {
    let db = store.connect()?;
    db.execute_batch("CREATE TABLE IF NOT EXISTS ion_inbox_items (key TEXT PRIMARY KEY, body TEXT NOT NULL); CREATE TABLE IF NOT EXISTS ion_inbox_preferences (id INTEGER PRIMARY KEY CHECK(id=1), revision INTEGER NOT NULL, policy TEXT NOT NULL); INSERT OR IGNORE INTO ion_inbox_preferences VALUES(1,0,'all');")?;
    Ok(db)
}
fn read(db: &Connection) -> Result<InboxSnapshot> {
    let (revision, policy): (u64, String) = db.query_row(
        "SELECT revision,policy FROM ion_inbox_preferences WHERE id=1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let notification_policy = serde_json::from_value(serde_json::Value::String(policy))?;
    let mut query = db.prepare("SELECT body FROM ion_inbox_items ORDER BY key LIMIT 10001")?;
    let mut items = Vec::new();
    for body in query.query_map([], |row| row.get::<_, String>(0))? {
        items.push(serde_json::from_str(&body?)?);
    }
    ensure!(items.len() <= MAX_ITEMS, "Inbox state exceeded its limit");
    Ok(InboxSnapshot {
        revision,
        notification_policy,
        items,
    })
}
fn validate_key(key: &str) -> Result<()> {
    // Keys contain an event digest, never transcript text, paths or credentials.
    ensure!(
        key.len() == 70
            && key.starts_with("ion:v1")
            && key.as_bytes()[6..].iter().all(u8::is_ascii_hexdigit),
        "Invalid Inbox event key"
    );
    Ok(())
}
impl ControlStore {
    pub fn load_inbox(&self) -> Result<InboxSnapshot> {
        let mut db = open(self)?;
        let tx = db.transaction()?;
        let snapshot = read(&tx)?;
        tx.commit()?;
        Ok(snapshot)
    }
    pub fn update_inbox_item(
        &self,
        expected_revision: u64,
        key: String,
        action: InboxAction,
    ) -> Result<InboxSnapshot> {
        validate_key(&key)?;
        let mut db = open(self)?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let snapshot = read(&tx)?;
        let previous = snapshot.items.iter().find(|item| item.key == key).cloned();
        let mut item = previous.clone().unwrap_or_else(|| InboxItemState {
            key,
            ..Default::default()
        });
        item.apply(action);
        // A retried acknowledgement after a lost response is safe only if already applied.
        if previous.as_ref() == Some(&item) {
            tx.commit()?;
            return Ok(snapshot);
        }
        ensure!(
            snapshot.revision == expected_revision,
            "Inbox changed on another client. Refresh before updating."
        );
        ensure!(
            previous.is_some() || snapshot.items.len() < MAX_ITEMS,
            "Inbox state limit reached; existing notifications can still be updated"
        );
        tx.execute("INSERT INTO ion_inbox_items VALUES(?,?) ON CONFLICT(key) DO UPDATE SET body=excluded.body", params![item.key,serde_json::to_string(&item)?])?;
        tx.execute(
            "UPDATE ion_inbox_preferences SET revision=revision+1 WHERE id=1",
            [],
        )?;
        let snapshot = read(&tx)?;
        tx.commit()?;
        Ok(snapshot)
    }
    pub fn set_notification_policy(
        &self,
        expected_revision: u64,
        policy: NotificationPolicy,
    ) -> Result<InboxSnapshot> {
        let mut db = open(self)?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let snapshot = read(&tx)?;
        if snapshot.notification_policy == policy {
            tx.commit()?;
            return Ok(snapshot);
        }
        ensure!(
            snapshot.revision == expected_revision,
            "Inbox changed on another client. Refresh before updating."
        );
        tx.execute(
            "UPDATE ion_inbox_preferences SET revision=revision+1,policy=? WHERE id=1",
            [serde_json::to_value(policy)?
                .as_str()
                .context("Invalid notification policy")?],
        )?;
        let snapshot = read(&tx)?;
        tx.commit()?;
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(value: u8) -> String {
        format!("ion:v1{:064x}", value)
    }
    #[test]
    fn inbox_state_concurrent_clients_cannot_overwrite_each_other() {
        let dir = std::env::temp_dir().join(format!("ion-inbox-concurrent-{}", Uuid::new_v4()));
        let store = std::sync::Arc::new(ControlStore::new(dir.join("control.db")));
        store.load_inbox().unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let handles = (1..=2)
            .map(|id| {
                let store = store.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    store.update_inbox_item(0, key(id), InboxAction::Archive)
                })
            })
            .collect::<Vec<_>>();
        let results = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        let snapshot = store.load_inbox().unwrap();
        assert_eq!(snapshot.revision, 1);
        assert_eq!(snapshot.items.len(), 1);
        drop(store);
        assert_eq!(
            std::fs::canonicalize(&dir).unwrap().parent(),
            Some(
                std::fs::canonicalize(std::env::temp_dir())
                    .unwrap()
                    .as_path()
            )
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn inbox_state_capacity_preserves_existing_controls_and_policy() {
        let dir = std::env::temp_dir().join(format!("ion-inbox-capacity-{}", Uuid::new_v4()));
        let store = ControlStore::new(dir.join("control.db"));
        let mut db = open(&store).unwrap();
        let tx = db.transaction().unwrap();
        for index in 0..MAX_ITEMS {
            let item = InboxItemState {
                key: format!("ion:v1{index:064x}"),
                ..Default::default()
            };
            tx.execute(
                "INSERT INTO ion_inbox_items VALUES(?,?)",
                params![item.key, serde_json::to_string(&item).unwrap()],
            )
            .unwrap();
        }
        tx.commit().unwrap();
        drop(db);
        assert!(
            store
                .update_inbox_item(0, format!("ion:v1{MAX_ITEMS:064x}"), InboxAction::Archive)
                .is_err()
        );
        let saved = store
            .update_inbox_item(0, key(0), InboxAction::Acknowledge)
            .unwrap();
        assert_eq!(saved.items.len(), MAX_ITEMS);
        assert!(
            store
                .set_notification_policy(saved.revision, NotificationPolicy::Off)
                .is_ok()
        );
        assert_eq!(
            std::fs::canonicalize(&dir).unwrap().parent(),
            Some(
                std::fs::canonicalize(std::env::temp_dir())
                    .unwrap()
                    .as_path()
            )
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn inbox_state_restarts_replays_and_never_resolves_or_revises_agent_work() {
        let dir = std::env::temp_dir().join(format!("ion-inbox-{}", Uuid::new_v4()));
        let store = ControlStore::new(dir.join("control.db"));
        let action = WorkspaceAction {
            id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            task_id: Uuid::new_v4(),
            title: "Decision".into(),
            detail: "Pending user choice".into(),
            action_kind: WorkspaceActionKind::Decision,
            status: WorkspaceActionStatus::Pending,
            created_at: 1,
        };
        let definitions = store.add_agent_action(action).unwrap();
        assert_eq!(store.load_inbox().unwrap(), InboxSnapshot::default());
        let read = store
            .update_inbox_item(0, key(1), InboxAction::MarkRead)
            .unwrap();
        let ack = store
            .update_inbox_item(read.revision, key(1), InboxAction::Acknowledge)
            .unwrap();
        let archived = store
            .update_inbox_item(ack.revision, key(1), InboxAction::Archive)
            .unwrap();
        assert_eq!(
            store
                .update_inbox_item(ack.revision, key(1), InboxAction::Archive)
                .unwrap(),
            archived
        );
        let reopened = ControlStore::new(dir.join("control.db"));
        assert_eq!(reopened.load_inbox().unwrap(), archived);
        assert_eq!(reopened.load().unwrap().revision, definitions.revision);
        assert!(
            matches!(&reopened.load().unwrap().records[0],ControlRecord::Action(a) if a.status==WorkspaceActionStatus::Pending)
        );
        let restored = reopened
            .update_inbox_item(archived.revision, key(1), InboxAction::Restore)
            .unwrap();
        assert!(
            restored.items[0].read && restored.items[0].acknowledged && !restored.items[0].archived
        );
        let unread = reopened
            .update_inbox_item(restored.revision, key(1), InboxAction::MarkUnread)
            .unwrap();
        assert!(!unread.items[0].read && !unread.items[0].acknowledged);
        assert!(unread.items.iter().all(|item| item.key != key(2))); // A later event starts unread.
        assert!(
            reopened
                .update_inbox_item(0, key(2), InboxAction::MarkRead)
                .is_err()
        );
        assert!(
            reopened
                .update_inbox_item(unread.revision, "private text".into(), InboxAction::Archive)
                .is_err()
        );
        let muted = reopened
            .set_notification_policy(unread.revision, NotificationPolicy::Off)
            .unwrap();
        assert_eq!(reopened.load_inbox().unwrap(), muted);
        assert_eq!(
            reopened
                .set_notification_policy(0, NotificationPolicy::Off)
                .unwrap(),
            muted
        );
        assert!(
            reopened
                .set_notification_policy(0, NotificationPolicy::All)
                .is_err()
        );
        assert!(!muted.notification_policy.allows(true));
        assert!(NotificationPolicy::IssuesOnly.allows(true));
        assert!(!NotificationPolicy::IssuesOnly.allows(false));
        assert_eq!(
            std::fs::canonicalize(&dir).unwrap().parent(),
            Some(
                std::fs::canonicalize(std::env::temp_dir())
                    .unwrap()
                    .as_path()
            )
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
