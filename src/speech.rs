//! Local NeMo-Speech.cpp adapter. All calls must run on a background executor.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{Receiver, RecvTimeoutError};
use uuid::Uuid;

const MAX_WAV_BYTES: u64 = 50 * 1024 * 1024;
const MAX_RECORD_SECONDS: u32 = 120;

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

/// Transcribe an existing local WAV. The model must be pulled in advance for
/// offline use; no audio file is sent to a cloud API by this command.
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
) -> Result<Option<String>> {
    let wav = match record_microphone(commands)? {
        Some(wav) => wav,
        None => return Ok(None),
    };
    transcribe_wav(&wav.0, language).map(Some)
}

/// Called only on a blocking worker. The callback holds a small in-memory
/// buffer; temporary WAV data is local and removed after transcription.
fn record_microphone(commands: Receiver<RecordCommand>) -> Result<Option<TemporaryWav>> {
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
    let decision = loop {
        if let Some(error) = stream_error.lock().unwrap().take() {
            bail!("Microphone stopped: {error}");
        }
        if samples.lock().unwrap().len() >= max_samples {
            break RecordCommand::Stop;
        }
        match commands.recv_timeout(Duration::from_millis(100)) {
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
    let path = std::env::temp_dir().join(format!("ion-dictation-{}.wav", Uuid::new_v4()));
    let wav = TemporaryWav(path);
    write_wav(&wav.0, sample_rate, &samples)?;
    Ok(Some(wav))
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
