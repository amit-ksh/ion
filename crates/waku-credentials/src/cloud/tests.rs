use super::*;
use std::collections::HashMap;
use std::sync::{Mutex, atomic::AtomicUsize};

#[derive(Clone, Default)]
struct MemoryVault {
    values: Arc<Mutex<HashMap<String, String>>>,
    writes: Arc<AtomicUsize>,
    fail_at: Arc<AtomicUsize>,
}
impl Vault for MemoryVault {
    fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(self.values.lock().unwrap().get(key).cloned())
    }
    fn set(&self, key: &str, value: &str) -> Result<()> {
        let count = self.writes.fetch_add(1, Ordering::SeqCst) + 1;
        if self.fail_at.load(Ordering::SeqCst) == count {
            bail!("Fixture vault write failed");
        }
        self.values.lock().unwrap().insert(key.into(), value.into());
        Ok(())
    }
    fn delete(&self, key: &str) -> Result<()> {
        self.values.lock().unwrap().remove(key);
        Ok(())
    }
}
#[derive(Clone, Default)]
struct FakeHttp {
    refreshes: Arc<AtomicUsize>,
    pairs: Arc<AtomicUsize>,
    failed: Arc<AtomicBool>,
    foreign: Arc<AtomicBool>,
    fail_token_commit: Arc<Mutex<Option<MemoryVault>>>,
    cancel_after_form: Arc<Mutex<Option<Arc<AtomicBool>>>>,
    supersede_after_form: Arc<Mutex<Option<MemoryVault>>>,
    calls: Arc<Mutex<Vec<(String, String, Option<Value>)>>>,
}
impl Http for FakeHttp {
    fn json(
        &self,
        method: &str,
        url: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> Result<Value> {
        self.calls
            .lock()
            .unwrap()
            .push((method.into(), url.into(), body.clone()));
        if self.failed.load(Ordering::SeqCst) {
            bail!("Fixture offline");
        }
        if url.ends_with("/v1/session") {
            assert!(token.is_some());
            return Ok(
                json!({"workspaceId": if self.foreign.load(Ordering::SeqCst) { "b".repeat(64) } else { "a".repeat(64) }}),
            );
        }
        if url.ends_with("/v1/apps/catalog") {
            assert!(token.is_some());
            return Ok(json!({"managed":true,"items":[],"nextCursor":null}));
        }
        if url.ends_with("/invites") {
            assert_eq!(body.unwrap()["grants"], json!(["publish"]));
            return Ok(json!({"invite":"one-use-invite"}));
        }
        if url.ends_with("/v1/pair") {
            let index = self.pairs.fetch_add(1, Ordering::SeqCst) + 1;
            return Ok(json!({"id":format!("11111111-1111-4111-8111-{index:012}"),
                "token":"a".repeat(43),"expiresAt":now_ms()?+86_400_000,"grants":["publish"]}));
        }
        if url.ends_with("/revoke") {
            return Ok(json!({"revoked":true}));
        }
        bail!("Unexpected fixture route")
    }
    fn form(&self, url: &str, fields: &[(&str, &str)]) -> Result<Value> {
        assert_eq!(url, format!("{ISSUER}oauth/token"));
        assert!(!fields.iter().any(|(key, _)| *key == "client_secret"));
        if fields
            .iter()
            .any(|(key, value)| *key == "grant_type" && *value == "refresh_token")
        {
            self.refreshes.fetch_add(1, Ordering::SeqCst);
            if let Some(vault) = self.fail_token_commit.lock().unwrap().take() {
                vault
                    .fail_at
                    .store(vault.writes.load(Ordering::SeqCst) + 1, Ordering::SeqCst);
            }
        }
        if self.failed.load(Ordering::SeqCst) {
            bail!("Fixture ambiguous transport failure");
        }
        if let Some(cancel) = self.cancel_after_form.lock().unwrap().take() {
            cancel.store(true, Ordering::Release);
        }
        if let Some(vault) = self.supersede_after_form.lock().unwrap().take() {
            Store::new(&vault, VAULT_KEY).write(&State::empty()?)?;
        }
        Ok(tokens())
    }
    fn revoke(&self, _refresh: &str) -> bool {
        !self.failed.load(Ordering::SeqCst)
    }
}
fn tokens() -> Value {
    json!({"access_token":"test.access.token","refresh_token":"rotatedRefresh","token_type":"Bearer","expires_in":900})
}
fn fixture() -> Account<MemoryVault, FakeHttp> {
    Account {
        config: Config::new("https://cloud.example").unwrap(),
        vault: MemoryVault::default(),
        http: FakeHttp::default(),
        lock_dir: std::env::temp_dir().join(format!("ion-cloud-test-{}", random().unwrap())),
    }
}
fn copy(account: &Account<MemoryVault, FakeHttp>) -> Account<MemoryVault, FakeHttp> {
    Account {
        config: account.config.clone(),
        vault: account.vault.clone(),
        http: account.http.clone(),
        lock_dir: account.lock_dir.clone(),
    }
}
fn seed(account: &Account<MemoryVault, FakeHttp>, expired: bool, connected: bool) {
    let mut session = account.tokens(&tokens(), None).unwrap();
    if expired {
        session.expires_at = 1;
    }
    if connected {
        session.device = Some(Device {
            id: "11111111-1111-4111-8111-111111111111".into(),
            token: "a".repeat(43),
            expires_at: now_ms().unwrap() + 600_000,
        });
    }
    account
        .store()
        .write(&State {
            version: 1,
            epoch: random().unwrap(),
            session: Some(session),
        })
        .unwrap();
}
fn clean(account: Account<MemoryVault, FakeHttp>) {
    let _ = std::fs::remove_dir_all(account.lock_dir);
}

#[test]
fn vault_parts_are_bounded_atomic_and_integrity_checked() {
    let vault = MemoryVault::default();
    let store = Store::new(&vault, "fixture");
    let first = json!({"secret":"a".repeat(5000)});
    store.write(&first).unwrap();
    assert!(
        vault
            .values
            .lock()
            .unwrap()
            .iter()
            .filter(|(key, _)| key.starts_with("fixture/"))
            .all(|(_, v)| v.len() <= 1200)
    );
    assert_eq!(store.read::<Value>().unwrap(), Some(first.clone()));
    vault
        .fail_at
        .store(vault.writes.load(Ordering::SeqCst) + 2, Ordering::SeqCst);
    assert!(store.write(&json!({"secret":"b".repeat(9000)})).is_err());
    assert_eq!(store.read::<Value>().unwrap(), Some(first));
    let manifest: Value = serde_json::from_str(&vault.get("fixture").unwrap().unwrap()).unwrap();
    let key = format!("fixture/{}/0", manifest["slot"]);
    vault.values.lock().unwrap().insert(key, "tampered".into());
    assert!(store.read::<Value>().is_err());
}
#[test]
fn refresh_is_once_across_independent_account_instances_and_worker_threads() {
    let account = fixture();
    seed(&account, true, true);
    let binding = account.binding().unwrap();
    let workers: Vec<_> = (0..8)
        .map(|_| {
            let account = copy(&account);
            let binding = binding.clone();
            std::thread::spawn(move || {
                account.with_publisher(&binding, |token| {
                    assert_eq!(token, "test.access.token");
                    Ok(())
                })
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap().unwrap();
    }
    assert_eq!(account.http.refreshes.load(Ordering::SeqCst), 1);
    assert!(!account.read().unwrap().session.unwrap().refreshing);
    clean(account);
}
#[test]
fn refresh_checkpoint_must_commit_before_consuming_a_token() {
    let account = fixture();
    seed(&account, true, true);
    let binding = account.binding().unwrap();
    account.vault.fail_at.store(
        account.vault.writes.load(Ordering::SeqCst) + 1,
        Ordering::SeqCst,
    );
    assert!(account.with_publisher(&binding, |_| Ok(())).is_err());
    assert_eq!(account.http.refreshes.load(Ordering::SeqCst), 0);
    clean(account);
}
#[test]
fn lost_or_uncommitted_refresh_never_replays_after_restart() {
    for failed_commit in [false, true] {
        let account = fixture();
        seed(&account, true, true);
        let binding = account.binding().unwrap();
        if failed_commit {
            *account.http.fail_token_commit.lock().unwrap() = Some(account.vault.clone());
        } else {
            account.http.failed.store(true, Ordering::SeqCst);
        }
        assert!(account.with_publisher(&binding, |_| Ok(())).is_err());
        assert!(copy(&account).binding().is_err());
        assert_eq!(account.http.refreshes.load(Ordering::SeqCst), 1);
        assert!(account.read().unwrap().session.is_none());
        clean(account);
    }
}
#[test]
fn crashed_refresh_and_foreign_account_identity_fail_closed() {
    let account = fixture();
    seed(&account, true, true);
    let binding = account.binding().unwrap();
    account.http.foreign.store(true, Ordering::SeqCst);
    assert!(account.with_publisher(&binding, |_| Ok(())).is_err());
    assert!(account.read().unwrap().session.is_none());
    seed(&account, true, true);
    let mut state = account.read().unwrap();
    state.session.as_mut().unwrap().refreshing = true;
    account.store().write(&state).unwrap();
    let before = account.http.refreshes.load(Ordering::SeqCst);
    assert!(account.binding().is_err());
    assert_eq!(account.http.refreshes.load(Ordering::SeqCst), before);
    clean(account);
}
#[test]
fn explicit_connect_gets_only_publish_and_replaces_old_grants() {
    let account = fixture();
    seed(&account, false, false);
    account.connect().unwrap();
    let old = account.binding().unwrap();
    account.connect().unwrap();
    let current = account.binding().unwrap();
    assert_ne!(old.device_id, current.device_id);
    assert!(
        account
            .http
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|(_, url, _)| url.ends_with(&format!("/{}/revoke", old.device_id)))
    );
    let called = AtomicBool::new(false);
    assert!(
        account
            .with_publisher(&old, |_| {
                called.store(true, Ordering::SeqCst);
                Ok(())
            })
            .is_err()
    );
    assert!(!called.load(Ordering::SeqCst));
    clean(account);
}
#[test]
fn sign_out_stops_cached_publishers_even_when_remote_revocation_is_offline() {
    let account = fixture();
    seed(&account, false, true);
    let binding = account.binding().unwrap();
    account.http.failed.store(true, Ordering::SeqCst);
    assert!(!account.sign_out().unwrap());
    assert!(!account.status().unwrap().signed_in);
    assert!(
        account
            .with_publisher(&binding, |_| -> Result<()> {
                panic!("stale publisher executed")
            })
            .is_err()
    );
    clean(account);
}
#[test]
fn superseded_and_cancelled_login_cannot_replace_the_account() {
    let account = fixture();
    let state = State::empty().unwrap();
    let epoch = state.epoch.clone();
    account.store().write(&state).unwrap();
    account.sign_out().unwrap();
    assert!(
        oauth::complete(
            &account,
            &epoch,
            "code",
            "verifier",
            &AtomicBool::new(false)
        )
        .is_err()
    );
    let state = account.read().unwrap();
    assert!(
        oauth::complete(
            &account,
            &state.epoch,
            "code",
            "verifier",
            &AtomicBool::new(true)
        )
        .is_err()
    );
    assert!(!account.status().unwrap().signed_in);
    clean(account);
}
#[test]
fn pkce_callback_rejects_wrong_state_duplicates_paths_and_provider_errors() {
    let authorize = Url::parse(
        &oauth::authorization_url("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk", "state").unwrap(),
    )
    .unwrap();
    let params: HashMap<_, _> = authorize.query_pairs().into_owned().collect();
    assert_eq!(
        params["code_challenge"],
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
    assert_eq!(params["code_challenge_method"], "S256");
    assert_eq!(params["redirect_uri"], CALLBACK);
    assert!(!params.contains_key("client_secret"));
    for target in [
        "/other?state=state&code=x",
        "/ion/callback?state=wrong&code=x",
        "/ion/callback?state=state&state=state&code=x",
    ] {
        assert_eq!(oauth::callback(target, "state").unwrap(), None);
    }
    assert_eq!(
        oauth::callback("/ion/callback?state=state&code=one", "state").unwrap(),
        Some("one".into())
    );
    assert!(oauth::callback("/ion/callback?state=state&code=one&code=two", "state").is_err());
    let error = oauth::callback(
        "/ion/callback?state=state&error=denied&error_description=private_detail",
        "state",
    )
    .unwrap_err();
    assert!(!error.to_string().contains("private_detail"));
}
#[test]
fn callback_listener_is_cancellable_and_accepts_only_the_pending_state() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let worker = std::thread::spawn(move || {
        oauth::receive(
            &listener,
            "pending",
            &AtomicBool::new(false),
            Duration::from_secs(3),
        )
    });
    for (state, host, accepted) in [
        ("wrong", address.to_string(), false),
        ("pending", "wrong.invalid".into(), false),
        ("pending", address.to_string(), true),
    ] {
        let mut stream = std::net::TcpStream::connect(address).unwrap();
        std::thread::sleep(Duration::from_millis(20)); // Browser may connect before sending headers.
        stream
            .write_all(
                format!(
                    "GET /ion/callback?state={state}&code=accepted HTTP/1.1\r\nHost: {host}\r\n\r\n"
                )
                .as_bytes(),
            )
            .unwrap();
        let mut reply = String::new();
        stream.read_to_string(&mut reply).unwrap();
        assert!(reply.contains(if accepted {
            "200 OK"
        } else {
            "400 Bad Request"
        }));
        assert!(!reply.contains("code=accepted"));
    }
    assert_eq!(worker.join().unwrap().unwrap(), "accepted");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    assert!(
        oauth::receive(
            &listener,
            "pending",
            &AtomicBool::new(true),
            Duration::from_secs(1)
        )
        .is_err()
    );
}

#[test]
fn sign_out_recovers_corrupt_metadata_and_removes_orphaned_token_parts() {
    let account = fixture();
    seed(&account, false, true);
    account.vault.set(VAULT_KEY, "corrupt metadata").unwrap();
    account
        .vault
        .set(&format!("{VAULT_KEY}/0/31"), "orphaned-refresh-token")
        .unwrap();
    account
        .vault
        .set(&format!("{VAULT_KEY}/1/31"), "orphaned-access-token")
        .unwrap();
    assert!(
        !account.sign_out().unwrap(),
        "Unknown old device must report unconfirmed revocation"
    );
    assert!(!account.status().unwrap().signed_in);
    let values = account.vault.values.lock().unwrap();
    assert_eq!(values.len(), 2); // One empty part and its manifest.
    assert!(values.values().all(|v| !v.contains("orphaned-")));
    drop(values);
    clean(account);
}

#[test]
fn account_lock_coordinates_distinct_desktop_and_daemon_processes() {
    const CHILD: &str = "ION_CLOUD_LOCK_FIXTURE";
    if let Some(directory) = std::env::var_os(CHILD) {
        let directory = PathBuf::from(directory);
        std::fs::write(directory.join("ready"), b"").unwrap();
        let _guard = AccountLock::acquire(&directory, Duration::from_secs(5)).unwrap();
        std::fs::write(directory.join("acquired"), b"").unwrap();
        return;
    }
    let account = fixture();
    let guard = account.lock().unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "cloud::tests::account_lock_coordinates_distinct_desktop_and_daemon_processes",
        ])
        .env(CHILD, &account.lock_dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let start = std::time::Instant::now();
    while !account.lock_dir.join("ready").exists() && start.elapsed() < Duration::from_secs(4) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        account.lock_dir.join("ready").exists(),
        "Child did not start"
    );
    std::thread::sleep(Duration::from_millis(120));
    assert!(
        !account.lock_dir.join("acquired").exists(),
        "Child bypassed the OS lock"
    );
    drop(guard);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            panic!("Child did not acquire released lock");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(account.lock_dir.join("acquired").exists());
    clean(account);
}

#[test]
fn publisher_http_separates_account_and_device_tokens_and_keeps_legacy_routes() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let mut captured = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while captured.len() < 4 && std::time::Instant::now() < deadline {
            let (mut stream, _) = match listener.accept() {
                Ok(v) => v,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(e) => panic!("{e}"),
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0; 2048];
            loop {
                let size = stream.read(&mut chunk).unwrap();
                assert_ne!(size, 0);
                bytes.extend_from_slice(&chunk[..size]);
                assert!(bytes.len() < 65_536);
                if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                    let length = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length: "))
                        .map(|v| v.parse::<usize>().unwrap())
                        .unwrap_or(0);
                    if bytes.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            captured.push(String::from_utf8(bytes).unwrap());
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}").unwrap();
        }
        assert_eq!(captured.len(), 4);
        captured
    });
    let account = fixture();
    seed(&account, true, true);
    let binding = account.binding().unwrap();
    let mut api = crate::GatewayApi::managed(binding.clone()).unwrap();
    // Only this fixture redirects the private transport to a local HTTP recorder.
    api.url = Url::parse(&format!("http://{address}/")).unwrap();
    for (method, path, body) in [
        (
            reqwest::Method::PUT,
            "v1/device/tasks/task-1",
            json!({"task":{"id":"task-1"}}),
        ),
        (
            reqwest::Method::PUT,
            "v1/device/automations",
            json!({"projection":{"definitions":[]}}),
        ),
        (
            reqwest::Method::PUT,
            "v1/device/portfolio",
            json!({"version":1}),
        ),
    ] {
        account
            .with_publisher(&binding, |token| {
                api.dispatch::<Value>(method, path, Some(body), token)
            })
            .unwrap();
    }
    api.managed = None;
    let _: Value = api
        .request(
            reqwest::Method::POST,
            "v1/device/heartbeat",
            Some(json!({})),
        )
        .unwrap();
    let captured = server.join().unwrap();
    for (index, raw) in captured.iter().enumerate() {
        let lower = raw.to_ascii_lowercase();
        assert!(lower.contains(&format!("x-waku-device-id: {}", binding.device_id)));
        if index < 3 {
            assert!(
                raw.lines()
                    .next()
                    .unwrap()
                    .contains(" /v1/gateway/v1/device/")
            );
            assert!(lower.contains("authorization: bearer test.access.token"));
            assert!(lower.contains(&format!("x-ion-device-token: {}", binding.device_token)));
        } else {
            assert!(raw.starts_with("POST /v1/device/heartbeat "));
            assert!(lower.contains(&format!("authorization: bearer {}", binding.device_token)));
            assert!(!lower.contains("x-ion-device-token"));
            assert!(!lower.contains("test.access.token"));
        }
    }
    assert!(captured[0].contains(r#""id":"task-1""#));
    assert_eq!(account.http.refreshes.load(Ordering::SeqCst), 1);
    clean(account);
}

#[test]
fn wrong_origin_or_workspace_binding_is_rejected_before_dispatch() {
    let account = fixture();
    seed(&account, false, true);
    for foreign_origin in [true, false] {
        let mut binding = account.binding().unwrap();
        if foreign_origin {
            binding.origin = "https://other.example".into();
        } else {
            binding.workspace_id = "b".repeat(64);
        }
        assert!(
            account
                .with_publisher(&binding, |_| -> Result<()> {
                    panic!("Mismatched account dispatched")
                })
                .is_err()
        );
    }
    clean(account);
}
#[test]
fn login_cancelled_or_superseded_during_code_exchange_cannot_commit_tokens() {
    for supersede in [true, false] {
        let account = fixture();
        let initial = State::empty().unwrap();
        let epoch = initial.epoch.clone();
        account.store().write(&initial).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        if supersede {
            *account.http.supersede_after_form.lock().unwrap() = Some(account.vault.clone());
        } else {
            *account.http.cancel_after_form.lock().unwrap() = Some(cancel.clone());
        }
        assert!(oauth::complete(&account, &epoch, "one-time-code", "verifier", &cancel).is_err());
        assert!(!account.status().unwrap().signed_in);
        clean(account);
    }
}

#[test]
fn renewable_credentials_are_opaque_not_assumed_to_be_base64url() {
    let account = fixture();
    let mut response = tokens();
    response["refresh_token"] = json!("v1.refresh+token/opaque=");
    assert!(
        account
            .tokens(&response, None)
            .unwrap()
            .valid(&account.config)
    );
    for invalid in ["line\nbreak", " ", "\t", "\u{7f}"] {
        response["refresh_token"] = json!(invalid);
        assert!(account.tokens(&response, None).is_err());
    }
    clean(account);
}
#[cfg(windows)]
#[test]
fn windows_vault_roundtrips_large_chunked_credentials_in_an_isolated_namespace() {
    struct Cleanup(String);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = OsVault.delete(&self.0);
            for slot in 0..=1 {
                for index in 0..32 {
                    let _ = OsVault.delete(&format!("{}/{slot}/{index}", self.0));
                }
            }
        }
    }
    let cleanup = Cleanup(format!("cloud-account-fixture-{}", random().unwrap()));
    OsVault
        .set(&cleanup.0, "small-dummy-entry")
        .expect("Small native vault probe");
    assert_eq!(
        OsVault.get(&cleanup.0).unwrap().as_deref(),
        Some("small-dummy-entry")
    );
    OsVault.delete(&cleanup.0).unwrap();
    let store = Store::new(&OsVault, &cleanup.0);
    let value = json!({"fixture":"dummy-not-a-credential".repeat(1500)});
    store.write(&value).unwrap();
    assert_eq!(store.read::<Value>().unwrap(), Some(value));
    store.reset(&State::empty().unwrap()).unwrap();
    assert!(store.read::<State>().unwrap().unwrap().session.is_none());
}

