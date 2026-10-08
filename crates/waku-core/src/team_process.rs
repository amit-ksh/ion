//! Bounded subprocess transport shared by trusted Teams adapters and Git provisioning.
use anyhow::{Context, Result, ensure};
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
/// Drains both pipes with a fixed memory bound; no output is shown on failures.
/// The child is always terminated/reaped on cancel, timeout, or an oversized pipe.
pub(crate) fn bounded_process(
    command: &mut Command,
    input: &[u8],
    cancelled: &AtomicBool,
    timeout: Duration,
) -> Result<Vec<u8>> {
    process(command, input, cancelled, timeout, false, None)
}

pub(crate) fn bounded_process_with_stderr(
    command: &mut Command,
    input: &[u8],
    cancelled: &AtomicBool,
    timeout: Duration,
) -> Result<Vec<u8>> {
    process(command, input, cancelled, timeout, true, None)
}
/// Read-only RPC probes keep stdin open until the addressed response arrives,
/// then terminate and reap the complete process tree. No prompt is sent.
pub(crate) fn bounded_rpc(
    command: &mut Command,
    input: &[u8],
    reply_id: &str,
    timeout: Duration,
) -> Result<Vec<u8>> {
    process(
        command,
        input,
        &AtomicBool::new(false),
        timeout,
        false,
        Some(reply_id),
    )
}
fn process(
    command: &mut Command,
    input: &[u8],
    cancelled: &AtomicBool,
    timeout: Duration,
    include_stderr: bool,
    reply_id: Option<&str>,
) -> Result<Vec<u8>> {
    ensure!(
        !cancelled.load(Ordering::Acquire),
        "Teams process canceled before launch"
    );
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child =
        crate::command_env::spawn(command).context("Could not launch the selected executable")?;
    #[cfg(windows)]
    let process_tree = match crate::command_env::ProcessTree::attach(&child) {
        Ok(tree) => tree,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let exceeded = Arc::new(AtomicBool::new(false));
    let completed = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicBool::new(false));
    let reader = |pipe: Box<dyn Read + Send>, limit: usize, reply: Option<String>| {
        let completed = completed.clone();
        let exceeded = exceeded.clone();
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let mut pipe = pipe;
            let mut chunk = [0; 4096];
            let mut parsed_until = 0;
            loop {
                let count = pipe.read(&mut chunk)?;
                if count == 0 {
                    break;
                }
                if bytes.len() + count > limit {
                    exceeded.store(true, Ordering::Release);
                    break;
                }
                bytes.extend_from_slice(&chunk[..count]);
                if let Some(id) = &reply {
                    while let Some(end) = bytes[parsed_until..].iter().position(|c| *c == b'\n') {
                        let end = parsed_until + end;
                        if let Ok(v) =
                            serde_json::from_slice::<serde_json::Value>(&bytes[parsed_until..end])
                        {
                            if v["type"] == "response" && v["id"].as_str() == Some(id) {
                                completed.store(true, Ordering::Release);
                            }
                        }
                        parsed_until = end + 1;
                    }
                }
            }
            Ok::<_, std::io::Error>(bytes)
        })
    };
    let stdout = reader(
        Box::new(child.stdout.take().context("Teams stdout missing")?),
        2 * 1024 * 1024,
        reply_id.map(str::to_owned),
    );
    let stderr = reader(
        Box::new(child.stderr.take().context("Teams stderr missing")?),
        64 * 1024,
        None,
    );
    let mut stdin = child.stdin.take().context("Teams stdin missing")?;
    let input = input.to_vec();
    let hold = reply_id.is_some();
    let writer_finished = finished.clone();
    let writer = thread::spawn(move || {
        stdin.write_all(&input)?;
        if hold {
            while !writer_finished.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(20));
            }
        }
        drop(stdin); // Close explicitly before waiting for native CLI EOF.
        Ok::<_, std::io::Error>(())
    });
    let deadline = Instant::now() + timeout;
    let result = loop {
        if cancelled.load(Ordering::Acquire) {
            break Err(anyhow::anyhow!("Teams process canceled"));
        }
        if exceeded.load(Ordering::Acquire) {
            break Err(anyhow::anyhow!("Teams process output exceeded its limit"));
        }
        if Instant::now() >= deadline {
            break Err(anyhow::anyhow!(
                "Teams process timed out; its token allowance remains reserved"
            ));
        }
        if reply_id.is_some() && completed.load(Ordering::Acquire) {
            break Ok(None);
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(Some(status)),
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(error) => break Err(error.into()),
        }
    };
    finished.store(true, Ordering::Release);
    #[cfg(unix)]
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    #[cfg(windows)]
    drop(process_tree);
    let _ = child.kill();
    let _ = child.wait();
    let write = writer
        .join()
        .map_err(|_| anyhow::anyhow!("Teams stdin writer failed"))?;
    let mut output = stdout
        .join()
        .map_err(|_| anyhow::anyhow!("Teams stdout reader failed"))??;
    let stderr = stderr
        .join()
        .map_err(|_| anyhow::anyhow!("Teams stderr reader failed"))??;
    let status = result.map_err(|e| process_error(e, &output, &stderr))?;
    ensure!(
        !exceeded.load(Ordering::Acquire),
        "Teams process output exceeded its limit"
    );
    if !status.is_none_or(|s| s.success()) {
        return Err(process_error(
            anyhow::anyhow!("Teams process failed; verify authentication and pinned model"),
            &output,
            &stderr,
        ));
    }
    write.context("Could not send the bounded Teams prompt")?;
    if include_stderr {
        output.extend_from_slice(&stderr);
    }
    Ok(output)
}

fn process_error(error: anyhow::Error, _stdout: &[u8], _stderr: &[u8]) -> anyhow::Error {
    #[cfg(test)]
    if FIXTURE_DIAGNOSTICS.with(|v| v.get()) {
        return error.context(format!(
            "Loopback fixture diagnostics: stdout={} stderr={}",
            String::from_utf8_lossy(&_stdout[.._stdout.len().min(4096)]),
            String::from_utf8_lossy(&_stderr[.._stderr.len().min(8192)])
        ));
    }
    error
}
#[cfg(test)]
thread_local! { static FIXTURE_DIAGNOSTICS: std::cell::Cell<bool> = const {std::cell::Cell::new(false)}; }
#[cfg(test)]
pub(crate) struct FixtureDiagnostics;
#[cfg(test)]
pub(crate) fn fixture_diagnostics() -> FixtureDiagnostics {
    FIXTURE_DIAGNOSTICS.with(|v| v.set(true));
    FixtureDiagnostics
}
#[cfg(test)]
impl Drop for FixtureDiagnostics {
    fn drop(&mut self) {
        FIXTURE_DIAGNOSTICS.with(|v| v.set(false));
    }
}
