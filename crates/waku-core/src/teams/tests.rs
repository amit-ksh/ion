use super::*;
use waku_protocol::model::ProviderKind;

struct Fixture {
    dir: PathBuf,
    store: TeamCoordinator,
    definitions: crate::control::ControlStore,
    team: Uuid,
    agents: [Uuid; 4],
    project: Uuid,
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("ion-team-tests-{}", Uuid::new_v4()));
        let path = dir.join("control.db");
        let definitions = crate::control::ControlStore::new(path.clone());
        let agents = std::array::from_fn(|_| Uuid::new_v4());
        for (i, id) in agents.iter().enumerate() {
            let revision = definitions.load().unwrap().revision;
            definitions
                .save(
                    revision,
                    ControlRecord::Agent(AgentProfile {
                        id: *id,
                        name: format!("Agent {i}"),
                        expertise: "Engineering".into(),
                        instructions: "Produce reviewable work".into(),
                        provider: [
                            ProviderKind::Nebius,
                            ProviderKind::Codex,
                            ProviderKind::Claude,
                            ProviderKind::OpenCode,
                        ][i],
                        model: Some(format!("model-{i}")),
                        expected_output: "Result and evidence".into(),
                        execution: AgentExecution::default(),
                        access: AgentAccess {
                            workspace_read: true,
                            workspace_write: true,
                            shell: false,
                            network: false,
                            app_actions: vec![],
                        },
                        max_parallel_tasks: 1,
                    }),
                )
                .unwrap();
        }
        let team = Uuid::new_v4();
        definitions
            .save(
                definitions.load().unwrap().revision,
                ControlRecord::Team(Team {
                    id: team,
                    name: "Product team".into(),
                    objective: "Deliver a reviewed change".into(),
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
                        TeamMember {
                            agent_id: agents[2],
                            parent_id: Some(agents[1]),
                        },
                        TeamMember {
                            agent_id: agents[3],
                            parent_id: Some(agents[1]),
                        },
                    ],
                }),
            )
            .unwrap();
        Self {
            dir,
            store: TeamCoordinator::new(path),
            definitions,
            team,
            agents,
            project: Uuid::new_v4(),
        }
    }
    fn request(&self) -> PrepareRun {
        PrepareRun {
            account_scope: None,
            id: Uuid::new_v4(),
            root_task_id: Uuid::new_v4(),
            project_id: self.project,
            team_id: self.team,
            expected_definition_revision: self.definitions.load().unwrap().revision,
            objective: "Implement and validate".into(),
            input_data: vec![],
            source_attachments: vec![],
            source_prompt: None,
            origin: None,
            limits: TeamRunLimits {
                require_child_review: false,
                ..Default::default()
            },
            access: AgentAccess {
                workspace_read: true,
                workspace_write: true,
                ..Default::default()
            },
        }
    }
    fn run(&self) -> TeamRunSnapshot {
        self.store.prepare(self.request(), 100).unwrap()
    }
    fn claim(&self, run: Uuid, work: Uuid) -> ClaimedAssignment {
        self.store
            .claim(run, work, DispatchHost::Local, 101, 100)
            .unwrap()
    }
    fn delegation(&self, agent: usize) -> Delegation {
        Delegation {
            request_id: Uuid::new_v4(),
            agent_id: self.agents[agent],
            objective: "Make the change".into(),
            expected_output: "Patch and checks".into(),
            dependencies: vec![],
            access: AgentAccess::default(),
        }
    }
    fn materialize(&self) {
        for task in self.store.pending_materializations(128).unwrap() {
            self.store
                .acknowledge_materialized(
                    task.run_id,
                    task.assignment.id,
                    task.assignment.task_id,
                    102,
                )
                .unwrap();
        }
    }
    fn complete(&self, lease: &AssignmentLease) -> TeamRunSnapshot {
        self.store
            .report(lease, AssignmentOutcome::Completed(result()), 104)
            .unwrap()
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
                    .starts_with("ion-team-tests-")
        );
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
fn result() -> AssignmentResult {
    AssignmentResult {
        executed_checks: vec![],
        summary: "Change delivered".into(),
        artifact_ids: vec![],
        checks: vec!["Validation evidence".into()],
    }
}

#[test]
fn legacy_profiles_migrate_to_fixed_local_read_only_settings() {
    let profile:AgentProfile=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"name":"Existing","expertise":"Code","instructions":"Review","provider":"nebius","model":null})).unwrap();
    assert_eq!(profile.execution.environment, ExecutionEnvironment::Local);
    assert_eq!(profile.access, AgentAccess::default());
    assert_eq!(profile.max_parallel_tasks, 1);
    assert!(serde_json::from_str::<ExecutionEnvironment>("\"auto\"").is_err());
}
#[test]
fn immutable_profiles_survive_edits_restart_and_repeated_prepare() {
    let f = Fixture::new();
    let request = f.request();
    let run = f.store.prepare(request.clone(), 100).unwrap();
    let mut profile = run.profiles[1].clone();
    profile.provider = ProviderKind::OpenCode;
    profile.model = Some("new-model".into());
    f.definitions
        .save(
            f.definitions.load().unwrap().revision,
            ControlRecord::Agent(profile),
        )
        .unwrap();
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    let old = reopened.prepare(request.clone(), 150).unwrap();
    assert_eq!(old.id, run.id);
    assert_eq!(old.profiles[1].provider, ProviderKind::Codex);
    assert_eq!(old.profiles[1].model.as_deref(), Some("model-1"));
    let mut conflict = request.clone();
    conflict.objective = "Changed objective".into();
    assert!(reopened.prepare(conflict, 150).is_err());
    let mut stale = request;
    stale.id = Uuid::new_v4();
    stale.root_task_id = Uuid::new_v4();
    assert!(reopened.prepare(stale, 150).is_err());
    let new = f.run();
    assert_eq!(new.profiles[1].provider, ProviderKind::OpenCode);
}
#[test]
fn three_level_hierarchy_uses_each_members_harness_model_and_access() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    assert!(f.store.delegate(&root.lease, f.delegation(2), 102).is_err());
    let mut outside = f.delegation(1);
    outside.agent_id = Uuid::new_v4();
    assert!(f.store.delegate(&root.lease, outside, 102).is_err());
    let mut broad = f.delegation(1);
    broad.access.shell = true;
    assert!(f.store.delegate(&root.lease, broad, 102).is_err());
    let mut ancestor = f.delegation(1);
    ancestor.dependencies.push(root.assignment.id);
    assert!(f.store.delegate(&root.lease, ancestor, 102).is_err());
    let request = f.delegation(1);
    let request_id = request.request_id;
    let manager = f.store.delegate(&root.lease, request, 102).unwrap();
    let mut repeat = f.delegation(1);
    repeat.request_id = request_id;
    assert_eq!(
        f.store.delegate(&root.lease, repeat, 102).unwrap().task_id,
        manager.task_id
    );
    let mut changed = f.delegation(1);
    changed.request_id = request_id;
    changed.objective = "Different".into();
    assert!(f.store.delegate(&root.lease, changed, 102).is_err());
    assert!(
        f.store
            .claim(run.id, manager.id, DispatchHost::Local, 102, 100)
            .is_err()
    );
    f.materialize();
    let manager = f.claim(run.id, manager.id);
    assert_eq!(manager.profile.provider, ProviderKind::Codex);
    assert_eq!(manager.profile.model.as_deref(), Some("model-1"));
    let worker = f
        .store
        .delegate(&manager.lease, f.delegation(2), 102)
        .unwrap();
    f.materialize();
    let worker = f.claim(run.id, worker.id);
    assert_eq!(worker.profile.provider, ProviderKind::Claude);
    assert_eq!(worker.profile.model.as_deref(), Some("model-2"));
    assert!(!worker.assignment.access.workspace_write);
    assert!(
        f.store
            .delegate(&worker.lease, f.delegation(0), 102)
            .is_err()
    );
    assert!(
        f.store
            .report(&manager.lease, AssignmentOutcome::Completed(result()), 103)
            .is_err()
    );
    f.complete(&worker.lease);
    f.complete(&manager.lease);
    assert_eq!(f.complete(&root.lease).status, TeamRunStatus::Completed);
}
#[test]
fn dependency_results_gate_dispatch_and_parent_completion() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let manager = f.store.delegate(&root.lease, f.delegation(1), 102).unwrap();
    f.materialize();
    let manager = f.claim(run.id, manager.id);
    let engineer = f
        .store
        .delegate(&manager.lease, f.delegation(2), 102)
        .unwrap();
    let mut request = f.delegation(3);
    request.dependencies.push(engineer.id);
    let reviewer = f.store.delegate(&manager.lease, request, 102).unwrap();
    f.materialize();
    assert!(
        f.store
            .claim(run.id, reviewer.id, DispatchHost::Local, 103, 100)
            .is_err()
    );
    let engineer = f.claim(run.id, engineer.id);
    f.complete(&engineer.lease);
    let reviewer = f.claim(run.id, reviewer.id);
    f.complete(&reviewer.lease);
    f.complete(&manager.lease);
    assert_eq!(f.complete(&root.lease).status, TeamRunStatus::Completed);
}
#[test]
fn outbox_replay_preserves_exact_child_uuid_and_ack_is_idempotent() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let child = f.store.delegate(&root.lease, f.delegation(1), 102).unwrap();
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    let pending = reopened.pending_materializations(128).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].assignment.task_id, child.task_id);
    assert!(
        reopened
            .acknowledge_materialized(run.id, child.id, Uuid::new_v4(), 103)
            .is_err()
    );
    reopened
        .acknowledge_materialized(run.id, child.id, child.task_id, 103)
        .unwrap();
    let revision = reopened.snapshot(run.id).unwrap().revision;
    reopened
        .acknowledge_materialized(run.id, child.id, child.task_id, 103)
        .unwrap();
    assert_eq!(reopened.snapshot(run.id).unwrap().revision, revision);
    assert!(reopened.pending_materializations(128).unwrap().is_empty());
    assert_eq!(
        reopened
            .audit_events(run.id, 0, 100)
            .unwrap()
            .iter()
            .filter(|e| e.event == "materialized")
            .count(),
        1
    );
}
#[test]
fn shared_agent_capacity_respects_all_pinned_profile_limits() {
    let f = Fixture::new();
    let a = f.run();
    let active = f.claim(a.id, a.work[0].id);
    let mut edited = a.profiles[0].clone();
    edited.max_parallel_tasks = 2;
    f.definitions
        .save(
            f.definitions.load().unwrap().revision,
            ControlRecord::Agent(edited),
        )
        .unwrap();
    let b = f.run();
    assert!(
        f.store
            .claim(b.id, b.work[0].id, DispatchHost::Local, 102, 100)
            .is_err()
    );
    f.complete(&active.lease);
    assert!(
        f.store
            .claim(b.id, b.work[0].id, DispatchHost::Local, 105, 100)
            .is_ok()
    );
}
#[test]
fn stop_revokes_dispatch_and_waits_for_confirmed_host_cancellation() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let child = f.store.delegate(&root.lease, f.delegation(1), 102).unwrap();
    let revision = f.store.snapshot(run.id).unwrap().revision;
    assert!(f.store.stop(run.id, revision - 1, 103).is_err());
    let stopping = f.store.stop(run.id, revision, 103).unwrap();
    assert_eq!(stopping.status, TeamRunStatus::Stopping);
    assert_eq!(stopping.work[0].status, AssignmentStatus::Canceling);
    assert_eq!(stopping.work[1].status, AssignmentStatus::Canceled);
    assert!(f.store.pending_materializations(128).unwrap().is_empty());
    assert!(f.store.delegate(&root.lease, f.delegation(1), 103).is_err());
    assert!(
        f.store
            .claim(run.id, child.id, DispatchHost::Local, 103, 100)
            .is_err()
    );
    assert!(f.store.renew(&root.lease, 103, 100).is_err());
    assert_eq!(
        f.store
            .report(&root.lease, AssignmentOutcome::Canceled, 104)
            .unwrap()
            .status,
        TeamRunStatus::Canceled
    );
}
#[test]
fn expired_remote_work_is_uncertain_and_cannot_fallback_or_release_capacity() {
    let f = Fixture::new();
    let mut lead = match &f
        .definitions
        .load()
        .unwrap()
        .records
        .iter()
        .find(|r| r.id() == f.agents[0])
        .unwrap()
    {
        ControlRecord::Agent(a) => a.clone(),
        _ => panic!(),
    };
    lead.execution = AgentExecution {
        environment: ExecutionEnvironment::NebiusSandbox,
        sandbox_target: Some("gpu-target".into()),
        local_fallback: true,
    };
    f.definitions
        .save(
            f.definitions.load().unwrap().revision,
            ControlRecord::Agent(lead),
        )
        .unwrap();
    let run = f.run();
    let root = f
        .store
        .claim(
            run.id,
            run.work[0].id,
            DispatchHost::NebiusSandbox {
                target: "gpu-target".into(),
            },
            101,
            10,
        )
        .unwrap();
    let expired = f.store.reconcile_expired(run.id, 112).unwrap();
    assert_eq!(expired.work[0].status, AssignmentStatus::Uncertain);
    assert!(
        f.store
            .report(&root.lease, AssignmentOutcome::Canceled, 112)
            .is_err()
    );
    assert!(
        f.store
            .claim(
                run.id,
                run.work[0].id,
                DispatchHost::LocalFallback {
                    failure: SandboxStartFailure::Unavailable
                },
                112,
                10
            )
            .is_err()
    );
    let next = f.run();
    assert!(
        f.store
            .claim(
                next.id,
                next.work[0].id,
                DispatchHost::NebiusSandbox {
                    target: "gpu-target".into()
                },
                112,
                10
            )
            .is_err()
    );
    assert!(
        f.store
            .resolve_uncertain(
                run.id,
                root.assignment.id,
                Uuid::new_v4(),
                AssignmentOutcome::Canceled,
                112
            )
            .is_err()
    );
    f.store
        .resolve_uncertain(
            run.id,
            root.assignment.id,
            root.lease.attempt_id(),
            AssignmentOutcome::Canceled,
            112,
        )
        .unwrap();
    assert!(
        f.store
            .claim(
                next.id,
                next.work[0].id,
                DispatchHost::NebiusSandbox {
                    target: "gpu-target".into()
                },
                113,
                10
            )
            .is_ok()
    );
}
#[test]
fn sandbox_start_failure_uses_local_without_substituting_harness_or_model() {
    let f = Fixture::new();
    let mut lead = f.run().profiles[0].clone();
    lead.execution = AgentExecution {
        environment: ExecutionEnvironment::NebiusSandbox,
        sandbox_target: Some("target".into()),
        local_fallback: true,
    };
    f.definitions
        .save(
            f.definitions.load().unwrap().revision,
            ControlRecord::Agent(lead.clone()),
        )
        .unwrap();
    let run = f.run();
    let work = run.work[0].id;
    assert!(
        f.store
            .claim(run.id, work, DispatchHost::Local, 101, 10)
            .is_err()
    );
    assert!(
        f.store
            .claim(
                run.id,
                work,
                DispatchHost::NebiusSandbox {
                    target: "different-target".into()
                },
                101,
                10
            )
            .is_err()
    );
    let claimed = f
        .store
        .claim(
            run.id,
            work,
            DispatchHost::LocalFallback {
                failure: SandboxStartFailure::Unavailable,
            },
            101,
            10,
        )
        .unwrap();
    assert_eq!(claimed.profile.provider, lead.provider);
    assert_eq!(claimed.profile.model, lead.model);
    assert_eq!(claimed.assignment.access, run.work[0].access);
    assert_eq!(
        claimed.assignment.attempts[0].environment,
        ExecutionEnvironment::Local
    );
    assert!(claimed.assignment.attempts[0].fallback_reason.is_some());
}
#[test]
fn confirmed_safe_retries_fence_old_leases_and_obey_attempt_limit() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let failed = f
        .store
        .report(
            &root.lease,
            AssignmentOutcome::FailedBeforeEffects("Harness failed to start".into()),
            102,
        )
        .unwrap();
    assert!(
        f.store
            .retry(run.id, root.assignment.id, failed.revision - 1, 103)
            .is_err()
    );
    f.store
        .retry(run.id, root.assignment.id, failed.revision, 103)
        .unwrap();
    let second = f.claim(run.id, root.assignment.id);
    assert_ne!(second.lease.attempt_id(), root.lease.attempt_id());
    assert!(f.store.renew(&root.lease, 103, 100).is_err());
    assert!(
        f.store
            .report(&root.lease, AssignmentOutcome::Canceled, 103)
            .is_err()
    );
    let failed = f
        .store
        .report(
            &second.lease,
            AssignmentOutcome::FailedBeforeEffects("Unavailable again".into()),
            104,
        )
        .unwrap();
    assert!(
        f.store
            .retry(run.id, root.assignment.id, failed.revision, 105)
            .is_err()
    );
}
#[test]
fn racing_claims_issue_only_one_lease() {
    let f = Fixture::new();
    let run = f.run();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let barrier = barrier.clone();
            let path = f.dir.join("control.db");
            let run_id = run.id;
            let work_id = run.work[0].id;
            std::thread::spawn(move || {
                barrier.wait();
                TeamCoordinator::new(path)
                    .claim(run_id, work_id, DispatchHost::Local, 101, 100)
                    .is_ok()
            })
        })
        .collect();
    assert_eq!(
        threads
            .into_iter()
            .filter_map(|t| t.join().ok())
            .filter(|ok| *ok)
            .count(),
        1
    );
    assert_eq!(f.store.snapshot(run.id).unwrap().work[0].attempts.len(), 1);
}
#[test]
fn results_require_evidence_and_documents_belonging_to_the_assignment() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let mut empty = result();
    empty.checks.clear();
    assert!(
        f.store
            .report(
                &root.lease,
                AssignmentOutcome::Completed(empty.clone()),
                102
            )
            .is_err()
    );
    empty.artifact_ids.push(Uuid::new_v4());
    assert!(
        f.store
            .report(&root.lease, AssignmentOutcome::Completed(empty), 102)
            .is_err()
    );
    let snapshot = f
        .definitions
        .import_document(
            f.definitions.load().unwrap().revision,
            "report.html".into(),
            "html".into(),
            Some(f.project),
            Some(root.assignment.task_id),
            b"<h1>Result</h1>".to_vec(),
        )
        .unwrap();
    let artifact = snapshot
        .records
        .iter()
        .find_map(|r| match r {
            ControlRecord::Artifact(a) => Some(a.id),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        f.store
            .report(
                &root.lease,
                AssignmentOutcome::Completed(AssignmentResult {
                    executed_checks: vec![],
                    summary: "Report".into(),
                    artifact_ids: vec![artifact],
                    checks: vec![]
                }),
                103
            )
            .unwrap()
            .status,
        TeamRunStatus::Completed
    );
}
#[test]
fn deadlines_and_delegation_caps_block_additional_work() {
    let f = Fixture::new();
    let mut request = f.request();
    request.limits.max_work_items = 1;
    request.limits.timeout_seconds = 60;
    let run = f.store.prepare(request, 100).unwrap();
    let root = f
        .store
        .claim(run.id, run.work[0].id, DispatchHost::Local, 101, 300)
        .unwrap();
    assert_eq!(root.assignment.attempts[0].lease_expires_at, 160);
    assert!(f.store.delegate(&root.lease, f.delegation(1), 102).is_err());
    assert!(f.store.renew(&root.lease, 160, 10).is_err());
    assert_eq!(
        f.store.reconcile_expired(run.id, 160).unwrap().work[0].status,
        AssignmentStatus::Uncertain
    );
}
#[test]
fn exact_app_grants_are_intersected_and_never_broadened() {
    let a = AppActionGrant {
        account_id: "account-a".into(),
        action: "GITHUB_GET_ISSUE".into(),
        version: String::new(),
    };
    let b = AppActionGrant {
        account_id: "account-b".into(),
        action: "GITHUB_GET_ISSUE".into(),
        version: String::new(),
    };
    let first = AgentAccess {
        network: true,
        app_actions: vec![a.clone(), b],
        ..Default::default()
    };
    let second = AgentAccess {
        network: false,
        app_actions: vec![a.clone()],
        ..Default::default()
    };
    let intersection = intersect_access(&first, &second);
    assert!(!intersection.network);
    assert_eq!(intersection.app_actions, vec![a]);
    assert!(
        validate_access(&AgentAccess {
            app_actions: vec![AppActionGrant {
                account_id: "*".into(),
                action: "*".into(),
                version: String::new()
            }],
            ..Default::default()
        })
        .is_err()
    );
}

