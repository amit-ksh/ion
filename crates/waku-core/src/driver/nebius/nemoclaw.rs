//! Optional, approval-gated execution in a user-managed NemoClaw sandbox.
//! The daemon still owns the task journal; this module never provisions or
//! configures the sandbox and never receives the Nebius credential.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use serde_json::Value;

const MAX_ARGS: usize = 24;
const MAX_ARG_BYTES: usize = 2_048;
const MAX_COMMAND_BYTES: usize = 8_192;
const MAX_OUTPUT_BYTES: u64 = 16_384;

pub(super) struct NemoClawAdapter {
    sandbox: String,
    #[cfg(windows)]
    wsl_distro: Option<String>,
}

impl NemoClawAdapter {
    pub(super) fn from_env() -> Result<Option<Self>> {
        let enabled = std::env::var("ION_NEMOCLAW_ADAPTER").unwrap_or_default();
        if enabled.is_empty() || enabled == "0" {
            return Ok(None);
        }
        if enabled != "1" {
            bail!("ION_NEMOCLAW_ADAPTER must be 0 or 1");
        }
        let sandbox = std::env::var("ION_NEMOCLAW_SANDBOX")
            .context("ION_NEMOCLAW_SANDBOX is required when the adapter is enabled")?;
        if sandbox.is_empty()
            || sandbox.len() > 64
            || !sandbox
                .bytes()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric())
            || !sandbox
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            bail!("ION_NEMOCLAW_SANDBOX must be a 1-64 character sandbox name");
        }
        #[cfg(windows)]
        let wsl_distro = std::env::var("ION_NEMOCLAW_WSL_DISTRO").ok();
        #[cfg(windows)]
        if wsl_distro.as_ref().is_some_and(|name| {
            name.is_empty()
                || name.len() > 64
                || !name
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        }) {
            bail!("ION_NEMOCLAW_WSL_DISTRO must be a valid WSL distribution name");
        }
        Ok(Some(Self {
            sandbox,
            #[cfg(windows)]
            wsl_distro,
        }))
    }

    pub(super) fn sandbox(&self) -> &str {
        &self.sandbox
    }

    pub(super) fn validate_args(args: &Value) -> Result<Vec<String>> {
        let values = args["argv"]
            .as_array()
            .filter(|values| !values.is_empty() && values.len() <= MAX_ARGS)
            .ok_or_else(|| anyhow::anyhow!("sandbox command needs 1-24 arguments"))?;
        let mut bytes = 0;
        let mut argv = Vec::with_capacity(values.len());
        for value in values {
            let item = value
                .as_str()
                .filter(|item| {
                    !item.is_empty() && item.len() <= MAX_ARG_BYTES && !item.contains('\0')
                })
                .ok_or_else(|| anyhow::anyhow!("sandbox command contains an invalid argument"))?;
            bytes += item.len();
            if bytes > MAX_COMMAND_BYTES {
                bail!("sandbox command exceeds the 8 KiB limit");
            }
            argv.push(item.to_owned());
        }
        Ok(argv)
    }

    fn command(&self, argv: &[String]) -> Command {
        #[cfg(windows)]
        let mut command = {
            let mut command = Command::new("wsl.exe");
            if let Some(distro) = &self.wsl_distro {
                command.args(["-d", distro]);
            }
            command.args(["--exec", "nemoclaw"]);
            command
        };
        #[cfg(not(windows))]
        let mut command = Command::new("nemoclaw");
        command
            .arg(&self.sandbox)
            .args(["exec", "--no-tty", "--"])
            .args(argv);
        command
    }

    pub(super) fn check_ready(&self, cancelled: &AtomicBool) -> Result<()> {
        let result = self.execute(&["true".into()], cancelled, Duration::from_secs(15))?;
        if !result.starts_with("NemoClaw command completed.") {
            bail!(
                "NemoClaw sandbox is unavailable. Check WSL, the registered sandbox, and its CLI"
            );
        }
        Ok(())
    }

    pub(super) fn execute(
        &self,
        argv: &[String],
        cancelled: &AtomicBool,
        timeout: Duration,
    ) -> Result<String> {
        let mut child = self
            .command(argv)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Could not start NemoClaw. Install its CLI and check the sandbox name")?;
        let stdout = child.stdout.take().context("NemoClaw stdout unavailable")?;
        let stderr = child.stderr.take().context("NemoClaw stderr unavailable")?;
        let out = thread::spawn(move || read_bounded(stdout));
        let err = thread::spawn(move || read_bounded(stderr));
        let start = Instant::now();
        let status = loop {
            if cancelled.load(Ordering::Relaxed) || start.elapsed() >= timeout {
                let _ = child.kill();
                let _ = child.wait();
                // The sandbox action may already have happened; its result is
                // deliberately left uncertain in the daemon journal.
                bail!("NemoClaw command stopped or timed out; inspect the sandbox before resuming");
            }
            if let Some(status) = child
                .try_wait()
                .context("Could not check NemoClaw command")?
            {
                break status;
            }
            thread::sleep(Duration::from_millis(100));
        };
        let stdout = out
            .join()
            .map_err(|_| anyhow::anyhow!("Could not read NemoClaw output"))??;
        let _stderr = err
            .join()
            .map_err(|_| anyhow::anyhow!("Could not read NemoClaw diagnostics"))??;
        if !status.success() {
            // Diagnostics may contain gateway details or credentials. Keep
            // them out of the provider transcript and give a stable result.
            return Ok(format!(
                "NemoClaw command exited with {status}. Inspect the sandbox for details."
            ));
        }
        // Do not mirror CLI diagnostics into model context.
        Ok(format!("NemoClaw command completed. stdout:\n{stdout}"))
    }
}

