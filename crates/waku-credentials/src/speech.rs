//! Authenticated personal speech endpoint. Call only on a worker.
use super::*;
use std::io::Read;

mod realtime;
pub use realtime::RealtimeSpeech;
pub const MODEL: &str = "nvidia/nemotron-3.5-asr-streaming-0.6b";
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechService {
    pub base_url: String,
    token: String,
}
impl SpeechService {
    pub fn new(base_url: String, token: String) -> Result<Self> {
        gateway_base_url(&base_url)?;
        anyhow::ensure!(
            !token.trim().is_empty() && !token.contains(['\r', '\n']),
            "Enter the speech service token"
        );
        Ok(Self { base_url, token })
    }
    pub(crate) fn validate_saved(&self) -> Result<()> {
        Self::new(self.base_url.clone(), self.token.clone())?;
        Ok(())
    }
    fn client(&self) -> Result<reqwest::blocking::Client> {
        Ok(reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .redirect(reqwest::redirect::Policy::none())
            .build()?)
    }
    pub fn test(&self) -> Result<()> {
        let response = self
            .client()?
            .get(gateway_base_url(&self.base_url)?.join("v1/models")?)
            .bearer_auth(&self.token)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .context("Could not reach the speech service")?;
        anyhow::ensure!(
            response.status().is_success(),
            "Speech service rejected the connection (HTTP {})",
            response.status()
        );
        let body: serde_json::Value = serde_json::from_reader(response.take(64 * 1024))
            .context("Invalid speech model inventory")?;
        anyhow::ensure!(
            body["data"]
                .as_array()
                .is_some_and(|items| items.iter().any(|item| item["id"]
                    .as_str()
                    .is_some_and(|id| id.contains("nemotron-3.5") || id.contains("nemotron-3_5")))),
            "Load the Nemotron 3.5 ASR model on the speech service"
        );
        Ok(())
    }
    /// Open an authenticated PCM stream on a blocking worker. No redirects or retries.
    pub fn start_live(&self, sample_rate: u32, language: &str) -> Result<RealtimeSpeech> {
        RealtimeSpeech::connect(self, sample_rate, language)
    }
    pub fn transcribe(&self, bytes: Vec<u8>, language: &str) -> Result<String> {
        anyhow::ensure!(
            !bytes.is_empty() && bytes.len() <= 50 * 1024 * 1024,
            "Choose a WAV smaller than 50 MB"
        );
        anyhow::ensure!(
            matches!(language, "auto" | "en-US" | "hi-IN"),
            "Unsupported speech language"
        );
        let boundary = format!(
            "ion{:x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        );
        let mut body = Vec::with_capacity(bytes.len() + 1024);
        for (name, value) in [
            ("model", MODEL),
            ("language", language),
            ("response_format", "json"),
        ] {
            body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes());
        }
        body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"dictation.wav\"\r\nContent-Type: audio/wav\r\n\r\n").as_bytes());
        body.extend(bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        let response = self
            .client()?
            .post(gateway_base_url(&self.base_url)?.join("v1/audio/transcriptions")?)
            .bearer_auth(&self.token)
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(body)
            .send()
            .context("Could not reach the speech service")?;
        anyhow::ensure!(
            response.status().is_success(),
            "Speech transcription failed (HTTP {})",
            response.status()
        );
        let result: serde_json::Value =
            serde_json::from_reader(response.take(64 * 1024)).context("Invalid speech response")?;
        let text = result["text"]
            .as_str()
            .context("Speech service returned no transcript")?
            .trim();
        anyhow::ensure!(!text.is_empty(), "No speech was recognized");
        Ok(text.to_owned())
    }
}
impl CredentialService {
    pub fn speech_service(&self) -> Result<Option<SpeechService>> {
        let value = if let Some(value) = self
            .volatile
            .lock()
            .unwrap()
            .get(&CredentialKind::SpeechService)
        {
            Some(value.clone())
        } else {
            vault::get(CredentialKind::SpeechService.account())?
        };
        value
            .map(|value| {
                let service: SpeechService =
                    serde_json::from_str(&value).context("Save the speech endpoint again")?;
                SpeechService::new(service.base_url, service.token)
            })
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn rejects_remote_plaintext_and_url_credentials() {
        for url in [
            "http://speech.example/",
            "https://secret@speech.example/",
            "https://speech.example/?token=x",
            "https://speech.example/v1",
        ] {
            assert!(SpeechService::new(url.into(), "token".into()).is_err());
        }
        assert!(SpeechService::new("https://speech.example/".into(), "\r\n".into()).is_err());
        assert!(SpeechService::new("https://speech.example/".into(), "token".into()).is_ok());
    }
    #[test]
    fn sends_authenticated_wav_and_does_not_follow_redirects() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let service = SpeechService::new(
            format!("http://{}/", listener.local_addr().unwrap()),
            "test-secret".into(),
        )
        .unwrap();
        let worker = std::thread::spawn(move || {
            for redirect in [false, true] {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 4096];
                loop {
                    let n = stream.read(&mut buffer).unwrap();
                    assert!(n > 0);
                    request.extend_from_slice(&buffer[..n]);
                    if let Some(end) = request.windows(4).position(|v| v == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&request[..end]).to_lowercase();
                        let size = headers
                            .lines()
                            .find_map(|line| line.strip_prefix("content-length: "))
                            .unwrap()
                            .parse::<usize>()
                            .unwrap();
                        if request.len() >= end + 4 + size {
                            break;
                        }
                    }
                }
                let request = String::from_utf8_lossy(&request);
                assert!(request.contains("Bearer test-secret"));
                assert!(request.contains("dictation.wav"));
                assert!(request.contains(MODEL));
                if redirect {
                    stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                } else {
                    let body = r#"{"text":"Hello ion"}"#;
                    write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
                }
            }
        });
        assert_eq!(
            service.transcribe(b"RIFF WAVE".to_vec(), "auto").unwrap(),
            "Hello ion"
        );
        assert!(
            service
                .transcribe(b"RIFF WAVE".to_vec(), "auto")
                .unwrap_err()
                .to_string()
                .contains("302")
        );
        worker.join().unwrap();
    }
}
