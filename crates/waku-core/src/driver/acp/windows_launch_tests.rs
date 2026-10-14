//! Isolated native Windows ACP framing; no provider account or model request.
use super::*;
use std::{fs, path::PathBuf, time::Instant};
use uuid::Uuid;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("ion-acp-launch-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // A unique directory created by this test; never a selected workspace.
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn windows_acp_launch_retains_native_program_arguments_and_environment() {
    let binary = r"C:\Program Files\Cursor !\cursor-agent.cmd";
    let args = vec!["acp".into(), "literal & % ! ^ \" argument".into()];
    let config = launch_configuration(
        binary,
        r"D:\Project & ! %\工作",
        args.clone(),
        vec![("ION_ACP_TEST".into(), "literal!&%".into())],
    );
    assert_eq!(config.command(), Path::new(binary));
    assert_eq!(config.arguments(), args);
    assert_eq!(
        config.environment().get("ION_ACP_TEST").map(String::as_str),
        Some("literal!&%")
    );
    assert!(!config.arguments().iter().any(|arg| arg == "-C"));
    let cwd = r"D:\Project & ! %\工作";
    for request in [
        serde_json::to_value(NewSessionRequest::new(cwd)).unwrap(),
        serde_json::to_value(LoadSessionRequest::new("saved", cwd)).unwrap(),
        serde_json::to_value(ResumeSessionRequest::new("saved", cwd)).unwrap(),
    ] {
        assert_eq!(request["cwd"], cwd);
    }
}

#[test]
fn windows_acp_native_session_preserves_wire_workspace_and_streams_result() {
    let fixture = Fixture::new();
    let script = fixture.0.join("fixture.ps1");
    fs::write(&script, r#"
$ErrorActionPreference = 'Stop'
$utf8 = New-Object System.Text.UTF8Encoding($false)
[Console]::InputEncoding = $utf8
[Console]::OutputEncoding = $utf8
$log = Join-Path $PSScriptRoot 'requests.jsonl'
while ($null -ne ($line = [Console]::ReadLine())) {
    $request = $line | ConvertFrom-Json
    [IO.File]::AppendAllText($log, (($request | ConvertTo-Json -Depth 30 -Compress) + "`n"), $utf8)
    if ($null -eq $request.id) { continue }
    $result = @{}
    switch ($request.method) {
        'initialize' { $result = @{protocolVersion=1;agentCapabilities=@{loadSession=$true};authMethods=@()} }
        'session/new' { $result = @{sessionId='ion-native-fixture'} }
        'session/prompt' {
            $update = @{jsonrpc='2.0';method='session/update';params=@{sessionId='ion-native-fixture';update=@{sessionUpdate='agent_message_chunk';content=@{type='text';text='Native fixture complete'}}}}
            [Console]::WriteLine(($update | ConvertTo-Json -Depth 30 -Compress))
            $result = @{stopReason='end_turn'}
        }
    }
    [Console]::WriteLine((@{jsonrpc='2.0';id=$request.id;result=$result} | ConvertTo-Json -Depth 30 -Compress))
}
"#).unwrap();
    let binary = fixture.0.join("fixture.cmd");
    fs::write(&binary, "@echo off\r\n%SystemRoot%\\System32\\WindowsPowerShell\\v1.0\\powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"%~dp0fixture.ps1\" %*\r\n").unwrap();
    let cwd = fixture.0.join("Project ! % & 工作");
    fs::create_dir(&cwd).unwrap();
    let (events, received) = crate::driver::test_event_channel();
    let driver = AcpDriver::start(
        ProviderKind::Fx,
        DriverStartOptions {
            binary,
            cwd: cwd.clone(),
            mode: RuntimeMode::FullAccess,
            model: None,
            reasoning_effort: None,
            service_tier: None,
            context_window: None,
            agent_preset: None,
            computer_use_enabled: false,
            provider_cursor: None,
        },
        events,
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match received
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("native ACP connection")
        {
            DriverEvent::Connected {
                provider_cursor: Some(ProviderResumeCursor::Fx { session_id }),
            } => {
                assert_eq!(session_id, "ion-native-fixture");
                break;
            }
            DriverEvent::Error(error) => panic!("native ACP startup failed: {error}"),
            _ => {}
        }
    }
    driver.prompt("No network or model call is made by this fixture.".into());
    let mut text = String::new();
    loop {
        match received
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("native ACP result")
        {
            DriverEvent::TextDelta(value) => text.push_str(&value),
            DriverEvent::TurnFinished { success, .. } => {
                assert!(success);
                break;
            }
            DriverEvent::Error(error) => panic!("native ACP result failed: {error}"),
            _ => {}
        }
    }
    assert_eq!(text, "Native fixture complete");
    drop(driver);
    loop {
        if matches!(
            received
                .recv_timeout(Duration::from_secs(5))
                .expect("native ACP shutdown"),
            DriverEvent::ProcessExited
        ) {
            break;
        }
    }
    let requests = fs::read_to_string(fixture.0.join("requests.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        requests
            .iter()
            .filter(|r| r["method"] == "session/new")
            .count(),
        1
    );
    let session = requests
        .iter()
        .find(|r| r["method"] == "session/new")
        .unwrap();
    assert_eq!(session["params"]["cwd"], cwd.to_str().unwrap());
    assert!(requests.iter().any(|r| r["method"] == "session/prompt"));
}
