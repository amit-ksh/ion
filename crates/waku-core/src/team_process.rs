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
    let process_tree = match ProcessTree::attach(&child) {
        Ok(tree) => tree,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let exceeded = Arc::new(AtomicBool::new(false));
    let reader = |pipe: Box<dyn Read + Send>, limit: usize| {
        let exceeded = exceeded.clone();
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let mut pipe = pipe;
            let mut chunk = [0; 4096];
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
            }
            Ok::<_, std::io::Error>(bytes)
        })
    };
    let stdout = reader(
        Box::new(child.stdout.take().context("Teams stdout missing")?),
        2 * 1024 * 1024,
    );
    let stderr = reader(
        Box::new(child.stderr.take().context("Teams stderr missing")?),
        64 * 1024,
    );
    let mut stdin = child.stdin.take().context("Teams stdin missing")?;
    let input = input.to_vec();
    let writer = thread::spawn(move || stdin.write_all(&input));
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
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(error) => break Err(error.into()),
        }
    };
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
    let output = stdout
        .join()
        .map_err(|_| anyhow::anyhow!("Teams stdout reader failed"))??;
    let _ = stderr.join();
    let status = result?;
    ensure!(
        !exceeded.load(Ordering::Acquire),
        "Teams process output exceeded its limit"
    );
    ensure!(
        status.success(),
        "Teams process failed ({}); verify authentication and pinned model",
        status
    );
    write.context("Could not send the bounded Teams prompt")?;
    Ok(output)
}

#[cfg(windows)]
struct ProcessTree(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl ProcessTree {
    fn attach(child: &std::process::Child) -> Result<Self> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::{Foundation::CloseHandle, System::JobObjects::*};
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            ensure!(!job.is_null(), "Could not contain the Teams process tree");
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let configured = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                std::mem::size_of_val(&limits) as u32,
            ) != 0;
            if !configured || AssignProcessToJobObject(job, child.as_raw_handle()) == 0 {
                CloseHandle(job);
                anyhow::bail!("Could not contain the Teams process tree");
            }
            Ok(Self(job))
        }
    }
}
#[cfg(windows)]
impl Drop for ProcessTree {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
