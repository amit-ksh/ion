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
            resources: None,
            project_snapshot: None,
            admission: None,
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
        project_sha256: None,
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

fn project_policy() -> crate::teams::project::ProjectPolicy {
    crate::teams::project::ProjectPolicy {
        version: 1,
        max_bytes: 2 * 1024 * 1024,
        max_file_bytes: 1024 * 1024,
        max_files: 256,
        max_run_bytes: 8 * 1024 * 1024,
    }
}
fn approve_project(f: &mut Fixture) {
    f.policy.resources = Some(ResourceLimits {
        version: 1,
        cpu_seconds: 5,
        address_space_bytes: 256 * 1024 * 1024,
        max_file_bytes: 16 * 1024 * 1024,
        processes: 256,
        open_files: 64,
        group: None,
    });
    f.policy.project_snapshot = Some(project_policy());
}
fn project_manifest() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/job-worker/job-host.json"
    )))
    .unwrap()
}
#[test]
#[ignore = "Requires project fixture exported by the reviewed native Linux CLI acceptance"]
fn command_check_native_project_upload_receipt_replay_and_parent_review() {
    let fixture = std::path::PathBuf::from(
        std::env::var_os("ION_NATIVE_JOB_WORKER_FIXTURE")
            .expect("Export native worker project fixtures first"),
    );
    let read = |name: &str, max: u64| {
        use std::io::Read;
        let mut bytes = vec![];
        std::fs::File::open(fixture.join(name))
            .unwrap()
            .take(max + 1)
            .read_to_end(&mut bytes)
            .unwrap();
        assert!(bytes.len() as u64 <= max);
        bytes
    };
    let request: JobRequest =
        serde_json::from_slice(&read("project-request.json", 512 * 1024)).unwrap();
    let receipt: JobResponse =
        serde_json::from_slice(&read("project-receipt.json", 512 * 1024)).unwrap();
    let archive = read("project.ion", crate::teams::project::MAX_ARCHIVE as u64);
    let facts = request.project_snapshot.as_ref().unwrap();
    crate::teams::project::matches(&archive, facts).unwrap();
    receipt.validate_against(&request).unwrap();
    let mut forged = receipt.clone();
    forged.project_sha256 = None;
    assert!(forged.validate_against(&request).is_err());
    forged.project_sha256 = Some("0".repeat(64));
    assert!(forged.validate_against(&request).is_err());
    assert_eq!(receipt.exit_code, 0);
    assert_eq!(facts.files, 98);
    assert!(facts.source_bytes > 512 * 1024);
    let mut f = Fixture::with_command(|c| {
        c.argv = request.argv.clone();
        c.timeout_seconds = request.timeout_seconds;
        c.input_paths.clear();
        c.output_paths = request.output_paths.clone();
    });
    approve_project(&mut f);
    f.policy.executables = vec![request.argv[0].clone()];
    std::fs::remove_file(f.workspace.join("check.py")).unwrap();
    let length = u32::from_be_bytes(archive[8..12].try_into().unwrap()) as usize;
    let entries: Vec<Value> = serde_json::from_slice(&archive[12..12 + length]).unwrap();
    let mut offset = 12 + length;
    for entry in entries {
        let name = entry["path"].as_str().unwrap();
        path(name).unwrap();
        assert_eq!(entry["executable"], false);
        let file = f.workspace.join(name);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        let end = offset + entry["bytes"].as_u64().unwrap() as usize;
        std::fs::write(file, &archive[offset..end]).unwrap();
        offset = end;
    }
    assert_eq!(
        crate::teams::project::snapshot(&f.workspace, &project_policy())
            .unwrap()
            .0,
        *facts
    );
    let file = Uuid::new_v4();
    let operation = Uuid::new_v4();
    let manifest = project_manifest();
    let terminal = json!({"uuid":operation,"kind":"instance","image_uuid":f.policy.image,"status":"SUCCESS",
        "metadata":{"result":{"state":{"exit_code":0},"stdout":{"encoding":"base64","value":STANDARD.encode(serde_json::to_vec(&receipt).unwrap()),"truncated":false}}}});
    let file_info = json!({"uuid":file,"sha256":facts.sha256,"size":facts.bytes});
    let store_path = f.dir.join("control.db");
    let lease = f.child.lease.clone();
    let request_id = request.request_id;
    let server = Server::with_hook(
        vec![
            (200, serde_json::to_vec(&manifest).unwrap()),
            (404, b"{}".to_vec()),
            (500, b"{}".to_vec()), // Upload acknowledgement lost.
            (200, serde_json::to_vec(&file_info).unwrap()),
            (201, serde_json::to_vec(&json!({"uuid":operation})).unwrap()),
            (200, serde_json::to_vec(&terminal).unwrap()),
            (200, serde_json::to_vec(&manifest).unwrap()),
        ],
        move |call| {
            if call.line.starts_with("POST /files ") {
                let coordinator = TeamCoordinator::new(store_path.clone());
                let pending = coordinator.unfinished_jobs(lease.attempt_id()).unwrap();
                assert_eq!(pending.len(), 1);
                assert_eq!(pending[0].id, request_id);
                assert_eq!(pending[0].state, "staging");
                assert!(coordinator.has_pending_effect(lease.run_id()).unwrap());
            }
        },
    );
    let invoke = || {
        super::project::run_with_host(
            &f.store,
            &f.child,
            &f.workspace,
            request.request_id,
            super::project::ProjectArgs {
                argv: request.argv.clone(),
                output_paths: request.output_paths.clone(),
                timeout_seconds: request.timeout_seconds,
            },
            &AtomicBool::new(false),
            |target| {
                assert_eq!(target, f.policy.target);
                Ok((
                    f.policy.clone(),
                    Client::fixture(&server.url, "fixture-project"),
                ))
            },
        )
    };
    let first = invoke().unwrap();
    assert_eq!(invoke().unwrap(), first);
    assert_eq!(f.store.project_file(request.request_id).unwrap(), file);
    assert!(!f.store.has_pending_effect(f.child.lease.run_id()).unwrap());
    let calls = server.finish();
    assert_eq!(
        calls
            .iter()
            .filter(|c| c.line.starts_with("POST /files "))
            .count(),
        1
    );
    assert_eq!(
        calls
            .iter()
            .find(|c| c.line.starts_with("POST /files "))
            .unwrap()
            .raw_body,
        archive
    );
    let spawn: Vec<_> = calls
        .iter()
        .filter(|c| c.line.starts_with("POST /instances "))
        .collect();
    assert_eq!(spawn.len(), 1);
    assert_eq!(
        spawn[0].body["files"],
        json!({"/ion/job-project.ion":{"uuid":file,"uid":0,"gid":0,"mode":"0400"}})
    );
    assert_eq!(
        spawn[0].body["args"],
        json!(["--project-file", "/ion/job-project.ion"])
    );
    let expected = super::project::body_for(&f.store, &f.policy, &request).unwrap();
    let original = json!({"metadata":expected});
    verify_original_command(&original, &expected).unwrap();
    let mut changed = original.clone();
    changed["metadata"]["files"]["/ion/job-project.ion"]["uuid"] = json!(Uuid::new_v4());
    assert!(verify_original_command(&changed, &expected).is_err());
    let db = rusqlite::Connection::open(f.dir.join("control.db")).unwrap();
    let run = f.store.snapshot(f.child.lease.run_id()).unwrap();
    let work = run
        .work
        .iter()
        .find(|w| w.id == f.child.assignment.id)
        .unwrap();
    let args = json!({"argv":request.argv,"output_paths":request.output_paths,"timeout_seconds":request.timeout_seconds});
    assert_eq!(
        crate::teams::jobs::recover_receipt(
            &db,
            &run,
            work,
            f.child.lease.attempt_id(),
            request.request_id,
            &args
        )
        .unwrap()
        .unwrap(),
        first
    );
    let result = f.result(&request, &receipt);
    f.report(result).unwrap();
    let parent = f.resume().claimed;
    let unchanged = std::fs::read(f.workspace.join("src/part-095.txt")).unwrap();
    std::fs::write(
        f.workspace.join("src/part-095.txt"),
        b"changed unlisted source",
    )
    .unwrap();
    assert!(f.accept(&parent).is_err()); // Whole-project hash protects unlisted sources.
    std::fs::write(f.workspace.join("src/part-095.txt"), unchanged).unwrap();
    f.accept(&parent).unwrap();
}
#[test]
fn command_check_project_bad_upload_and_revocation_never_launch_compute() {
    for revoked in [false, true] {
        let mut f = Fixture::new();
        approve_project(&mut f);
        let stopped = std::sync::Arc::new(AtomicBool::new(false));
        let stop = stopped.clone();
        let manifest = project_manifest();
        let (facts, _) = crate::teams::project::snapshot(&f.workspace, &project_policy()).unwrap();
        let info = json!({"uuid":Uuid::new_v4(),"sha256":if revoked{facts.sha256.clone()}else{"0".repeat(64)},"size":facts.bytes});
        let server = Server::with_hook(
            vec![
                (200, serde_json::to_vec(&manifest).unwrap()),
                (404, b"{}".to_vec()),
                (201, serde_json::to_vec(&info).unwrap()),
            ],
            move |call| {
                if revoked && call.line.starts_with("POST /files ") {
                    stop.store(true, Ordering::Release);
                }
            },
        );
        let result = super::project::run_with_host(
            &f.store,
            &f.child,
            &f.workspace,
            Uuid::new_v4(),
            super::project::ProjectArgs {
                argv: f.command.argv.clone(),
                output_paths: vec![],
                timeout_seconds: 10,
            },
            &stopped,
            |_| {
                Ok((
                    f.policy.clone(),
                    Client::fixture(&server.url, "fixture-project"),
                ))
            },
        );
        assert!(result.is_err());
        assert!(!f.store.has_pending_effect(f.child.lease.run_id()).unwrap());
        assert!(
            f.store
                .unfinished_jobs(f.child.lease.attempt_id())
                .unwrap()
                .is_empty()
        );
        assert!(
            server
                .finish()
                .iter()
                .all(|c| !c.line.starts_with("POST /instances "))
        );
    }
}
#[test]
fn command_check_project_staging_restart_stops_without_upload_or_compute() {
    let mut f = Fixture::new();
    approve_project(&mut f);
    let context = f
        .store
        .project_source_context(&f.child.lease, &f.workspace, now())
        .unwrap();
    let (facts, archive) =
        crate::teams::project::snapshot(&f.workspace, &project_policy()).unwrap();
    let request = JobRequest {
        version: 1,
        request_id: Uuid::new_v4(),
        argv: f.command.argv.clone(),
        timeout_seconds: 10,
        inputs: vec![],
        output_paths: vec![],
        project_snapshot: Some(facts),
    };
    f.store
        .reserve_remote_project(
            &f.child.lease,
            &request,
            f.policy.image,
            "fixture-project",
            &serde_json::to_string(&f.policy).unwrap(),
            8,
            now(),
            &context,
            &archive,
        )
        .unwrap();
    assert!(f.store.has_pending_effect(f.child.lease.run_id()).unwrap());
    assert!(f.store.project_file(request.request_id).is_err());
    f.store.reconcile_owner_restart(now() + 1).unwrap();
    let server = Server::new(vec![]);
    assert!(
        reconcile_with(
            &Client::fixture(&server.url, "fixture-project"),
            &f.store,
            f.store.unfinished_jobs(f.child.lease.attempt_id()).unwrap()
        )
        .unwrap()
    );
    assert!(server.finish().is_empty());
    assert!(!f.store.has_pending_effect(f.child.lease.run_id()).unwrap());
}

