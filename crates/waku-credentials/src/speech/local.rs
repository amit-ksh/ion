//! Shared local Parakeet runtime for desktop dictation and paired phones.
//! Every operation here belongs on a blocking worker, never in render paths.
use anyhow::{Context as _, Result, bail, ensure};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub const MODEL: &str = "nvidia/parakeet-tdt_ctc-110m";
pub const MODEL_LABEL: &str = "Parakeet TDT-CTC 110M";
const OUTPUT_LIMIT: usize = 64 * 1024;

fn executable() -> PathBuf {
    let name = if cfg!(windows) {
        "nemo-speech.exe"
    } else {
        "nemo-speech"
    };
    if let Ok(app) = std::env::current_exe()
        && let Some(parent) = app.parent()
    {
        for path in [parent.join("nemo-speech/bin").join(name), parent.join(name)] {
            if path.is_file() {
                return path;
            }
        }
    }
    #[cfg(windows)]
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let path = PathBuf::from(local)
            .join("Programs/NeMoSpeech/bin")
            .join(name);
        if path.is_file() {
            return path;
        }
    }
    #[cfg(not(windows))]
    if let Some(home) = dirs::home_dir() {
        let path = home.join(".local/bin").join(name);
        if path.is_file() {
            return path;
        }
    }
    PathBuf::from(name)
}

fn command() -> Command {
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut cmd = Command::new(executable());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        cmd.creation_flags(0x0800_0000 | 0x0000_4000); // No window; CPU work yields to normal-priority UI work.
    }
    cmd
}

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn collect(
    mut reader: impl Read + Send + 'static,
) -> std::sync::mpsc::Receiver<std::io::Result<(Vec<u8>, bool)>> {
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let result = (|| {
            let mut bytes = Vec::new();
            let mut exceeded = false;
            let mut buffer = [0; 4096];
            loop {
                let count = reader.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                let remaining = OUTPUT_LIMIT.saturating_sub(bytes.len());
                bytes.extend_from_slice(&buffer[..count.min(remaining)]);
                exceeded |= count > remaining;
            }
            Ok((bytes, exceeded))
        })();
        let _ = send.send(result);
    });
    receive
}

