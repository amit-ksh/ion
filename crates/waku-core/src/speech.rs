//! Short remote recordings transcribed on the daemon host with NeMo-Speech.cpp.

use std::fs::{self, OpenOptions};
use std::io::Read as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use uuid::Uuid;

const MAX_WAV_BYTES: usize = 8 * 1024 * 1024;
const MAX_SECONDS: usize = 60;
static BUSY: AtomicBool = AtomicBool::new(false);

/// A crashed process can leave a recording behind. Sweep only our own old
/// temporary names when the daemon starts, outside any request or frame path.
pub fn cleanup_stale_recordings() {
    let Ok(entries) = fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(id) = name.strip_prefix("ion-phone-dictation-") else {
            continue;
        };
        let valid_suffix = id.strip_suffix(".wav").or_else(|| id.strip_suffix(".txt"));
        if !valid_suffix.is_some_and(|id| Uuid::parse_str(id).is_ok()) {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !metadata.file_type().is_file() {
            continue;
        }
        let stale = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age > Duration::from_secs(10 * 60));
        if stale {
            let _ = fs::remove_file(path);
        }
    }
}

struct RecordingFiles {
    wav: PathBuf,
}

impl Drop for RecordingFiles {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.wav);
    }
}

struct BusyGuard;
impl Drop for BusyGuard {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}

fn validate_wav(bytes: &[u8]) -> Result<()> {
    if bytes.len() < 44
        || bytes.len() > MAX_WAV_BYTES
        || &bytes[0..4] != b"RIFF"
        || &bytes[8..12] != b"WAVE"
        || &bytes[12..16] != b"fmt "
        || u32::from_le_bytes(bytes[16..20].try_into()?) != 16
        || u16::from_le_bytes(bytes[20..22].try_into()?) != 1
        || u16::from_le_bytes(bytes[22..24].try_into()?) != 1
        || u16::from_le_bytes(bytes[34..36].try_into()?) != 16
        || &bytes[36..40] != b"data"
    {
        bail!("Dictation requires a mono 16-bit PCM WAV recording of at most 60 seconds");
    }
    let rate = u32::from_le_bytes(bytes[24..28].try_into()?) as usize;
    let byte_rate = u32::from_le_bytes(bytes[28..32].try_into()?) as usize;
    let data_len = u32::from_le_bytes(bytes[40..44].try_into()?) as usize;
    let riff_len = u32::from_le_bytes(bytes[4..8].try_into()?) as usize;
    if !(16_000..=48_000).contains(&rate)
        || byte_rate != rate * 2
        || u16::from_le_bytes(bytes[32..34].try_into()?) != 2
        || data_len == 0
        || data_len % 2 != 0
        || data_len > rate * 2 * MAX_SECONDS
        || data_len + 44 != bytes.len()
        || riff_len + 8 != bytes.len()
    {
        bail!("Invalid or overlong dictation WAV recording");
    }
    Ok(())
}

fn executable() -> PathBuf {
    let name = if cfg!(windows) {
        "nemo-speech.exe"
    } else {
        "nemo-speech"
    };
    if let Ok(app) = std::env::current_exe()
        && let Some(parent) = app.parent()
    {
        for candidate in [
            parent.join("nemo-speech").join("bin").join(name),
            parent.join(name),
        ] {
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    #[cfg(windows)]
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let candidate = PathBuf::from(local)
            .join("Programs/NeMoSpeech/bin")
            .join(name);
        if candidate.is_file() {
            return candidate;
        }
    }
    #[cfg(not(windows))]
    if let Some(home) = dirs::home_dir() {
        let candidate = home.join(".local/bin").join(name);
        if candidate.is_file() {
            return candidate;
        }
    }
    PathBuf::from(name)
}

/// Runs on a daemon request worker, never the UI thread. Temporary audio is
/// deleted on every normal exit, including CLI failure and timeout.
pub fn transcribe_wav(bytes: &[u8], language: &str) -> Result<String> {
    validate_wav(bytes)?;
    let language = match language {
        "auto" | "en-US" | "hi-IN" => language,
        _ => bail!("Unsupported dictation language"),
    };
    if BUSY.swap(true, Ordering::AcqRel) {
        bail!("Dictation is already running on this daemon")
    }
    let _busy = BusyGuard;
    let id = Uuid::new_v4();
    let dir = std::env::temp_dir();
    let paths = RecordingFiles {
        wav: dir.join(format!("ion-phone-dictation-{id}.wav")),
    };
    let mut wav = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&paths.wav)
        .context("Could not create temporary dictation recording")?;
    use std::io::Write as _;
    wav.write_all(bytes)?;
    drop(wav);
    let mut child = Command::new(executable())
        .args(["--quiet", "transcribe"])
        .arg(&paths.wav)
        .args([
            "--model",
            "nemotron-3.5",
            "--language",
            language,
            "--stream",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("NeMo-Speech.cpp is not installed on the daemon host")?;
    let mut stdout = child
        .stdout
        .take()
        .context("Could not read local dictation")?;
    let (output_sender, output_receiver) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let result = (|| -> std::io::Result<(Vec<u8>, bool)> {
            let mut output = Vec::new();
            let mut exceeded = false;
            let mut buffer = [0_u8; 4096];
            loop {
                let count = stdout.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                let remaining = (64 * 1024_usize).saturating_sub(output.len());
                output.extend_from_slice(&buffer[..count.min(remaining)]);
                exceeded |= count > remaining;
            }
            Ok((output, exceeded))
        })();
        let _ = output_sender.send(result);
    });
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    bail!("Local dictation failed; check NeMo-Speech.cpp on the daemon host")
                }
                break;
            }
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error).context("Could not check local dictation");
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("Local dictation timed out");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let (output, exceeded) = output_receiver
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .context("Could not read local dictation before the timeout")??;
    if exceeded {
        bail!("Dictation output is too large")
    }
    let text = String::from_utf8(output).context("Invalid local dictation output")?;
    let text = text.trim();
    if text.is_empty() {
        bail!("No speech was recognized in this recording")
    }
    Ok(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_bounded_pcm_wav() {
        let mut wav = vec![0; 48];
        wav[0..4].copy_from_slice(b"RIFF");
        wav[4..8].copy_from_slice(&40u32.to_le_bytes());
        wav[8..16].copy_from_slice(b"WAVEfmt ");
        wav[16..20].copy_from_slice(&16u32.to_le_bytes());
        wav[20..22].copy_from_slice(&1u16.to_le_bytes());
        wav[22..24].copy_from_slice(&1u16.to_le_bytes());
        wav[24..28].copy_from_slice(&16_000u32.to_le_bytes());
        wav[28..32].copy_from_slice(&32_000u32.to_le_bytes());
        wav[32..34].copy_from_slice(&2u16.to_le_bytes());
        wav[34..36].copy_from_slice(&16u16.to_le_bytes());
        wav[36..40].copy_from_slice(b"data");
        wav[40..44].copy_from_slice(&4u32.to_le_bytes());
        assert!(validate_wav(&wav).is_ok());
        wav[24..28].copy_from_slice(&96_000u32.to_le_bytes());
        assert!(validate_wav(&wav).is_err());
    }
}
