//! Local and authenticated cloud NeMo-Speech.cpp adapter. All calls must run on a background executor.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{Receiver, RecvTimeoutError, Sender};
use uuid::Uuid;

const MAX_WAV_BYTES: u64 = 50 * 1024 * 1024;
const MAX_RECORD_SECONDS: u32 = 120;

#[derive(Debug)]
pub enum SpeechProgress {
    Ready { live_preview: bool, cloud: bool },
    Preview(String),
}

struct LiveRecognizer {
    _child: Option<LocalSpeechProcess>,
    session: waku_credentials::speech::RealtimeSpeech,
    sample_rate: u32,
    last_preview_at: std::time::Instant,
    pending_preview: Option<String>,
}

struct LocalSpeechProcess(Child);

impl Drop for LocalSpeechProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl LiveRecognizer {
    fn start_local(language: SpeechLanguage, sample_rate: u32) -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        drop(listener);
        let key = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let mut child = LocalSpeechProcess(
            Command::new(executable())
                .args(["serve", "--host", "127.0.0.1", "--port"])
                .arg(port.to_string())
                .args(["--asr-model", "nemotron-3.5", "--no-ui"])
                .env("NEMO_SPEECH_HTTP_API_KEY", &key)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .context("Could not start the local speech preview")?,
        );
        let ready_url = format!("http://127.0.0.1:{port}/ready");
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(400))
            .no_proxy()
            .build()?;
        let deadline = std::time::Instant::now() + Duration::from_secs(35);
        loop {
            if let Ok(response) = client.get(&ready_url).send()
                && response.status().is_success()
            {
                break;
            }
            if child.0.try_wait()?.is_some() || std::time::Instant::now() >= deadline {
                bail!("Local speech preview could not become ready");
            }
            std::thread::sleep(Duration::from_millis(150));
        }
        let service =
            waku_credentials::speech::SpeechService::new(format!("http://127.0.0.1:{port}/"), key)?;
        Ok(Self {
            _child: Some(child),
            session: service.start_live(sample_rate, language.code())?,
            sample_rate,
            last_preview_at: std::time::Instant::now() - Duration::from_millis(125),
            pending_preview: None,
        })
    }
    fn start_cloud(
        service: &waku_credentials::speech::SpeechService,
        language: SpeechLanguage,
        sample_rate: u32,
    ) -> Result<Self> {
        Ok(Self {
            _child: None,
            session: service.start_live(sample_rate, language.code())?,
            sample_rate,
            last_preview_at: std::time::Instant::now() - Duration::from_millis(125),
            pending_preview: None,
        })
    }
    fn send_audio(&mut self, samples: &[i16], progress: &Sender<SpeechProgress>) -> Result<()> {
        for chunk in samples.chunks(self.sample_rate as usize) {
            self.session.send_pcm(chunk)?;
        }
        self.poll_preview(progress)
    }
    fn poll_preview(&mut self, progress: &Sender<SpeechProgress>) -> Result<()> {
        if let Some(preview) = self.session.poll()? {
            self.pending_preview = Some(preview);
        }
        if self.last_preview_at.elapsed() >= Duration::from_millis(125)
            && let Some(preview) = self.pending_preview.take()
        {
            let _ = progress.try_send(SpeechProgress::Preview(preview));
            self.last_preview_at = std::time::Instant::now();
        }
        Ok(())
    }
    fn finish(
        mut self,
        commands: &Receiver<RecordCommand>,
        progress: &Sender<SpeechProgress>,
    ) -> Result<Option<String>> {
        self.session.commit()?;
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while std::time::Instant::now() < deadline {
            if recording_canceled(commands) {
                return Ok(None);
            }
            self.poll_preview(progress)?;
            if let Some(transcript) = self.session.final_transcript()? {
                return Ok(Some(transcript));
            }
        }
        bail!("Speech service did not confirm a final transcript. Try a shorter recording.")
    }
}