#[test]
fn owner_restart_revokes_unexpired_workers_without_replaying_effects() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    reopened.reconcile_owner_restart(105).unwrap();
    assert_eq!(
        reopened.snapshot(run.id).unwrap().work[0].status,
        AssignmentStatus::Uncertain
    );
    assert!(reopened.renew(&root.lease, 106, 100).is_err());
    assert!(
        reopened
            .report(&root.lease, AssignmentOutcome::Completed(result()), 106)
            .is_err()
    );
    let events = reopened.audit_events(run.id, 0, 100).unwrap();
    reopened.reconcile_owner_restart(107).unwrap();
    assert_eq!(
        reopened.audit_events(run.id, 0, 100).unwrap().len(),
        events.len()
    );
}
#[test]
fn repeated_results_create_one_durable_event_and_changed_results_are_rejected() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let done = f.complete(&root.lease);
    let repeated = f
        .store
        .report(&root.lease, AssignmentOutcome::Completed(result()), 300)
        .unwrap();
    assert_eq!(repeated.revision, done.revision);
    assert_eq!(
        f.store
            .audit_events(run.id, 0, 100)
            .unwrap()
            .iter()
            .filter(|e| e.event == "reported")
            .count(),
        1
    );
    assert!(
        f.store
            .report(&root.lease, AssignmentOutcome::Canceled, 300)
            .is_err()
    );
}

#[test]
fn confirmed_remote_failure_can_retry_locally_and_cannot_bounce_back() {
    let f = Fixture::new();
    let mut lead = f.run().profiles[0].clone();
    lead.execution = AgentExecution {
        environment: ExecutionEnvironment::NebiusSandbox,
        sandbox_target: Some("target".into()),
        local_fallback: true,
    };
    f.definitions
        .save(
            f.definitions.load().unwrap().revision,
            ControlRecord::Agent(lead),
        )
        .unwrap();
    let remote_run = f.run();
    let remote = f
        .store
        .claim(
            remote_run.id,
            remote_run.work[0].id,
            DispatchHost::NebiusSandbox {
                target: "target".into(),
            },
            101,
            100,
        )
        .unwrap();
    let failed = f
        .store
        .report(
            &remote.lease,
            AssignmentOutcome::FailedBeforeEffects("Host confirmed startup failed".into()),
            102,
        )
        .unwrap();
    f.store
        .retry(remote_run.id, remote.assignment.id, failed.revision, 103)
        .unwrap();
    let fallback = f
        .store
        .claim(
            remote_run.id,
            remote.assignment.id,
            DispatchHost::LocalFallback {
                failure: SandboxStartFailure::StartupFailed,
            },
            104,
            100,
        )
        .unwrap();
    assert_eq!(fallback.profile.provider, remote.profile.provider);
    assert_eq!(fallback.profile.model, remote.profile.model);
    let local_run = f.run();
    // Release the shared lead from the previous run first.
    f.store
        .report(&fallback.lease, AssignmentOutcome::Canceled, 105)
        .unwrap();
    let local = f
        .store
        .claim(
            local_run.id,
            local_run.work[0].id,
            DispatchHost::LocalFallback {
                failure: SandboxStartFailure::Unavailable,
            },
            106,
            100,
        )
        .unwrap();
    let failed = f
        .store
        .report(
            &local.lease,
            AssignmentOutcome::FailedBeforeEffects("Local harness failed to start".into()),
            107,
        )
        .unwrap();
    f.store
        .retry(local_run.id, local.assignment.id, failed.revision, 108)
        .unwrap();
    assert!(
        f.store
            .claim(
                local_run.id,
                local.assignment.id,
                DispatchHost::NebiusSandbox {
                    target: "target".into()
                },
                109,
                100
            )
            .is_err()
    );
}
#[test]
fn result_payloads_remain_bounded_and_disabled_fallback_is_respected() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let mut large = result();
    large.checks = vec!["x".repeat(4096); 64];
    assert!(
        f.store
            .report(&root.lease, AssignmentOutcome::Completed(large), 102)
            .is_err()
    );
    f.store
        .report(&root.lease, AssignmentOutcome::Canceled, 103)
        .unwrap();
    let mut lead = run.profiles[0].clone();
    lead.execution = AgentExecution {
        environment: ExecutionEnvironment::NebiusSandbox,
        sandbox_target: None,
        local_fallback: false,
    };
    f.definitions
        .save(
            f.definitions.load().unwrap().revision,
            ControlRecord::Agent(lead),
        )
        .unwrap();
    let next = f.run();
    assert!(
        f.store
            .claim(
                next.id,
                next.work[0].id,
                DispatchHost::LocalFallback {
                    failure: SandboxStartFailure::Unavailable
                },
                104,
                100
            )
            .is_err()
    );
}

#[test]
fn task_db_commit_before_outbox_ack_replays_without_duplicate_or_lost_transcript() {
    use crate::{
        model::{Message, MessageRole, Project},
        persistence::StateStore,
    };
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let child = f.store.delegate(&root.lease, f.delegation(1), 102).unwrap();
    {
        let task_store = StateStore::daemon(f.dir.join("tasks.db"));
        let mut state = task_store.load().unwrap();
        state.projects.push(Project {
            id: f.project,
            name: "Project".into(),
            path: f.dir.clone(),
            created_at: 100,
        });
        let pending = f.store.pending_materializations(1).unwrap();
        let mut session = materialize::task_session(&pending[0]);
        // Older task rows have no binding; replay adopts it and preserves detail.
        session.team_assignment = None;
        session.messages.push(Message::new(
            MessageRole::Assistant,
            "Preserve this existing transcript",
        ));
        state.push_session(session);
        task_store.save(&mut state).unwrap();
        // Simulated crash: task committed, acknowledgment never written.
    }
    let task_store = StateStore::daemon(f.dir.join("tasks.db"));
    let mut state = task_store.load().unwrap();
    assert_eq!(
        f.store
            .materialize_pending(&task_store, &mut state, 103)
            .unwrap(),
        1
    );
    assert_eq!(
        f.store
            .materialize_pending(&task_store, &mut state, 104)
            .unwrap(),
        0
    );
    assert_eq!(state.sessions.len(), 1);
    let session = &mut state.sessions[0];
    task_store.hydrate(session).unwrap();
    assert_eq!(session.id, child.task_id);
    assert_eq!(session.provider, ProviderKind::Codex);
    assert_eq!(session.model.as_deref(), Some("model-1"));
    assert_eq!(session.messages.len(), 2);
    assert_eq!(
        session.messages[1].content,
        "Preserve this existing transcript"
    );
    assert!(f.store.pending_materializations(128).unwrap().is_empty());
    assert!(session.team_assignment.is_some());
    let restarted = StateStore::daemon(f.dir.join("tasks.db"));
    let mut loaded = restarted.load().unwrap();
    restarted.hydrate(&mut loaded.sessions[0]).unwrap();
    assert_eq!(loaded.sessions[0].team_assignment, session.team_assignment);
    assert_eq!(loaded.sessions[0].messages.len(), 2);
}

#[test]
fn explicit_invocation_recovers_its_root_with_pinned_settings() {
    use crate::{
        model::Project,
        persistence::{PersistedState, StateStore},
    };
    let f = Fixture::new();
    let mut request = f.request();
    request.access = Default::default();
    request.source_prompt = Some(format!("@team:{} {}", f.team, request.objective));
    let run = f.store.queue_task(request.clone(), 100).unwrap();
    assert_eq!(run.source_prompt, request.source_prompt);
    assert_eq!(
        f.store
            .binding_for_task(run.root_task_id)
            .unwrap()
            .unwrap()
            .run_id,
        run.id
    );
    assert!(
        f.store
            .claim(run.id, run.work[0].id, DispatchHost::Local, 101, 100)
            .is_err()
    );
    let mut state = PersistedState::empty();
    state.projects.push(Project {
        id: f.project,
        name: "Project".into(),
        path: f.dir.clone(),
        created_at: 100,
    });
    let task_store = StateStore::daemon(f.dir.join("tasks.db"));
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    assert_eq!(
        reopened
            .materialize_run(run.id, &task_store, &mut state, 102)
            .unwrap(),
        1
    );
    let task = state
        .sessions
        .iter()
        .find(|s| s.id == run.root_task_id)
        .unwrap()
        .clone();
    assert_eq!(task.provider, ProviderKind::Nebius);
    assert_eq!(task.model.as_deref(), Some("model-0"));
    assert_eq!(task.messages[0].content, request.source_prompt.unwrap());
    assert_eq!(
        task.team_assignment.as_ref().unwrap().assignment_id,
        run.work[0].id
    );
    assert_eq!(
        reopened
            .materialize_run(run.id, &task_store, &mut state, 103)
            .unwrap(),
        0
    );
    assert_eq!(state.sessions.len(), 1);
    let mut loaded = task_store.load().unwrap();
    task_store.hydrate(&mut loaded.sessions[0]).unwrap();
    assert_eq!(loaded.sessions[0].team_assignment, task.team_assignment);
}
#[test]
fn agent_invocation_pins_scope_deduplicates_and_rejects_changed_arguments() {
    let f = Fixture::new();
    let task = Uuid::new_v4();
    let revision = f.definitions.load().unwrap().revision;
    let run = f
        .store
        .queue_from_agent(
            task,
            f.project,
            "call-1".into(),
            f.team,
            revision,
            "Research".into(),
            100,
        )
        .unwrap();
    assert_eq!(run.origin.as_ref().unwrap().task_id, task);
    assert_eq!(run.access, AgentAccess::default());
    assert!(!run.work[0].access.workspace_write);
    assert!(run.work[0].access.app_actions.is_empty());
    assert_eq!(
        f.store
            .queue_from_agent(
                task,
                f.project,
                "call-1".into(),
                f.team,
                revision,
                "Research".into(),
                101
            )
            .unwrap()
            .root_task_id,
        run.root_task_id
    );
    assert!(
        f.store
            .queue_from_agent(
                task,
                f.project,
                "call-1".into(),
                f.team,
                revision,
                "Changed".into(),
                102
            )
            .is_err()
    );
    assert!(
        f.store
            .queue_from_agent(
                task,
                Uuid::new_v4(),
                "call-1".into(),
                f.team,
                revision,
                "Research".into(),
                102
            )
            .is_err()
    );
    assert_eq!(f.store.runs_for_source(task).unwrap().len(), 1);
    assert!(f.store.runs_for_source(Uuid::new_v4()).unwrap().is_empty());
    // A retry is the same immutable run even if the published profile changed.
    let mut agent = run.profiles[0].clone();
    agent.model = Some("new-model".into());
    f.definitions
        .save(revision, ControlRecord::Agent(agent))
        .unwrap();
    assert_eq!(
        f.store
            .queue_from_agent(
                task,
                f.project,
                "call-1".into(),
                f.team,
                revision,
                "Research".into(),
                103
            )
            .unwrap()
            .profiles[0]
            .model,
        run.profiles[0].model
    );
}
#[test]
fn concurrent_agent_invocations_create_one_run_and_enforce_the_source_cap() {
    let f = Fixture::new();
    let task = Uuid::new_v4();
    let revision = f.definitions.load().unwrap().revision;
    let path = f.dir.join("control.db");
    let project = f.project;
    let team = f.team;
    let results = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let path = path.clone();
                scope.spawn(move || {
                    TeamCoordinator::new(path)
                        .queue_from_agent(
                            task,
                            project,
                            "same-call".into(),
                            team,
                            revision,
                            "Inspect".into(),
                            100,
                        )
                        .unwrap()
                        .id
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(results.iter().all(|id| *id == results[0]));
    for i in 1..4 {
        f.store
            .queue_from_agent(
                task,
                project,
                format!("call-{i}"),
                team,
                revision,
                "Inspect".into(),
                101,
            )
            .unwrap();
    }
    assert!(
        f.store
            .queue_from_agent(
                task,
                project,
                "call-5".into(),
                team,
                revision,
                "Inspect".into(),
                102
            )
            .is_err()
    );
    assert_eq!(f.store.runs_for_source(task).unwrap().len(), 4);
}
#[test]
fn queue_rejects_plain_or_mismatched_invocations_without_outbox_effects() {
    let f = Fixture::new();
    for text in [
        "Implement and validate".to_owned(),
        format!("@team:{} Different objective", f.team),
        format!("@team:{} Implement and validate", Uuid::new_v4()),
    ] {
        let mut request = f.request();
        request.source_prompt = Some(text);
        assert!(f.store.queue_task(request, 100).is_err());
    }
    assert!(f.store.pending_materializations(128).unwrap().is_empty());
}

#[test]
fn waiting_parents_release_capacity_and_resume_a_three_level_team_after_restart() {
    let f = Fixture::new();
    let mut request = f.request();
    request.limits.max_parallel_tasks = 1;
    request.limits.max_attempts = 1;
    let run = f.store.prepare(request, 100).unwrap();
    let root = f.claim(run.id, run.work[0].id);
    let manager = f.store.delegate(&root.lease, f.delegation(1), 102).unwrap();
    f.materialize();
    let checkpoint = r#"{"messages":[{"role":"assistant","content":"Manager will investigate"}]}"#;
    let waiting = f
        .store
        .suspend_for_children(&root.lease, &[manager.id], checkpoint, 102)
        .unwrap();
    assert_eq!(waiting.work[0].status, AssignmentStatus::Waiting);
    assert_eq!(
        f.store
            .suspend_for_children(&root.lease, &[manager.id], checkpoint, 103)
            .unwrap()
            .revision,
        waiting.revision
    );
    assert!(f.store.renew(&root.lease, 103, 100).is_err());
    assert!(
        f.store
            .save_runtime_checkpoint(&root.lease, "{}", 103)
            .is_err()
    );
    assert!(f.store.delegate(&root.lease, f.delegation(1), 103).is_err());
    assert!(
        f.store
            .report(
                &root.lease,
                AssignmentOutcome::Blocked("Old worker".into()),
                103
            )
            .is_err()
    );
    let manager = f
        .store
        .claim(run.id, manager.id, DispatchHost::Local, 103, 100)
        .unwrap();
    let leaf = f
        .store
        .delegate(&manager.lease, f.delegation(2), 104)
        .unwrap();
    f.materialize();
    f.store
        .suspend_for_children(&manager.lease, &[leaf.id], "{\"messages\":[]}", 104)
        .unwrap();
    let leaf = f
        .store
        .claim(run.id, leaf.id, DispatchHost::Local, 105, 100)
        .unwrap();
    f.store
        .report(&leaf.lease, AssignmentOutcome::Completed(result()), 106)
        .unwrap();
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    reopened.reconcile_owner_restart(107).unwrap();
    let manager = reopened
        .resume_after_children(run.id, manager.assignment.id, 108, 100)
        .unwrap();
    assert_eq!(manager.claimed.profile.provider, ProviderKind::Codex);
    assert_eq!(manager.claimed.profile.model.as_deref(), Some("model-1"));
    assert_eq!(manager.claimed.assignment.attempts.len(), 1);
    assert_eq!(manager.children[0].task_id, leaf.assignment.task_id);
    assert_eq!(manager.children[0].status, AssignmentStatus::Completed);
    assert_eq!(
        reopened
            .runtime_checkpoint(&manager.claimed.lease, 108)
            .unwrap()
            .as_deref(),
        Some("{\"messages\":[]}")
    );
    reopened
        .report(
            &manager.claimed.lease,
            AssignmentOutcome::Completed(result()),
            109,
        )
        .unwrap();
    let resumed = reopened
        .resume_after_children(run.id, root.assignment.id, 110, 100)
        .unwrap();
    assert_eq!(resumed.checkpoint, checkpoint);
    assert_eq!(resumed.claimed.assignment.attempts.len(), 1);
    assert!(
        reopened
            .report(&root.lease, AssignmentOutcome::Completed(result()), 111)
            .is_err()
    );
    assert_eq!(
        reopened
            .report(
                &resumed.claimed.lease,
                AssignmentOutcome::Completed(result()),
                111
            )
            .unwrap()
            .status,
        TeamRunStatus::Completed
    );
    assert_eq!(
        reopened
            .audit_events(run.id, 0, 100)
            .unwrap()
            .iter()
            .filter(|e| e.event == "resumed")
            .count(),
        2
    );
}

#[test]
fn continuation_scope_checkpoint_and_replay_arguments_are_validated() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let child = f.store.delegate(&root.lease, f.delegation(1), 102).unwrap();
    assert!(
        f.store
            .suspend_for_children(&root.lease, &[], "{}", 103)
            .is_err()
    );
    assert!(
        f.store
            .suspend_for_children(&root.lease, &[child.id, child.id], "{}", 103)
            .is_err()
    );
    assert!(
        f.store
            .suspend_for_children(&root.lease, &[Uuid::new_v4()], "{}", 103)
            .is_err()
    );
    assert!(
        f.store
            .suspend_for_children(&root.lease, &[root.assignment.id], "{}", 103)
            .is_err()
    );
    assert!(
        f.store
            .suspend_for_children(&root.lease, &[child.id], "not JSON", 103)
            .is_err()
    );
    assert!(
        f.store
            .save_runtime_checkpoint(&root.lease, &format!("\"{}\"", "x".repeat(128 * 1024)), 103)
            .is_err()
    );
    f.store
        .save_runtime_checkpoint(&root.lease, "{\"step\":1}", 103)
        .unwrap();
    assert_eq!(
        f.store
            .runtime_checkpoint(&root.lease, 103)
            .unwrap()
            .as_deref(),
        Some("{\"step\":1}")
    );
    f.store
        .suspend_for_children(&root.lease, &[child.id], "{\"step\":2}", 104)
        .unwrap();
    assert!(
        f.store
            .suspend_for_children(&root.lease, &[child.id], "{\"step\":3}", 104)
            .is_err()
    );
    assert!(
        f.store
            .resume_after_children(run.id, root.assignment.id, 105, 100)
            .is_err()
    );
    assert!(f.store.runtime_checkpoint(&root.lease, 105).is_err());
}

#[test]
fn racing_parent_resumes_issue_one_continuation_without_consuming_a_retry() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let child = f.store.delegate(&root.lease, f.delegation(1), 102).unwrap();
    f.materialize();
    f.store
        .suspend_for_children(&root.lease, &[child.id], "{}", 103)
        .unwrap();
    let child = f
        .store
        .claim(run.id, child.id, DispatchHost::Local, 104, 100)
        .unwrap();
    f.store
        .report(&child.lease, AssignmentOutcome::Completed(result()), 105)
        .unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let barrier = barrier.clone();
            let path = f.dir.join("control.db");
            let work = root.assignment.id;
            std::thread::spawn(move || {
                barrier.wait();
                TeamCoordinator::new(path).resume_after_children(run.id, work, 106, 100)
            })
        })
        .collect();
    let winners: Vec<_> = threads
        .into_iter()
        .filter_map(|t| t.join().unwrap().ok())
        .collect();
    assert_eq!(winners.len(), 1);
    assert_eq!(winners[0].claimed.assignment.attempts.len(), 1);
    assert!(
        f.store
            .suspend_for_children(&root.lease, &[child.assignment.id], "{}", 107)
            .is_err()
    );
}

