//! Local NeMo-Speech.cpp adapter. All calls must run on a background executor.

use std::path::Path;
use std::process::Command;

use anyhow::{Context as _, Result, bail};

const MAX_WAV_BYTES: u64 = 50 * 1024 * 1024;

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
    let output = Command::new("nemo-speech")
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
    let output = Command::new("nemo-speech")
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
