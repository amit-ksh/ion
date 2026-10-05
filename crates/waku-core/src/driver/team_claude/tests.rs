use super::*;
use std::time::Instant;
fn tools() -> Value {
    json!([{"function":{"name":"save_document"}},{"function":{"name":"report_result"}},{"function":{"name":"request_decision"}}])
}
fn result() -> Value {
    json!({"type":"result","subtype":"success","is_error":false,"num_turns":1,"result":json!({"text":"Reviewed the evidence","calls":[{"name":"save_document","arguments":{"name":"Report.md","format":"md","content":"# Evidence"}}]}).to_string(),"modelUsage":{"claude-sonnet-4-6":{"inputTokens":100,"outputTokens":80,"cacheReadInputTokens":200,"cacheCreationInputTokens":300}}})
}
#[test]
fn team_claude_receipt_binds_model_and_counts_all_cache_tokens() {
    let result = parse_completion(
        &serde_json::to_vec(&result()).unwrap(),
        "claude-sonnet-4-6",
        &tools(),
    )
    .unwrap();
    assert_eq!(
        result.usage,
        Some(ModelTokenUsage {
            input_tokens: 600,
            output_tokens: 80
        })
    );
    assert_eq!(result.calls.len(), 1);
    assert_eq!(result.calls[0].name, "save_document");
    assert_eq!(result.text, "Reviewed the evidence");
    assert!(Uuid::parse_str(&result.calls[0].activity_id).is_ok());
}
#[test]
fn team_claude_rejects_missing_usage_model_substitution_and_unbounded_loops() {
    for bad in ["modelUsage", "num_turns", "is_error"] {
        let mut value = result();
        value.as_object_mut().unwrap().remove(bad);
        assert!(
            parse_completion(
                &serde_json::to_vec(&value).unwrap(),
                "claude-sonnet-4-6",
                &tools()
            )
            .is_err()
        );
    }
    let mut value = result();
    value["num_turns"] = json!(2);
    assert!(
        parse_completion(
            &serde_json::to_vec(&value).unwrap(),
            "claude-sonnet-4-6",
            &tools()
        )
        .is_err()
    );
    assert!(
        parse_completion(
            &serde_json::to_vec(&result()).unwrap(),
            "claude-opus-4-6",
            &tools()
        )
        .is_err()
    );
    let mut value = result();
    value["modelUsage"]["claude-sonnet-4-6"]
        .as_object_mut()
        .unwrap()
        .remove("cacheReadInputTokens");
    assert!(
        parse_completion(
            &serde_json::to_vec(&value).unwrap(),
            "claude-sonnet-4-6",
            &tools()
        )
        .is_err()
    );
    let mut value = result();
    value["modelUsage"]["other-model"] = json!({});
    assert!(
        parse_completion(
            &serde_json::to_vec(&value).unwrap(),
            "claude-sonnet-4-6",
            &tools()
        )
        .is_err()
    );
    let mut value = result();
    value["subtype"] = json!("error_max_turns");
    assert!(
        parse_completion(
            &serde_json::to_vec(&value).unwrap(),
            "claude-sonnet-4-6",
            &tools()
        )
        .is_err()
    );
}
#[test]
fn team_claude_never_turns_text_or_unknown_actions_into_host_effects() {
    for actions in [
        json!({"text":"Done","calls":[{"name":"Bash","arguments":{"command":"bad"}}]}),
        json!({"text":"Done","calls":[{"name":"save_document","arguments":[]}]}),
        json!({"text":"Done","calls":[],"host":"other"}),
        json!({"text":"Done","calls":(0..9).map(|_|json!({"name":"request_decision","arguments":{}})).collect::<Vec<_>>()}),
    ] {
        let mut value = result();
        value["result"] = json!(actions.to_string());
        assert!(
            parse_completion(
                &serde_json::to_vec(&value).unwrap(),
                "claude-sonnet-4-6",
                &tools()
            )
            .is_err()
        );
    }
    let mut value = result();
    value["result"] = json!("Work completed");
    assert!(
        parse_completion(
            &serde_json::to_vec(&value).unwrap(),
            "claude-sonnet-4-6",
            &tools()
        )
        .is_err()
    );
}
#[test]
fn team_claude_isolated_launch_has_no_builtin_tools_config_hooks_or_fallback() {
    let mut command = Command::new("claude");
    isolated_options(&mut command);
    let args = command
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(args.windows(2).any(|p| p == ["--tools", ""]));
    assert!(args.windows(2).any(|p| p == ["--setting-sources", ""]));
    assert!(
        args.windows(2)
            .any(|p| p == ["--mcp-config", r#"{"mcpServers":{}}"#])
    );
    assert!(args.contains(&"--safe-mode".into()));
    assert!(args.contains(&"--restricted".into()));
    assert!(!args.contains(&"--dangerously-skip-permissions".into()));
    assert!(!args.contains(&"--fallback-model".into()));
    assert!(allowance("sonnet").is_err());
    assert!(allowance("custom/model").is_err());
    assert_eq!(
        allowance("claude-sonnet-4-6").unwrap().max_input_tokens,
        1_000_000
    );
}
#[cfg(windows)]
#[test]
fn team_claude_process_reaps_timeout_cancel_and_bounded_pipes() {
    let cancellation = AtomicBool::new(true);
    let mut command = crate::command_env::plain_command("cmd.exe");
    command.args(["/d", "/c", "exit", "0"]);
    assert!(bounded_process(&mut command, b"", &cancellation, Duration::from_secs(1)).is_err());
    let cancellation = AtomicBool::new(false);
    let mut command = crate::command_env::plain_command("powershell.exe");
    command.args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "Start-Sleep -Seconds 10",
    ]);
    let started = Instant::now();
    assert!(bounded_process(&mut command, b"", &cancellation, Duration::from_millis(150)).is_err());
    assert!(started.elapsed() < Duration::from_secs(5));
    let mut command = crate::command_env::plain_command("powershell.exe");
    command.args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "[Console]::Write('x' * 2200000)",
    ]);
    assert!(bounded_process(&mut command, b"", &cancellation, Duration::from_secs(5)).is_err());
    let mut command = crate::command_env::plain_command("cmd.exe");
    command.args(["/d", "/c", "echo", "bounded"]);
    let bytes = bounded_process(&mut command, b"", &cancellation, Duration::from_secs(5)).unwrap();
    assert!(String::from_utf8(bytes).unwrap().contains("bounded"));
}