#[test]
fn resumption_respects_shared_agent_capacity_and_preserves_the_selected_host() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let child = f.store.delegate(&root.lease, f.delegation(1), 102).unwrap();
    f.materialize();
    f.store
        .suspend_for_children(&root.lease, &[child.id], "{}", 103)
        .unwrap();
    let other = f.run();
    let other_root = f
        .store
        .claim(other.id, other.work[0].id, DispatchHost::Local, 104, 100)
        .unwrap();
    let child = f
        .store
        .claim(run.id, child.id, DispatchHost::Local, 104, 100)
        .unwrap();
    f.store
        .report(&child.lease, AssignmentOutcome::Completed(result()), 105)
        .unwrap();
    assert!(
        f.store
            .resume_after_children(run.id, root.assignment.id, 106, 100)
            .is_err()
    );
    f.store
        .report(
            &other_root.lease,
            AssignmentOutcome::Completed(result()),
            107,
        )
        .unwrap();
    let resumed = f
        .store
        .resume_after_children(run.id, root.assignment.id, 108, 100)
        .unwrap();
    let attempt = &resumed.claimed.assignment.attempts[0];
    assert_eq!(attempt.host, "local");
    assert_eq!(attempt.environment, ExecutionEnvironment::Local);
    assert_eq!(resumed.claimed.profile.model, root.profile.model);
}

#[test]
fn stop_and_deadline_prevent_a_waiting_parent_from_resuming() {
    let f = Fixture::new();
    let mut request = f.request();
    request.limits.timeout_seconds = 60;
    let run = f.store.prepare(request, 100).unwrap();
    let root = f.claim(run.id, run.work[0].id);
    let child = f.store.delegate(&root.lease, f.delegation(1), 102).unwrap();
    f.store
        .suspend_for_children(&root.lease, &[child.id], "{}", 103)
        .unwrap();
    let expired = f.store.reconcile_expired(run.id, 160).unwrap();
    assert!(
        expired
            .work
            .iter()
            .all(|w| w.status == AssignmentStatus::Blocked)
    );
    assert!(
        f.store
            .resume_after_children(run.id, root.assignment.id, 160, 100)
            .is_err()
    );
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let child = f.store.delegate(&root.lease, f.delegation(1), 102).unwrap();
    f.materialize();
    f.store
        .suspend_for_children(&root.lease, &[child.id], "{}", 103)
        .unwrap();
    let child = f
        .store
        .claim(run.id, child.id, DispatchHost::Local, 104, 100)
        .unwrap();
    let current = f.store.snapshot(run.id).unwrap();
    let stopping = f.store.stop(run.id, current.revision, 105).unwrap();
    assert_eq!(stopping.work[0].status, AssignmentStatus::Canceled);
    assert_eq!(stopping.work[1].status, AssignmentStatus::Canceling);
    assert!(
        f.store
            .resume_after_children(run.id, root.assignment.id, 106, 100)
            .is_err()
    );
    assert_eq!(
        f.store
            .report(&child.lease, AssignmentOutcome::Canceled, 107)
            .unwrap()
            .status,
        TeamRunStatus::Canceled
    );
}

#[test]
fn uncertain_children_do_not_wake_the_parent_and_handoffs_stay_bounded() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let child = f.store.delegate(&root.lease, f.delegation(1), 102).unwrap();
    f.materialize();
    f.store
        .suspend_for_children(&root.lease, &[child.id], "{}", 103)
        .unwrap();
    let child = f
        .store
        .claim(run.id, child.id, DispatchHost::Local, 104, 10)
        .unwrap();
    f.store.reconcile_expired(run.id, 114).unwrap();
    assert!(
        f.store
            .resume_after_children(run.id, root.assignment.id, 115, 100)
            .is_err()
    );
    let evidence = AssignmentResult {
        executed_checks: vec![],
        summary: "文\u{1}\"".repeat(1000),
        artifact_ids: vec![],
        checks: vec!["Confirmed evidence".into(); 64],
    };
    f.store
        .resolve_uncertain(
            run.id,
            child.assignment.id,
            child.lease.attempt_id(),
            AssignmentOutcome::Completed(evidence),
            116,
        )
        .unwrap();
    let resumed = f
        .store
        .resume_after_children(run.id, root.assignment.id, 117, 100)
        .unwrap();
    assert_eq!(resumed.children[0].omitted_checks, 62);
    assert_eq!(resumed.children[0].checks.len(), 2);
    assert!(serde_json::to_vec(&resumed.children).unwrap().len() < 2048);
}

#[test]
fn assignment_documents_commit_payload_provenance_and_exact_replay_across_restart() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let request = Uuid::new_v4();
    let artifact = f
        .store
        .publish_document(
            &root.lease,
            request,
            "report.md",
            "md",
            "Reviewed project",
            102,
        )
        .unwrap();
    assert_eq!(artifact.project_id, Some(f.project));
    assert_eq!(artifact.task_id, Some(root.assignment.task_id));
    let revision = f.definitions.load().unwrap().revision;
    let reopened = TeamCoordinator::new(f.definitions.path());
    let replay = reopened
        .publish_document(
            &root.lease,
            request,
            "report.md",
            "md",
            "Reviewed project",
            103,
        )
        .unwrap();
    assert_eq!(artifact.id, replay.id);
    assert_eq!(f.definitions.load().unwrap().revision, revision);
    assert_eq!(
        f.definitions.read_document(artifact.id).unwrap(),
        b"Reviewed project"
    );
    assert!(
        reopened
            .publish_document(
                &root.lease,
                request,
                "report.md",
                "md",
                "Changed result",
                103
            )
            .is_err()
    );
    assert_eq!(
        f.definitions.read_document(artifact.id).unwrap(),
        b"Reviewed project"
    );
    assert_eq!(
        f.store
            .audit_events(run.id, 0, 100)
            .unwrap()
            .iter()
            .filter(|e| e.event == "document")
            .count(),
        1
    );
    f.store
        .report(
            &root.lease,
            AssignmentOutcome::Completed(AssignmentResult {
                executed_checks: vec![],
                summary: "Review complete".into(),
                artifact_ids: vec![artifact.id],
                checks: vec![],
            }),
            104,
        )
        .unwrap();
}

#[test]
fn artifact_integrity_team_receipt_tampering_cannot_be_read_replayed_or_reported() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let request = Uuid::new_v4();
    let artifact = f
        .store
        .publish_document(
            &root.lease,
            request,
            "report.md",
            "md",
            "Original report",
            102,
        )
        .unwrap();
    let evidence = artifact.evidence.as_ref().unwrap();
    assert!(evidence.matches(b"Original report"));
    assert_eq!(evidence.producer.origin, ArtifactOrigin::TeamAssignment);
    assert_eq!(evidence.producer.run_id, Some(run.id));
    assert_eq!(evidence.producer.assignment_id, Some(root.assignment.id));
    assert_eq!(evidence.producer.agent_id, Some(root.assignment.agent_id));
    assert_eq!(evidence.producer.request_id, Some(request));
    let db = f.store.connect().unwrap();
    db.execute(
        "UPDATE ion_team_document_receipts SET content=? WHERE artifact_id=?",
        params![b"Changed report".as_slice(), artifact.id.to_string()],
    )
    .unwrap();
    assert!(f.definitions.read_document(artifact.id).is_err());
    assert!(
        f.store
            .read_assignment_document(&root.lease, artifact.id, 103)
            .is_err()
    );
    assert!(
        f.store
            .publish_document(
                &root.lease,
                request,
                "report.md",
                "md",
                "Original report",
                103
            )
            .is_err()
    );
    let result = AssignmentResult {
        executed_checks: vec![],
        summary: "Report".into(),
        artifact_ids: vec![artifact.id],
        checks: vec![],
    };
    assert!(
        f.store
            .report(
                &root.lease,
                AssignmentOutcome::Completed(result.clone()),
                103
            )
            .is_err()
    );
    db.execute(
        "UPDATE ion_team_document_receipts SET content=zeroblob(?) WHERE artifact_id=?",
        params![16 * 1024 * 1024 + 1, artifact.id.to_string()],
    )
    .unwrap();
    assert!(
        f.definitions
            .read_document(artifact.id)
            .unwrap_err()
            .to_string()
            .contains("bound")
    );
    db.execute(
        "UPDATE ion_team_document_receipts SET content=? WHERE artifact_id=?",
        params![b"Original report".as_slice(), artifact.id.to_string()],
    )
    .unwrap();
    assert_eq!(
        f.definitions.read_document(artifact.id).unwrap(),
        b"Original report"
    );
    assert_eq!(
        f.store
            .report(&root.lease, AssignmentOutcome::Completed(result), 104)
            .unwrap()
            .status,
        TeamRunStatus::Completed
    );
}

#[test]
fn racing_assignment_document_publications_create_one_artifact_and_receipt() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let request = Uuid::new_v4();
    let path = f.definitions.path();
    let ids = std::thread::scope(|scope| {
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let path = path.clone();
                let lease = root.lease.clone();
                scope.spawn(move || {
                    TeamCoordinator::new(path)
                        .publish_document(&lease, request, "result.json", "json", "{}", 102)
                        .unwrap()
                        .id
                })
            })
            .collect();
        threads
            .into_iter()
            .map(|t| t.join().unwrap())
            .collect::<HashSet<_>>()
    });
    assert_eq!(ids.len(), 1);
    assert_eq!(
        f.definitions
            .load()
            .unwrap()
            .records
            .iter()
            .filter(|r| matches!(r, ControlRecord::Artifact(_)))
            .count(),
        1
    );
}

#[test]
fn assignment_document_reads_and_writes_are_fenced_by_lease_and_handoff_scope() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let child = f.store.delegate(&root.lease, f.delegation(1), 102).unwrap();
    f.materialize();
    let child = f
        .store
        .claim(run.id, child.id, DispatchHost::Local, 103, 100)
        .unwrap();
    let parent_doc = f
        .store
        .publish_document(
            &root.lease,
            Uuid::new_v4(),
            "parent.md",
            "md",
            "Parent private context",
            104,
        )
        .unwrap();
    assert!(
        f.store
            .read_assignment_document(&child.lease, parent_doc.id, 105)
            .is_err()
    );
    let child_doc = f
        .store
        .publish_document(
            &child.lease,
            Uuid::new_v4(),
            "child.md",
            "md",
            "Child evidence",
            104,
        )
        .unwrap();
    assert_eq!(
        f.store
            .read_assignment_document(&root.lease, child_doc.id, 105)
            .unwrap(),
        b"Child evidence"
    );
    assert!(
        f.store
            .publish_document(&root.lease, Uuid::new_v4(), "../bad.md", "md", "bad", 105)
            .is_err()
    );
    f.store
        .suspend_for_children(&root.lease, &[child.assignment.id], "{}", 106)
        .unwrap();
    assert!(
        f.store
            .publish_document(&root.lease, Uuid::new_v4(), "stale.md", "md", "bad", 107)
            .is_err()
    );
    assert!(
        f.store
            .read_assignment_document(&root.lease, child_doc.id, 107)
            .is_err()
    );
    let other = f.run();
    let other_root = f.claim(other.id, other.work[0].id);
    assert!(
        f.store
            .read_assignment_document(&other_root.lease, child_doc.id, 107)
            .is_err()
    );
}

