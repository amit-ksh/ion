//! Local Parakeet TDT dictation. All calls must run on a background executor.

use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
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
    Ready,
    Warning(String),
    Model(waku_credentials::speech::download::Progress),
}

fn recording_canceled(commands: &Receiver<RecordCommand>) -> bool {
    matches!(
        commands.try_recv(),
        Ok(RecordCommand::Cancel | RecordCommand::Stop)
            | Err(crossbeam_channel::TryRecvError::Disconnected)
    )
}

#[derive(Clone, Copy, Debug)]
pub enum RecordCommand {
    Stop,
    Cancel,
}

pub fn runtime_version() -> Result<String> {
    waku_credentials::speech::local::runtime_version()
}

/// Download once and verify the local Parakeet artifact before microphone capture.
pub fn prepare_recording(
    commands: &Receiver<RecordCommand>,
    progress: Option<&Sender<SpeechProgress>>,
) -> Result<()> {
    waku_credentials::speech::local::prepare_model(&|| recording_canceled(commands), &mut |event| {
        if let Some(progress) = progress {
            let _ = progress.try_send(SpeechProgress::Model(event));
        }
    })
    .map(|_| ())
}

pub fn transcribe_wav(path: &Path, commands: &Receiver<RecordCommand>) -> Result<String> {
    if !path.is_file()
        || !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
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
    waku_credentials::speech::local::transcribe_wav(path, &|| recording_canceled(commands))
}

struct TemporaryWav(PathBuf);

impl Drop for TemporaryWav {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub fn record_and_transcribe(
    commands: Receiver<RecordCommand>,
    progress: Sender<SpeechProgress>,
) -> Result<Option<String>> {
    record_microphone(commands, &progress)
}

/// Called only on a blocking worker. The callback holds a small in-memory
/// buffer; temporary WAV data is local and removed after transcription.
fn record_microphone(
    commands: Receiver<RecordCommand>,
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
                {
                    let progress = progress.clone();
                    move |error| microphone_stream_error(error, &error_slot, &progress)
                },
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
                {
                    let progress = progress.clone();
                    move |error| microphone_stream_error(error, &error_slot, &progress)
                },
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
                {
                    let progress = progress.clone();
                    move |error| microphone_stream_error(error, &error_slot, &progress)
                },
                None,
            )?
        }
        format => bail!("Microphone sample format {format} is unsupported"),
    };
    stream
        .play()
        .context("Could not start the microphone. Check OS permission.")?;
    let _ = progress.try_send(SpeechProgress::Ready);
    let deadline = std::time::Instant::now() + Duration::from_secs(u64::from(MAX_RECORD_SECONDS));
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
    let wav =
        TemporaryWav(std::env::temp_dir().join(format!("ion-dictation-{}.wav", Uuid::new_v4())));
    write_wav(&wav.0, sample_rate, &samples)?;
    waku_credentials::speech::local::transcribe_wav(&wav.0, &|| recording_canceled(&commands))
        .map(Some)
}

fn microphone_stream_error(
    error: cpal::Error,
    fatal: &Mutex<Option<String>>,
    progress: &Sender<SpeechProgress>,
) {
    // CPAL reports WASAPI discontinuities while continuing to deliver samples.
    // Route changes and scheduling warnings also leave the stream active.
    // Preserve those samples and cancellation; actual device/permission failures
    // still stop capture and are never overwritten by a later warning.
    if matches!(
        error.kind(),
        cpal::ErrorKind::Xrun | cpal::ErrorKind::DeviceChanged | cpal::ErrorKind::RealtimeDenied
    ) {
        let _ = progress.try_send(SpeechProgress::Warning(
            "Audio was briefly interrupted; recording continues. Check the transcript before sending."
                .into(),
        ));
    } else {
        let mut fatal = fatal.lock().unwrap();
        if fatal.is_none() {
            *fatal = Some(error.to_string());
        }
    }
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
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = BufWriter::with_capacity(
        64 * 1024,
        options
            .open(path)
            .context("Could not create temporary recording")?,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn microphone_discontinuity_keeps_samples_and_reports_warning() {
        let fatal = Mutex::new(None);
        let samples = Mutex::new(Vec::new());
        let (progress, events) = crossbeam_channel::bounded(4);
        append_mono(&[0.5_f32, 0.25], 1, 10, &samples, |s| s);
        microphone_stream_error(cpal::ErrorKind::Xrun.into(), &fatal, &progress);
        append_mono(&[0.125_f32], 1, 10, &samples, |s| s);
        assert!(fatal.lock().unwrap().is_none());
        assert_eq!(samples.lock().unwrap().len(), 3);
        assert!(matches!(events.try_recv(), Ok(SpeechProgress::Warning(_))));
    }

    #[test]
    fn microphone_device_and_permission_failures_remain_fatal() {
        for kind in [
            cpal::ErrorKind::PermissionDenied,
            cpal::ErrorKind::DeviceNotAvailable,
        ] {
            let fatal = Mutex::new(None);
            let (progress, _) = crossbeam_channel::bounded(4);
            microphone_stream_error(kind.into(), &fatal, &progress);
            let first = fatal.lock().unwrap().clone();
            assert!(first.is_some());
            microphone_stream_error(cpal::ErrorKind::Xrun.into(), &fatal, &progress);
            assert_eq!(*fatal.lock().unwrap(), first);
        }
    }
}