fn recording_canceled(commands: &Receiver<RecordCommand>) -> bool {
    matches!(
        commands.try_recv(),
        Ok(RecordCommand::Cancel) | Err(crossbeam_channel::TryRecvError::Disconnected)
    )
}

#[derive(Clone, Copy, Debug)]
pub enum RecordCommand {
    Stop,
    Cancel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpeechLanguage {
    Auto,
    English,
    Hindi,
}

impl SpeechLanguage {
    pub fn code(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::English => "en-US",
            Self::Hindi => "hi-IN",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::English => "English",
            Self::Hindi => "Hindi",
        }
    }
}

pub fn runtime_version() -> Result<String> {
    if let Some(service) = waku_credentials::CredentialService::global().speech_service()? {
        service.test_inventory()?;
        return Ok("Cloud speech - Nemotron 3.5 ASR".into());
    }
    let output = Command::new(executable())
        .arg("--version")
        .output()
        .context("NeMo-Speech.cpp is not installed or is absent from PATH")?;
    if !output.status.success() {
        bail!("NeMo-Speech.cpp could not start (exit {})", output.status);
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if version.is_empty() {
        bail!("NeMo-Speech.cpp returned no version");
    }
    Ok(version)
}

/// Transcribe a WAV on a worker using the configured endpoint, or the local runtime.
pub fn transcribe_wav(path: &Path, language: SpeechLanguage) -> Result<String> {
    if !path.is_file()
        || !path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("wav"))
    {
        bail!("Choose a local .wav file");
    }
    let len = path
        .metadata()
        .context("Could not read the WAV file")?
        .len();
    if len == 0 || len > MAX_WAV_BYTES {
        bail!("Choose a WAV file between 1 byte and 50 MB");
    }
    if let Some(service) = waku_credentials::CredentialService::global().speech_service()? {
        return service.transcribe(std::fs::read(path)?, language.code());
    }
    transcribe_local_wav(path, language)
}

fn transcribe_local_wav(path: &Path, language: SpeechLanguage) -> Result<String> {
    let output = Command::new(executable())
        .args(["--quiet", "transcribe"])
        .arg(path)
        .args([
            "--model",
            "nemotron-3.5",
            "--language",
            language.code(),
            "--stream",
        ])
        .output()
        .context("NeMo-Speech.cpp is not installed or is absent from PATH")?;
    if !output.status.success() {
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        let diagnostic = diagnostic.trim().chars().take(600).collect::<String>();
        bail!("Local transcription failed: {diagnostic}");
    }
    let transcript = String::from_utf8(output.stdout)
        .context("NeMo-Speech.cpp returned text that is not UTF-8")?
        .trim()
        .to_owned();
    if transcript.is_empty() {
        bail!("No speech was recognized in this recording");
    }
    Ok(transcript)
}

/// Locate the app-bundled runtime first, then documented per-user installs.
/// The latter matters when an installer has updated PATH after ion launched.
fn executable() -> PathBuf {
    let name = if cfg!(windows) {
        "nemo-speech.exe"
    } else {
        "nemo-speech"
    };
    if let Ok(app) = std::env::current_exe()
        && let Some(parent) = app.parent()
    {
        for bundled in [
            parent.join("nemo-speech").join("bin").join(name),
            parent.join(name),
        ] {
            if bundled.is_file() {
                return bundled;
            }
        }
    }
    #[cfg(windows)]
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let installed = PathBuf::from(local)
            .join("Programs")
            .join("NeMoSpeech")
            .join("bin")
            .join(name);
        if installed.is_file() {
            return installed;
        }
    }
    #[cfg(not(windows))]
    if let Some(home) = dirs::home_dir() {
        let installed = home.join(".local").join("bin").join(name);
        if installed.is_file() {
            return installed;
        }
    }
    PathBuf::from(name)
}