#[test]
fn aggregate_model_call_reservations_are_transactional_and_replay_is_rejected() {
    let f = Fixture::new();
    let mut request = f.request();
    request.limits.max_model_calls = 1;
    let run = f.store.prepare(request, 100).unwrap();
    f.materialize();
    let claimed = f.claim(run.id, run.work[0].id);
    let gate = std::sync::Arc::new(std::sync::Barrier::new(3));
    let handles: Vec<_> = (1..=2)
        .map(|step| {
            let gate = gate.clone();
            let lease = claimed.lease.clone();
            let path = f.dir.join("control.db");
            std::thread::spawn(move || {
                gate.wait();
                TeamCoordinator::new(path).reserve_model_call(
                    &lease,
                    step,
                    allowance("model-0", 100, 10),
                    102,
                )
            })
        })
        .collect();
    gate.wait();
    assert_eq!(
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .filter(|result| result.is_ok())
            .count(),
        1
    );
    assert!(
        f.store
            .reserve_model_call(&claimed.lease, 1, allowance("model-0", 100, 10), 102)
            .is_err()
    );
    assert_eq!(
        f.store
            .audit_events(run.id, 0, 128)
            .unwrap()
            .iter()
            .filter(|e| e.event == "model_call")
            .count(),
        1
    );
}
#[test]
fn dispatcher_pages_active_runs_and_setup_deferrals_do_not_churn() {
    let f = Fixture::new();
    let a = f.run();
    let b = f.run();
    let first = f.store.dispatch_runs(None, 1).unwrap();
    assert_eq!(first.len(), 1);
    let second = f.store.dispatch_runs(Some(first[0].id), 1).unwrap();
    assert_eq!(second.len(), 1);
    assert_ne!(first[0].id, second[0].id);
    assert!(
        f.store
            .dispatch_runs(Some(second[0].id), 1)
            .unwrap()
            .is_empty()
    );
    let reason = Some("Harness setup required".to_owned());
    f.store
        .defer_dispatch(a.id, a.work[0].id, reason.clone(), 101)
        .unwrap();
    let revision = f.store.snapshot(a.id).unwrap().revision;
    f.store
        .defer_dispatch(a.id, a.work[0].id, reason, 102)
        .unwrap();
    assert_eq!(f.store.snapshot(a.id).unwrap().revision, revision);
    assert!(f.store.snapshot(b.id).unwrap().work[0].attempts.is_empty());
}

#[test]
fn legacy_run_limit_default_preserves_exact_replay_without_allowing_changed_quota() {
    let f = Fixture::new();
    let mut request = f.request();
    request.origin = Some(TeamTaskOrigin {
        task_id: Uuid::new_v4(),
        request_id: "legacy-call".into(),
    });
    let run = f.store.prepare(request.clone(), 100).unwrap();
    let db = Connection::open(f.dir.join("control.db")).unwrap();
    db.execute(
        "UPDATE ion_team_runs SET request=json_remove(request,'$.limits.maxModelCalls') WHERE id=?",
        [run.id.to_string()],
    )
    .unwrap();
    db.execute("UPDATE ion_team_invocations SET signature=json_remove(signature,'$.limits.maxModelCalls') WHERE run_id=?",[run.id.to_string()]).unwrap();
    drop(db);
    assert_eq!(f.store.prepare(request.clone(), 101).unwrap().id, run.id);
    request.limits.max_model_calls = 65;
    assert!(f.store.prepare(request, 102).is_err());
    let projections = f
        .store
        .task_projections([run.root_task_id, Uuid::new_v4()])
        .unwrap();
    assert_eq!(projections.len(), 1);
    assert_eq!(
        projections[&run.root_task_id].0.assignment_id,
        run.work[0].id
    );
    assert_eq!(projections[&run.root_task_id].1, AssignmentStatus::Queued);
}

fn review_run(f: &Fixture) -> TeamRunSnapshot {
    let mut request = f.request();
    request.limits.require_child_review = true;
    f.store.prepare(request, 100).unwrap()
}
fn review_request(child: Uuid, verdict: ReviewVerdict) -> ReviewRequest {
    ReviewRequest {
        request_id: Uuid::new_v4(),
        assignment_id: child,
        verdict,
        feedback: "Reviewed against the requested output; source evidence needs correction".into(),
        artifact_ids: vec![],
        checks: vec!["Compared declared output and evidence".into()],
    }
}

#[test]
fn review_candidates_release_capacity_and_block_consumers_until_parent_acceptance() {
    let f = Fixture::new();
    let run = review_run(&f);
    let root = f.claim(run.id, run.work[0].id);
    let child = f.store.delegate(&root.lease, f.delegation(1), 102).unwrap();
    let mut consumer_request = f.delegation(1);
    consumer_request.dependencies = vec![child.id];
    let consumer = f
        .store
        .delegate(&root.lease, consumer_request, 102)
        .unwrap();
    f.materialize();
    let child_lease = f.claim(run.id, child.id);
    let candidate = f.complete(&child_lease.lease);
    let submitted = candidate.work.iter().find(|w| w.id == child.id).unwrap();
    assert_eq!(submitted.status, AssignmentStatus::InReview);
    assert!(!submitted.status.holds_capacity());
    assert!(
        f.store
            .claim(run.id, consumer.id, DispatchHost::Local, 105, 100)
            .is_err()
    );
    assert!(
        f.store
            .report(&root.lease, AssignmentOutcome::Completed(result()), 105)
            .is_err()
    );
    f.store
        .suspend_for_children(&root.lease, &[child.id], "{}", 105)
        .unwrap();
    let resumed = f
        .store
        .resume_after_children(run.id, root.assignment.id, 106, 100)
        .unwrap();
    assert_eq!(resumed.children[0].status, AssignmentStatus::InReview);
    assert!(
        f.store
            .review_child(
                &root.lease,
                review_request(child.id, ReviewVerdict::Accepted),
                107
            )
            .is_err()
    );
    let review = f
        .store
        .review_child(
            &resumed.claimed.lease,
            review_request(child.id, ReviewVerdict::Accepted),
            107,
        )
        .unwrap();
    assert_eq!(review.reviewer_task_id, root.assignment.task_id);
    assert_eq!(
        f.store
            .snapshot(run.id)
            .unwrap()
            .work
            .iter()
            .find(|w| w.id == child.id)
            .unwrap()
            .status,
        AssignmentStatus::Completed
    );
    assert!(
        f.store
            .claim(run.id, consumer.id, DispatchHost::Local, 108, 100)
            .is_ok()
    );
}