#[test]
fn command_check_project_run_byte_cap_and_scope_remain_pinned() {
    let mut f = Fixture::with_command(|c| c.input_paths.clear());
    approve_project(&mut f);
    f.policy.project_snapshot = Some(crate::teams::project::ProjectPolicy {
        version: 1,
        max_bytes: 64 * 1024,
        max_file_bytes: 64 * 1024,
        max_files: 4,
        max_run_bytes: 64 * 1024,
    });
    std::fs::write(f.workspace.join("large.txt"), vec![7; 64000]).unwrap();
    let context = f
        .store
        .project_source_context(&f.child.lease, &f.workspace, now())
        .unwrap();
    let (facts, archive) =
        crate::teams::project::snapshot(&f.workspace, f.policy.project_snapshot.as_ref().unwrap())
            .unwrap();
    let mut request = JobRequest {
        version: 1,
        request_id: Uuid::new_v4(),
        argv: f.command.argv.clone(),
        timeout_seconds: 10,
        inputs: vec![],
        output_paths: vec![],
        project_snapshot: Some(facts.clone()),
    };
    let reserve = |request: &JobRequest, p: &Policy| {
        f.store.reserve_remote_project(
            &f.child.lease,
            request,
            p.image,
            "fixture-project",
            &serde_json::to_string(p).unwrap(),
            8,
            now(),
            &context,
            &archive,
        )
    };
    // A foreign workspace is refused before durable upload reservation.
    assert!(
        f.store
            .project_source_context(&f.child.lease, &f.source, now())
            .is_err()
    );
    assert!(reserve(&request, &f.policy).unwrap().is_none());
    assert!(f.store.has_pending_effect(f.child.lease.run_id()).unwrap());
    let file = Uuid::new_v4();
    f.store
        .confirm_project_upload(&f.child.lease, request.request_id, file, now())
        .unwrap();
    assert!(
        f.store
            .confirm_project_upload(&f.child.lease, request.request_id, Uuid::new_v4(), now())
            .is_err()
    );
    f.store
        .bind_job_operation(request.request_id, Uuid::new_v4())
        .unwrap();
    let mut receipt = response(&request);
    receipt.project_sha256 = Some(facts.sha256);
    f.store
        .finish_job(request.request_id, Some(&receipt))
        .unwrap();
    request.request_id = Uuid::new_v4();
    assert!(reserve(&request, &f.policy).is_err()); // Count allows it; run bytes do not.
    let mut expanded = f.policy.clone();
    expanded.project_snapshot.as_mut().unwrap().max_run_bytes = 128 * 1024;
    assert!(reserve(&request, &expanded).is_err()); // Registry edit cannot enlarge original allowance.
    assert!(!f.store.has_pending_effect(f.child.lease.run_id()).unwrap());
}

