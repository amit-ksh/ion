//! NeMo transcription WebSocket, distinct from its VoiceChat protocol.
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use anyhow::{Result, bail, ensure};
use serde_json::Value;
use tungstenite::client::IntoClientRequest;
use tungstenite::protocol::WebSocketConfig;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

use super::{SpeechService, gateway_base_url};

const MAX_EVENT: usize = 65_536;
const MAX_TEXT: usize = 16_384;

/// Contains only session-local audio/transcript state. Dropping closes the socket;
/// a partial or unacknowledged result is never a final transcript.
pub struct RealtimeSpeech {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    state: TranscriptState,
    sample_rate: u32,
    audio_samples: usize,
    commit_requested: bool,
    last_preview: String,
}

#[derive(Default)]
struct TranscriptState {
    created: bool,
    updated: bool,
    update_requested: bool,
    committed: String,
    partial: String,
    acknowledged: bool,
}

impl TranscriptState {
    fn accept(&mut self, text: &str, finishing: bool) -> Result<()> {
        ensure!(text.len() <= MAX_EVENT, "Speech service event is too large");
        let event: Value = serde_json::from_str(text)
            .map_err(|_| anyhow::anyhow!("Invalid speech service event"))?;
        match event["type"].as_str() {
            Some("session.created") => {
                ensure!(!self.created, "Unexpected repeated speech session");
                self.created = true;
            }
            Some("session.updated") => {
                ensure!(
                    self.created && self.update_requested && !self.updated,
                    "Unexpected speech format acknowledgment"
                );
                self.updated = true;
            }
            Some("conversation.item.input_audio_transcription.delta") => {
                ensure!(
                    self.updated,
                    "Speech preview arrived before format acknowledgment"
                );
                let delta = event["delta"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Invalid speech preview"))?;
                ensure!(
                    self.committed.len() + self.partial.len() + delta.len() + 1 <= MAX_TEXT,
                    "Speech transcript is too long"
                );
                self.partial.push_str(delta);
            }
            Some("conversation.item.input_audio_transcription.completed") => {
                ensure!(
                    self.updated,
                    "Speech segment arrived before format acknowledgment"
                );
                let transcript = event["transcript"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Invalid final speech segment"))?
                    .trim();
                ensure!(
                    self.committed.len() + transcript.len() + 1 <= MAX_TEXT,
                    "Speech transcript is too long"
                );
                if !transcript.is_empty() {
                    if !self.committed.is_empty() {
                        self.committed.push(' ');
                    }
                    self.committed.push_str(transcript);
                }
                self.partial.clear();
            }
            Some("input_audio_buffer.committed") => {
                ensure!(
                    self.updated && finishing,
                    "Unexpected speech commit acknowledgment"
                );
                self.acknowledged = true;
            }
            // Never display arbitrary provider diagnostic text (which can echo audio or secrets).
            Some("error") => bail!("Speech service rejected the streaming session"),
            _ => {}
        }
        Ok(())
    }
    fn preview(&self) -> String {
        format!("{} {}", self.committed, self.partial)
            .trim()
            .to_owned()
    }
    fn final_text(&self) -> Result<Option<String>> {
        if !self.acknowledged {
            return Ok(None);
        }
        ensure!(
            self.partial.is_empty(),
            "Speech service did not finalize the whole recording"
        );
        ensure!(!self.committed.is_empty(), "No speech was recognized");
        Ok(Some(self.committed.clone()))
    }
}

fn tls_connector() -> Result<tungstenite::Connector> {
    let mut roots = rustls::RootCertStore::empty();
    let (added, _) =
        roots.add_parsable_certificates(rustls_native_certs::load_native_certs().certs);
    ensure!(
        added > 0,
        "No trusted speech TLS certificates are available on this computer"
    );
    // Other app dependencies enable both Ring and AWS-LC. Choose this connection's
    // provider explicitly without changing the process-wide default. Standard
    // certificate and hostname verification remain enabled.
    let config = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()?
    .with_root_certificates(roots)
    .with_no_client_auth();
    Ok(tungstenite::Connector::Rustls(std::sync::Arc::new(config)))
}

impl RealtimeSpeech {
    pub(super) fn connect(
        service: &SpeechService,
        sample_rate: u32,
        language: &str,
    ) -> Result<Self> {
        ensure!(
            (8_000..=96_000).contains(&sample_rate),
            "Unsupported speech sample rate"
        );
        ensure!(
            matches!(language, "auto" | "en-US" | "hi-IN"),
            "Unsupported speech language"
        );
        let mut url =
            gateway_base_url(&service.base_url)?.join("v1/audio/transcriptions/realtime")?;
        let scheme = if url.scheme() == "https" { "wss" } else { "ws" };
        url.set_scheme(scheme)
            .map_err(|_| anyhow::anyhow!("Invalid speech URL"))?;
        let host = url
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid speech host"))?
            .trim_start_matches('[')
            .trim_end_matches(']');
        let port = url
            .port_or_known_default()
            .ok_or_else(|| anyhow::anyhow!("Invalid speech port"))?;
        let addresses = (host, port)
            .to_socket_addrs()
            .map_err(|_| anyhow::anyhow!("Could not resolve the speech service"))?;
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut stream = None;
        for address in addresses {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                break;
            };
            if let Ok(candidate) =
                TcpStream::connect_timeout(&address, remaining.min(Duration::from_secs(3)))
            {
                stream = Some(candidate);
                break;
            }
        }
        let stream =
            stream.ok_or_else(|| anyhow::anyhow!("Could not connect to the speech service"))?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        let mut request = url.as_str().into_client_request()?;
        let mut authorization =
            tungstenite::http::HeaderValue::from_str(&format!("Bearer {}", service.token))
                .map_err(|_| anyhow::anyhow!("Invalid speech service token"))?;
        authorization.set_sensitive(true);
        request.headers_mut().insert("Authorization", authorization);
        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_EVENT))
            .max_frame_size(Some(MAX_EVENT));
        // One direct handshake: certificate/hostname verification uses system trust;
        // client_tls_with_config does not follow HTTP redirects.
        let connector = if scheme == "wss" {
            Some(tls_connector()?)
        } else {
            None
        };
        let (socket, _) =
            tungstenite::client_tls_with_config(request, stream, Some(config), connector).map_err(
                |_| {
                    anyhow::anyhow!(
                        "Speech streaming handshake failed; check TLS, token and service version"
                    )
                },
            )?;
        let mut session = Self {
            socket,
            state: TranscriptState::default(),
            sample_rate,
            audio_samples: 0,
            commit_requested: false,
            last_preview: String::new(),
        };
        session
            .tcp()
            .set_read_timeout(Some(Duration::from_millis(25)))?;
        let deadline = Instant::now() + Duration::from_secs(15);
        while !session.state.created && Instant::now() < deadline {
            session.poll()?;
        }
        ensure!(
            session.state.created,
            "Speech service did not create a streaming session"
        );
        session.send(Message::Text(serde_json::json!({
            "type": "session.update", "session": {"sample_rate": sample_rate, "language": language}
        }).to_string().into()))?;
        session.state.update_requested = true;
        while !session.state.updated && Instant::now() < deadline {
            session.poll()?;
        }
        ensure!(
            session.state.updated,
            "Speech service did not accept the microphone format"
        );
        Ok(session)
    }
    fn tcp(&mut self) -> &mut TcpStream {
        match self.socket.get_mut() {
            MaybeTlsStream::Plain(stream) => stream,
            MaybeTlsStream::Rustls(stream) => &mut stream.sock,
            _ => unreachable!("speech transport uses only plain loopback or rustls"),
        }
    }
    fn send(&mut self, message: Message) -> Result<()> {
        self.socket
            .send(message)
            .map_err(|_| anyhow::anyhow!("Speech streaming connection failed"))
    }
    pub fn send_pcm(&mut self, samples: &[i16]) -> Result<()> {
        ensure!(
            !self.commit_requested,
            "The speech recording is already finishing"
        );
        ensure!(
            samples.len() <= self.sample_rate as usize,
            "Speech chunk exceeds one second"
        );
        ensure!(
            self.audio_samples + samples.len() <= self.sample_rate as usize * 120,
            "Speech recording reached the 120-second limit"
        );
        if samples.is_empty() {
            return Ok(());
        }
        let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        self.send(Message::Binary(bytes.into()))?;
        self.audio_samples += samples.len();
        Ok(())
    }
    /// Bounded polling on the microphone worker; no UI-thread I/O.
    pub fn poll(&mut self) -> Result<Option<String>> {
        for _ in 0..8 {
            match self.socket.read() {
                Ok(Message::Text(text)) => self.state.accept(&text, self.commit_requested)?,
                Ok(Message::Ping(_)) => self
                    .socket
                    .flush()
                    .map_err(|_| anyhow::anyhow!("Speech streaming connection failed"))?,
                Ok(Message::Close(_)) => bail!("Speech streaming disconnected before completion"),
                Ok(Message::Binary(_)) => bail!("Unexpected binary speech response"),
                Ok(_) => {}
                Err(tungstenite::Error::Io(error))
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock
                            | std::io::ErrorKind::TimedOut
                            | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    break;
                }
                Err(_) => bail!("Speech streaming connection failed"),
            }
            if self.state.acknowledged {
                break;
            }
        }
        let preview = self.state.preview();
        if preview != self.last_preview {
            self.last_preview = preview.clone();
            return Ok(Some(preview));
        }
        Ok(None)
    }
    pub fn commit(&mut self) -> Result<()> {
        ensure!(
            !self.commit_requested && self.audio_samples > 0,
            "No new microphone audio to finalize"
        );
        self.send(Message::Text(
            r#"{"type":"input_audio_buffer.commit"}"#.into(),
        ))?;
        self.commit_requested = true;
        Ok(())
    }
    pub fn final_transcript(&self) -> Result<Option<String>> {
        self.state.final_text()
    }
}
impl Drop for RealtimeSpeech {
    fn drop(&mut self) {
        let _ = self.tcp().shutdown(Shutdown::Both);
    }
}

#[cfg(test)]
mod tests;