#[test]
fn review_rework_commits_one_new_task_preserves_scope_and_redirects_consumers() {
    let f = Fixture::new();
    let run = review_run(&f);
    let root = f.claim(run.id, run.work[0].id);
    let child = f.store.delegate(&root.lease, f.delegation(1), 102).unwrap();
    let mut consumer_request = f.delegation(1);
    consumer_request.dependencies = vec![child.id];
    let consumer = f
        .store
        .delegate(&root.lease, consumer_request, 102)
        .unwrap();
    f.materialize();
    let original = f.claim(run.id, child.id);
    f.complete(&original.lease);
    let request_id = Uuid::new_v4();
    let replies = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|_| {
                scope.spawn(|| {
                    let mut request = review_request(child.id, ReviewVerdict::Rework);
                    request.request_id = request_id;
                    f.store.review_child(&root.lease, request, 105).unwrap()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(
        replies[0].replacement_assignment_id,
        replies[1].replacement_assignment_id
    );
    let replacement = replies[0].replacement_assignment_id.unwrap();
    let saved = f.store.snapshot(run.id).unwrap();
    assert_eq!(saved.work.len(), 4);
    let old = saved.work.iter().find(|w| w.id == child.id).unwrap();
    assert_eq!(old.status, AssignmentStatus::Superseded);
    assert!(old.result.is_some());
    let corrected = saved.work.iter().find(|w| w.id == replacement).unwrap();
    assert_ne!(corrected.task_id, old.task_id);
    assert_eq!(corrected.agent_id, old.agent_id);
    assert_eq!(corrected.objective, old.objective);
    assert_eq!(corrected.expected_output, old.expected_output);
    assert_eq!(corrected.access, old.access);
    assert_eq!(corrected.output_contract, old.output_contract);
    assert!(corrected.attempts.is_empty());
    assert!(corrected.requires_review);
    assert_eq!(
        saved
            .work
            .iter()
            .find(|w| w.id == consumer.id)
            .unwrap()
            .dependencies,
        vec![replacement]
    );
    let mut changed = review_request(child.id, ReviewVerdict::Accepted);
    changed.request_id = request_id;
    assert!(f.store.review_child(&root.lease, changed, 106).is_err());
    f.materialize();
    let claimed = f.claim(run.id, replacement);
    assert_eq!(claimed.profile.model.as_deref(), Some("model-1"));
    assert_eq!(
        claimed.profile.provider,
        waku_protocol::model::ProviderKind::Codex
    );
    assert_eq!(
        claimed.assignment.attempts[0].environment,
        ExecutionEnvironment::Local
    );
    let audits = f.store.audit_events(run.id, 0, 100).unwrap();
    assert_eq!(
        audits
            .iter()
            .filter(|e| e.event == "rework_requested")
            .count(),
        1
    );
    let restarted = TeamCoordinator::new(f.definitions.path());
    assert_eq!(restarted.snapshot(run.id).unwrap().work.len(), 4);
}

#[test]
fn review_rework_round_limit_does_not_create_partial_replacement_or_acceptance() {
    let f = Fixture::new();
    let run = review_run(&f);
    let parent = f.claim(run.id, run.work[0].id);
    let mut child = f
        .store
        .delegate(&parent.lease, f.delegation(1), 102)
        .unwrap()
        .id;
    for _ in 0..2 {
        f.materialize();
        let lease = f.claim(run.id, child);
        f.complete(&lease.lease);
        child = f
            .store
            .review_child(
                &parent.lease,
                review_request(child, ReviewVerdict::Rework),
                105,
            )
            .unwrap()
            .replacement_assignment_id
            .unwrap();
    }
    f.materialize();
    let final_child = f.claim(run.id, child);
    f.complete(&final_child.lease);
    let before = f.store.snapshot(run.id).unwrap();
    assert!(
        f.store
            .review_child(
                &parent.lease,
                review_request(child, ReviewVerdict::Rework),
                106
            )
            .unwrap_err()
            .to_string()
            .contains("round limit")
    );
    let after = f.store.snapshot(run.id).unwrap();
    assert_eq!(before.revision, after.revision);
    assert_eq!(before.work.len(), after.work.len());
    assert_eq!(
        after.work.iter().find(|w| w.id == child).unwrap().status,
        AssignmentStatus::InReview
    );
    assert!(
        f.store
            .report(&parent.lease, AssignmentOutcome::Completed(result()), 106)
            .is_err()
    );
    f.store
        .review_child(
            &parent.lease,
            review_request(child, ReviewVerdict::Accepted),
            107,
        )
        .unwrap();
    let completed = f
        .store
        .report(&parent.lease, AssignmentOutcome::Completed(result()), 108)
        .unwrap();
    assert_eq!(completed.status, TeamRunStatus::Completed);
    assert_eq!(
        completed
            .work
            .iter()
            .filter(|w| w.status == AssignmentStatus::Superseded)
            .count(),
        2
    );
}

#[test]
fn output_contract_requires_unique_documents_format_and_declared_checks_before_submission() {
    let f = Fixture::new();
    let run = review_run(&f);
    let parent = f.claim(run.id, run.work[0].id);
    let request = f.delegation(1);
    let request_id = request.request_id;
    let contract = OutputContract {
        required_commands: vec![],
        minimum_documents: 1,
        required_formats: vec!["md".into()],
        required_checks: vec!["Source audit".into()],
    };
    let child = f
        .store
        .delegate_with_contract(&parent.lease, request, contract.clone(), 102)
        .unwrap();
    let mut changed = f.delegation(1);
    changed.request_id = request_id;
    assert!(
        f.store
            .delegate_with_contract(&parent.lease, changed, OutputContract::default(), 102)
            .is_err()
    );
    f.materialize();
    let worker = f.claim(run.id, child.id);
    let html = f
        .store
        .publish_document(
            &worker.lease,
            Uuid::new_v4(),
            "Report.html",
            "html",
            "<h1>Report</h1>",
            103,
        )
        .unwrap();
    assert!(
        f.store
            .report(
                &worker.lease,
                AssignmentOutcome::Completed(AssignmentResult {
                    executed_checks: vec![],
                    summary: "Evidence".into(),
                    artifact_ids: vec![html.id],
                    checks: vec!["Source audit".into()]
                }),
                104
            )
            .is_err()
    );
    let md = f
        .store
        .publish_document(
            &worker.lease,
            Uuid::new_v4(),
            "Report.md",
            "md",
            "# Source evidence",
            103,
        )
        .unwrap();
    // Reads authorize provenance, rather than demanding a completion contract.
    assert!(
        f.store
            .read_assignment_document(&worker.lease, md.id, 103)
            .is_ok()
    );
    for (ids, checks) in [
        (vec![md.id], vec![]),
        (vec![md.id, md.id], vec!["Source audit".into()]),
    ] {
        assert!(
            f.store
                .report(
                    &worker.lease,
                    AssignmentOutcome::Completed(AssignmentResult {
                        executed_checks: vec![],
                        summary: "Evidence".into(),
                        artifact_ids: ids,
                        checks
                    }),
                    104
                )
                .is_err()
        );
    }
    let candidate = f
        .store
        .report(
            &worker.lease,
            AssignmentOutcome::Completed(AssignmentResult {
                executed_checks: vec![],
                summary: "Evidence".into(),
                artifact_ids: vec![md.id],
                checks: vec!["Source audit".into()],
            }),
            104,
        )
        .unwrap();
    assert_eq!(
        candidate
            .work
            .iter()
            .find(|w| w.id == child.id)
            .unwrap()
            .status,
        AssignmentStatus::InReview
    );
    let review = f
        .store
        .review_child(
            &parent.lease,
            ReviewRequest {
                request_id: Uuid::new_v4(),
                assignment_id: child.id,
                verdict: ReviewVerdict::Rework,
                feedback: "Correct the cited source".into(),
                artifact_ids: vec![md.id],
                checks: vec![],
            },
            105,
        )
        .unwrap();
    f.materialize();
    let corrected = f.claim(run.id, review.replacement_assignment_id.unwrap());
    assert!(
        f.store
            .read_assignment_document(&corrected.lease, md.id, 106)
            .is_ok()
    );
    assert!(
        f.store
            .report(
                &corrected.lease,
                AssignmentOutcome::Completed(AssignmentResult {
                    executed_checks: vec![],
                    summary: "Reused rejected output".into(),
                    artifact_ids: vec![md.id],
                    checks: vec!["Source audit".into()]
                }),
                106
            )
            .is_err()
    );
    assert_eq!(corrected.assignment.output_contract, contract);
}

#[test]
fn review_rejects_self_wrong_parent_missing_evidence_stop_and_stale_outcome() {
    let f = Fixture::new();
    let run = review_run(&f);
    let parent = f.claim(run.id, run.work[0].id);
    let child = f
        .store
        .delegate(&parent.lease, f.delegation(1), 102)
        .unwrap();
    f.materialize();
    let worker = f.claim(run.id, child.id);
    assert!(
        f.store
            .review_child(
                &parent.lease,
                review_request(child.id, ReviewVerdict::Accepted),
                103
            )
            .is_err()
    );
    f.complete(&worker.lease);
    assert!(
        f.store
            .review_child(
                &worker.lease,
                review_request(child.id, ReviewVerdict::Accepted),
                105
            )
            .is_err()
    );
    assert!(
        f.store
            .review_child(
                &parent.lease,
                review_request(parent.assignment.id, ReviewVerdict::Accepted),
                105
            )
            .is_err()
    );
    let mut request = review_request(child.id, ReviewVerdict::Accepted);
    request.checks.clear();
    assert!(f.store.review_child(&parent.lease, request, 105).is_err());
    let latest = f.store.snapshot(run.id).unwrap();
    let stopped = f.store.stop(run.id, latest.revision, 106).unwrap();
    assert_eq!(
        stopped
            .work
            .iter()
            .find(|w| w.id == child.id)
            .unwrap()
            .status,
        AssignmentStatus::Canceled
    );
    assert!(
        f.store
            .review_child(
                &parent.lease,
                review_request(child.id, ReviewVerdict::Rework),
                107
            )
            .is_err()
    );
    assert_eq!(f.store.snapshot(run.id).unwrap().work.len(), 2);
}

#[test]
fn legacy_review_policy_and_contract_defaults_preserve_pinned_behavior() {
    assert!(TeamRunLimits::default().require_child_review);
    let limits: TeamRunLimits =
        serde_json::from_value(serde_json::json!({"maxModelCalls":64})).unwrap();
    assert!(!limits.require_child_review);
    assert_eq!(limits.max_rework_rounds, 2);
    let f = Fixture::new();
    let run = f.run();
    let mut body = serde_json::to_value(&run.work[0]).unwrap();
    for key in ["outputContract", "requiresReview", "reworkOf", "review"] {
        body.as_object_mut().unwrap().remove(key);
    }
    let restored: TeamAssignment = serde_json::from_value(body).unwrap();
    assert!(!restored.requires_review);
    assert_eq!(restored.output_contract, OutputContract::default());
    let parent = f.claim(run.id, run.work[0].id);
    let request = f.delegation(1);
    let id = request.request_id;
    let child = f.store.delegate(&parent.lease, request, 102).unwrap();
    let db = f.store.connect().unwrap();
    db.execute(
        "UPDATE ion_team_work SET request=json_remove(request,'$.contract') WHERE id=?",
        [child.id.to_string()],
    )
    .unwrap();
    drop(db);
    let mut replay = f.delegation(1);
    replay.request_id = id;
    assert_eq!(
        f.store.delegate(&parent.lease, replay, 103).unwrap().id,
        child.id
    );
    f.materialize();
    let worker = f.claim(run.id, child.id);
    assert_eq!(
        f.complete(&worker.lease)
            .work
            .iter()
            .find(|w| w.id == child.id)
            .unwrap()
            .status,
        AssignmentStatus::Completed
    );
}

#[test]
fn review_deadline_blocks_pending_candidates_and_waiting_parents() {
    let f = Fixture::new();
    let run = review_run(&f);
    let parent = f.claim(run.id, run.work[0].id);
    let child = f
        .store
        .delegate(&parent.lease, f.delegation(1), 102)
        .unwrap();
    f.materialize();
    let worker = f.claim(run.id, child.id);
    f.complete(&worker.lease);
    f.store
        .suspend_for_children(&parent.lease, &[child.id], "{}", 105)
        .unwrap();
    let expired = f.store.reconcile_expired(run.id, run.deadline + 1).unwrap();
    assert!(
        expired
            .work
            .iter()
            .all(|w| w.status == AssignmentStatus::Blocked)
    );
    assert!(
        f.store
            .review_child(
                &parent.lease,
                review_request(child.id, ReviewVerdict::Rework),
                run.deadline + 1
            )
            .is_err()
    );
}

#[test]
fn review_revalidates_candidate_documents_before_acceptance() {
    let f = Fixture::new();
    let run = review_run(&f);
    let parent = f.claim(run.id, run.work[0].id);
    let child = f
        .store
        .delegate_with_contract(
            &parent.lease,
            f.delegation(1),
            OutputContract {
                required_commands: vec![],
                minimum_documents: 1,
                ..Default::default()
            },
            102,
        )
        .unwrap();
    f.materialize();
    let worker = f.claim(run.id, child.id);
    let document = f
        .store
        .publish_document(
            &worker.lease,
            Uuid::new_v4(),
            "Evidence.md",
            "md",
            "# Source evidence",
            103,
        )
        .unwrap();
    f.store
        .report(
            &worker.lease,
            AssignmentOutcome::Completed(AssignmentResult {
                executed_checks: vec![],
                summary: "Evidence".into(),
                artifact_ids: vec![document.id],
                checks: vec![],
            }),
            104,
        )
        .unwrap();
    // Active publication evidence cannot be removed through the client API.
    assert!(
        f.definitions
            .remove(f.definitions.load().unwrap().revision, document.id)
            .is_err()
    );
    // Simulate storage corruption after submission, bypassing the protected
    // API only inside this disposable fixture. Review must recheck the bytes.
    f.store
        .connect()
        .unwrap()
        .execute(
            "UPDATE ion_team_document_receipts SET content=? WHERE artifact_id=?",
            params![
                b"Changed after submission".as_slice(),
                document.id.to_string()
            ],
        )
        .unwrap();
    let before = f.store.snapshot(run.id).unwrap();
    assert!(
        f.store
            .review_child(
                &parent.lease,
                review_request(child.id, ReviewVerdict::Accepted),
                105
            )
            .is_err()
    );
    let after = f.store.snapshot(run.id).unwrap();
    assert_eq!(before.revision, after.revision);
    assert_eq!(
        after.work.iter().find(|w| w.id == child.id).unwrap().status,
        AssignmentStatus::InReview
    );
    let correction = f
        .store
        .review_child(
            &parent.lease,
            review_request(child.id, ReviewVerdict::Rework),
            106,
        )
        .unwrap();
    assert!(correction.replacement_assignment_id.is_some());
}

#[test]
fn review_handoffs_fit_context_for_sixteen_superseded_children_with_escaped_evidence() {
    let f = Fixture::new();
    let run = review_run(&f);
    let parent = f.claim(run.id, run.work[0].id);
    let mut children = vec![];
    for _ in 0..16 {
        let mut request = f.delegation(1);
        request.expected_output = "\n\t\"Evidence 界\"".repeat(300);
        let child = f.store.delegate(&parent.lease, request, 102).unwrap();
        f.materialize();
        let worker = f.claim(run.id, child.id);
        f.store
            .report(
                &worker.lease,
                AssignmentOutcome::Completed(AssignmentResult {
                    executed_checks: vec![],
                    summary: "\n\t\"Source 界\"".repeat(400),
                    artifact_ids: vec![],
                    checks: vec!["\n\t\"Checked 界\"".repeat(100)],
                }),
                104,
            )
            .unwrap();
        let mut review = review_request(child.id, ReviewVerdict::Rework);
        review.feedback = "\n\t\"Correct 界\"".repeat(100);
        f.store.review_child(&parent.lease, review, 105).unwrap();
        children.push(child.id);
    }
    f.store
        .suspend_for_children(&parent.lease, &children, "{}", 106)
        .unwrap();
    let resumed = f
        .store
        .resume_after_children(run.id, parent.assignment.id, 107, 100)
        .unwrap();
    assert_eq!(resumed.children.len(), 16);
    assert!(serde_json::to_vec(&resumed.children).unwrap().len() <= 32 * 1024);
    assert!(
        resumed
            .children
            .iter()
            .all(|w| w.status == AssignmentStatus::Superseded
                && w.expected_output_truncated
                && w.review_feedback_truncated)
    );
}

fn allowance(model: &str, input: u64, output: u64) -> ModelCallAllowance {
    ModelCallAllowance {
        model: model.into(),
        max_input_tokens: input,
        max_output_tokens: output,
    }
}
#[test]
fn team_token_reservations_race_across_agents_without_overspending() {
    let f = Fixture::new();
    let mut request = f.request();
    request.limits.max_tokens = Some(110);
    let run = f.store.prepare(request, 100).unwrap();
    f.materialize();
    let root = f.claim(run.id, run.work[0].id);
    let child = f.store.delegate(&root.lease, f.delegation(1), 102).unwrap();
    f.materialize();
    let child = f.claim(run.id, child.id);
    let gate = std::sync::Arc::new(std::sync::Barrier::new(3));
    let handles: Vec<_> = [root.lease.clone(), child.lease.clone()]
        .into_iter()
        .enumerate()
        .map(|(i, lease)| {
            let gate = gate.clone();
            let path = f.dir.join("control.db");
            std::thread::spawn(move || {
                gate.wait();
                TeamCoordinator::new(path).reserve_model_call(
                    &lease,
                    1,
                    allowance(&format!("model-{i}"), 100, 10),
                    102,
                )
            })
        })
        .collect();
    gate.wait();
    assert_eq!(
        handles
            .into_iter()
            .filter_map(|h| h.join().unwrap().ok())
            .count(),
        1
    );
    let usage = f.store.snapshot(run.id).unwrap().usage;
    assert_eq!(usage.reserved_tokens, 110);
    assert_eq!(usage.model_calls, 1);
    assert_eq!(usage.unknown_calls, 1);
    assert_eq!(usage.consumed_tokens, 0);
}
#[test]
fn team_usage_receipts_are_atomic_exact_and_release_only_their_reservation() {
    let f = Fixture::new();
    let mut request = f.request();
    request.limits.max_tokens = Some(220);
    let run = f.store.prepare(request, 100).unwrap();
    f.materialize();
    let root = f.claim(run.id, run.work[0].id);
    let call = f
        .store
        .reserve_model_call(&root.lease, 1, allowance("model-0", 100, 10), 102)
        .unwrap();
    let other = f
        .store
        .reserve_model_call(&root.lease, 2, allowance("model-0", 100, 10), 102)
        .unwrap();
    let gate = std::sync::Arc::new(std::sync::Barrier::new(3));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let gate = gate.clone();
            let call = call.clone();
            let path = f.dir.join("control.db");
            std::thread::spawn(move || {
                gate.wait();
                TeamCoordinator::new(path).record_model_usage(
                    &call,
                    ModelTokenUsage {
                        input_tokens: 10,
                        output_tokens: 5,
                    },
                    103,
                )
            })
        })
        .collect();
    gate.wait();
    for h in handles {
        h.join().unwrap().unwrap();
    }
    let run = f.store.snapshot(run.id).unwrap();
    assert_eq!(run.usage.consumed_tokens, 15);
    assert_eq!(run.usage.reserved_tokens, 110);
    assert_eq!(run.usage.unknown_calls, 1);
    assert_eq!(run.usage.unmetered_calls, 0);
    let revision = run.revision;
    f.store
        .record_model_usage(
            &call,
            ModelTokenUsage {
                input_tokens: 10,
                output_tokens: 5,
            },
            104,
        )
        .unwrap();
    assert_eq!(f.store.snapshot(run.id).unwrap().revision, revision);
    assert!(
        f.store
            .record_model_usage(
                &call,
                ModelTokenUsage {
                    input_tokens: 1,
                    output_tokens: 0
                },
                104
            )
            .is_err()
    );
    assert!(
        f.store
            .reserve_model_call(&root.lease, 3, allowance("model-0", 100, 10), 104)
            .is_err()
    );
    f.store
        .record_model_usage(
            &other,
            ModelTokenUsage {
                input_tokens: 10,
                output_tokens: 5,
            },
            104,
        )
        .unwrap();
    f.store
        .reserve_model_call(&root.lease, 3, allowance("model-0", 100, 10), 104)
        .unwrap();
    assert_eq!(
        f.store
            .audit_events(run.id, 0, 128)
            .unwrap()
            .iter()
            .filter(|e| e.event == "model_usage")
            .count(),
        2
    );
}
#[test]
fn team_unknown_call_allowances_survive_yield_resume_and_owner_restart() {
    let f = Fixture::new();
    let mut request = f.request();
    request.limits.max_tokens = Some(110);
    let run = f.store.prepare(request, 100).unwrap();
    f.materialize();
    let root = f.claim(run.id, run.work[0].id);
    f.store
        .reserve_model_call(&root.lease, 1, allowance("model-0", 100, 10), 102)
        .unwrap();
    let child = f.store.delegate(&root.lease, f.delegation(1), 103).unwrap();
    f.materialize();
    f.store
        .suspend_for_children(&root.lease, &[child.id], "{}", 103)
        .unwrap();
    let child = f.claim(run.id, child.id);
    f.complete(&child.lease);
    let resumed = f
        .store
        .resume_after_children(run.id, root.assignment.id, 105, 100)
        .unwrap();
    assert!(
        f.store
            .reserve_model_call(&root.lease, 2, allowance("model-0", 1, 1), 105)
            .is_err()
    );
    assert!(
        f.store
            .reserve_model_call(&resumed.claimed.lease, 2, allowance("model-0", 1, 1), 105)
            .is_err()
    );
    f.store.reconcile_owner_restart(106).unwrap();
    let restored = TeamCoordinator::new(f.dir.join("control.db"))
        .snapshot(run.id)
        .unwrap();
    assert_eq!(restored.usage.reserved_tokens, 110);
    assert_eq!(restored.usage.unknown_calls, 1);
    assert_eq!(
        restored
            .work
            .iter()
            .find(|w| w.id == root.assignment.id)
            .unwrap()
            .status,
        AssignmentStatus::Uncertain
    );
}
#[test]
fn team_stopped_call_can_account_usage_without_authorizing_more_execution() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    assert!(
        f.store
            .reserve_model_call(&root.lease, 1, allowance("different-model", 10, 10), 102)
            .is_err()
    );
    let call = f
        .store
        .reserve_model_call(&root.lease, 1, allowance("model-0", 100, 10), 102)
        .unwrap();
    let before = f.store.snapshot(run.id).unwrap();
    f.store.stop(run.id, before.revision, 103).unwrap();
    f.store
        .record_model_usage(
            &call,
            ModelTokenUsage {
                input_tokens: 25,
                output_tokens: 4,
            },
            104,
        )
        .unwrap();
    assert!(
        f.store
            .reserve_model_call(&root.lease, 2, allowance("model-0", 100, 10), 104)
            .is_err()
    );
    let after = f.store.snapshot(run.id).unwrap();
    assert_eq!(after.status, TeamRunStatus::Stopping);
    assert_eq!(after.work[0].status, AssignmentStatus::Canceling);
    assert_eq!(after.usage.reserved_tokens, 0);
    assert_eq!(after.usage.consumed_tokens, 29);
    assert!(after.work[0].status.holds_capacity());
}
#[test]
fn team_provider_overage_is_retained_and_blocks_all_further_calls() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let call = f
        .store
        .reserve_model_call(&root.lease, 1, allowance("model-0", 100, 10), 102)
        .unwrap();
    let receipt = ModelTokenUsage {
        input_tokens: 33_000_000,
        output_tokens: 12,
    };
    assert!(f.store.record_model_usage(&call, receipt, 103).is_err());
    assert!(f.store.record_model_usage(&call, receipt, 104).is_err());
    let after = f.store.snapshot(run.id).unwrap();
    assert_eq!(after.usage.consumed_tokens, 33_000_012);
    assert_eq!(after.usage.allowance_violations, 1);
    assert_eq!(after.usage.reserved_tokens, 0);
    assert!(
        f.store
            .reserve_model_call(&root.lease, 2, allowance("model-0", 100, 10), 104)
            .is_err()
    );
}
#[test]
fn team_legacy_budget_replay_does_not_enable_new_budget_or_hide_unmetered_calls() {
    let f = Fixture::new();
    let mut request = f.request();
    request.limits.max_tokens = None;
    let run = f.store.prepare(request.clone(), 100).unwrap();
    f.materialize();
    let root = f.claim(run.id, run.work[0].id);
    let db = Connection::open(f.dir.join("control.db")).unwrap();
    db.execute("UPDATE ion_team_runs SET request=json_remove(request,'$.limits.maxTokens'),body=json_remove(body,'$.limits.maxTokens','$.usage') WHERE id=?",[run.id.to_string()]).unwrap();
    db.execute(
        "INSERT INTO ion_team_model_calls VALUES(?,?,?,?,?,?,?,?)",
        params![
            Uuid::new_v4().to_string(),
            run.id.to_string(),
            root.assignment.id.to_string(),
            root.lease.attempt_id.to_string(),
            0,
            1,
            "model-0",
            102
        ],
    )
    .unwrap();
    drop(db);
    assert_eq!(f.store.prepare(request.clone(), 103).unwrap().id, run.id);
    let after = f.store.snapshot(run.id).unwrap();
    assert_eq!(after.limits.max_tokens, None);
    assert_eq!(after.usage.unmetered_calls, 1);
    assert_eq!(after.usage.unknown_calls, 1);
    request.limits.max_tokens = Some(2_000_000);
    assert!(f.store.prepare(request, 103).is_err());
    f.store
        .reserve_model_call(&root.lease, 2, allowance("model-0", 100, 10), 103)
        .unwrap();
    let after = f.store.snapshot(run.id).unwrap();
    assert_eq!(after.usage.model_calls, 2);
    assert_eq!(after.usage.reserved_tokens, 110);
}
#[test]
fn team_token_limit_validation_rejects_zero_and_unbounded_projection_values() {
    let f = Fixture::new();
    for tokens in [0, 1_000_000_001, u64::MAX] {
        let mut request = f.request();
        request.limits.max_tokens = Some(tokens);
        assert!(f.store.prepare(request, 100).is_err());
    }
}

