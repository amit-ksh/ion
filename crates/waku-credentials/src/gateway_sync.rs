//! One authenticated read batch per owner pass. Never reuse it as an effect grant.
use super::*;
mod cadence;
mod lease;
pub use cadence::Cadence;
pub use lease::PublisherLease;
use sha2::{Digest, Sha256};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

#[derive(Debug)]
pub(super) struct HttpFailure(pub reqwest::StatusCode);
impl std::fmt::Display for HttpFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Gateway request failed (HTTP {})", self.0)
    }
}
impl std::error::Error for HttpFailure {}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Reply {
    version: u32,
    publisher_id: String,
    features: HashMap<String, u64>,
    pub projection_digest: Option<String>,
    pub commands: Vec<GatewayAutomationCommand>,
    pub uncertain: Vec<GatewayAutomationCommand>,
    pub jobs: Vec<GatewayPortfolioJob>,
    pub inbox: Vec<serde_json::Value>,
    pub after: Option<u64>,
    notification_policy: Option<String>,
}
pub(super) struct OwnerCycle {
    pub reply: Reply,
    started: Instant,
    pub active: AtomicBool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TaskCycle {
    version: u32,
    publisher_id: String,
    pub snapshot_present: bool,
    pub commands: Vec<GatewayRelayCommand>,
}
impl OwnerCycle {
    pub fn feature(&self, name: &str) -> bool {
        self.reply.features.get(name) == Some(&1)
    }
}

// Match the relay's sorted-key JSON digest. A digest mismatch can only cause an
// extra publication; authorization and content validation still happen remotely.
pub(super) fn digest(value: &serde_json::Value) -> String {
    fn canonical(value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::Object(object) => {
                let mut keys = object.keys().collect::<Vec<_>>();
                keys.sort();
                format!(
                    "{{{}}}",
                    keys.into_iter()
                        .map(|key| format!(
                            "{}:{}",
                            serde_json::to_string(key).unwrap(),
                            canonical(&object[key])
                        ))
                        .collect::<Vec<_>>()
                        .join(",")
                )
            }
            serde_json::Value::Array(values) => format!(
                "[{}]",
                values.iter().map(canonical).collect::<Vec<_>>().join(",")
            ),
            _ => value.to_string(),
        }
    }
    format!("{:x}", Sha256::digest(canonical(value).as_bytes()))
}
impl GatewayApi {
    /// Opaque local lock identity; token rotation does not create a second owner.
    pub fn owner_lock_key(&self) -> String {
        digest(&serde_json::json!([
            self.url.as_str(),
            self.device_id,
            self.managed.as_ref().map(|b| b.workspace_id.as_str())
        ]))
    }
    pub fn task_bridge_lock_key(&self) -> String {
        digest(&serde_json::json!(["desktop-tasks", self.owner_lock_key()]))
    }
    /// Retain only pairing comparison state across cycles, never cached queues.
    pub fn pairing_snapshot(&self) -> Self {
        Self {
            url: self.url.clone(),
            device_id: self.device_id.clone(),
            token: self.token.clone(),
            managed: self.managed.clone(),
            client: self.client.clone(),
            owner_sync: None,
            task_sync: None,
        }
    }
    pub fn with_task_sync(mut self) -> Result<Self> {
        let reply: TaskCycle =
            match self.request(reqwest::Method::GET, "v1/device/tasks/sync", None) {
                Ok(reply) => reply,
                Err(error)
                    if error
                        .downcast_ref::<HttpFailure>()
                        .is_some_and(|e| e.0 == reqwest::StatusCode::NOT_FOUND) =>
                {
                    return Ok(self);
                }
                Err(error) => return Err(error),
            };
        anyhow::ensure!(
            reply.version == 1
                && reply.publisher_id == self.device_id
                && reply.commands.len() <= 50,
            "Gateway returned an invalid task synchronization batch"
        );
        self.task_sync = Some(reply);
        Ok(self)
    }
    pub fn task_snapshot_present(&self) -> Option<bool> {
        self.task_sync.as_ref().map(|cycle| cycle.snapshot_present)
    }
    pub fn task_cycle_active(&self) -> bool {
        self.task_sync
            .as_ref()
            .is_none_or(|cycle| !cycle.commands.is_empty())
    }
    pub fn with_owner_sync(mut self) -> Result<Self> {
        let reply: Reply = match self.request(reqwest::Method::GET, "v1/device/sync", None) {
            Ok(reply) => reply,
            // Only an older server's missing route falls back. Auth/network or
            // schema failures cannot become a successful legacy cycle.
            Err(error)
                if error
                    .downcast_ref::<HttpFailure>()
                    .is_some_and(|e| e.0 == reqwest::StatusCode::NOT_FOUND) =>
            {
                return Ok(self);
            }
            Err(error) => return Err(error),
        };
        anyhow::ensure!(
            reply.version == 1
                && reply.publisher_id == self.device_id
                && reply.commands.len() <= 32
                && reply.uncertain.len() <= 32
                && reply.jobs.len() <= 16
                && reply.inbox.len() <= 16
                && reply.after.is_none_or(|n| n <= 9_007_199_254_740_991)
                && reply
                    .projection_digest
                    .as_ref()
                    .is_none_or(|s| s.len() == 64
                        && s.bytes()
                            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())),
            "Gateway returned an invalid owner synchronization batch"
        );
        let active = !reply.commands.is_empty()
            || !reply.uncertain.is_empty()
            || !reply.jobs.is_empty()
            || !reply.inbox.is_empty();
        self.owner_sync = Some(OwnerCycle {
            reply,
            started: Instant::now(),
            active: AtomicBool::new(active),
        });
        Ok(self)
    }
    /// An initial read check only. Effect-time callers must use has_publish_grant.
    pub fn owner_preflight_grant(&self) -> Result<bool> {
        if self
            .owner_sync
            .as_ref()
            .is_some_and(|cycle| cycle.started.elapsed() < Duration::from_secs(5))
        {
            return Ok(true);
        }
        self.has_publish_grant()
    }
    pub fn owner_cycle_active(&self) -> bool {
        self.owner_sync
            .as_ref()
            .is_none_or(|cycle| cycle.active.load(Ordering::Relaxed))
    }
    /// A policy change must reach the relay even with no new events, because it
    /// cancels disallowed queued alerts. Legacy servers never skip this update.
    pub fn inbox_publication_unchanged(&self, batch: &serde_json::Value) -> bool {
        self.owner_sync.as_ref().is_some_and(|cycle| {
            cycle.reply.after.is_some()
                && cycle.reply.after == batch["after"].as_u64()
                && cycle.reply.after == batch["next"].as_u64()
                && cycle.reply.notification_policy.as_deref() == batch["policy"].as_str()
                && batch["events"]
                    .as_array()
                    .is_some_and(|events| events.is_empty())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    const ID: &str = "10000000-0000-4000-8000-000000000001";
    fn client(
        responses: Vec<(u16, serde_json::Value)>,
    ) -> (GatewayApi, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let thread = std::thread::spawn(move || {
            let mut paths = Vec::new();
            for (status, value) in responses {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut chunk = [0u8; 4096];
                    let len = socket.read(&mut chunk).unwrap();
                    assert!(len > 0);
                    bytes.extend_from_slice(&chunk[..len]);
                    if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                        let len: usize = headers
                            .lines()
                            .find_map(|s| s.strip_prefix("content-length: "))
                            .map(|s| s.parse().unwrap())
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + len {
                            break;
                        }
                    }
                }
                paths.push(
                    String::from_utf8_lossy(&bytes)
                        .lines()
                        .next()
                        .unwrap()
                        .split_whitespace()
                        .nth(1)
                        .unwrap()
                        .to_owned(),
                );
                let body = value.to_string();
                write!(socket, "HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
            }
            paths
        });
        (
            GatewayApi {
                url: gateway_base_url(&url).unwrap(),
                device_id: ID.into(),
                token: "t".repeat(43),
                managed: None,
                client: gateway_client().unwrap(),
                owner_sync: None,
                task_sync: None,
            },
            thread,
        )
    }
    fn batch(projection: &serde_json::Value) -> serde_json::Value {
        serde_json::json!({"version":1,"publisherId":ID,"features":{"automationRelay":1,"portfolioRelay":1,"inboxControl":1,"inboxNotifications":1},"projectionDigest":digest(projection),"commands":[],"uncertain":[],"jobs":[],"inbox":[],"after":0,"notificationPolicy":"all"})
    }
    #[test]
    fn batch_replaces_idle_preflights_and_reads_but_never_effect_time_authorization() {
        let projection = serde_json::json!({"version":1,"revision":1,"projects":[],"definitions":[],"runs":[],"truncated":false});
        let (api, server) = client(vec![
            (200, batch(&projection)),
            (401, serde_json::json!({"error":"revoked"})),
        ]);
        let api = api.with_owner_sync().unwrap();
        assert!(api.owner_preflight_grant().unwrap());
        assert!(api.has_automation_relay().unwrap() && api.has_portfolio_relay().unwrap());
        assert!(api.has_inbox_control().unwrap() && api.has_inbox_notifications().unwrap());
        assert!(api.pending_automations().unwrap().is_empty());
        assert!(api.uncertain_automations().unwrap().is_empty());
        assert!(
            api.pending_portfolio().unwrap().is_empty() && api.pending_inbox().unwrap().is_empty()
        );
        assert_eq!(api.inbox_publication_cursor().unwrap(), Some(0));
        assert!(api.inbox_publication_unchanged(
            &serde_json::json!({"after":0,"next":0,"policy":"all","events":[]})
        ));
        assert!(!api.inbox_publication_unchanged(
            &serde_json::json!({"after":0,"next":0,"policy":"off","events":[]})
        ));
        api.publish_automations(projection).unwrap();
        api.publish_portfolio_owner().unwrap();
        assert!(!api.owner_cycle_active());
        assert!(api.has_publish_grant().is_err());
        assert_eq!(
            server.join().unwrap(),
            ["/v1/device/sync", "/v1/device/session"]
        );
    }
    #[test]
    fn auth_failures_and_foreign_batches_cannot_fall_back_or_leak_queues() {
        for status in [401, 403, 500] {
            let (api, server) = client(vec![(status, serde_json::json!({"error":"denied"}))]);
            assert!(api.with_owner_sync().is_err());
            server.join().unwrap();
        }
        let mut foreign = batch(&serde_json::json!({}));
        foreign["publisherId"] = "other".into();
        let (api, server) = client(vec![(200, foreign)]);
        assert!(api.with_owner_sync().is_err());
        server.join().unwrap();
        let (api, server) = client(vec![(404, serde_json::json!({"error":"older server"}))]);
        assert!(api.with_owner_sync().unwrap().owner_sync.is_none());
        server.join().unwrap();
    }
    #[test]
    fn changed_or_missing_remote_projection_is_not_suppressed() {
        let mut projection = serde_json::json!({"version":1,"revision":1});
        let response = batch(&projection);
        projection["revision"] = 2.into();
        let (api, server) = client(vec![
            (200, response),
            (200, serde_json::json!({"published":true})),
        ]);
        let api = api.with_owner_sync().unwrap();
        api.publish_automations(projection).unwrap();
        assert!(api.owner_cycle_active());
        assert_eq!(
            server.join().unwrap(),
            ["/v1/device/sync", "/v1/device/automations"]
        );
    }
    #[test]
    fn task_batch_replaces_idle_queue_and_presence_but_not_effect_grants() {
        let (api, server) = client(vec![
            (
                200,
                serde_json::json!({"version":1,"publisherId":ID,"snapshotPresent":true,"commands":[]}),
            ),
            (401, serde_json::json!({"error":"revoked"})),
        ]);
        let api = api.with_task_sync().unwrap();
        assert_eq!(api.task_snapshot_present(), Some(true));
        assert!(api.pending_commands().unwrap().is_empty());
        assert!(!api.task_cycle_active());
        api.heartbeat().unwrap();
        assert!(api.has_publish_grant().is_err());
        assert_ne!(api.owner_lock_key(), api.task_bridge_lock_key());
        assert_eq!(
            server.join().unwrap(),
            ["/v1/device/tasks/sync", "/v1/device/session"]
        );
    }
    #[test]
    fn task_batch_refuses_foreign_identity_and_auth_failure_without_legacy_fallback() {
        for (status, body) in [
            (401, serde_json::json!({"error":"revoked"})),
            (
                200,
                serde_json::json!({"version":1,"publisherId":"other","snapshotPresent":true,"commands":[]}),
            ),
        ] {
            let (api, server) = client(vec![(status, body)]);
            assert!(api.with_task_sync().is_err());
            server.join().unwrap();
        }
        let (api, server) = client(vec![(404, serde_json::json!({"error":"older server"}))]);
        assert_eq!(api.with_task_sync().unwrap().task_snapshot_present(), None);
        server.join().unwrap();
    }
}