#[test]
fn command_check_project_mode_only_change_invalidates_report_and_parent_review() {
    for parent_gate in [false, true] {
        let mut f = Fixture::with_command(|c| c.input_paths.clear());
        approve_project(&mut f);
        git(&f.workspace, &["config", "core.filemode", "false"]);
        let context = f
            .store
            .project_source_context(&f.child.lease, &f.workspace, now())
            .unwrap();
        let (facts, archive) =
            crate::teams::project::snapshot(&f.workspace, &project_policy()).unwrap();
        let request = JobRequest {
            version: 1,
            request_id: Uuid::new_v4(),
            argv: f.command.argv.clone(),
            timeout_seconds: f.command.timeout_seconds,
            inputs: vec![],
            output_paths: vec![],
            project_snapshot: Some(facts),
        };
        f.store
            .reserve_remote_project(
                &f.child.lease,
                &request,
                f.policy.image,
                "fixture-project",
                &serde_json::to_string(&f.policy).unwrap(),
                8,
                now(),
                &context,
                &archive,
            )
            .unwrap();
        f.store
            .confirm_project_upload(&f.child.lease, request.request_id, Uuid::new_v4(), now())
            .unwrap();
        f.store
            .bind_job_operation(request.request_id, Uuid::new_v4())
            .unwrap();
        let mut receipt = response(&request);
        receipt.project_sha256 = request.project_snapshot.as_ref().map(|p| p.sha256.clone());
        f.store
            .finish_job(request.request_id, Some(&receipt))
            .unwrap();
        let result = f.result(&request, &receipt);
        let parent = if parent_gate {
            f.report(result.clone()).unwrap();
            Some(f.resume().claimed)
        } else {
            None
        };
        let original = std::fs::read(f.workspace.join("check.py")).unwrap();
        git(&f.workspace, &["update-index", "--chmod=+x", "check.py"]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                f.workspace.join("check.py"),
                std::fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }
        assert_eq!(
            std::fs::read(f.workspace.join("check.py")).unwrap(),
            original
        );
        if let Some(parent) = &parent {
            assert!(f.accept(parent).is_err());
        } else {
            assert!(f.report(result.clone()).is_err());
        }
        git(&f.workspace, &["update-index", "--chmod=-x", "check.py"]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                f.workspace.join("check.py"),
                std::fs::Permissions::from_mode(0o644),
            )
            .unwrap();
        }
        if let Some(parent) = &parent {
            f.accept(parent).unwrap();
        } else {
            f.report(result).unwrap();
        }
    }
}