#[test]
fn team_default_user_queue_replay_retains_old_limits_but_rejects_changed_work() {
    let f = Fixture::new();
    let mut old = f.request();
    old.limits.max_tokens = None;
    old.source_prompt = Some(format!("@team:{} {}", f.team, old.objective));
    let run = f.store.queue_task(old.clone(), 100).unwrap();
    let mut replay = old.clone();
    replay.limits = Default::default();
    let saved = f.store.queue_default_task(replay.clone(), 101).unwrap();
    assert_eq!(saved.id, run.id);
    assert_eq!(saved.limits.max_tokens, None);
    assert!(!saved.limits.require_child_review);
    replay.objective = "Different task".into();
    replay.source_prompt = Some(format!("@team:{} Different task", f.team));
    assert!(f.store.queue_default_task(replay, 102).is_err());
    let mut next = old;
    next.id = Uuid::new_v4();
    next.root_task_id = Uuid::new_v4();
    let new = f.store.queue_default_task(next, 103).unwrap();
    assert_ne!(new.id, run.id);
    assert_eq!(new.limits.max_tokens, Some(2_000_000));
    assert!(new.limits.require_child_review);
}
#[test]
fn team_default_agent_queue_replay_keeps_legacy_budget_and_review_policy() {
    let f = Fixture::new();
    let source = Uuid::new_v4();
    let revision = f.definitions.load().unwrap().revision;
    let run = f
        .store
        .queue_from_agent(
            source,
            f.project,
            "provider-call".into(),
            f.team,
            revision,
            "Inspect".into(),
            100,
        )
        .unwrap();
    let db = Connection::open(f.dir.join("control.db")).unwrap();
    db.execute("UPDATE ion_team_runs SET request=json_remove(request,'$.limits.maxTokens','$.limits.requireChildReview'),body=json_remove(body,'$.limits.maxTokens','$.limits.requireChildReview') WHERE id=?",[run.id.to_string()]).unwrap();
    db.execute("UPDATE ion_team_invocations SET signature=json_remove(signature,'$.limits.maxTokens','$.limits.requireChildReview') WHERE run_id=?",[run.id.to_string()]).unwrap();
    drop(db);
    let saved = f
        .store
        .queue_from_agent(
            source,
            f.project,
            "provider-call".into(),
            f.team,
            revision,
            "Inspect".into(),
            101,
        )
        .unwrap();
    assert_eq!(saved.id, run.id);
    assert_eq!(saved.root_task_id, run.root_task_id);
    assert_eq!(saved.limits.max_tokens, None);
    assert!(!saved.limits.require_child_review);
    assert!(
        f.store
            .queue_from_agent(
                source,
                f.project,
                "provider-call".into(),
                f.team,
                revision,
                "Changed".into(),
                102
            )
            .is_err()
    );
    assert_eq!(f.store.runs_for_source(source).unwrap().len(), 1);
}

#[test]
fn team_collection_is_bounded_filtered_and_preserves_pinned_history() {
    let f = Fixture::new();
    let mut ids = Vec::new();
    let other_project = Uuid::new_v4();
    for n in 0..35 {
        let mut request = f.request();
        request.project_id = if n % 2 == 0 { f.project } else { other_project };
        let run = f.store.prepare(request, 100 + n).unwrap();
        ids.push(run.id);
    }
    ids.reverse();
    assert!(f.store.list_runs(None, None, None, 0).is_err());
    assert!(f.store.list_runs(None, None, None, 65).is_err());
    let first = f.store.list_runs(None, None, None, 32).unwrap();
    assert_eq!(
        first.runs.iter().map(|r| r.id).collect::<Vec<_>>(),
        ids[..32]
    );
    assert_eq!(first.next_cursor, Some(ids[31]));
    let next = f
        .store
        .list_runs(None, None, first.next_cursor, 32)
        .unwrap();
    assert_eq!(
        next.runs.iter().map(|r| r.id).collect::<Vec<_>>(),
        ids[32..]
    );
    assert!(next.next_cursor.is_none());
    let filtered = f
        .store
        .list_runs(Some(f.project), Some(f.team), None, 64)
        .unwrap();
    assert_eq!(filtered.runs.len(), 18);
    assert!(
        filtered
            .runs
            .iter()
            .all(|r| r.project_id == f.project && r.assignments == 1 && r.running == 0)
    );
    assert!(
        f.store
            .list_runs(None, Some(Uuid::new_v4()), None, 64)
            .unwrap()
            .runs
            .is_empty()
    );
    f.definitions
        .remove(f.definitions.load().unwrap().revision, f.team)
        .unwrap();
    assert_eq!(
        f.store
            .list_runs(None, Some(f.team), None, 64)
            .unwrap()
            .runs
            .len(),
        35
    );
}
#[test]
fn team_collection_reports_review_blockers_and_metered_usage() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let reservation = f
        .store
        .reserve_model_call(
            &root.lease,
            1,
            ModelCallAllowance {
                model: root.profile.model.clone().unwrap(),
                max_input_tokens: 100,
                max_output_tokens: 20,
            },
            102,
        )
        .unwrap();
    f.store
        .record_model_usage(
            &reservation,
            ModelTokenUsage {
                input_tokens: 40,
                output_tokens: 5,
            },
            103,
        )
        .unwrap();
    let child = f.store.delegate(&root.lease, f.delegation(1), 104).unwrap();
    f.store
        .defer_dispatch(run.id, child.id, Some("Harness needs setup".into()), 105)
        .unwrap();
    let page = f.store.list_runs(None, None, None, 32).unwrap();
    let summary = &page.runs[0];
    assert_eq!(summary.assignments, 2);
    assert_eq!(summary.running, 1);
    assert_eq!(summary.needs_attention, 1);
    assert_eq!(summary.usage.consumed_tokens, 45);
    assert_eq!(summary.updated_at, 105);
    assert_eq!(summary.revision, f.store.snapshot(run.id).unwrap().revision);
}

#[test]
fn team_saved_limits_apply_to_user_and_agent_invocation_but_never_rewrite_runs() {
    let f = Fixture::new();
    let mut definition = f
        .definitions
        .load()
        .unwrap()
        .records
        .into_iter()
        .find_map(|r| match r {
            ControlRecord::Team(t) => Some(t),
            _ => None,
        })
        .unwrap();
    definition.limits = Some(TeamRunLimits {
        max_tokens: Some(700),
        max_model_calls: 2,
        max_parallel_tasks: 1,
        ..Default::default()
    });
    f.definitions
        .save(
            f.definitions.load().unwrap().revision,
            ControlRecord::Team(definition.clone()),
        )
        .unwrap();
    let mut request = f.request();
    request.source_prompt = Some(format!("@team:{} {}", f.team, request.objective));
    let run = f.store.queue_default_task(request.clone(), 100).unwrap();
    assert_eq!(run.limits.max_tokens, Some(700));
    assert_eq!(run.limits.max_model_calls, 2);
    assert_eq!(run.limits.max_parallel_tasks, 1);
    let from_agent = f
        .store
        .queue_from_agent(
            Uuid::new_v4(),
            f.project,
            "tool-1".into(),
            f.team,
            f.definitions.load().unwrap().revision,
            "Review change".into(),
            101,
        )
        .unwrap();
    assert_eq!(from_agent.limits.max_tokens, Some(700));
    definition.limits.as_mut().unwrap().max_tokens = Some(900);
    f.definitions
        .save(
            f.definitions.load().unwrap().revision,
            ControlRecord::Team(definition),
        )
        .unwrap();
    assert_eq!(
        f.store
            .queue_default_task(request, 102)
            .unwrap()
            .limits
            .max_tokens,
        Some(700)
    );
    assert_eq!(
        f.store.snapshot(run.id).unwrap().limits.max_tokens,
        Some(700)
    );
    let mut fresh = f.request();
    fresh.source_prompt = Some(format!("@team:{} {}", f.team, fresh.objective));
    assert_eq!(
        f.store
            .queue_default_task(fresh, 103)
            .unwrap()
            .limits
            .max_tokens,
        Some(900)
    );
}
#[test]
fn team_saved_limit_validation_rejects_invalid_definitions_atomically() {
    let f = Fixture::new();
    let before = f.definitions.load().unwrap();
    let mut definition = before
        .records
        .iter()
        .find_map(|r| match r {
            ControlRecord::Team(t) => Some(t.clone()),
            _ => None,
        })
        .unwrap();
    definition.limits = Some(TeamRunLimits {
        max_tokens: Some(0),
        ..Default::default()
    });
    assert!(
        f.definitions
            .save(before.revision, ControlRecord::Team(definition))
            .is_err()
    );
    assert_eq!(f.definitions.load().unwrap().revision, before.revision);
    for limits in [
        TeamRunLimits {
            max_parallel_tasks: 0,
            ..Default::default()
        },
        TeamRunLimits {
            max_model_calls: 0,
            ..Default::default()
        },
        TeamRunLimits {
            timeout_seconds: 59,
            ..Default::default()
        },
        TeamRunLimits {
            max_attempts: 9,
            ..Default::default()
        },
    ] {
        assert!(validate_limits(&limits).is_err());
    }
}

#[test]
fn team_decision_fences_worker_releases_capacity_and_resumes_once_after_restart() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let request = Uuid::new_v4();
    let decision = f
        .store
        .suspend_for_decision(
            &root.lease,
            request,
            "Choose the release scope",
            "Include the documented migration?",
            "{\"private\":true}",
            102,
        )
        .unwrap();
    assert_eq!(decision.task_id, root.assignment.task_id);
    assert_eq!(decision.host, "local");
    assert_eq!(decision.definition_revision, run.definition_revision);
    assert!(f.store.renew(&root.lease, 103, 100).is_err());
    assert!(
        f.store
            .resume_after_children(run.id, root.assignment.id, 103, 100)
            .is_err()
    );
    let other = f.run();
    let active_other = f.claim(other.id, other.work[0].id);
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    let approved = reopened
        .resolve_decision(decision.id, decision.revision, true, 103)
        .unwrap();
    assert_eq!(approved.status, TeamDecisionStatus::Approved);
    assert_eq!(
        reopened
            .resolve_decision(decision.id, 1, true, 103)
            .unwrap()
            .revision,
        2
    );
    assert!(
        reopened
            .resolve_decision(decision.id, 1, false, 103)
            .is_err()
    );
    assert!(
        reopened
            .resume_after_children(run.id, root.assignment.id, 104, 100)
            .is_err()
    );
    f.complete(&active_other.lease);
    let resumed = reopened
        .resume_after_children(run.id, root.assignment.id, 105, 100)
        .unwrap();
    assert_eq!(resumed.claimed.lease.attempt_id(), root.lease.attempt_id());
    assert_eq!(resumed.claimed.assignment.access, root.assignment.access);
    assert_eq!(resumed.checkpoint, "{\"private\":true}");
    assert!(resumed.children.is_empty());
    assert_eq!(resumed.decision.unwrap().proposal, decision.proposal);
    assert!(
        reopened
            .resume_after_children(run.id, root.assignment.id, 106, 100)
            .is_err()
    );
    f.complete(&resumed.claimed.lease);
    assert_eq!(
        reopened
            .resolve_decision(decision.id, 1, true, 5000)
            .unwrap()
            .status,
        TeamDecisionStatus::Approved
    );
    assert!(
        !serde_json::to_string(&f.store.snapshot(run.id).unwrap())
            .unwrap()
            .contains("private")
    );
}
#[test]
fn team_decision_replay_preserves_proposal_and_decline_is_a_durable_receipt() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let request = Uuid::new_v4();
    let decision = f
        .store
        .suspend_for_decision(&root.lease, request, "Scope", "Leave CI paused", "{}", 102)
        .unwrap();
    assert_eq!(
        f.store
            .suspend_for_decision(&root.lease, request, "Scope", "Leave CI paused", "{}", 103)
            .unwrap()
            .id,
        decision.id
    );
    assert!(
        f.store
            .suspend_for_decision(&root.lease, request, "Scope", "Deploy", "{}", 103)
            .is_err()
    );
    assert!(
        f.store
            .suspend_for_decision(&root.lease, request, "Scope", "Leave CI paused", "[]", 103)
            .is_err()
    );
    assert!(f.store.resolve_decision(decision.id, 0, true, 103).is_err());
    let declined = f
        .store
        .resolve_decision(decision.id, 1, false, 103)
        .unwrap();
    assert_eq!(declined.status, TeamDecisionStatus::Declined);
    let resumed = f
        .store
        .resume_after_children(run.id, root.assignment.id, 104, 100)
        .unwrap();
    assert_eq!(
        resumed.decision.unwrap().status,
        TeamDecisionStatus::Declined
    );
    assert_eq!(
        serde_json::to_value(&resumed.claimed.profile).unwrap(),
        serde_json::to_value(&root.profile).unwrap()
    );
    assert!(
        f.store
            .suspend_for_decision(&root.lease, request, "Scope", "Leave CI paused", "{}", 105)
            .is_err()
    );
    let fresh = f
        .store
        .suspend_for_decision(
            &resumed.claimed.lease,
            Uuid::new_v4(),
            "Revised scope",
            "Produce a review document",
            "{}",
            105,
        )
        .unwrap();
    assert_ne!(fresh.id, decision.id);
    assert_eq!(f.store.snapshot(run.id).unwrap().decisions.len(), 2);
}
#[test]
fn team_decision_stop_and_deadline_cancel_pending_without_resuming() {
    for expired in [false, true] {
        let f = Fixture::new();
        let run = f.run();
        let root = f.claim(run.id, run.work[0].id);
        let decision = f
            .store
            .suspend_for_decision(
                &root.lease,
                Uuid::new_v4(),
                "Scope",
                "Wait for release",
                "{}",
                102,
            )
            .unwrap();
        let now = if expired { run.deadline } else { 103 };
        if expired {
            assert!(f.store.resolve_decision(decision.id, 1, true, now).is_err());
            f.store.reconcile_expired(run.id, now).unwrap();
        } else {
            f.store
                .stop(run.id, f.store.snapshot(run.id).unwrap().revision, now)
                .unwrap();
        }
        assert_eq!(
            f.store.snapshot(run.id).unwrap().decisions[0].status,
            TeamDecisionStatus::Canceled
        );
        assert_eq!(
            f.store
                .list_decisions(None, true, None, 64)
                .unwrap()
                .pending_count,
            0
        );
        assert!(f.store.resolve_decision(decision.id, 1, true, now).is_err());
        assert!(
            f.store
                .resume_after_children(run.id, root.assignment.id, now, 100)
                .is_err()
        );
    }
}
#[test]
fn team_decision_racing_verdicts_have_one_winner() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let decision = f
        .store
        .suspend_for_decision(
            &root.lease,
            Uuid::new_v4(),
            "Scope",
            "Approve plan",
            "{}",
            102,
        )
        .unwrap();
    let gate = std::sync::Arc::new(std::sync::Barrier::new(3));
    let handles: Vec<_> = [true, false]
        .into_iter()
        .map(|approve| {
            let store = TeamCoordinator::new(f.dir.join("control.db"));
            let gate = gate.clone();
            std::thread::spawn(move || {
                gate.wait();
                store.resolve_decision(decision.id, 1, approve, 103).is_ok()
            })
        })
        .collect();
    gate.wait();
    assert_eq!(
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .filter(|ok| *ok)
            .count(),
        1
    );
    assert_eq!(f.store.snapshot(run.id).unwrap().decisions[0].revision, 2);
}
#[test]
fn team_decision_collection_is_bounded_filtered_and_immutable_cursor_paged() {
    let f = Fixture::new();
    let mut ids = std::collections::HashSet::new();
    for i in 0..3 {
        let mut request = f.request();
        request.project_id = if i == 2 { Uuid::new_v4() } else { f.project };
        let run = f.store.prepare(request, 100).unwrap();
        let root = f.claim(run.id, run.work[0].id);
        let d = f
            .store
            .suspend_for_decision(
                &root.lease,
                Uuid::new_v4(),
                "Scope",
                "Concrete proposal",
                "{}",
                102 + i,
            )
            .unwrap();
        ids.insert(d.id);
        if i == 0 {
            f.store.resolve_decision(d.id, 1, false, 103).unwrap();
        }
    }
    let page = f.store.list_decisions(None, false, None, 2).unwrap();
    assert_eq!(page.pending_count, 2);
    assert_eq!(page.decisions.len(), 2);
    let older = f
        .store
        .list_decisions(None, false, page.next_cursor, 2)
        .unwrap();
    assert_eq!(older.decisions.len(), 1);
    assert!(older.next_cursor.is_none());
    let seen: std::collections::HashSet<_> = page
        .decisions
        .iter()
        .chain(&older.decisions)
        .map(|d| d.id)
        .collect();
    assert_eq!(seen, ids);
    let filtered = f
        .store
        .list_decisions(Some(f.project), true, None, 64)
        .unwrap();
    assert_eq!(filtered.pending_count, 1);
    assert_eq!(filtered.decisions.len(), 1);
    assert!(
        filtered
            .decisions
            .iter()
            .all(|d| d.project_id == f.project && d.status == TeamDecisionStatus::Pending)
    );
    assert!(f.store.list_decisions(None, false, None, 65).is_err());
    assert!(
        f.store
            .list_decisions(None, false, Some(Uuid::new_v4()), 1)
            .is_err()
    );
}