fn read_bounded(input: impl Read) -> Result<String> {
    let mut bytes = Vec::new();
    input.take(MAX_OUTPUT_BYTES + 1).read_to_end(&mut bytes)?;
    let truncated = bytes.len() as u64 > MAX_OUTPUT_BYTES;
    bytes.truncate(MAX_OUTPUT_BYTES as usize);
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if truncated {
        text.push_str("\n[output truncated]");
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn command_arguments_stay_bounded_and_literal() {
        assert_eq!(
            NemoClawAdapter::validate_args(&json!({"argv":["sh", "-c", "printf '%s' '$HOME'"]}))
                .unwrap(),
            ["sh", "-c", "printf '%s' '$HOME'"]
        );
        assert!(NemoClawAdapter::validate_args(&json!({"argv":[]})).is_err());
        assert!(NemoClawAdapter::validate_args(&json!({"argv":["cat", "a\0b"]})).is_err());
        assert!(NemoClawAdapter::validate_args(&json!({"argv":["a".repeat(2_049)]})).is_err());
    }

    #[test]
    fn launcher_passes_exact_arguments_without_host_shell() {
        let adapter = NemoClawAdapter {
            sandbox: "review-sandbox".into(),
            #[cfg(windows)]
            wsl_distro: Some("Ubuntu-24.04".into()),
        };
        let command = adapter.command(&["printf".into(), "$HOME; echo unsafe".into()]);
        let args = command
            .get_args()
            .map(|value| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        #[cfg(windows)]
        assert_eq!(
            args,
            [
                "-d",
                "Ubuntu-24.04",
                "--exec",
                "nemoclaw",
                "review-sandbox",
                "exec",
                "--no-tty",
                "--",
                "printf",
                "$HOME; echo unsafe"
            ]
        );
        #[cfg(not(windows))]
        assert_eq!(
            args,
            [
                "review-sandbox",
                "exec",
                "--no-tty",
                "--",
                "printf",
                "$HOME; echo unsafe"
            ]
        );
    }
}
