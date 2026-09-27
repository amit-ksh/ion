use anyhow::{Context as _, Result, bail};
use std::io::Write;
use std::process::{Command, Stdio};

pub fn set(account: &str, value: &str) -> Result<()> {
    let mut child = Command::new("secret-tool")
        .args([
            "store",
            "--label=ion credential",
            "app",
            "ion",
            "account",
            account,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    child
        .stdin
        .take()
        .context("secret-tool stdin")?
        .write_all(value.as_bytes())?;
    if !child.wait()?.success() {
        bail!("Secret Service unavailable")
    }
    Ok(())
}
pub fn get(account: &str) -> Result<Option<String>> {
    let output = Command::new("secret-tool")
        .args(["lookup", "app", "ion", "account", account])
        .stderr(Stdio::null())
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(
        String::from_utf8(output.stdout)?
            .trim_end_matches('\n')
            .to_owned(),
    ))
}
pub fn delete(account: &str) -> Result<()> {
    let status = Command::new("secret-tool")
        .args(["clear", "app", "ion", "account", account])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !status.success() {
        bail!("Secret Service unavailable")
    }
    Ok(())
}