#[cfg(windows)]
#[test]
fn team_claude_process_bridge_completes_real_document_task_without_provider_billing() {
    use crate::{
        control::ControlStore,
        teams::{DispatchHost, PrepareRun},
    };
    use waku_protocol::{control::*, teams::*};
    let directory = std::env::temp_dir().join(format!("ion-team-claude-test-{}", Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let _cleanup = EmptyDirectory(directory.clone());
    let workspace = directory.join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let program = directory.join("fixture.cmd");
    let script = directory.join("fixture.ps1");
    std::fs::write(
        &program,
        "@powershell.exe -NoProfile -NonInteractive -File \"%~dp0fixture.ps1\"\r\n",
    )
    .unwrap();
    std::fs::write(&script,r#"
$inputJson = [Console]::In.ReadToEnd() | ConvertFrom-Json
$artifact = $null
foreach ($message in $inputJson.messages) {
  if ($message.role -eq 'tool') { $receipt = $message.content | ConvertFrom-Json; if ($receipt.artifact_id) { $artifact = $receipt.artifact_id } }
}
if ($artifact) { $action = @{name='report_result';arguments=@{summary='Delivered actual fixture document';artifact_ids=@($artifact)}} }
else { $action = @{name='save_document';arguments=@{name='Report.md';format='md';content='# Actual fixture document'}} }
$content = @{text='';calls=@($action)} | ConvertTo-Json -Depth 12 -Compress
@{type='result';subtype='success';is_error=$false;num_turns=1;result=$content;modelUsage=@{'claude-sonnet-4-6'=@{inputTokens=100;outputTokens=20;cacheReadInputTokens=0;cacheCreationInputTokens=0}}} | ConvertTo-Json -Depth 12 -Compress
"#).unwrap();
    let control = ControlStore::new(directory.join("control.db"));
    let store = TeamCoordinator::new(control.path());
    let agent = Uuid::new_v4();
    let team = Uuid::new_v4();
    control
        .save(
            0,
            ControlRecord::Agent(AgentProfile {
                id: agent,
                name: "Claude reviewer".into(),
                expertise: "Review".into(),
                instructions: "Create the requested report".into(),
                provider: ProviderKind::Claude,
                model: Some("claude-sonnet-4-6".into()),
                expected_output: "Markdown evidence".into(),
                execution: Default::default(),
                access: Default::default(),
                max_parallel_tasks: 1,
            }),
        )
        .unwrap();
    control
        .save(
            control.load().unwrap().revision,
            ControlRecord::Team(Team {
                id: team,
                name: "Review".into(),
                objective: "Review project".into(),
                limits: None,
                members: vec![TeamMember {
                    agent_id: agent,
                    parent_id: None,
                }],
            }),
        )
        .unwrap();
    let run = store
        .prepare(
            PrepareRun {
                id: Uuid::new_v4(),
                root_task_id: Uuid::new_v4(),
                project_id: Uuid::new_v4(),
                team_id: team,
                expected_definition_revision: control.load().unwrap().revision,
                objective: "Create report".into(),
                source_prompt: None,
                origin: None,
                limits: Default::default(),
                access: Default::default(),
            },
            crate::model::unix_time(),
        )
        .unwrap();
    store
        .acknowledge_materialized(
            run.id,
            run.work[0].id,
            run.root_task_id,
            crate::model::unix_time(),
        )
        .unwrap();
    let start = store
        .claim(
            run.id,
            run.work[0].id,
            DispatchHost::Local,
            crate::model::unix_time(),
            180,
        )
        .unwrap();
    let (events, _receiver) = crate::driver::test_event_channel();
    assert_eq!(
        super::run(
            &store,
            NativeTeamStart::Fresh(start),
            &workspace,
            &events,
            &AtomicBool::new(false),
            &program
        )
        .unwrap(),
        NativeTeamExit::Settled
    );
    let finished = store.snapshot(run.id).unwrap();
    assert_eq!(finished.status, TeamRunStatus::Completed);
    assert_eq!(finished.usage.model_calls, 2);
    assert_eq!(finished.usage.consumed_tokens, 240);
    assert_eq!(finished.usage.unknown_calls, 0);
    let artifacts = finished.work[0]
        .result
        .as_ref()
        .unwrap()
        .artifact_ids
        .clone();
    assert_eq!(artifacts.len(), 1);
    assert!(control.load().unwrap().records.iter().any(|r|matches!(r,ControlRecord::Artifact(a) if a.id==artifacts[0] && a.task_id==Some(run.root_task_id))));
    // Only this fixture's known files are removed, without recursive deletion.
    std::fs::remove_file(&program).unwrap();
    std::fs::remove_file(&script).unwrap();
    std::fs::remove_dir(&workspace).unwrap();
    for entry in std::fs::read_dir(&directory).unwrap() {
        let path = entry.unwrap().path();
        if path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("control.db")
        {
            std::fs::remove_file(path).unwrap();
        }
    }
}