#[test]
fn managed_inference_refreshes_and_never_borrows_a_replacement_accounts_token() {
    let account = fixture();
    seed(&account, true, false);
    let client = reqwest::blocking::Client::new();
    let expected = "a".repeat(64);
    let request = account
        .inference_request(&client, reqwest::Method::GET, "models", &expected)
        .unwrap()
        .build()
        .unwrap();
    assert!(request.url().as_str().ends_with("/v1/inference/models"));
    assert_eq!(account.http.refreshes.load(Ordering::SeqCst), 1);
    assert!(
        account
            .inference_request(&client, reqwest::Method::GET, "../hosting", &expected)
            .is_err()
    );
    let mut changed = account.read().unwrap();
    changed.session.as_mut().unwrap().workspace_id = "b".repeat(64);
    changed.session.as_mut().unwrap().expires_at = 1;
    account.store().write(&changed).unwrap();
    assert!(
        account
            .inference_request(
                &client,
                reqwest::Method::POST,
                "chat/completions",
                &expected
            )
            .unwrap_err()
            .to_string()
            .contains("account changed")
    );
    assert_eq!(
        account.http.refreshes.load(Ordering::SeqCst),
        1,
        "An old task must not refresh the replacement account"
    );
    account.store().write(&State::empty().unwrap()).unwrap();
    assert!(
        account
            .inference_request(&client, reqwest::Method::GET, "models", &expected)
            .is_err()
    );
    clean(account);
}