fn app_fixture() -> (Fixture, AppActionGrant, TeamRunSnapshot, ClaimedAssignment) {
    let f = Fixture::new();
    let grant = AppActionGrant {
        account_id: "ca_github_1".into(),
        action: "GITHUB_CREATE_ISSUE".into(),
        version: "20250905_00".into(),
    };
    let catalog = f.definitions.load().unwrap();
    let mut profile = catalog
        .records
        .iter()
        .find_map(|r| match r {
            ControlRecord::Agent(a) if a.id == f.agents[0] => Some(a.clone()),
            _ => None,
        })
        .unwrap();
    profile.access.app_actions = vec![grant.clone()];
    f.definitions
        .save(catalog.revision, ControlRecord::Agent(profile))
        .unwrap();
    let mut request = f.request();
    request.access.app_actions = vec![grant.clone()];
    let run = f.store.prepare(request, 100).unwrap();
    let claimed = f.claim(run.id, run.work[0].id);
    (f, grant, run, claimed)
}
#[test]
fn team_app_approval_pins_arguments_version_host_and_preserves_exact_receipts() {
    use serde_json::json;
    let (f, grant, run, root) = app_fixture();
    let args = json!({"repo":"ion","title":"Review this exact issue"});
    let request = Uuid::new_v4();
    let decision = f
        .store
        .suspend_for_app_action(&root.lease, request, grant.clone(), args.clone(), "{}", 102)
        .unwrap();
    assert!(
        decision.proposal.contains("ca_github_1")
            && decision.proposal.contains("20250905_00")
            && decision.proposal.contains("Review this exact issue")
    );
    assert!(
        f.store
            .begin_app_action(&root.lease, decision.id, 103)
            .is_err()
    );
    assert_eq!(
        f.store
            .suspend_for_app_action(&root.lease, request, grant.clone(), args.clone(), "{}", 103)
            .unwrap()
            .id,
        decision.id
    );
    assert!(
        f.store
            .suspend_for_app_action(
                &root.lease,
                request,
                grant.clone(),
                json!({"title":"changed"}),
                "{}",
                103
            )
            .is_err()
    );
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    reopened
        .resolve_decision(decision.id, 1, true, 103)
        .unwrap();
    let resumed = reopened
        .resume_after_children(run.id, root.assignment.id, 104, 100)
        .unwrap();
    let action = reopened
        .approved_app_action(&resumed.claimed.lease, decision.id, 104)
        .unwrap();
    assert_eq!(action.grant, grant);
    assert_eq!(action.arguments_json, args.to_string());
    let ticket = match reopened
        .begin_app_action(&resumed.claimed.lease, decision.id, 104)
        .unwrap()
    {
        AppActionStart::Execute(t) => t,
        _ => panic!("Not executed"),
    };
    assert!(reopened.has_pending_effect(run.id).unwrap());
    assert!(
        reopened
            .begin_app_action(&resumed.claimed.lease, decision.id, 105)
            .is_err()
    );
    let receipt = json!({"successful":true,"data":{"id":123}}).to_string();
    assert!(reopened.finish_app_action(&ticket, "{}", 105).is_err());
    reopened.finish_app_action(&ticket, &receipt, 105).unwrap();
    assert!(!reopened.has_pending_effect(run.id).unwrap());
    reopened.finish_app_action(&ticket, &receipt, 105).unwrap();
    assert!(
        reopened
            .finish_app_action(&ticket, &json!({"successful":false}).to_string(), 106)
            .is_err()
    );
    assert!(
        matches!(reopened.begin_app_action(&resumed.claimed.lease,decision.id,106).unwrap(), AppActionStart::Receipt(r) if r == receipt)
    );
    f.complete(&resumed.claimed.lease);
}
#[test]
fn team_app_grants_reject_expansion_and_decline_never_executes() {
    use serde_json::json;
    let (f, grant, run, root) = app_fixture();
    for denied in [
        AppActionGrant {
            account_id: "ca_other".into(),
            ..grant.clone()
        },
        AppActionGrant {
            action: "GITHUB_DELETE_REPOSITORY".into(),
            ..grant.clone()
        },
        AppActionGrant {
            version: "20250906_00".into(),
            ..grant.clone()
        },
    ] {
        assert!(
            f.store
                .suspend_for_app_action(&root.lease, Uuid::new_v4(), denied, json!({}), "{}", 102)
                .is_err()
        );
    }
    assert!(
        f.store
            .suspend_for_app_action(
                &root.lease,
                Uuid::new_v4(),
                grant.clone(),
                json!([]),
                "{}",
                102
            )
            .is_err()
    );
    assert!(
        f.store
            .suspend_for_app_action(
                &root.lease,
                Uuid::new_v4(),
                grant.clone(),
                json!({"body":"x".repeat(4097)}),
                "{}",
                102
            )
            .is_err()
    );
    let d = f
        .store
        .suspend_for_app_action(&root.lease, Uuid::new_v4(), grant, json!({}), "{}", 102)
        .unwrap();
    f.store.resolve_decision(d.id, 1, false, 103).unwrap();
    let resumed = f
        .store
        .resume_after_children(run.id, root.assignment.id, 104, 100)
        .unwrap();
    assert!(
        f.store
            .approved_app_action(&resumed.claimed.lease, d.id, 104)
            .is_err()
    );
    assert!(
        f.store
            .begin_app_action(&resumed.claimed.lease, d.id, 104)
            .is_err()
    );
    assert!(!f.store.has_pending_effect(run.id).unwrap());
}
#[test]
fn team_app_unknown_effect_survives_restart_and_blocks_spending_yield_and_completion() {
    use serde_json::json;
    let (f, grant, run, root) = app_fixture();
    let d = f
        .store
        .suspend_for_app_action(&root.lease, Uuid::new_v4(), grant, json!({}), "{}", 102)
        .unwrap();
    f.store.resolve_decision(d.id, 1, true, 103).unwrap();
    let resumed = f
        .store
        .resume_after_children(run.id, root.assignment.id, 104, 100)
        .unwrap();
    let ticket = match f
        .store
        .begin_app_action(&resumed.claimed.lease, d.id, 104)
        .unwrap()
    {
        AppActionStart::Execute(t) => t,
        _ => panic!(),
    };
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    assert!(reopened.has_pending_effect(run.id).unwrap());
    assert!(
        reopened
            .begin_app_action(&resumed.claimed.lease, d.id, 105)
            .is_err()
    );
    assert!(
        reopened
            .reserve_model_call(
                &resumed.claimed.lease,
                1,
                ModelCallAllowance {
                    model: "model-0".into(),
                    max_input_tokens: 100,
                    max_output_tokens: 100
                },
                105
            )
            .is_err()
    );
    assert!(
        reopened
            .suspend_for_decision(
                &resumed.claimed.lease,
                Uuid::new_v4(),
                "Retry",
                "Retry the action",
                "{}",
                105
            )
            .is_err()
    );
    assert!(
        reopened
            .report(
                &resumed.claimed.lease,
                AssignmentOutcome::Completed(result()),
                105
            )
            .is_err()
    );
    reopened
        .stop(run.id, reopened.snapshot(run.id).unwrap().revision, 105)
        .unwrap();
    let receipt = json!({"successful":true,"data":{}}).to_string();
    reopened.finish_app_action(&ticket, &receipt, 106).unwrap();
    assert!(!reopened.has_pending_effect(run.id).unwrap());
    assert!(
        reopened
            .begin_app_action(&resumed.claimed.lease, d.id, 106)
            .is_err()
    );
}

#[test]
fn team_source_authorization_allows_only_supplied_grants_and_reads_accepted_result_documents() {
    let f = Fixture::new();
    let source = Uuid::new_v4();
    let authorization = AgentAccess {
        workspace_write: true,
        ..Default::default()
    };
    let run = f
        .store
        .queue_from_agent_with_access(
            source,
            f.project,
            "source-call".into(),
            f.team,
            f.definitions.load().unwrap().revision,
            "Produce code evidence".into(),
            authorization.clone(),
            100,
        )
        .unwrap();
    assert_eq!(run.access, authorization);
    assert_eq!(run.work[0].access, authorization);
    f.materialize();
    let root = f.claim(run.id, run.work[0].id);
    let doc = f
        .store
        .publish_document(
            &root.lease,
            Uuid::new_v4(),
            "Reviewed.md",
            "md",
            "Actual evidence",
            102,
        )
        .unwrap();
    assert!(
        f.store
            .read_source_document(source, f.project, run.id, doc.id)
            .is_err()
    );
    assert!(
        f.store
            .source_run(Uuid::new_v4(), f.project, run.id)
            .is_err()
    );
    assert!(f.store.source_run(source, Uuid::new_v4(), run.id).is_err());
    f.store
        .report(
            &root.lease,
            AssignmentOutcome::Completed(AssignmentResult {
                executed_checks: vec![],
                summary: "Reviewed implementation".into(),
                artifact_ids: vec![doc.id],
                checks: vec![],
            }),
            103,
        )
        .unwrap();
    assert_eq!(
        f.store
            .read_source_document(source, f.project, run.id, doc.id)
            .unwrap(),
        b"Actual evidence"
    );
    assert!(
        f.store
            .read_source_document(Uuid::new_v4(), f.project, run.id, doc.id)
            .is_err()
    );
    assert!(
        f.store
            .read_source_document(source, Uuid::new_v4(), run.id, doc.id)
            .is_err()
    );
    assert!(
        f.store
            .read_source_document(source, f.project, run.id, Uuid::new_v4())
            .is_err()
    );
    f.store
        .connect()
        .unwrap()
        .execute(
            "DELETE FROM ion_control_records WHERE id=?",
            [doc.id.to_string()],
        )
        .unwrap();
    assert!(
        f.store
            .read_source_document(source, f.project, run.id, doc.id)
            .is_err()
    );
}
#[test]
fn team_source_stop_only_revokes_its_own_runs_and_leaves_other_sources_active() {
    let f = Fixture::new();
    let source = Uuid::new_v4();
    let other = Uuid::new_v4();
    let first = f
        .store
        .queue_from_agent(
            source,
            f.project,
            "first".into(),
            f.team,
            f.definitions.load().unwrap().revision,
            "Research".into(),
            100,
        )
        .unwrap();
    let second = f
        .store
        .queue_from_agent(
            source,
            f.project,
            "second".into(),
            f.team,
            f.definitions.load().unwrap().revision,
            "Review".into(),
            100,
        )
        .unwrap();
    let unrelated = f
        .store
        .queue_from_agent(
            other,
            f.project,
            "other".into(),
            f.team,
            f.definitions.load().unwrap().revision,
            "Unrelated".into(),
            100,
        )
        .unwrap();
    f.materialize();
    let root = f.claim(first.id, first.work[0].id);
    let stopped = f.store.stop_source_runs(source, 102).unwrap();
    assert!(stopped.contains(&first.id) && stopped.contains(&second.id));
    assert_eq!(
        f.store.snapshot(first.id).unwrap().status,
        TeamRunStatus::Stopping
    );
    assert_eq!(
        f.store.snapshot(second.id).unwrap().status,
        TeamRunStatus::Canceled
    );
    assert_eq!(
        f.store.snapshot(unrelated.id).unwrap().status,
        TeamRunStatus::Active
    );
    assert!(f.store.renew(&root.lease, 102, 100).is_err());
    assert_eq!(
        f.store.stop_source_runs(source, 103).unwrap(),
        vec![first.id]
    );
}

#[test]
fn team_checkpoint_resume_preserves_attempt_host_grants_and_unknown_usage() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let checkpoint = r#"{"messages":[{"role":"user","content":"Pinned"}],"pending":[]}"#;
    f.store
        .save_recoverable_runtime_checkpoint(&root.lease, checkpoint, true, 102)
        .unwrap();
    let reservation = f
        .store
        .reserve_model_call(
            &root.lease,
            1,
            ModelCallAllowance {
                model: "model-0".into(),
                max_input_tokens: 1000,
                max_output_tokens: 100,
            },
            102,
        )
        .unwrap();
    let blocked = f
        .store
        .report(
            &root.lease,
            AssignmentOutcome::Blocked("Temporary connection failure".into()),
            103,
        )
        .unwrap();
    assert!(blocked.work[0].can_resume);
    assert!(
        f.store
            .queue_checkpoint_resume(run.id, root.assignment.id, blocked.revision - 1, 104)
            .is_err()
    );
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    let waiting = reopened
        .queue_checkpoint_resume(run.id, root.assignment.id, blocked.revision, 104)
        .unwrap();
    assert_eq!(waiting.work[0].status, AssignmentStatus::Waiting);
    assert!(!waiting.work[0].can_resume);
    let repeated = reopened
        .queue_checkpoint_resume(run.id, root.assignment.id, blocked.revision, 104)
        .unwrap();
    assert_eq!(repeated.revision, waiting.revision);
    let resumed = reopened
        .resume_after_children(run.id, root.assignment.id, 105, 100)
        .unwrap();
    assert_eq!(resumed.checkpoint, checkpoint);
    assert_eq!(
        resumed.recovery_note.as_deref(),
        Some("Temporary connection failure")
    );
    assert_eq!(resumed.claimed.lease.attempt_id(), root.lease.attempt_id());
    assert_eq!(resumed.claimed.assignment.attempts.len(), 1);
    assert_eq!(resumed.claimed.assignment.access, root.assignment.access);
    assert_eq!(resumed.claimed.profile.model, root.profile.model);
    assert_eq!(
        resumed.claimed.assignment.attempts[0].host,
        root.assignment.attempts[0].host
    );
    assert!(f.store.renew(&root.lease, 105, 100).is_err());
    assert!(
        f.store
            .report(&root.lease, AssignmentOutcome::Blocked("stale".into()), 105)
            .is_err()
    );
    assert!(
        reopened
            .resume_after_children(run.id, root.assignment.id, 105, 100)
            .is_err()
    );
    assert_eq!(
        reopened.snapshot(run.id).unwrap().usage.reserved_tokens,
        1100
    );
    reopened
        .record_model_usage(
            &reservation,
            ModelTokenUsage {
                input_tokens: 25,
                output_tokens: 5,
            },
            106,
        )
        .unwrap();
    let done = reopened
        .report(
            &resumed.claimed.lease,
            AssignmentOutcome::Completed(result()),
            106,
        )
        .unwrap();
    assert_eq!(done.status, TeamRunStatus::Completed);
    assert_eq!(done.usage.consumed_tokens, 30);
    assert_eq!(done.usage.model_calls, 1);
}
#[test]
fn team_checkpoint_resume_rejects_unmarked_superseded_unknown_and_stopped_work() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    f.store
        .save_runtime_checkpoint(&root.lease, "{}", 102)
        .unwrap();
    let blocked = f
        .store
        .report(&root.lease, AssignmentOutcome::Blocked("setup".into()), 103)
        .unwrap();
    assert!(!blocked.work[0].can_resume);
    assert!(
        f.store
            .queue_checkpoint_resume(run.id, root.assignment.id, blocked.revision, 104)
            .is_err()
    );

    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    f.store
        .save_recoverable_runtime_checkpoint(&root.lease, "{}", true, 102)
        .unwrap();
    f.store
        .save_runtime_checkpoint(&root.lease, r#"{"pending":["unfinished"]}"#, 102)
        .unwrap();
    let blocked = f
        .store
        .report(
            &root.lease,
            AssignmentOutcome::Blocked("tool stopped".into()),
            103,
        )
        .unwrap();
    assert!(!blocked.work[0].can_resume);

    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    f.store
        .save_recoverable_runtime_checkpoint(&root.lease, "{}", true, 102)
        .unwrap();
    f.store.reconcile_owner_restart(103).unwrap();
    let unknown = f.store.snapshot(run.id).unwrap();
    assert_eq!(unknown.work[0].status, AssignmentStatus::Uncertain);
    assert!(!unknown.work[0].can_resume);
    assert!(
        f.store
            .queue_checkpoint_resume(run.id, root.assignment.id, unknown.revision, 104)
            .is_err()
    );
    let blocked = f
        .store
        .resolve_uncertain(
            run.id,
            root.assignment.id,
            root.lease.attempt_id(),
            AssignmentOutcome::Blocked("Host confirmed stopped".into()),
            104,
        )
        .unwrap();
    assert!(blocked.work[0].can_resume);
    let canceled = f.store.stop(run.id, blocked.revision, 105).unwrap();
    assert!(!canceled.work[0].can_resume);
    assert!(
        f.store
            .queue_checkpoint_resume(run.id, root.assignment.id, canceled.revision, 106)
            .is_err()
    );
}
#[test]
fn team_checkpoint_resume_cannot_bypass_unfinished_file_effects_or_result_reconciliation() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    f.store
        .save_recoverable_runtime_checkpoint(&root.lease, "{}", true, 102)
        .unwrap();
    f.store
        .connect()
        .unwrap()
        .execute(
            "INSERT INTO ion_team_workspace_edits VALUES(?,?,?,?,?,?,'pending',NULL)",
            params![
                Uuid::new_v4().to_string(),
                run.id.to_string(),
                root.assignment.id.to_string(),
                root.lease.attempt_id().to_string(),
                Uuid::new_v4().to_string(),
                "{}"
            ],
        )
        .unwrap();
    let blocked = f
        .store
        .report(
            &root.lease,
            AssignmentOutcome::Blocked("Unknown file effect".into()),
            103,
        )
        .unwrap();
    assert!(!blocked.work[0].can_resume);
    assert!(
        f.store
            .queue_checkpoint_resume(run.id, root.assignment.id, blocked.revision, 104)
            .is_err()
    );
    // A trusted terminal host confirmation still cannot turn a pending effect
    // into successful work. Restart uncertainty cannot bypass the result gate.
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    f.store
        .connect()
        .unwrap()
        .execute(
            "INSERT INTO ion_team_workspace_edits VALUES(?,?,?,?,?,?,'pending',NULL)",
            params![
                Uuid::new_v4().to_string(),
                run.id.to_string(),
                root.assignment.id.to_string(),
                root.lease.attempt_id().to_string(),
                Uuid::new_v4().to_string(),
                "{}"
            ],
        )
        .unwrap();
    f.store.reconcile_owner_restart(103).unwrap();
    assert!(
        f.store
            .resolve_uncertain(
                run.id,
                root.assignment.id,
                root.lease.attempt_id(),
                AssignmentOutcome::Completed(result()),
                104
            )
            .is_err()
    );
    assert_eq!(
        f.store.snapshot(run.id).unwrap().work[0].status,
        AssignmentStatus::Uncertain
    );
}
#[test]
fn team_checkpoint_resume_keeps_deadlines_and_shared_capacity() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    f.store
        .save_recoverable_runtime_checkpoint(&root.lease, "{}", true, 102)
        .unwrap();
    let blocked = f
        .store
        .report(
            &root.lease,
            AssignmentOutcome::Blocked("transient".into()),
            103,
        )
        .unwrap();
    assert!(
        f.store
            .queue_checkpoint_resume(run.id, root.assignment.id, blocked.revision, run.deadline)
            .is_err()
    );
    f.store
        .queue_checkpoint_resume(run.id, root.assignment.id, blocked.revision, 104)
        .unwrap();
    let other = f.run();
    let busy = f.claim(other.id, other.work[0].id);
    assert!(
        f.store
            .resume_after_children(run.id, root.assignment.id, 105, 100)
            .is_err()
    );
    f.store
        .report(
            &busy.lease,
            AssignmentOutcome::Blocked("stopped".into()),
            106,
        )
        .unwrap();
    assert!(
        f.store
            .resume_after_children(run.id, root.assignment.id, 107, 100)
            .is_ok()
    );
}
#[test]
fn team_checkpoint_resume_request_races_queue_one_continuation_and_obey_limit() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    f.store
        .save_recoverable_runtime_checkpoint(&root.lease, "{}", true, 102)
        .unwrap();
    let blocked = f
        .store
        .report(
            &root.lease,
            AssignmentOutcome::Blocked("transient".into()),
            103,
        )
        .unwrap();
    let callers: Vec<_> = (0..4)
        .map(|_| {
            let db = TeamCoordinator::new(f.dir.join("control.db"));
            let revision = blocked.revision;
            let work = root.assignment.id;
            std::thread::spawn(move || {
                db.queue_checkpoint_resume(run.id, work, revision, 104)
                    .unwrap()
                    .revision
            })
        })
        .collect();
    let revisions: Vec<_> = callers.into_iter().map(|t| t.join().unwrap()).collect();
    assert!(revisions.iter().all(|r| *r == revisions[0]));
    let mut next = f
        .store
        .resume_after_children(run.id, root.assignment.id, 105, 100)
        .unwrap();
    for _ in 1..8 {
        f.store
            .save_recoverable_runtime_checkpoint(&next.claimed.lease, "{}", true, 106)
            .unwrap();
        let blocked = f
            .store
            .report(
                &next.claimed.lease,
                AssignmentOutcome::Blocked("transient".into()),
                107,
            )
            .unwrap();
        f.store
            .queue_checkpoint_resume(run.id, root.assignment.id, blocked.revision, 108)
            .unwrap();
        next = f
            .store
            .resume_after_children(run.id, root.assignment.id, 109, 100)
            .unwrap();
    }
    f.store
        .save_recoverable_runtime_checkpoint(&next.claimed.lease, "{}", true, 110)
        .unwrap();
    let blocked = f
        .store
        .report(
            &next.claimed.lease,
            AssignmentOutcome::Blocked("transient".into()),
            111,
        )
        .unwrap();
    assert!(
        f.store
            .queue_checkpoint_resume(run.id, root.assignment.id, blocked.revision, 112)
            .is_err()
    );
    assert_eq!(f.store.snapshot(run.id).unwrap().work[0].attempts.len(), 1);
}

