//! Inbox state and its exact operation receipt commit in one owner transaction.
use super::*;
use waku_protocol::{
    gateway_inbox::{InboxWriteReceipt, InboxWriteRequest},
    inbox::InboxItemState,
};
impl ControlStore {
    pub fn apply_gateway_inbox(
        &self,
        request: InboxWriteRequest,
        now: u64,
    ) -> Result<InboxWriteReceipt> {
        ensure!(
            request.sequence > 0 && request.sequence <= i64::MAX as u64,
            "Invalid Inbox notification"
        );
        let mut db = self.connect()?;
        db.execute_batch("CREATE TABLE IF NOT EXISTS ion_gateway_inbox_receipts(publisher_id TEXT NOT NULL,device_id TEXT NOT NULL,nonce TEXT NOT NULL,expires_at INTEGER NOT NULL,request TEXT NOT NULL,receipt TEXT NOT NULL,PRIMARY KEY(publisher_id,device_id,nonce)); CREATE INDEX IF NOT EXISTS ion_gateway_inbox_receipts_expiry ON ion_gateway_inbox_receipts(expires_at);")?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let publisher = request.publisher_id.to_string();
        let device = request.device_id.to_string();
        let nonce = request.idempotency_key.to_string();
        let previous: Option<(String,String)>=tx.query_row("SELECT request,receipt FROM ion_gateway_inbox_receipts WHERE publisher_id=?1 AND device_id=?2 AND nonce=?3",params![publisher,device,nonce],|row|Ok((row.get(0)?,row.get(1)?))).optional()?;
        if let Some((body, receipt)) = previous {
            ensure!(
                serde_json::from_str::<InboxWriteRequest>(&body)? == request,
                "Inbox receipt identity changed"
            );
            return Ok(serde_json::from_str(&receipt)?);
        }
        ensure!(
            request.expires_at <= now.saturating_add(120_000),
            "Invalid Inbox request deadline"
        );
        tx.execute(
            "DELETE FROM ion_gateway_inbox_receipts WHERE expires_at<?1",
            [now.saturating_sub(7 * 86_400_000)],
        )?;
        let count: u64 = tx.query_row(
            "SELECT count(*) FROM ion_gateway_inbox_receipts",
            [],
            |row| row.get(0),
        )?;
        ensure!(count < 10_000, "Inbox receipt capacity reached");
        let revision: u64 = tx.query_row(
            "SELECT revision FROM ion_inbox_preferences WHERE id=1",
            [],
            |row| row.get(0),
        )?;
        let state: Option<InboxItemState> = tx
            .query_row(
                "SELECT read,acknowledged,archived FROM ion_inbox_history WHERE seq=?1",
                [request.sequence],
                |row| {
                    Ok(InboxItemState {
                        key: format!("ion:v2{:016x}", request.sequence),
                        read: row.get(0)?,
                        acknowledged: row.get(1)?,
                        archived: row.get(2)?,
                    })
                },
            )
            .optional()?;
        let mut receipt = InboxWriteReceipt {
            applied: false,
            revision,
        };
        if let Some(mut state) = state
            && revision == request.expected_revision
            && request.expires_at > now
        {
            let before = state.clone();
            state.apply(request.action);
            if state != before {
                tx.execute(
                    "UPDATE ion_inbox_history SET read=?1,acknowledged=?2,archived=?3 WHERE seq=?4",
                    params![
                        state.read,
                        state.acknowledged,
                        state.archived,
                        request.sequence
                    ],
                )?;
                tx.execute(
                    "UPDATE ion_inbox_preferences SET revision=revision+1 WHERE id=1",
                    [],
                )?;
                receipt.revision += 1;
            }
            receipt.applied = true;
        }
        tx.execute(
            "INSERT INTO ion_gateway_inbox_receipts VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                publisher,
                device,
                nonce,
                request.expires_at,
                serde_json::to_string(&request)?,
                serde_json::to_string(&receipt)?
            ],
        )?;
        tx.commit()?;
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waku_protocol::inbox::InboxAction;
    struct Fixture {
        store: ControlStore,
        path: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("ion-inbox-receipt-{}", Uuid::new_v4()))
                .join("control.sqlite");
            let store = ControlStore::new(path.clone());
            store.connect().unwrap().execute("INSERT INTO ion_inbox_history(source,source_id,fingerprint,kind,title,detail,occurred_at,needs_action) VALUES('task',?1,'fixture','completed','Complete','Fixture',1,1)",[Uuid::new_v4().to_string()]).unwrap();
            Self { store, path }
        }
        fn request(&self, action: InboxAction, revision: u64) -> InboxWriteRequest {
            InboxWriteRequest {
                device_id: Uuid::new_v4(),
                publisher_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
                sequence: 1,
                expected_revision: revision,
                action,
                expires_at: 120_000,
            }
        }
        fn state(&self) -> (u64, bool, bool, bool, bool) {
            self.store.connect().unwrap().query_row("SELECT p.revision,h.read,h.acknowledged,h.archived,h.needs_action FROM ion_inbox_preferences p,ion_inbox_history h WHERE h.seq=1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(self.path.parent().unwrap());
        }
    }
    #[test]
    fn notification_receipt_survives_restart_and_intervening_changes_without_replay() {
        let f = Fixture::new();
        let request = f.request(InboxAction::Acknowledge, 0);
        let receipt = f.store.apply_gateway_inbox(request.clone(), 1_000).unwrap();
        assert_eq!(
            receipt,
            InboxWriteReceipt {
                applied: true,
                revision: 1
            }
        );
        f.store
            .update_history_item(1, 1, InboxAction::MarkUnread)
            .unwrap();
        assert_eq!(f.state(), (2, false, false, false, true));
        let restarted = ControlStore::new(f.path.clone());
        assert_eq!(
            restarted.apply_gateway_inbox(request, 130_000).unwrap(),
            receipt
        );
        assert_eq!(f.state(), (2, false, false, false, true));
        assert_eq!(f.store.load().unwrap().revision, 0); // no work/definition approval
    }
    #[test]
    fn changed_nonce_body_rejects_even_if_desired_state_matches() {
        let f = Fixture::new();
        let mut request = f.request(InboxAction::Archive, 0);
        f.store.apply_gateway_inbox(request.clone(), 1_000).unwrap();
        request.action = InboxAction::Restore;
        assert!(f.store.apply_gateway_inbox(request, 1_000).is_err());
        assert_eq!(f.state(), (1, true, false, true, true));
    }
    #[test]
    fn stale_missing_and_expired_notification_writes_leave_owner_work_unchanged() {
        let f = Fixture::new();
        let mut expired = f.request(InboxAction::Archive, 0);
        expired.expires_at = 999;
        let stale = f.request(InboxAction::Archive, 1);
        let mut missing = f.request(InboxAction::Archive, 0);
        missing.sequence = 2;
        for request in [expired, stale, missing] {
            assert!(!f.store.apply_gateway_inbox(request, 1_000).unwrap().applied);
            assert_eq!(f.state(), (0, false, false, false, true));
        }
    }
    #[test]
    fn receipt_commit_failure_rolls_back_state_and_exact_retry_can_finish() {
        let f = Fixture::new();
        let mut seed = f.request(InboxAction::Archive, 0);
        seed.expires_at = 999;
        f.store.apply_gateway_inbox(seed, 1_000).unwrap();
        f.store.connect().unwrap().execute_batch("CREATE TRIGGER deny_notification_receipt BEFORE INSERT ON ion_gateway_inbox_receipts BEGIN SELECT RAISE(ABORT,'fixture commit failure'); END;").unwrap();
        let request = f.request(InboxAction::Acknowledge, 0);
        assert!(f.store.apply_gateway_inbox(request.clone(), 1_000).is_err());
        assert_eq!(f.state(), (0, false, false, false, true));
        f.store
            .connect()
            .unwrap()
            .execute_batch("DROP TRIGGER deny_notification_receipt")
            .unwrap();
        assert!(f.store.apply_gateway_inbox(request, 1_000).unwrap().applied);
        assert_eq!(f.state(), (1, true, true, false, true));
    }
    #[test]
    fn concurrent_notification_revisions_commit_only_one_distinct_operation() {
        let f = Fixture::new();
        let requests = [
            f.request(InboxAction::Archive, 0),
            f.request(InboxAction::Acknowledge, 0),
        ];
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let handles = requests
            .into_iter()
            .map(|request| {
                let path = f.path.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    ControlStore::new(path)
                        .apply_gateway_inbox(request, 1_000)
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            handles
                .into_iter()
                .map(|h| h.join().unwrap())
                .filter(|r| r.applied)
                .count(),
            1
        );
        assert_eq!(f.state().0, 1);
        assert!(f.state().4);
    }
}
