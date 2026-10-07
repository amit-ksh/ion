//! Owner report/review gates exercised through the real command HTTP transport.
use super::*;
use crate::driver::team_test_http::Server;
use crate::{
    control::ControlStore,
    teams::{
        AssignmentContinuation, AssignmentOutcome, Delegation, DispatchHost, PrepareRun,
        ReviewRequest,
    },
};
use std::path::PathBuf;
use waku_protocol::{control::*, model::ProviderKind, teams::*};

struct Fixture {
    dir: PathBuf,
    source: PathBuf,
    workspace: PathBuf,
    store: TeamCoordinator,
    root: ClaimedAssignment,
    child: ClaimedAssignment,
    policy: Policy,
    command: RequiredCommandCheck,
}
fn now() -> u64 {
    crate::model::unix_time()
}
fn git(path: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .current_dir(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
impl Fixture {
    fn new() -> Self {
        Self::with_command(|_| {})
    }
    fn with_command(modify: impl FnOnce(&mut RequiredCommandCheck)) -> Self {
        let dir =
            std::env::temp_dir().join(format!("ion-command-check-fixture-{}", Uuid::new_v4()));
        let source = dir.join("project");
        std::fs::create_dir_all(&source).unwrap();
        git(&source, &["init", "-b", "main"]);
        git(&source, &["config", "core.autocrlf", "false"]);
        std::fs::write(source.join("check.py"), "assert 1 + 1 == 2\n").unwrap();
        git(&source, &["add", "check.py"]);
        git(
            &source,
            &[
                "-c",
                "user.name=Ion Tests",
                "-c",
                "user.email=ion@example.test",
                "commit",
                "-m",
                "baseline",
            ],
        );
        let definitions = ControlStore::new(dir.join("control.db"));
        let agents = [Uuid::new_v4(), Uuid::new_v4()];
        let target = Uuid::new_v4();
        let access = AgentAccess {
            workspace_read: true,
            workspace_write: true,
            shell: true,
            ..Default::default()
        };
        for (i, id) in agents.iter().enumerate() {
            definitions
                .save(
                    definitions.load().unwrap().revision,
                    ControlRecord::Agent(AgentProfile {
                        id: *id,
                        name: format!("Check agent {i}"),
                        expertise: "Testing".into(),
                        instructions: "Use actual receipts".into(),
                        provider: ProviderKind::Nebius,
                        model: Some("fixture".into()),
                        expected_output: "Receipt".into(),
                        execution: AgentExecution {
                            environment: if i == 0 {
                                ExecutionEnvironment::Local
                            } else {
                                ExecutionEnvironment::NebiusSandbox
                            },
                            sandbox_target: (i == 1).then(|| target.to_string()),
                            local_fallback: true,
                        },
                        access: access.clone(),
                        max_parallel_tasks: 1,
                    }),
                )
                .unwrap();
        }
        let team = Team {
            id: Uuid::new_v4(),
            name: "Check team".into(),
            objective: "Verified changes".into(),
            limits: None,
            members: vec![
                TeamMember {
                    agent_id: agents[0],
                    parent_id: None,
                },
                TeamMember {
                    agent_id: agents[1],
                    parent_id: Some(agents[0]),
                },
            ],
        };
        definitions
            .save(
                definitions.load().unwrap().revision,
                ControlRecord::Team(team.clone()),
            )
            .unwrap();
        let store = TeamCoordinator::new(definitions.path());
        let run = store
            .prepare(
                PrepareRun {
                    id: Uuid::new_v4(),
                    root_task_id: Uuid::new_v4(),
                    project_id: Uuid::new_v4(),
                    team_id: team.id,
                    expected_definition_revision: definitions.load().unwrap().revision,
                    objective: "Verify code".into(),
                    limits: TeamRunLimits::default(),
                    access: access.clone(),
                    input_data: vec![],
                    source_attachments: vec![],
                    source_prompt: None,
                    origin: None,
                },
                now(),
            )
            .unwrap();
        let workspace = store
            .prepare_local_workspace(run.id, &source, now())
            .unwrap();
        let root = store
            .claim(run.id, run.work[0].id, DispatchHost::Local, now(), 180)
            .unwrap();
        let mut command = RequiredCommandCheck {
            name: "Unit check".into(),
            argv: vec!["/usr/bin/python3".into(), "check.py".into()],
            input_paths: vec!["check.py".into()],
            output_paths: vec![],
            timeout_seconds: 10,
        };
        modify(&mut command);
        let child = store
            .delegate_with_contract(
                &root.lease,
                Delegation {
                    request_id: Uuid::new_v4(),
                    agent_id: agents[1],
                    objective: "Run pinned check".into(),
                    expected_output: "Successful test and report".into(),
                    dependencies: vec![],
                    access,
                },
                OutputContract {
                    minimum_documents: 1,
                    required_formats: vec!["json".into()],
                    required_commands: vec![command.clone()],
                    ..Default::default()
                },
                now(),
            )
            .unwrap();
        store
            .suspend_for_children(&root.lease, &[child.id], "{}", now())
            .unwrap();
        for pending in store.pending_materializations(128).unwrap() {
            store
                .acknowledge_materialized(
                    pending.run_id,
                    pending.assignment.id,
                    pending.assignment.task_id,
                    now(),
                )
                .unwrap();
        }
        let child = store
            .claim(
                run.id,
                child.id,
                DispatchHost::NebiusSandbox {
                    target: target.to_string(),
                },
                now(),
                180,
            )
            .unwrap();
        let policy = Policy {
            version: 1,
            target,
            image: Uuid::new_v4(),
            credential_free: true,
            max_commands: 8,
            max_seconds: 30,
            max_layer_bytes: 256 * 1024 * 1024,
            uid: 1000,
            gid: 1000,
            executables: vec!["/usr/bin/python3".into()],
        };
        Self {
            dir,
            source,
            workspace,
            store,
            root,
            child,
            policy,
            command,
        }
    }
    fn args(&self) -> CommandArgs {
        CommandArgs {
            argv: self.command.argv.clone(),
            input_paths: self.command.input_paths.clone(),
            output_paths: self.command.output_paths.clone(),
            timeout_seconds: self.command.timeout_seconds,
        }
    }
    fn request(&self) -> JobRequest {
        bundle(Uuid::new_v4(), &self.args(), &self.workspace).unwrap()
    }
    fn reserve(&self, request: &JobRequest) -> Result<Option<JobResponse>> {
        let context =
            self.store
                .command_check_context(&self.child.lease, &self.workspace, now())?;
        self.store.reserve_remote_job_with_context(
            &self.child.lease,
            request,
            self.policy.image,
            "fixture-project",
            &serde_json::to_string(&self.policy)?,
            self.policy.max_commands,
            now(),
            context.as_ref(),
        )
    }
    fn execute(&self, request: &JobRequest, mut reply: JobResponse) -> JobResponse {
        reply.request_id = request.request_id;
        self.reserve(request).unwrap();
        let operation = Uuid::new_v4();
        let terminal = json!({"uuid":operation,"kind":"instance","image_uuid":self.policy.image,"status":"SUCCESS",
            "metadata":{"result":{"state":{"exit_code":0},"stdout":{"encoding":"base64","value":STANDARD.encode(serde_json::to_vec(&reply).unwrap()),"truncated":false}}}});
        let server = Server::new(vec![
            (201, serde_json::to_vec(&json!({"uuid":operation})).unwrap()),
            (200, serde_json::to_vec(&terminal).unwrap()),
        ]);
        let result = execute_with(
            &Client::fixture(&server.url, "fixture-project"),
            &self.store,
            &self.child.lease,
            &self.policy,
            request,
            &AtomicBool::new(false),
        )
        .unwrap();
        let calls = server.finish();
        assert_eq!(calls.len(), 2);
        assert!(calls[0].line.starts_with("POST /instances "));
        assert!(
            calls[1]
                .line
                .starts_with(&format!("GET /operations/{operation} "))
        );
        result
    }
    fn result(&self, request: &JobRequest, reply: &JobResponse) -> AssignmentResult {
        let artifact = self
            .store
            .publish_document(
                &self.child.lease,
                request.request_id,
                &format!("command-{}", request.request_id),
                "json",
                &reply.summary().unwrap().to_string(),
                now(),
            )
            .unwrap();
        AssignmentResult {
            summary: "Verified check with actual receipt".into(),
            artifact_ids: vec![artifact.id],
            checks: vec![],
            executed_checks: vec![ExecutedCommandCheck {
                name: self.command.name.clone(),
                request_id: request.request_id,
            }],
        }
    }
    fn report(&self, result: AssignmentResult) -> Result<TeamRunSnapshot> {
        self.store.report(
            &self.child.lease,
            AssignmentOutcome::Completed(result),
            now(),
        )
    }
    fn resume(&self) -> AssignmentContinuation {
        self.store
            .resume_after_children(
                self.root.lease.run_id(),
                self.root.assignment.id,
                now(),
                180,
            )
            .unwrap()
    }
    fn accept(&self, parent: &ClaimedAssignment) -> Result<AssignmentReview> {
        self.store.review_child(
            &parent.lease,
            ReviewRequest {
                request_id: Uuid::new_v4(),
                assignment_id: self.child.assignment.id,
                verdict: ReviewVerdict::Accepted,
                feedback: "Reviewed successful pinned test receipt".into(),
                artifact_ids: vec![],
                checks: vec!["Receipt reviewed".into()],
            },
            now(),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert!(
            self.dir.starts_with(std::env::temp_dir())
                && self
                    .dir
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("ion-command-check-fixture-")
        );
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
fn response(request: &JobRequest) -> JobResponse {
    JobResponse {
        version: 1,
        request_id: request.request_id,
        exit_code: 0,
        timed_out: false,
        canceled: false,
        elapsed_milliseconds: 100,
        stdout: JobStream {
            content_base64: STANDARD.encode("passed"),
            bytes: 6,
            truncated: false,
        },
        stderr: JobStream {
            content_base64: String::new(),
            bytes: 0,
            truncated: false,
        },
        outputs: vec![],
        output_errors: vec![],
    }
}
#[test]
fn command_check_actual_receipt_survives_restart_and_parent_review() {
    let f = Fixture::new();
    let request = f.request();
    let reply = f.execute(&request, response(&request));
    let result = f.result(&request, &reply);
    assert!(f.reserve(&request).unwrap().is_some()); // Exact replay needs no second POST.
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    let submitted = reopened
        .report(
            &f.child.lease,
            AssignmentOutcome::Completed(result.clone()),
            now(),
        )
        .unwrap();
    assert_eq!(
        submitted
            .work
            .iter()
            .find(|w| w.id == f.child.assignment.id)
            .unwrap()
            .status,
        AssignmentStatus::InReview
    );
    let parent = f.resume();
    assert_eq!(parent.children[0].executed_checks, result.executed_checks);
    f.accept(&parent.claimed).unwrap();
    assert_eq!(
        f.store
            .snapshot(f.root.lease.run_id())
            .unwrap()
            .work
            .iter()
            .find(|w| w.id == f.child.assignment.id)
            .unwrap()
            .status,
        AssignmentStatus::Completed
    );
    assert_eq!(
        std::fs::read_to_string(f.source.join("check.py")).unwrap(),
        "assert 1 + 1 == 2\n"
    );
}
#[test]
fn command_check_labels_missing_foreign_and_duplicate_receipts_cannot_complete() {
    let f = Fixture::new();
    let request = f.request();
    let reply = f.execute(&request, response(&request));
    let result = f.result(&request, &reply);
    let mut forged = result.clone();
    forged.executed_checks.clear();
    forged.checks.push(f.command.name.clone());
    assert!(
        f.report(forged)
            .err()
            .unwrap()
            .to_string()
            .contains("receipts are missing")
    );
    let other = Fixture::new();
    let other_request = other.request();
    let other_reply = other.execute(&other_request, response(&other_request));
    other.result(&other_request, &other_reply);
    let mut forged = result.clone();
    forged.executed_checks[0].request_id = other_request.request_id;
    assert!(
        f.report(forged)
            .err()
            .unwrap()
            .to_string()
            .contains("another assignment/attempt")
    );
    let mut forged = result.clone();
    forged.executed_checks[0].name = "Different check".into();
    assert!(
        f.report(forged)
            .err()
            .unwrap()
            .to_string()
            .contains("not required")
    );
    let mut forged = result.clone();
    forged
        .executed_checks
        .push(forged.executed_checks[0].clone());
    assert!(f.report(forged).is_err());
    f.report(result).unwrap();
}
#[test]
fn command_check_failed_timeout_and_canceled_receipts_cannot_complete() {
    for condition in 0..3 {
        let f = Fixture::new();
        let request = f.request();
        let mut reply = response(&request);
        match condition {
            0 => reply.exit_code = 7,
            1 => reply.timed_out = true,
            _ => reply.canceled = true,
        }
        let reply = f.execute(&request, reply);
        let result = f.result(&request, &reply);
        assert!(
            f.report(result)
                .err()
                .unwrap()
                .to_string()
                .contains("Required command check failed")
        );
        assert_eq!(
            f.store
                .snapshot(f.root.lease.run_id())
                .unwrap()
                .work
                .iter()
                .find(|w| w.id == f.child.assignment.id)
                .unwrap()
                .status,
            AssignmentStatus::Running
        );
    }
}
#[test]
fn command_check_changed_source_denies_report_and_parent_acceptance() {
    let f = Fixture::new();
    let request = f.request();
    let reply = f.execute(&request, response(&request));
    let result = f.result(&request, &reply);
    let original = std::fs::read(f.workspace.join("check.py")).unwrap();
    std::fs::write(f.workspace.join("check.py"), "assert False\n").unwrap();
    assert!(
        f.report(result.clone())
            .err()
            .unwrap()
            .to_string()
            .contains("Source changed")
    );
    std::fs::write(f.workspace.join("check.py"), &original).unwrap();
    f.report(result).unwrap();
    let parent = f.resume();
    std::fs::write(f.workspace.join("check.py"), "assert False\n").unwrap();
    assert!(
        f.accept(&parent.claimed)
            .err()
            .unwrap()
            .to_string()
            .contains("Source changed")
    );
    assert_eq!(
        f.store
            .snapshot(f.root.lease.run_id())
            .unwrap()
            .work
            .iter()
            .find(|w| w.id == f.child.assignment.id)
            .unwrap()
            .status,
        AssignmentStatus::InReview
    );
    // Rework remains available; correction preserves the same pinned requirements.
    let review = f
        .store
        .review_child(
            &parent.claimed.lease,
            ReviewRequest {
                request_id: Uuid::new_v4(),
                assignment_id: f.child.assignment.id,
                verdict: ReviewVerdict::Rework,
                feedback: "Run the check against corrected current source".into(),
                artifact_ids: vec![],
                checks: vec!["Source changed".into()],
            },
            now(),
        )
        .unwrap();
    let replacement = f
        .store
        .snapshot(f.root.lease.run_id())
        .unwrap()
        .work
        .into_iter()
        .find(|w| Some(w.id) == review.replacement_assignment_id)
        .unwrap();
    assert_eq!(
        replacement.output_contract.required_commands,
        vec![f.command.clone()]
    );
    assert_ne!(replacement.id, f.child.assignment.id);
}
#[test]
fn command_check_pins_arguments_input_and_timeout() {
    for change in 0..4 {
        let f = Fixture::new();
        let mut request = f.request();
        match change {
            0 => request.argv.push("--skip-check".into()),
            1 => request.timeout_seconds += 1,
            2 => request.inputs.clear(),
            _ => {
                std::fs::write(f.workspace.join("other.py"), "assert True\n").unwrap();
                request.inputs[0].path = "other.py".into();
            }
        }
        let reply = f.execute(&request, response(&request));
        assert!(
            f.report(f.result(&request, &reply))
                .err()
                .unwrap()
                .to_string()
                .contains("differs from its pinned check")
        );
    }
}
#[test]
fn command_check_new_workspace_edit_invalidates_original_receipt_and_pre_post_context() {
    let f = Fixture::new();
    let request = f.request();
    let reply = f.execute(&request, response(&request));
    let result = f.result(&request, &reply);
    let context = f
        .store
        .command_check_context(&f.child.lease, &f.workspace, now())
        .unwrap();
    let old = std::fs::read_to_string(f.workspace.join("check.py")).unwrap();
    f.store
        .edit_workspace_file(
            &f.child.lease,
            Uuid::new_v4(),
            "check.py",
            Some(&old),
            "assert 2 + 2 == 4\n",
            now(),
        )
        .unwrap();
    assert!(
        f.report(result)
            .err()
            .unwrap()
            .to_string()
            .contains("source changed")
    );
    let request = f.request();
    assert!(
        f.store
            .reserve_remote_job_with_context(
                &f.child.lease,
                &request,
                f.policy.image,
                "fixture-project",
                &serde_json::to_string(&f.policy).unwrap(),
                f.policy.max_commands,
                now(),
                context.as_ref()
            )
            .err()
            .unwrap()
            .to_string()
            .contains("source changed")
    );
    assert!(
        f.store
            .unfinished_jobs(f.child.lease.attempt_id())
            .unwrap()
            .is_empty()
    );
    // A newly executed check can complete against the new file/epoch.
    let reply = f.execute(&request, response(&request));
    f.report(f.result(&request, &reply)).unwrap();
}
#[test]
fn command_check_original_source_path_and_receipt_attempt_are_fenced() {
    let f = Fixture::new();
    assert!(
        f.store
            .command_check_context(&f.child.lease, &f.source, now())
            .is_err()
    );
    let request = f.request();
    let reply = f.execute(&request, response(&request));
    let result = f.result(&request, &reply);
    let db = rusqlite::Connection::open(f.dir.join("control.db")).unwrap();
    db.execute(
        "UPDATE ion_team_jobs SET attempt_id=? WHERE request_id=?",
        rusqlite::params![Uuid::new_v4().to_string(), request.request_id.to_string()],
    )
    .unwrap();
    assert!(
        f.report(result)
            .err()
            .unwrap()
            .to_string()
            .contains("another assignment/attempt")
    );
}
#[test]
fn command_check_legacy_receipt_without_original_context_cannot_satisfy_new_gate() {
    let f = Fixture::new();
    let request = f.request();
    let reply = f.execute(&request, response(&request));
    let result = f.result(&request, &reply);
    let db = rusqlite::Connection::open(f.dir.join("control.db")).unwrap();
    db.execute("UPDATE ion_team_jobs SET signature=json_remove(signature,'$.check_context') WHERE request_id=?",[request.request_id.to_string()]).unwrap();
    assert!(
        f.report(result)
            .err()
            .unwrap()
            .to_string()
            .contains("no original check context")
    );
}

#[test]
fn command_check_missing_declared_output_cannot_complete() {
    let f = Fixture::with_command(|command| command.output_paths.push("test-report.json".into()));
    let request = f.request();
    let mut reply = response(&request);
    reply
        .output_errors
        .push(crate::teams::jobs::JobOutputError {
            path: "test-report.json".into(),
            reason: "missing".into(),
        });
    let reply = f.execute(&request, reply);
    assert!(
        f.report(f.result(&request, &reply))
            .err()
            .unwrap()
            .to_string()
            .contains("Required command check failed")
    );
}
