use super::*;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Instant;

pub struct LoginAttempt {
    account: Account<OsVault, Network>,
    listener: TcpListener,
    authorize: String,
    verifier: String,
    state: String,
    epoch: String,
    cancelled: Arc<AtomicBool>,
}
impl LoginAttempt {
    pub fn authorize_url(&self) -> &str {
        &self.authorize
    }
    pub fn cancellation(&self) -> Arc<AtomicBool> {
        self.cancelled.clone()
    }
    pub fn finish(self) -> Result<AccountStatus> {
        let code = receive(
            &self.listener,
            &self.state,
            &self.cancelled,
            Duration::from_secs(300),
        )?;
        complete(
            &self.account,
            &self.epoch,
            &code,
            &self.verifier,
            &self.cancelled,
        )?;
        self.account.status()
    }
}

pub(super) fn begin(account: Account<OsVault, Network>) -> Result<LoginAttempt> {
    let listener = TcpListener::bind("127.0.0.1:43891").map_err(|_| {
        anyhow::anyhow!("Another desktop sign-in is open; finish or cancel it first")
    })?;
    listener.set_nonblocking(true)?;
    let meta = account.http.json(
        "GET",
        &format!("{}/v1/bootstrap", account.config.origin),
        None,
        None,
    )?;
    ensure!(
        meta["auth"]["issuer"] == ISSUER
            && meta["auth"]["audience"] == AUDIENCE
            && meta["auth"]["desktopClientId"] == DESKTOP_CLIENT_ID
            && meta["auth"]["pkce"] == "S256"
            && meta["services"]["gateway"] == format!("{}/v1/gateway", account.config.origin),
        "Cloud sign-in configuration does not match this app"
    );
    let verifier = random()?;
    let state = random()?;
    let epoch = {
        let _guard = account.lock()?;
        let mut current = account.read()?;
        ensure!(
            current.session.is_none(),
            "Already signed in; sign out first to change accounts"
        );
        current.epoch = random()?;
        account.store().write(&current)?;
        current.epoch
    };
    let authorize = authorization_url(&verifier, &state)?;
    Ok(LoginAttempt {
        account,
        listener,
        authorize,
        verifier,
        state,
        epoch,
        cancelled: Arc::new(AtomicBool::new(false)),
    })
}
pub(super) fn authorization_url(verifier: &str, state: &str) -> Result<String> {
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let mut authorize = Url::parse(&format!("{ISSUER}authorize"))?;
    authorize.query_pairs_mut().extend_pairs([
        ("response_type", "code"),
        ("client_id", DESKTOP_CLIENT_ID),
        ("audience", AUDIENCE),
        ("redirect_uri", CALLBACK),
        ("scope", "openid profile email offline_access ion:gateway"),
        ("prompt", "login"),
        ("code_challenge_method", "S256"),
        ("code_challenge", &challenge),
        ("state", state),
    ]);
    Ok(authorize.to_string())
}
/// An unknown callback does not consume the pending login. Provider denial with
/// the correct state does; no provider text or callback secrets reach the UI.
pub(super) fn callback(target: &str, expected_state: &str) -> Result<Option<String>> {
    if !target.starts_with("/ion/callback?") || target.len() > 6000 {
        return Ok(None);
    }
    let url = Url::parse(&format!("http://127.0.0.1:43891{target}"))
        .map_err(|_| anyhow::anyhow!("Invalid sign-in callback"))?;
    if url.path() != "/ion/callback" || url.fragment().is_some() {
        return Ok(None);
    }
    let values: Vec<_> = url.query_pairs().collect();
    let states: Vec<_> = values.iter().filter(|(key, _)| key == "state").collect();
    if states.len() != 1 || states[0].1 != expected_state {
        return Ok(None);
    }
    ensure!(
        !values.iter().any(|(key, _)| key == "error"),
        "Sign-in was declined; try again when ready"
    );
    let codes: Vec<_> = values.iter().filter(|(key, _)| key == "code").collect();
    ensure!(
        codes.len() == 1 && !codes[0].1.is_empty() && codes[0].1.len() <= 4096,
        "Sign-in callback was invalid"
    );
    Ok(Some(codes[0].1.to_string()))
}
fn reply(stream: &mut TcpStream, accepted: bool) {
    let body = if accepted {
        "Sign-in received. You can return to Ion."
    } else {
        "This sign-in request was not accepted. Return to Ion."
    };
    let status = if accepted {
        "200 OK"
    } else {
        "400 Bad Request"
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; frame-ancestors 'none'\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}
pub(super) fn receive(
    listener: &TcpListener,
    state: &str,
    cancelled: &AtomicBool,
    timeout: Duration,
) -> Result<String> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        ensure!(!cancelled.load(Ordering::Acquire), "Sign-in cancelled");
        match listener.accept() {
            Ok((mut stream, peer)) => {
                if !peer.ip().is_loopback() {
                    continue;
                }
                stream.set_nonblocking(false)?;
                stream.set_read_timeout(Some(Duration::from_millis(300)))?;
                stream.set_write_timeout(Some(Duration::from_millis(300)))?;
                let mut bytes = Vec::new();
                let mut chunk = [0u8; 1024];
                let connection_deadline = Instant::now() + Duration::from_millis(600);
                while bytes.len() < 8192
                    && !bytes.windows(4).any(|v| v == b"\r\n\r\n")
                    && Instant::now() < connection_deadline.min(deadline)
                    && !cancelled.load(Ordering::Acquire)
                {
                    match stream.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(size) => bytes.extend_from_slice(&chunk[..size]),
                    }
                }
                if bytes.len() > 8192 || !bytes.windows(4).any(|v| v == b"\r\n\r\n") {
                    reply(&mut stream, false);
                    continue;
                }
                ensure!(!cancelled.load(Ordering::Acquire), "Sign-in cancelled");
                let request = String::from_utf8_lossy(&bytes);
                let expected_host = listener.local_addr()?.to_string();
                let hosts: Vec<_> = request
                    .lines()
                    .skip(1)
                    .filter_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("host").then_some(value.trim())
                    })
                    .collect();
                if hosts.len() != 1 || hosts[0] != expected_host {
                    reply(&mut stream, false);
                    continue;
                }
                let mut line = request
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .split_whitespace();
                if line.next() != Some("GET") {
                    reply(&mut stream, false);
                    continue;
                }
                let target = line.next().unwrap_or_default();
                match callback(target, state) {
                    Ok(Some(code)) => {
                        reply(&mut stream, true);
                        return Ok(code);
                    }
                    Ok(None) => reply(&mut stream, false),
                    Err(error) => {
                        reply(&mut stream, false);
                        return Err(error);
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50))
            }
            Err(_) => bail!("Desktop sign-in callback is unavailable"),
        }
    }
    bail!("Sign-in timed out; return to Ion and try again")
}
pub(super) fn complete<V: Vault, H: Http>(
    account: &Account<V, H>,
    epoch: &str,
    code: &str,
    verifier: &str,
    cancelled: &AtomicBool,
) -> Result<()> {
    ensure!(!cancelled.load(Ordering::Acquire), "Sign-in cancelled");
    {
        let _guard = account.lock()?;
        ensure!(
            account.read()?.epoch == epoch,
            "This sign-in was superseded"
        );
    }
    // A code exchange is one-shot. Its outcome is never blindly replayed.
    let response = account.http.form(
        &format!("{ISSUER}oauth/token"),
        &[
            ("grant_type", "authorization_code"),
            ("client_id", DESKTOP_CLIENT_ID),
            ("redirect_uri", CALLBACK),
            ("code_verifier", verifier),
            ("code", code),
        ],
    )?;
    let session = account.tokens(&response, None)?;
    let _guard = account.lock()?;
    let mut current = account.read()?;
    ensure!(
        current.epoch == epoch && !cancelled.load(Ordering::Acquire),
        "This sign-in was cancelled or superseded"
    );
    current.session = Some(session);
    account.store().write(&current)
}
