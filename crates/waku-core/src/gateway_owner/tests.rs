use super::*;
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    path::PathBuf,
};
use waku_protocol::{
    control::*,
    model::{Project, ProviderKind},
    teams::{AgentAccess, AgentExecution},
};

struct Fixture {
    dir: PathBuf,
    engine: OwnerEngine,
    workflow: Uuid,
    team: Uuid,
    project: Uuid,
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("ion-gateway-owner-test-{}", Uuid::new_v4()));
        let control = Arc::new(crate::control::ControlStore::new(dir.join("control.db")));
        let save = |record| {
            let rev = control.load().unwrap().revision;
            control.save(rev, record).unwrap();
        };
        let agent = Uuid::new_v4();
        save(ControlRecord::Agent(AgentProfile {
            id: agent,
            name: "Reviewer".into(),
            expertise: "Review".into(),
            instructions: "Private profile body".into(),
            provider: ProviderKind::Nebius,
            model: None,
            expected_output: "Evidence".into(),
            execution: AgentExecution::default(),
            access: AgentAccess {
                workspace_read: true,
                workspace_write: false,
                shell: false,
                network: false,
                app_actions: vec![],
            },
            max_parallel_tasks: 1,
        }));
        let team = Uuid::new_v4();
        save(ControlRecord::Team(Team {
            id: team,
            name: "Review team".into(),
            objective: "Private team objective".into(),
            limits: None,
            members: vec![TeamMember {
                agent_id: agent,
                parent_id: None,
            }],
        }));
        let project = Uuid::new_v4();
        let workflow = Uuid::new_v4();
        save(ControlRecord::Workflow(Workflow {
            id: workflow,
            name: "Release".into(),
            project_id: project,
            prompt: "Private workflow prompt".into(),
            limits: None,
            stages: vec![WorkflowStage {
                inherit_workspace: false,
                retry: None,
                title: "Review".into(),
                instructions: "Review this stage".into(),
                requires_decision: true,
                team_id: Some(team),
                required_formats: vec![],
                transition: WorkflowTransition::default(),
            }],
        }));
        let mut state = crate::persistence::StateStore::daemon(dir.join("tasks.json"))
            .load()
            .unwrap();
        state.projects = vec![Project {
            id: project,
            name: "Project".into(),
            path: dir.join("private-workspace"),
            created_at: 100,
        }];
        let engine = OwnerEngine {
            control,
            teams: Arc::new(TeamCoordinator::new(dir.join("control.db"))),
            tasks: Arc::new(Mutex::new(state)),
            task_store: Arc::new(crate::persistence::StateStore::daemon(
                dir.join("tasks.json"),
            )),
            runtime: std::sync::Weak::new(),
        };
        Self {
            dir,
            engine,
            workflow,
            team,
            project,
        }
    }
    fn start(&self, id: Uuid) -> GatewayAutomationAction {
        GatewayAutomationAction::Start {
            definition_id: self.workflow.to_string(),
            run_id: id.to_string(),
            expected_revision: self.engine.control.load().unwrap().revision,
        }
    }
    fn sync(&self, relay: &FakeRelay) {
        self.engine.sync(relay, &StopFence::default()).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

struct FakeRelay {
    owner: String,
    commands: RefCell<Vec<GatewayAutomationCommand>>,
    projections: RefCell<Vec<serde_json::Value>>,
    events: RefCell<Vec<String>>,
    supported: Cell<bool>,
    authorized: Cell<bool>,
    revoke_after_claim: Cell<bool>,
    fail_settlement: Cell<bool>,
    claim_override: RefCell<Option<GatewayAutomationCommand>>,
    claim_hook: RefCell<Option<Box<dyn Fn()>>>,
}
impl FakeRelay {
    fn new(action: GatewayAutomationAction) -> Self {
        let owner = Uuid::new_v4().to_string();
        Self {
            owner: owner.clone(),
            commands: RefCell::new(vec![GatewayAutomationCommand {
                id: Uuid::new_v4().to_string(),
                publisher_id: owner,
                status: "queued".into(),
                action,
            }]),
            projections: RefCell::new(vec![]),
            events: RefCell::new(vec![]),
            supported: Cell::new(true),
            authorized: Cell::new(true),
            revoke_after_claim: Cell::new(false),
            fail_settlement: Cell::new(false),
            claim_override: RefCell::new(None),
            claim_hook: RefCell::new(None),
        }
    }
    fn status(&self) -> String {
        self.commands.borrow()[0].status.clone()
    }
    fn queue(&self, action: GatewayAutomationAction) {
        self.commands.borrow_mut().push(GatewayAutomationCommand {
            id: Uuid::new_v4().to_string(),
            publisher_id: self.owner.clone(),
            status: "queued".into(),
            action,
        });
    }
}
impl Relay for FakeRelay {
    fn publisher(&self) -> &str {
        &self.owner
    }
    fn authorized(&self) -> Result<bool> {
        Ok(self.authorized.get())
    }
    fn supported(&self) -> Result<bool> {
        Ok(self.supported.get())
    }
    fn publish(&self, projection: serde_json::Value) -> Result<()> {
        self.projections.borrow_mut().push(projection);
        self.events.borrow_mut().push("publish".into());
        Ok(())
    }
    fn pending(&self) -> Result<Vec<GatewayAutomationCommand>> {
        Ok(self
            .commands
            .borrow()
            .iter()
            .filter(|c| c.status == "queued")
            .cloned()
            .collect())
    }
    fn claim(&self, id: &str) -> Result<GatewayAutomationCommand> {
        self.events.borrow_mut().push("claim".into());
        let command = {
            let mut commands = self.commands.borrow_mut();
            let command = commands
                .iter_mut()
                .find(|c| c.id == id && c.status == "queued")
                .unwrap();
            command.status = "uncertain".into();
            command.clone()
        };
        if self.revoke_after_claim.get() {
            self.authorized.set(false);
        }
        if let Some(hook) = self.claim_hook.borrow_mut().take() {
            hook();
        }
        Ok(self.claim_override.borrow_mut().take().unwrap_or(command))
    }
    fn uncertain(&self) -> Result<Vec<GatewayAutomationCommand>> {
        Ok(self
            .commands
            .borrow()
            .iter()
            .filter(|c| c.status == "uncertain")
            .cloned()
            .collect())
    }
    fn settle(&self, id: &str, applied: bool) -> Result<()> {
        self.events.borrow_mut().push("settle".into());
        if self.fail_settlement.get() {
            anyhow::bail!("Lost settlement acknowledgement");
        }
        self.commands
            .borrow_mut()
            .iter_mut()
            .find(|c| c.id == id)
            .unwrap()
            .status = if applied { "applied" } else { "rejected" }.into();
        Ok(())
    }
}

#[test]
fn headless_owner_publishes_metadata_claims_start_and_recovers_exact_receipt_after_restart() {
    let f = Fixture::new();
    let id = Uuid::new_v4();
    let relay = FakeRelay::new(f.start(id));
    relay.fail_settlement.set(true);
    assert!(f.engine.sync(&relay, &StopFence::default()).is_err());
    assert_eq!(relay.status(), "uncertain");
    let first = f.engine.teams.automation_run(id).unwrap();
    assert_eq!(first.workflow_id, Some(f.workflow));
    assert_eq!(&relay.events.borrow()[..3], &["publish", "claim", "settle"]);
    let json = relay.projections.borrow()[0].to_string();
    for private in [
        "Private profile body",
        "Private team objective",
        "Private workflow prompt",
        "private-workspace",
    ] {
        assert!(!json.contains(private));
    }
    // A fresh coordinator is the durable owner restart, with no desktop client.
    let restarted = OwnerEngine {
        control: f.engine.control.clone(),
        teams: Arc::new(TeamCoordinator::new(f.dir.join("control.db"))),
        tasks: f.engine.tasks.clone(),
        task_store: f.engine.task_store.clone(),
        runtime: std::sync::Weak::new(),
    };
    relay.fail_settlement.set(false);
    restarted.sync(&relay, &StopFence::default()).unwrap();
    assert_eq!(relay.status(), "applied");
    assert_eq!(
        relay
            .events
            .borrow()
            .iter()
            .filter(|s| s.as_str() == "claim")
            .count(),
        1
    );
    let second = restarted.teams.automation_run(id).unwrap();
    assert_eq!(second.revision, first.revision);
    assert_eq!(second.stages[0].team_run_id, first.stages[0].team_run_id);
    assert_eq!(
        restarted
            .teams
            .automation_runs(false, None, None, None, 64)
            .unwrap()
            .runs
            .len(),
        1
    );
}
#[test]
fn unknown_absent_start_and_gate_are_never_replayed() {
    let f = Fixture::new();
    let id = Uuid::new_v4();
    let relay = FakeRelay::new(f.start(id));
    relay.commands.borrow_mut()[0].status = "uncertain".into();
    relay.queue(GatewayAutomationAction::Gate {
        run_id: id.to_string(),
        expected_revision: 0,
        stage: 0,
        approved: true,
    });
    relay.commands.borrow_mut()[1].status = "uncertain".into();
    f.sync(&relay);
    assert!(f.engine.teams.automation_run(id).is_err());
    assert!(
        relay
            .commands
            .borrow()
            .iter()
            .all(|c| c.status == "uncertain")
    );
    assert!(
        !relay
            .events
            .borrow()
            .iter()
            .any(|s| s == "claim" || s == "settle")
    );
}
#[test]
fn authorization_revocation_and_shutdown_after_claim_fence_new_effects() {
    let f = Fixture::new();
    let id = Uuid::new_v4();
    let relay = FakeRelay::new(f.start(id));
    relay.revoke_after_claim.set(true);
    f.sync(&relay);
    assert_eq!(relay.status(), "uncertain");
    assert!(f.engine.teams.automation_run(id).is_err());
    let fence = Arc::new(StopFence::default());
    let stopped = fence.clone();
    let id = Uuid::new_v4();
    let relay = FakeRelay::new(f.start(id));
    *relay.claim_hook.borrow_mut() = Some(Box::new(move || {
        stopped.stopping.store(true, Ordering::Release)
    }));
    f.engine.sync(&relay, &fence).unwrap();
    assert_eq!(relay.status(), "uncertain");
    assert!(f.engine.teams.automation_run(id).is_err());
}
#[test]
fn owner_checks_grants_support_publisher_and_exact_claim_before_effects() {
    let f = Fixture::new();
    let id = Uuid::new_v4();
    let relay = FakeRelay::new(f.start(id));
    relay.authorized.set(false);
    f.sync(&relay);
    assert!(relay.projections.borrow().is_empty());
    relay.authorized.set(true);
    relay.supported.set(false);
    f.sync(&relay);
    assert!(relay.projections.borrow().is_empty());
    relay.supported.set(true);
    relay.commands.borrow_mut()[0].publisher_id = Uuid::new_v4().to_string();
    f.sync(&relay);
    assert_eq!(relay.status(), "queued");
    assert!(f.engine.teams.automation_run(id).is_err());
    relay.commands.borrow_mut()[0].publisher_id = relay.owner.clone();
    let mut wrong = relay.commands.borrow()[0].clone();
    wrong.status = "uncertain".into();
    wrong.action = f.start(Uuid::new_v4());
    *relay.claim_override.borrow_mut() = Some(wrong);
    f.sync(&relay);
    assert_eq!(relay.status(), "uncertain");
    assert!(f.engine.teams.automation_run(id).is_err());
}
#[test]
fn stale_revision_missing_project_and_malformed_ids_reject_without_dispatch() {
    let f = Fixture::new();
    let id = Uuid::new_v4();
    let mut stale = f.start(id);
    if let GatewayAutomationAction::Start {
        expected_revision, ..
    } = &mut stale
    {
        *expected_revision -= 1;
    }
    let relay = FakeRelay::new(stale);
    f.sync(&relay);
    assert_eq!(relay.status(), "rejected");
    assert!(f.engine.teams.automation_run(id).is_err());
    let relay = FakeRelay::new(f.start(id));
    f.engine.tasks.lock().projects.clear();
    f.sync(&relay);
    assert_eq!(relay.status(), "rejected");
    assert_eq!(
        f.engine.apply(&GatewayAutomationAction::Stop {
            run_id: "invalid".into(),
            expected_revision: 0
        }),
        Effect::Rejected
    );
}
#[test]
fn durable_start_receipt_remains_readable_after_project_removal_without_new_effect() {
    let f = Fixture::new();
    let id = Uuid::new_v4();
    let action = f.start(id);
    assert_eq!(f.engine.apply(&action), Effect::Applied);
    let first = f.engine.teams.automation_run(id).unwrap();
    f.engine.tasks.lock().projects.clear();
    assert_eq!(f.engine.apply(&action), Effect::Applied);
    assert_eq!(
        f.engine.teams.automation_run(id).unwrap().stages[0].team_run_id,
        first.stages[0].team_run_id
    );
    assert_eq!(f.engine.apply(&f.start(Uuid::new_v4())), Effect::Rejected);
}
#[test]
fn headless_gate_stop_and_routine_controls_use_transactional_revision_checks() {
    let f = Fixture::new();
    let id = Uuid::new_v4();
    assert_eq!(f.engine.apply(&f.start(id)), Effect::Applied);
    f.engine
        .teams
        .tick_automations(&HashSet::from([f.project]), crate::model::unix_time())
        .unwrap();
    let run = f.engine.teams.automation_run(id).unwrap();
    assert_eq!(run.status, AutomationStatus::WaitingDecision);
    let gate = GatewayAutomationAction::Gate {
        run_id: id.to_string(),
        expected_revision: run.revision,
        stage: 0,
        approved: true,
    };
    let mut stale = gate.clone();
    if let GatewayAutomationAction::Gate {
        expected_revision, ..
    } = &mut stale
    {
        *expected_revision -= 1;
    }
    assert_eq!(f.engine.apply(&stale), Effect::Rejected);
    assert_eq!(f.engine.apply(&gate), Effect::Applied);
    let approved = f.engine.teams.automation_run(id).unwrap();
    assert!(approved.stages[0].approved);
    assert_eq!(
        f.engine.apply(&GatewayAutomationAction::Stop {
            run_id: id.to_string(),
            expected_revision: approved.revision
        }),
        Effect::Applied
    );
    assert_eq!(
        f.engine.teams.automation_run(id).unwrap().status,
        AutomationStatus::Canceled
    );
    let routine = Uuid::new_v4();
    let rev = f.engine.control.load().unwrap().revision;
    let saved = f
        .engine
        .control
        .save(
            rev,
            ControlRecord::Routine(Routine {
                id: routine,
                name: "Review".into(),
                project_id: f.project,
                instructions: "Review".into(),
                interval_minutes: 10,
                calendar: None,
                missed_run_policy: MissedRunPolicy::default(),
                overlap_policy: OverlapPolicy::default(),
                paused: true,
                next_run: 0,
                provider: ProviderKind::Nebius,
                team_id: Some(f.team),
                workflow_id: None,
                last_error: None,
            }),
        )
        .unwrap();
    assert_eq!(
        f.engine.apply(&GatewayAutomationAction::Routine {
            definition_id: routine.to_string(),
            expected_revision: rev,
            paused: false
        }),
        Effect::Rejected
    );
    assert_eq!(
        f.engine.apply(&GatewayAutomationAction::Routine {
            definition_id: routine.to_string(),
            expected_revision: saved.revision,
            paused: false
        }),
        Effect::Applied
    );
    assert!(f.engine.control.load().unwrap().records.iter().any(
        |r| matches!(r, ControlRecord::Routine(r) if r.id == routine && !r.paused && r.next_run > 0)
    ));
}
#[test]
fn database_failure_keeps_unknown_claim_and_does_not_dispatch() {
    let f = Fixture::new();
    let id = Uuid::new_v4();
    let action = f.start(id);
    let corrupt = f.dir.join("invalid.db");
    std::fs::write(&corrupt, b"invalid database").unwrap();
    let engine = OwnerEngine {
        control: f.engine.control.clone(),
        teams: Arc::new(TeamCoordinator::new(corrupt)),
        tasks: f.engine.tasks.clone(),
        task_store: f.engine.task_store.clone(),
        runtime: std::sync::Weak::new(),
    };
    assert_eq!(engine.apply(&action), Effect::Unknown);
    assert!(f.engine.teams.automation_run(id).is_err());
}

#[test]
fn shutdown_drains_an_inflight_local_effect_and_blocks_following_commands() {
    let f = Fixture::new();
    let (wake, receiver) = bounded(1);
    let owner = Arc::new(GatewayOwner {
        fence: Arc::new(StopFence::default()),
        wake,
        receiver: Mutex::new(Some(receiver)),
        engine: Arc::new(OwnerEngine {
            control: f.engine.control.clone(),
            teams: f.engine.teams.clone(),
            tasks: f.engine.tasks.clone(),
            task_store: f.engine.task_store.clone(),
            runtime: std::sync::Weak::new(),
        }),
        file_managed: false,
    });
    let guard = owner.fence.effects.lock();
    let stop_owner = owner.clone();
    let (done, finished) = bounded(1);
    let stopping = std::thread::spawn(move || {
        stop_owner.shutdown();
        done.send(()).unwrap();
    });
    // Wait for the real atomic shutdown fence, not a guessed timing delay.
    let deadline = Instant::now() + Duration::from_secs(2);
    while !owner.fence.stopped() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(finished.try_recv().is_err());
    drop(guard);
    finished.recv_timeout(Duration::from_secs(2)).unwrap();
    stopping.join().unwrap();
    let relay = FakeRelay::new(GatewayAutomationAction::Start {
        definition_id: f.workflow.to_string(),
        run_id: Uuid::new_v4().to_string(),
        expected_revision: owner.engine.control.load().unwrap().revision,
    });
    owner.engine.sync(&relay, &owner.fence).unwrap();
    assert!(relay.events.borrow().is_empty());
}

#[test]
fn registered_owner_portfolio_reads_real_filtered_history_and_ledger_without_starting_work() {
    let f = Fixture::new();
    let result = super::portfolio::read(
        &f.engine,
        serde_json::json!({"kind":"projects","query":{"after":null,"limit":1}}),
    )
    .unwrap();
    assert_eq!(result["page"]["projects"][0]["id"], f.project.to_string());
    assert!(!result.to_string().contains("private-workspace"));
    let query = serde_json::json!({"kind":"inbox","query":{"projectId":null,"taskId":null,"kind":null,"view":"all","status":"all","after":null,"limit":32}});
    let before = super::portfolio::read(&f.engine, query.clone()).unwrap();
    assert_eq!(before["page"]["matchingCount"], 0);
    let run = Uuid::new_v4();
    assert_eq!(f.engine.apply(&f.start(run)), Effect::Applied);
    f.engine
        .teams
        .tick_automations(&HashSet::from([f.project]), crate::model::unix_time())
        .unwrap();
    let history = super::portfolio::read(&f.engine, query).unwrap();
    assert!(history["page"]["matchingCount"].as_u64().unwrap() > 0);
    assert!(
        history["page"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["kind"] == "decision" && row["runId"] == run.to_string())
    );
    let usage=super::portfolio::read(&f.engine,serde_json::json!({"kind":"usage","query":{"projectId":f.project,"taskId":null,"agentId":null,"provider":null,"environment":null,"kind":null,"model":null,"since":null,"after":null,"limit":32}})).unwrap();
    assert_eq!(usage["page"]["summary"]["records"], 0); // no dispatched model call
    assert_eq!(
        f.engine.teams.automation_run(run).unwrap().status,
        AutomationStatus::WaitingDecision
    );
    assert!(
        super::portfolio::read(&f.engine, serde_json::json!({"kind":"stop","runId":run})).is_err()
    );
}

#[test]
fn historical_owner_queries_page_by_creation_time_and_read_sealed_workflow_documents() {
    use crate::teams::{AssignmentOutcome, DispatchHost};
    use waku_protocol::teams::AssignmentResult;
    let f = Fixture::new();
    let now = crate::model::unix_time();
    let revision = f.engine.control.load().unwrap().revision;
    let newer = Uuid::from_u128(1);
    let older = Uuid::from_u128(2);
    f.engine
        .teams
        .start_automation(older, f.workflow, revision, now - 10)
        .unwrap();
    f.engine
        .teams
        .start_automation(newer, f.workflow, revision, now)
        .unwrap();
    let history=super::portfolio::read(&f.engine,serde_json::json!({"kind":"workflowHistory","query":{"projectId":f.project,"definitionId":null,"pendingOnly":false,"after":null,"limit":1}})).unwrap();
    assert_eq!(history["page"]["runs"][0]["id"], newer.to_string());
    assert_eq!(history["page"]["nextCursor"], newer.to_string());
    let next=super::portfolio::read(&f.engine,serde_json::json!({"kind":"workflowHistory","query":{"projectId":f.project,"definitionId":f.workflow,"pendingOnly":false,"after":newer,"limit":1}})).unwrap();
    assert_eq!(next["page"]["runs"][0]["id"], older.to_string());
    f.engine
        .teams
        .tick_automations(&HashSet::from([f.project]), now)
        .unwrap();
    let waiting = f.engine.teams.automation_run(newer).unwrap();
    f.engine
        .teams
        .resolve_automation_gate(newer, waiting.revision, 0, true, now)
        .unwrap();
    f.engine
        .teams
        .tick_automations(&HashSet::from([f.project]), now)
        .unwrap();
    let run = f.engine.teams.automation_run(newer).unwrap();
    let stage = &run.stages[0];
    f.engine
        .teams
        .materialize_run(
            stage.team_run_id,
            &f.engine.task_store,
            &mut f.engine.tasks.lock(),
            now,
        )
        .unwrap();
    let team = f.engine.teams.snapshot(stage.team_run_id).unwrap();
    let lease = f
        .engine
        .teams
        .claim(team.id, team.work[0].id, DispatchHost::Local, now, 60)
        .unwrap()
        .lease;
    let artifact = f
        .engine
        .teams
        .publish_document(
            &lease,
            Uuid::new_v4(),
            "report.md",
            "md",
            "Verified report",
            now,
        )
        .unwrap();
    f.engine
        .teams
        .report(
            &lease,
            AssignmentOutcome::Completed(AssignmentResult {
                summary: "Accepted report".into(),
                artifact_ids: vec![artifact.id],
                checks: vec![],
                executed_checks: vec![],
            }),
            now,
        )
        .unwrap();
    let page=super::portfolio::read(&f.engine,serde_json::json!({"kind":"workflowEvidence","query":{"runId":newer,"after":null,"limit":32}})).unwrap();
    assert_eq!(page["page"]["documents"][0]["id"], artifact.id.to_string());
    let chunk=super::portfolio::read(&f.engine,serde_json::json!({"kind":"workflowDocument","query":{"runId":newer,"artifactId":artifact.id,"sha256":artifact.evidence.unwrap().sha256,"offset":0,"limit":16384}})).unwrap();
    assert_eq!(chunk["page"]["data"], "VmVyaWZpZWQgcmVwb3J0");
    assert!(!page.to_string().contains("Private profile"));
    let other = super::portfolio::read(
        &f.engine,
        serde_json::json!({"kind":"workflowDocument","query":{"runId":older,"artifactId":artifact.id,"sha256":"a".repeat(64),"offset":0,"limit":1}}),
    );
    assert!(other.is_err());
}

#[test]
fn registered_owner_remote_inventory_read_has_no_effect_and_rejects_foreign_cursor() {
    let f = Fixture::new();
    let before_revision = f.engine.control.load().unwrap().revision;
    let before_tasks = f.engine.tasks.lock().sessions.len();
    let query = serde_json::json!({"kind":"remoteExecutions","query":{"projectId":f.project,"kind":null,"unresolvedOnly":false,"after":null,"limit":32}});
    let page = super::portfolio::read(&f.engine, query.clone()).unwrap();
    assert_eq!(page["kind"], "remoteExecutions");
    assert_eq!(page["page"]["entries"], serde_json::json!([]));
    assert!(page["page"]["nextCursor"].is_null());
    assert_eq!(page["pageTruncated"], false);
    let mut oversized = query.clone();
    oversized["query"]["limit"] = 33.into();
    assert!(super::portfolio::read(&f.engine, oversized).is_err());
    let mut foreign = query;
    foreign["query"]["after"] = serde_json::json!({"kind":"gpu","requestId":Uuid::new_v4()});
    assert!(super::portfolio::read(&f.engine, foreign).is_err());
    assert_eq!(f.engine.control.load().unwrap().revision, before_revision);
    assert_eq!(f.engine.tasks.lock().sessions.len(), before_tasks);
}
