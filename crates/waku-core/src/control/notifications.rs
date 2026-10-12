//! Indexed ascending notification facts. Publication never transfers title or content.
use super::*;
use waku_protocol::{inbox::NotificationPolicy, inbox_notifications::*};
impl ControlStore {
    pub fn inbox_notification_batch(
        &self,
        after: Option<u64>,
        now: u64,
    ) -> Result<InboxNotificationBatch> {
        ensure!(
            after.is_none_or(|v| v <= 9_007_199_254_740_991),
            "Invalid notification cursor"
        );
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let latest: u64 = tx.query_row(
            "SELECT coalesce(max(seq),0) FROM ion_inbox_history",
            [],
            |r| r.get(0),
        )?;
        ensure!(
            after.is_none_or(|v| v <= latest),
            "Notification cursor belongs to another owner history; pair a new publisher"
        );
        ensure!(
            latest <= 9_007_199_254_740_991,
            "Notification history exceeds the supported cursor range"
        );
        let policy: String = tx.query_row(
            "SELECT policy FROM ion_inbox_preferences WHERE id=1",
            [],
            |r| r.get(0),
        )?;
        let policy: NotificationPolicy = serde_json::from_value(policy.into())?;
        let Some(cursor) = after else {
            tx.commit()?;
            return Ok(InboxNotificationBatch {
                after,
                next: latest,
                policy,
                events: vec![],
            });
        };
        let rows:Vec<(u64,u64,bool,bool,bool,bool,String)>=tx.prepare("SELECT seq,occurred_at,needs_action,read,acknowledged,archived,kind FROM ion_inbox_history WHERE seq>?1 ORDER BY seq LIMIT 64")?.query_map([cursor],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?)))?.collect::<rusqlite::Result<_>>()?;
        let next = rows.last().map(|r| r.0).unwrap_or(cursor);
        let events = rows
            .into_iter()
            .filter_map(|(sequence, time, pending, read, ack, archived, kind)| {
                let needs_attention = pending || kind == "issue";
                (!(matches!(
                    kind.as_str(),
                    "proposal" | "decision" | "approval" | "question"
                ) && !pending)
                    && !read
                    && !ack
                    && !archived
                    && time >= now.saturating_sub(86400)
                    && time <= now
                    && policy.allows(needs_attention))
                .then_some(InboxNotificationEvent {
                    sequence,
                    needs_attention,
                })
            })
            .collect();
        tx.commit()?;
        Ok(InboxNotificationBatch {
            after,
            next,
            policy,
            events,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> ControlStore {
        ControlStore::new(
            std::env::temp_dir()
                .join(format!("ion-inbox-push-{}", Uuid::new_v4()))
                .join("control.db"),
        )
    }
    fn fact(store: &ControlStore, id: Uuid, time: u64, pending: bool, kind: &str) -> u64 {
        let db = store.connect().unwrap();
        db.execute("INSERT INTO ion_inbox_history(source,source_id,fingerprint,kind,title,detail,occurred_at,needs_action) VALUES('task',?1,?2,?3,'private title','secret content',?4,?5)",params![id.to_string(),Uuid::new_v4().to_string(),kind,time,pending]).unwrap();
        db.last_insert_rowid() as u64
    }
    #[test]
    fn baseline_suppresses_old_facts_then_pages_new_eligible_notifications_without_content() {
        let s = fixture();
        fact(&s, Uuid::new_v4(), 100, true, "question");
        let baseline = s.inbox_notification_batch(None, 100).unwrap();
        assert_eq!(baseline.next, 1);
        assert!(baseline.events.is_empty());
        let a = fact(&s, Uuid::new_v4(), 101, false, "completed");
        let b = fact(&s, Uuid::new_v4(), 102, true, "decision");
        let feed = s.inbox_notification_batch(Some(1), 102).unwrap();
        assert_eq!(feed.next, b);
        assert_eq!(feed.events.len(), 2);
        assert_eq!(feed.events[0].sequence, a);
        assert!(!feed.events[0].needs_attention);
        assert!(feed.events[1].needs_attention);
        assert!(!serde_json::to_string(&feed).unwrap().contains("private"));
        assert!(!serde_json::to_string(&feed).unwrap().contains("secret"));
    }
    #[test]
    fn read_ack_archive_old_future_and_superseded_notifications_are_skipped_with_cursor_progress() {
        let s = fixture();
        let id = Uuid::new_v4();
        fact(&s, id, 100, true, "question");
        fact(&s, id, 101, true, "question");
        let db = s.connect().unwrap();
        db.execute("UPDATE ion_inbox_history SET read=1 WHERE seq=2", [])
            .unwrap();
        fact(&s, Uuid::new_v4(), 102, true, "decision");
        db.execute("UPDATE ion_inbox_history SET archived=1 WHERE seq=3", [])
            .unwrap();
        fact(&s, Uuid::new_v4(), 103, true, "issue");
        db.execute(
            "UPDATE ion_inbox_history SET acknowledged=1 WHERE seq=4",
            [],
        )
        .unwrap();
        fact(&s, Uuid::new_v4(), 200000, true, "decision");
        for now in [104, 100000] {
            let feed = s.inbox_notification_batch(Some(0), now).unwrap();
            assert_eq!(feed.next, 5);
            assert!(feed.events.is_empty());
        }
    }
    #[test]
    fn issue_policy_filters_completed_work_off_clears_events_and_restart_keeps_cursor() {
        let s = fixture();
        fact(&s, Uuid::new_v4(), 100, false, "completed");
        fact(&s, Uuid::new_v4(), 101, false, "issue");
        fact(&s, Uuid::new_v4(), 102, true, "proposal");
        let db = s.connect().unwrap();
        db.execute("UPDATE ion_inbox_preferences SET policy='issuesOnly'", [])
            .unwrap();
        let feed = s.inbox_notification_batch(Some(0), 102).unwrap();
        assert_eq!(feed.events.len(), 2);
        db.execute("UPDATE ion_inbox_preferences SET policy='off'", [])
            .unwrap();
        let muted = s.inbox_notification_batch(Some(0), 102).unwrap();
        assert!(muted.events.is_empty());
        assert_eq!(muted.next, 3);
        assert!(s.inbox_notification_batch(Some(4), 102).is_err());
        assert!(s.inbox_notification_batch(Some(u64::MAX), 102).is_err());
        assert!(
            s.inbox_notification_batch(Some(3), 102)
                .unwrap()
                .events
                .is_empty()
        );
    }
    #[test]
    fn ascending_pages_bound_backlog_and_advance_even_when_suppressed() {
        let s = fixture();
        for _ in 0..70 {
            fact(&s, Uuid::new_v4(), 100, true, "question");
        }
        let page = s.inbox_notification_batch(Some(0), 100).unwrap();
        assert_eq!(page.events.len(), 64);
        assert_eq!(page.next, 64);
        let next = s.inbox_notification_batch(Some(page.next), 100).unwrap();
        assert_eq!(next.events.len(), 6);
        assert_eq!(next.next, 70);
    }
}