#[test]
fn managed_apps_bind_identity_before_refresh_and_never_dispatch_for_another_account() {
    let account = fixture();
    seed(&account, true, false);
    let expected = "a".repeat(64);
    let page = account
        .app_request(&expected, "/v1/apps/catalog", json!({}))
        .unwrap();
    assert_eq!(page["managed"], true);
    assert_eq!(account.http.refreshes.load(Ordering::SeqCst), 1);
    let count = account.http.calls.lock().unwrap().len();
    assert!(
        account
            .app_request(&expected, "/v1/apps/../services/controls", json!({}))
            .is_err()
    );
    let mut state = account.read().unwrap();
    let session = state.session.as_mut().unwrap();
    session.workspace_id = "b".repeat(64);
    session.expires_at = 1;
    account.store().write(&state).unwrap();
    assert!(
        account
            .app_request(&expected, "/v1/apps/connect", json!({}))
            .unwrap_err()
            .to_string()
            .contains("account changed")
    );
    assert_eq!(account.http.refreshes.load(Ordering::SeqCst), 1);
    assert_eq!(account.http.calls.lock().unwrap().len(), count);
    account.store().write(&State::empty().unwrap()).unwrap();
    assert!(
        account
            .app_request(&expected, "/v1/apps/disconnect", json!({}))
            .is_err()
    );
    clean(account);
}