#[test]
fn command_check_project_admission_refuses_before_upload_and_retains_no_partial_reservation() {
    let mut f = Fixture::with_command(|c| c.input_paths.clear());
    approve_project(&mut f);
    f.policy.resources.as_mut().unwrap().group = Some(ResourceGroupLimits {
        version: 1,
        cpu_millis: 500,
        memory_bytes: 128 * 1024 * 1024,
        processes: 32,
    });
    let cap = crate::teams::jobs::admission::CommandResourceVector {
        jobs: 1,
        cpu_millis: 500,
        memory_bytes: 128 * 1024 * 1024,
        processes: 32,
        storage_bytes: f.policy.max_layer_bytes,
    };
    f.policy.admission = Some(crate::teams::jobs::admission::AdmissionPolicy {
        version: 1,
        owner_limit: cap.clone(),
        project_limit: cap,
    });
    let server = Server::new(vec![(
        200,
        serde_json::to_vec(&project_manifest()).unwrap(),
    )]);
    let result = super::project::run_with_host(
        &f.store,
        &f.child,
        &f.workspace,
        Uuid::new_v4(),
        super::project::ProjectArgs {
            argv: f.command.argv.clone(),
            output_paths: vec![],
            timeout_seconds: 10,
        },
        &AtomicBool::new(false),
        |_| {
            Ok((
                f.policy.clone(),
                Client::fixture(&server.url, "fixture-project"),
            ))
        },
    );
    assert!(result.is_err());
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].line.starts_with("GET /inspect/"));
    assert_eq!(f.store.command_resource_usage(None).unwrap().used.jobs, 0);
    let db = rusqlite::Connection::open(f.dir.join("control.db")).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM ion_team_jobs", [], |r| r
            .get::<_, u64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM ion_team_job_projects", [], |r| r
            .get::<_, u64>(0))
            .unwrap(),
        0
    );
}