/// Drain both pipes while enforcing time/output bounds. Drop always reaps our child.
fn run(cmd: &mut Command, timeout: Duration, canceled: &impl Fn() -> bool) -> Result<String> {
    ensure!(!canceled(), "Dictation canceled");
    let mut child = OwnedChild(cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().context("Local speech runtime is unavailable. Reinstall Ion with its bundled NeMo-Speech runtime.")?);
    let stdout = collect(child.0.stdout.take().context("Missing speech output")?);
    let stderr = collect(
        child
            .0
            .stderr
            .take()
            .context("Missing speech diagnostics")?,
    );
    let deadline = Instant::now() + timeout;
    let status = loop {
        ensure!(!canceled(), "Dictation canceled");
        if let Some(status) = child.0.try_wait()? {
            break status;
        }
        ensure!(
            Instant::now() < deadline,
            "Local speech timed out. Retry with a shorter recording or check the model download connection."
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    let (output, exceeded) = stdout
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .context("Could not finish reading local speech output")??;
    let (diagnostic, _) = stderr
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .context("Could not finish reading local speech diagnostics")??;
    if !status.success() {
        let diagnostic: String = String::from_utf8_lossy(&diagnostic)
            .trim()
            .chars()
            .take(600)
            .collect();
        bail!("{MODEL_LABEL} failed (exit {status}). {diagnostic}");
    }
    ensure!(!exceeded, "Local speech output is too large");
    Ok(String::from_utf8(output)
        .context("Local speech returned invalid text")?
        .trim()
        .to_owned())
}

pub fn runtime_version() -> Result<String> {
    let version = run(command().arg("--version"), Duration::from_secs(15), &|| {
        false
    })?;
    ensure!(!version.is_empty(), "Local speech returned no version");
    Ok(version)
}

/// Sweep only old Ion-owned recordings, on a worker, once after a process restart.
fn cleanup_stale_recordings() {
    static CLEANUP: std::sync::Once = std::sync::Once::new();
    CLEANUP.call_once(|| {
        let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            let Some(id) = name
                .strip_prefix("ion-dictation-")
                .or_else(|| name.strip_prefix("ion-phone-dictation-"))
            else {
                continue;
            };
            let Some(id) = id.strip_suffix(".wav").or_else(|| id.strip_suffix(".txt")) else {
                continue;
            };
            if id.len() != 36
                || !id.bytes().enumerate().all(|(i, b)| {
                    if [8, 13, 18, 23].contains(&i) {
                        b == b'-'
                    } else {
                        b.is_ascii_hexdigit()
                    }
                })
            {
                continue;
            }
            let Ok(info) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if info.is_file()
                && !info.is_symlink()
                && info
                    .modified()
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .is_some_and(|age| age > Duration::from_secs(86_400))
            {
                let _ = std::fs::remove_file(path);
            }
        }
    });
}

pub fn prepare_model(
    canceled: &impl Fn() -> bool,
    progress: &mut impl FnMut(super::download::Progress),
) -> Result<PathBuf> {
    cleanup_stale_recordings();
    super::download::prepare(canceled, progress)
}

/// English-only full-utterance TDT decoding with the checksum-verified local model.
pub fn transcribe_wav(path: &Path, canceled: &impl Fn() -> bool) -> Result<String> {
    let model = prepare_model(canceled, &mut |_| {})?;
    // Batch decoding after Stop avoids pretending this offline model is a live stream.
    let result = run(
        command()
            .args(["--quiet", "transcribe"])
            .arg(path)
            .arg("--model")
            .arg(model)
            .args(["--backend", "cpu"]),
        Duration::from_secs(120),
        canceled,
    );
    let text = result?;
    ensure!(
        !text.is_empty(),
        "No speech was recognized in this recording"
    );
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(mode: &str) -> Command {
        let mut cmd = Command::new(std::env::current_exe().unwrap());
        cmd.args([
            "--exact",
            "speech::local::tests::child_fixture",
            "--nocapture",
        ])
        .env("ION_LOCAL_SPEECH_TEST_CHILD", mode);
        cmd
    }
    #[test]
    #[ignore = "requires the staged 110M model cache, installed CPU runtime and synthetic fixture paths"]
    fn verified_110m_runtime_smoke() {
        let speech =
            std::env::var("ION_SPEECH_TEST_WAV").expect("set the fixed synthetic speech fixture");
        let silence =
            std::env::var("ION_SPEECH_TEST_SILENCE").expect("set the fixed silent fixture");
        assert!(runtime_version().unwrap().contains("0.1.0"));
        assert_eq!(
            transcribe_wav(Path::new(&speech), &|| false).unwrap(),
            "Create a new project and run the tests before deploying the application."
        );
        assert!(
            transcribe_wav(Path::new(&silence), &|| false)
                .unwrap_err()
                .to_string()
                .contains("No speech")
        );
    }
    #[test]
    fn child_fixture() {
        match std::env::var("ION_LOCAL_SPEECH_TEST_CHILD").as_deref() {
            Ok("success") => {
                println!("local transcript");
                eprintln!("diagnostic");
            }
            Ok("large") => {
                use std::io::Write;
                std::io::stdout()
                    .write_all(&vec![b'x'; OUTPUT_LIMIT + 1024])
                    .unwrap();
            }
            Ok("wait") => std::thread::sleep(Duration::from_secs(20)),
            Ok("error") => std::process::exit(9),
            _ => {}
        }
    }
    #[test]
    fn drains_both_pipes_and_propagates_failure() {
        assert!(
            run(&mut fixture("success"), Duration::from_secs(5), &|| false)
                .unwrap()
                .contains("local transcript")
        );
        assert!(run(&mut fixture("error"), Duration::from_secs(5), &|| false).is_err());
    }
    #[test]
    fn oversized_output_is_rejected() {
        assert!(
            run(&mut fixture("large"), Duration::from_secs(5), &|| false)
                .unwrap_err()
                .to_string()
                .contains("too large")
        );
    }
    #[test]
    fn timeout_and_cancel_reap_the_child_promptly() {
        let start = Instant::now();
        assert!(run(&mut fixture("wait"), Duration::from_millis(100), &|| false).is_err());
        assert!(start.elapsed() < Duration::from_secs(5));
        let start = Instant::now();
        assert!(
            run(&mut fixture("wait"), Duration::from_secs(10), &|| start
                .elapsed()
                > Duration::from_millis(100))
            .is_err()
        );
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}
