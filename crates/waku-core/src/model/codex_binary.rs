//! A same-named npm package must not be mistaken for OpenAI's Codex CLI.
use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Mutex, OnceLock, mpsc},
    time::{Duration, Instant},
};

type Cache = HashMap<Option<String>, (Instant, Option<PathBuf>)>;
static VERIFIED: OnceLock<Mutex<Cache>> = OnceLock::new();

pub(super) fn resolve(override_value: Option<&str>) -> Option<PathBuf> {
    let key = override_value.map(str::to_owned);
    let cache = VERIFIED.get_or_init(Default::default);
    if let Some((checked, path)) = cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&key)
        .cloned()
        && checked.elapsed() < Duration::from_secs(5)
    {
        return path;
    }
    let candidates = if let Some(value) = override_value {
        crate::command_env::resolve_binary_override(value)
            .into_iter()
            .collect()
    } else {
        let mut candidates = crate::command_env::find_executables("codex");
        // Codex's Windows desktop installation also ships its official CLI.
        // Bound the search to immediate version directories; never walk a disk.
        #[cfg(windows)]
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let root = PathBuf::from(local).join("OpenAI/Codex/bin");
            if let Ok(entries) = std::fs::read_dir(root) {
                let mut installed = entries
                    .take(32)
                    .filter_map(Result::ok)
                    .map(|entry| entry.path().join("codex.exe"))
                    .filter(|path| path.is_file())
                    .collect::<Vec<_>>();
                installed.sort();
                candidates.extend(installed);
            }
        }
        candidates
    };
    let selected = candidates.into_iter().find(|path| is_openai_codex(path));
    cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(key, (Instant::now(), selected.clone()));
    selected
}

fn is_openai_codex(binary: &Path) -> bool {
    let Some(version) = bounded_output(binary, &["--version"]) else {
        return false;
    };
    if !identifies_openai_codex(&version) {
        return false;
    }
    bounded_output(binary, &["app-server", "--help"])
        .is_some_and(|help| help.contains("--stdio") && help.contains("app-server"))
}

fn bounded_output(binary: &Path, args: &[&str]) -> Option<String> {
    let mut command = crate::command_env::command(binary);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let Ok(mut child) = crate::command_env::spawn(&mut command) else {
        return None;
    };
    #[cfg(windows)]
    let _tree = crate::command_env::ProcessTree::attach(&child).ok();
    let stdout = child.stdout.take()?;
    let (send, receive) = mpsc::sync_channel(1);
    if std::thread::Builder::new()
        .name("ion-codex-identity".into())
        .spawn(move || {
            let mut bytes = Vec::new();
            let result = stdout
                .take(16 * 1024)
                .read_to_end(&mut bytes)
                .map(|_| bytes);
            let _ = send.send(result);
        })
        .is_err()
    {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    }
    let status = crate::command_env::wait_for_child(&mut child, Duration::from_secs(2)).ok()?;
    if !status.success() {
        return None;
    }
    // A wrapper may leave a descendant holding stdout after its own exit.
    // Never let that descendant stall provider detection indefinitely.
    let bytes = receive
        .recv_timeout(Duration::from_millis(200))
        .ok()?
        .ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn identifies_openai_codex(version: &str) -> bool {
    version.lines().any(|line| {
        line.trim()
            .strip_prefix("codex-cli ")
            .is_some_and(|number| number.trim().starts_with(|c: char| c.is_ascii_digit()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unrelated_npm_codex_is_rejected() {
        assert!(!identifies_openai_codex("0.2.3\n"));
        assert!(!identifies_openai_codex("Warning: codex-cli 0.2.3\n0.2.3"));
        assert!(!identifies_openai_codex("codex-cli unknown"));
    }
    #[test]
    fn official_stable_and_desktop_cli_versions_are_accepted() {
        assert!(identifies_openai_codex("codex-cli 0.163.0-alpha.10.2\r\n"));
        assert!(identifies_openai_codex("codex-cli 0.114.0\n"));
    }
}