/// Uses the already-signed-in native account. No browser, service enablement,
/// provider tool effect or inference request is permitted by this check.
#[test]
#[ignore = "requires the operator's admitted native Ion login and explicit live-check environment"]
fn live_managed_gateway_catalog_and_stop_controls() {
    assert_eq!(std::env::var("ION_MANAGED_ACCEPTANCE").as_deref(), Ok("1"));
    let status = super::status().expect("Read native Ion account");
    assert!(
        status.signed_in,
        "Sign in to the rebuilt Ion app before native gateway acceptance"
    );
    let workspace = status.workspace_id.expect("Native workspace");
    let controls =
        super::services::controls().expect("Read live owner controls without changing them");
    let page = super::services::app_request(
        &workspace,
        "/v1/apps/catalog",
        json!({"search":"","connectedOnly":true}),
    )
    .expect("Load only this admitted account's connected app catalog");
    assert_eq!(page["managed"], true);
    assert!(page["items"].is_array());
    let usage = super::services::request("GET", "/v1/services/usage", None)
        .expect("Read combined spending");
    assert_eq!(usage["combined"]["unit"], "microUSD");
    assert!(
        usage["combined"]["total"]["spent"]
            .as_u64()
            .is_some_and(|n| n <= 12_000_000)
    );
    if !controls.token_factory {
        let response = super::services::inference_request(
            &reqwest::blocking::Client::new(),
            reqwest::Method::GET,
            "models",
            &workspace,
        )
        .unwrap()
        .send()
        .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::FORBIDDEN);
    }
    if !controls.app_actions {
        // An invalid proposal is not an authorization to dispatch anything.
        assert!(
            super::services::app_request(
                &workspace,
                "/v1/apps/actions/execute",
                json!({"proposalId":"invalid"})
            )
            .is_err()
        );
    }
}