#[path = "recovery/tests.rs"]
mod effect_recovery;

#[path = "inputs/tests.rs"]
mod input_handoffs;

#[path = "followup_tests.rs"]
mod followup_tests;

#[path = "automation_tests.rs"]
mod automation_tests;

#[test]
fn team_remote_submission_ledger_is_fenced_pinned_and_survives_lost_acknowledgement() {
    let f = Fixture::new();
    let image = Uuid::new_v4();
    let mut lead = f.run().profiles[0].clone();
    lead.execution = AgentExecution {
        environment: ExecutionEnvironment::NebiusSandbox,
        sandbox_target: Some(format!("worker@{image}")),
        local_fallback: true,
    };
    f.definitions
        .save(
            f.definitions.load().unwrap().revision,
            ControlRecord::Agent(lead),
        )
        .unwrap();
    let run = f.run();
    let claim = f
        .store
        .claim(
            run.id,
            run.work[0].id,
            DispatchHost::NebiusSandbox {
                target: format!("worker@{image}"),
            },
            101,
            100,
        )
        .unwrap();
    assert!(
        f.store
            .begin_remote_call(&claim.lease, image, "fixture-project", 102)
            .is_err()
    );
    let reservation = f
        .store
        .reserve_model_call(&claim.lease, 1, allowance("model-0", 100, 10), 102)
        .unwrap();
    assert!(
        f.store
            .begin_remote_call(&claim.lease, Uuid::new_v4(), "fixture-project", 102)
            .is_err()
    );
    let call = f
        .store
        .begin_remote_call(&claim.lease, image, "fixture-project", 102)
        .unwrap();
    assert!(
        f.store
            .begin_remote_call(&claim.lease, image, "fixture-project", 102)
            .is_err()
    );
    let reopened = TeamCoordinator::new(f.dir.join("control.db"));
    assert!(
        !reopened
            .remote_attempt_stopped(claim.lease.attempt_id())
            .unwrap()
    );
    assert!(
        reopened.remote_calls(claim.lease.attempt_id()).unwrap()[0]
            .operation
            .is_none()
    );
    let operation = Uuid::new_v4();
    reopened.bind_remote_operation(call, operation).unwrap();
    assert!(
        reopened
            .bind_remote_operation(call, Uuid::new_v4())
            .is_err()
    );
    assert!(
        reopened
            .finish_remote_call(call, Some(operation), "EXECUTING", None)
            .is_err()
    );
    assert!(
        reopened
            .finish_remote_call(call, Some(Uuid::new_v4()), "CANCELLED", None)
            .is_err()
    );
    reopened
        .finish_remote_call(call, Some(operation), "CANCELLED", None)
        .unwrap();
    assert!(
        reopened
            .remote_attempt_stopped(claim.lease.attempt_id())
            .unwrap()
    );
    assert!(reopened.snapshot(run.id).unwrap().usage.reserved_tokens > 0);
    reopened
        .record_model_usage(
            &reservation,
            ModelTokenUsage {
                input_tokens: 1,
                output_tokens: 1,
            },
            103,
        )
        .unwrap();
    assert!(
        reopened
            .begin_remote_call(&claim.lease, image, "fixture-project", 103)
            .is_err()
    );
}
#[test]
fn team_local_assignment_cannot_acquire_remote_transport() {
    let f = Fixture::new();
    let run = f.run();
    let claim = f.claim(run.id, run.work[0].id);
    f.store
        .reserve_model_call(&claim.lease, 1, allowance("model-0", 100, 10), 102)
        .unwrap();
    assert!(
        f.store
            .begin_remote_call(&claim.lease, Uuid::new_v4(), "fixture-project", 102)
            .is_err()
    );
}

#[path = "usage_ledger_tests.rs"]
mod usage_accounting;

#[path = "document_tests.rs"]
pub(crate) mod binary_documents;

fn remote_inventory_query() -> waku_protocol::remote_control::RemoteExecutionQuery {
    waku_protocol::remote_control::RemoteExecutionQuery {
        project_id: None,
        kind: None,
        unresolved_only: true,
        after: None,
        limit: 2,
    }
}
fn remote_inventory_seed(
    f: &Fixture,
    run: &TeamRunSnapshot,
    kind: waku_protocol::remote_control::RemoteExecutionKind,
    id: Uuid,
    state: &str,
) {
    use waku_protocol::remote_control::RemoteExecutionKind::*;
    let db = f.store.connect().unwrap();
    let work = &run.work[0];
    let attempt = Uuid::new_v4().to_string();
    let values = params![
        id.to_string(),
        run.id.to_string(),
        work.id.to_string(),
        attempt,
        state
    ];
    match kind {
        Model => {
            db.execute("INSERT INTO ion_team_model_calls(id,run_id,work_id,attempt_id,generation,step,model,started_at) VALUES(?1,?2,?3,?4,1,1,'private-model',100)",params![id.to_string(),run.id.to_string(),work.id.to_string(),attempt]).unwrap();
            db.execute("INSERT INTO ion_team_remote_calls(call_id,work_id,attempt_id,image,project,operation,state,started_at) VALUES(?1,?3,?4,'image','cloud-a',NULL,?5,100)",values).unwrap();
        }
        Command => {
            db.execute("INSERT INTO ion_team_jobs(request_id,run_id,work_id,attempt_id,image,project,signature,request,state,operation) VALUES(?1,?2,?3,?4,'image','cloud-a','signature','private-command-and-environment',?5,NULL)",values).unwrap();
        }
        Gpu => {
            db.execute("INSERT INTO ion_team_gpu_jobs(request_id,run_id,work_id,attempt_id,project,signature,body,policy,reserved_micros,currency,deadline_at,state,compute_ids) VALUES(?1,?2,?3,?4,'cloud-gpu','signature','private-body','private-policy',999,'USD',1000,?5,'[]')",values).unwrap();
        }
    }
}
#[test]
fn remote_inventory_pages_all_kinds_without_exposing_requests() {
    use waku_protocol::remote_control::RemoteExecutionKind::*;
    let f = Fixture::new();
    let run = f.run();
    for (kind, n) in [(Model, 1), (Model, 2), (Command, 1), (Gpu, 1)] {
        remote_inventory_seed(&f, &run, kind, Uuid::from_u128(n), "submitting");
    }
    let mut query = remote_inventory_query();
    let page = f.store.list_remote_executions(query.clone()).unwrap();
    assert_eq!(page.entries.len(), 2);
    assert!(page.entries.iter().all(|e| e.kind == Model));
    assert_eq!(
        page.next_cursor.as_ref().unwrap().request_id,
        Uuid::from_u128(2)
    );
    query.after = page.next_cursor;
    let next = f.store.list_remote_executions(query).unwrap();
    assert_eq!(
        next.entries.iter().map(|e| e.kind).collect::<Vec<_>>(),
        vec![Command, Gpu]
    );
    assert!(next.next_cursor.is_none());
    let serialized = serde_json::to_string(&next).unwrap();
    assert!(!serialized.contains("private"));
    assert!(!serialized.contains("signature"));
    assert!(next.entries.iter().all(|e| e.project_id == run.project_id
        && e.root_task_id == run.root_task_id
        && !e.settled));
}
#[test]
fn remote_inventory_terminal_gpu_requires_cleanup() {
    use waku_protocol::remote_control::RemoteExecutionKind::*;
    let f = Fixture::new();
    let run = f.run();
    let gpu = Uuid::new_v4();
    for (kind, state) in [(Model, "SUCCESS"), (Command, "settled"), (Gpu, "CANCELLED")] {
        remote_inventory_seed(
            &f,
            &run,
            kind,
            if kind == Gpu { gpu } else { Uuid::new_v4() },
            state,
        );
    }
    let page = f
        .store
        .list_remote_executions(remote_inventory_query())
        .unwrap();
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.entries[0].request_id, gpu);
    assert!(!page.entries[0].settled);
    f.store.connect().unwrap().execute("INSERT INTO ion_team_gpu_cleanup(request_id,body,state) VALUES(?,'private-cleanup-body','verified')",[gpu.to_string()]).unwrap();
    assert!(
        f.store
            .list_remote_executions(remote_inventory_query())
            .unwrap()
            .entries
            .is_empty()
    );
    let mut query = remote_inventory_query();
    query.unresolved_only = false;
    query.limit = 64;
    let page = f.store.list_remote_executions(query).unwrap();
    assert_eq!(page.entries.len(), 3);
    assert!(page.entries.iter().all(|e| e.settled));
    assert_eq!(
        page.entries
            .iter()
            .find(|e| e.kind == Gpu)
            .unwrap()
            .cleanup_state
            .as_deref(),
        Some("verified")
    );
}
#[test]
fn remote_inventory_filters_and_cursor_scope_fences() {
    use waku_protocol::remote_control::{RemoteExecutionCursor, RemoteExecutionKind::*};
    let f = Fixture::new();
    let run = f.run();
    let mut other_request = f.request();
    other_request.project_id = Uuid::new_v4();
    let other = f.store.prepare(other_request, 100).unwrap();
    let id = Uuid::new_v4();
    f.claim(run.id, run.work[0].id);
    remote_inventory_seed(&f, &run, Model, Uuid::new_v4(), "PENDING");
    remote_inventory_seed(&f, &run, Command, id, "canceling");
    remote_inventory_seed(&f, &other, Gpu, Uuid::new_v4(), "running");
    let mut query = remote_inventory_query();
    query.project_id = Some(run.project_id);
    query.kind = Some(Command);
    let page = f.store.list_remote_executions(query.clone()).unwrap();
    assert_eq!(page.entries.len(), 1);
    assert!(page.entries[0].stop_requested);
    f.store
        .stop(run.id, f.store.snapshot(run.id).unwrap().revision, 102)
        .unwrap();
    let mut model_query = query.clone();
    model_query.kind = Some(Model);
    let model = f.store.list_remote_executions(model_query).unwrap();
    assert_eq!(model.entries.len(), 1);
    assert!(model.entries[0].stop_requested);
    assert!(!model.entries[0].settled);
    query.after = Some(RemoteExecutionCursor {
        kind: Command,
        request_id: id,
    });
    query.kind = Some(Gpu);
    assert!(f.store.list_remote_executions(query.clone()).is_err());
    query.kind = None;
    query.project_id = Some(Uuid::new_v4());
    assert!(f.store.list_remote_executions(query.clone()).is_err());
    query.after = None;
    query.limit = 65;
    assert!(f.store.list_remote_executions(query.clone()).is_err());
    query.limit = 0;
    assert!(f.store.list_remote_executions(query).is_err());
}
#[test]
fn remote_inventory_cursor_survives_terminal_state_change() {
    use waku_protocol::remote_control::{RemoteExecutionCursor, RemoteExecutionKind::*};
    let f = Fixture::new();
    let run = f.run();
    let first = Uuid::from_u128(1);
    let next = Uuid::from_u128(2);
    remote_inventory_seed(&f, &run, Command, first, "running");
    remote_inventory_seed(&f, &run, Command, next, "running");
    f.store
        .connect()
        .unwrap()
        .execute(
            "UPDATE ion_team_jobs SET state='settled' WHERE request_id=?",
            [first.to_string()],
        )
        .unwrap();
    let mut query = remote_inventory_query();
    query.after = Some(RemoteExecutionCursor {
        kind: Command,
        request_id: first,
    });
    let page = f.store.list_remote_executions(query).unwrap();
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.entries[0].request_id, next);
}

#[path = "gateway_pdf_tests.rs"]
mod gateway_pdf_tests;


#[test]
fn managed_account_scope_is_pinned_with_creation_and_survives_restart() {
    let f=Fixture::new();let owner="a".repeat(64);let other="b".repeat(64);
    let mut request=f.request();request.account_scope=Some(owner.clone());
    let run=f.store.prepare(request.clone(),100).unwrap();
    let restarted=TeamCoordinator::new(f.definitions.path());
    restarted.bind_account_scope(run.id,Some(&owner)).unwrap();
    assert!(restarted.bind_account_scope(run.id,Some(&other)).is_err());
    assert!(restarted.bind_account_scope(run.id,None).is_err());
    request.account_scope=Some(other);
    assert!(restarted.prepare(request,101).is_err());
}
