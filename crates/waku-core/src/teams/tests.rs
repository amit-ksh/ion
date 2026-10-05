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
            id: Uuid::new_v4(),
            root_task_id: Uuid::new_v4(),
            project_id: self.project,
            team_id: self.team,
            expected_definition_revision: self.definitions.load().unwrap().revision,
            objective: "Implement and validate".into(),
            source_prompt: None,
            origin: None,
            limits: TeamRunLimits::default(),
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
    };
    let b = AppActionGrant {
        account_id: "account-b".into(),
        action: "GITHUB_GET_ISSUE".into(),
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
                action: "*".into()
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