/// Idempotent first-use model preparation. NeMo-Speech.cpp verifies the
/// artifact against its pinned size and SHA-256 before returning success.
pub fn prepare_model() -> Result<()> {
    if let Some(service) = waku_credentials::CredentialService::global().speech_service()? {
        return service.test_inventory();
    }
    runtime_version()?;
    let output = Command::new(executable())
        .args(["pull", "nemotron-3.5"])
        .output()
        .context("Could not prepare the Nemotron 3.5 model")?;
    if !output.status.success() {
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        let diagnostic = diagnostic.trim().chars().take(600).collect::<String>();
        bail!("Nemotron 3.5 model setup failed: {diagnostic}");
    }
    Ok(())
}

struct TemporaryWav(PathBuf);

impl Drop for TemporaryWav {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub fn record_and_transcribe(
    commands: Receiver<RecordCommand>,
    language: SpeechLanguage,
    progress: Sender<SpeechProgress>,
) -> Result<Option<String>> {
    record_microphone(commands, language, &progress)
}

/// Called only on a blocking worker. The callback holds a small in-memory
/// buffer; temporary WAV data is local and removed after transcription.
fn record_microphone(
    commands: Receiver<RecordCommand>,
    language: SpeechLanguage,
    progress: &Sender<SpeechProgress>,
) -> Result<Option<String>> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .context("No microphone is available. Connect one and allow microphone access.")?;
    let supported = device
        .default_input_config()
        .context("Could not read the microphone format or permission")?;
    let sample_rate = supported.sample_rate();
    if !(8_000..=96_000).contains(&sample_rate) {
        bail!(
            "Microphone sample rate {sample_rate} Hz is outside NeMo-Speech.cpp's 8–96 kHz range"
        );
    }
    let channels = usize::from(supported.channels());
    if channels == 0 {
        bail!("Microphone has no audio channels");
    }
    let max_samples = sample_rate as usize * MAX_RECORD_SECONDS as usize;
    let samples = Arc::new(Mutex::new(Vec::<i16>::with_capacity(
        max_samples.min(sample_rate as usize * 30),
    )));
    let stream_error = Arc::new(Mutex::new(None::<String>));
    let config = supported.config();
    let service = waku_credentials::CredentialService::global().speech_service()?;
    let cloud = service.is_some();
    let mut live = if let Some(service) = &service {
        Some(LiveRecognizer::start_cloud(service, language, sample_rate)?)
    } else {
        LiveRecognizer::start_local(language, sample_rate).ok()
    };
    if recording_canceled(&commands) {
        return Ok(None);
    }
    let error_slot = stream_error.clone();
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => {
            let samples = samples.clone();
            device.build_input_stream(
                config,
                move |data: &[f32], _| {
                    append_mono(data, channels, max_samples, &samples, |s| s);
                },
                move |error| *error_slot.lock().unwrap() = Some(error.to_string()),
                None,
            )?
        }
        cpal::SampleFormat::I16 => {
            let samples = samples.clone();
            device.build_input_stream(
                config,
                move |data: &[i16], _| {
                    append_mono(data, channels, max_samples, &samples, |s| {
                        s as f32 / 32768.0
                    });
                },
                move |error| *error_slot.lock().unwrap() = Some(error.to_string()),
                None,
            )?
        }
        cpal::SampleFormat::U16 => {
            let samples = samples.clone();
            device.build_input_stream(
                config,
                move |data: &[u16], _| {
                    append_mono(data, channels, max_samples, &samples, |s| {
                        (s as f32 - 32768.0) / 32768.0
                    });
                },
                move |error| *error_slot.lock().unwrap() = Some(error.to_string()),
                None,
            )?
        }
        format => bail!("Microphone sample format {format} is unsupported"),
    };
    stream
        .play()
        .context("Could not start the microphone. Check OS permission.")?;
    let _ = progress.try_send(SpeechProgress::Ready {
        live_preview: live.is_some(),
        cloud,
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(u64::from(MAX_RECORD_SECONDS));
    let mut sent_samples = 0usize;
    let decision = loop {
        if let Some(error) = stream_error.lock().unwrap().take() {
            bail!("Microphone stopped: {error}");
        }
        let command = commands.recv_timeout(Duration::from_millis(100));
        if matches!(
            command,
            Ok(RecordCommand::Cancel) | Err(RecvTimeoutError::Disconnected)
        ) {
            break RecordCommand::Cancel;
        }
        if let Some(recognizer) = live.as_mut() {
            let fresh = {
                let recorded = samples.lock().unwrap();
                let fresh = recorded[sent_samples..].to_vec();
                sent_samples = recorded.len();
                fresh
            };
            if let Err(error) = recognizer.send_audio(&fresh, progress) {
                if cloud {
                    return Err(error);
                }
                live = None;
                let _ = progress.try_send(SpeechProgress::Ready {
                    live_preview: false,
                    cloud: false,
                });
            }
        }
        if samples.lock().unwrap().len() >= max_samples || std::time::Instant::now() >= deadline {
            break RecordCommand::Stop;
        }
        match command {
            Ok(command) => break command,
            Err(RecvTimeoutError::Disconnected) => break RecordCommand::Cancel,
            Err(RecvTimeoutError::Timeout) => {}
        }
    };
    drop(stream);
    if matches!(decision, RecordCommand::Cancel) {
        return Ok(None);
    }
    let samples = std::mem::take(&mut *samples.lock().unwrap());
    if samples.is_empty() {
        bail!("No microphone audio was captured");
    }
    if recording_canceled(&commands) {
        return Ok(None);
    }
    if let Some(mut recognizer) = live {
        match recognizer.send_audio(&samples[sent_samples..], progress) {
            Ok(()) => match recognizer.finish(&commands, progress) {
                Ok(result) => return Ok(result),
                Err(error) if cloud => return Err(error),
                Err(_) => {}
            },
            Err(error) if cloud => return Err(error),
            Err(_) => {}
        }
    }
    if recording_canceled(&commands) {
        return Ok(None);
    }
    // Offline preview can degrade to the existing local file recognizer. A configured
    // cloud session never re-uploads audio or switches destination after a failure.
    let wav =
        TemporaryWav(std::env::temp_dir().join(format!("ion-dictation-{}.wav", Uuid::new_v4())));
    write_wav(&wav.0, sample_rate, &samples)?;
    transcribe_local_wav(&wav.0, language).map(Some)
}

fn append_mono<T: Copy>(
    data: &[T],
    channels: usize,
    max_samples: usize,
    samples: &Mutex<Vec<i16>>,
    to_f32: impl Fn(T) -> f32,
) {
    let mut recorded = samples.lock().unwrap();
    for frame in data.chunks_exact(channels) {
        if recorded.len() >= max_samples {
            break;
        }
        let mono = frame.iter().copied().map(&to_f32).sum::<f32>() / channels as f32;
        recorded.push((mono.clamp(-1.0, 1.0) * 32767.0) as i16);
    }
}

fn write_wav(path: &Path, sample_rate: u32, samples: &[i16]) -> Result<()> {
    let data_bytes = u32::try_from(samples.len() * 2).context("Recording is too long")?;
    let mut file = BufWriter::with_capacity(
        64 * 1024,
        File::create(path).context("Could not create temporary recording")?,
    );
    file.write_all(b"RIFF")?;
    file.write_all(&(36 + data_bytes).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16u32.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&sample_rate.to_le_bytes())?;
    file.write_all(&(sample_rate * 2).to_le_bytes())?;
    file.write_all(&2u16.to_le_bytes())?;
    file.write_all(&16u16.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&data_bytes.to_le_bytes())?;
    for sample in samples {
        file.write_all(&sample.to_le_bytes())?;
    }
    file.flush()?;
    Ok(())
}
