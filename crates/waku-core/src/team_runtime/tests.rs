use super::*;
use crate::{control::ControlStore, teams::PrepareRun};
use serde_json::{Value, json};
use std::sync::atomic::AtomicUsize;
use waku_protocol::control::*;

struct Scripted {
    ready: AtomicBool,
    leaf_ready: AtomicBool,
    rework_first: AtomicBool,
    starts: Mutex<Vec<(Uuid, Uuid)>>,
    models: Mutex<Vec<String>>,
}
fn tool_sse(messages: &[Value], name: &str, args: Value) -> String {
    let call = json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":format!("call-{}",messages.len()),"function":{"name":name,"arguments":args.to_string()}}]}}]});
    format!("data: {call}\n\ndata: [DONE]\n\n")
}
impl Executor for Scripted {
    fn ready(&self, profile: &AgentProfile) -> Result<DispatchHost> {
        ensure!(
            self.ready.load(Ordering::Acquire),
            "Test credentials missing"
        );
        ensure!(
            profile.model.as_deref() != Some("model-2") || self.leaf_ready.load(Ordering::Acquire),
            "Leaf setup missing"
        );
        Ok(DispatchHost::Local)
    }
    fn run(
        &self,
        coordinator: &TeamCoordinator,
        start: NativeTeamStart,
        workspace: &Path,
        events: &DriverEventSender,
        cancelled: &AtomicBool,
    ) -> Result<NativeTeamExit> {
        let (lease, model) = match &start {
            NativeTeamStart::Fresh(c) => (&c.lease, c.profile.model.clone()),
            NativeTeamStart::Resumed(c) => (&c.claimed.lease, c.claimed.profile.model.clone()),
        };
        let run_id = lease.run_id();
        let work_id = lease.work_id();
        self.starts.lock().push((work_id, lease.attempt_id()));
        driver::run_scripted_team_assignment(
            coordinator,
            start,
            workspace,
            events,
            cancelled,
            |actual, system, messages, _| {
                assert_eq!(Some(actual), model.as_deref());
                self.models.lock().push(actual.into());
                let run = coordinator.snapshot(run_id)?;
                let work = run.work.iter().find(|w| w.id == work_id).unwrap();
                if work.rework_of.is_some() {
                    assert!(
                        system.contains("Include concrete source evidence in the corrected report")
                    );
                }
                // Tool receipts come from the real native adapter, not fabricated coordinator results.
                if let Some(artifact) = messages
                    .iter()
                    .filter(|m| m["role"] == "tool")
                    .filter_map(|m| serde_json::from_str::<Value>(m["content"].as_str()?).ok())
                    .find(|v| v.get("format").is_some() && v.get("artifact_id").is_some())
                {
                    return Ok(tool_sse(
                        messages,
                        "report_result",
                        json!({"summary":"Reviewed delivered document","artifact_ids":[artifact["artifact_id"]]}),
                    ));
                }
                if let Some(member) = run
                    .team
                    .members
                    .iter()
                    .find(|m| m.parent_id == Some(work.agent_id))
                {
                    let children: Vec<_> = run
                        .work
                        .iter()
                        .filter(|w| w.parent_id == Some(work_id))
                        .collect();
                    if children.is_empty() {
                        assert!(system.contains(&member.agent_id.to_string()));
                        return Ok(tool_sse(
                            messages,
                            "delegate",
                            json!({"agent_id":member.agent_id,"objective":"Produce document evidence","expected_output":"Markdown report"}),
                        ));
                    }
                    if let Some(child) = children
                        .iter()
                        .find(|w| w.status == AssignmentStatus::InReview)
                    {
                        let artifact = child.result.as_ref().unwrap().artifact_ids[0];
                        let has_read = messages
                            .iter()
                            .filter(|m| m["role"] == "tool")
                            .filter_map(|m| {
                                serde_json::from_str::<Value>(m["content"].as_str()?).ok()
                            })
                            .any(|reply| {
                                reply["artifact_id"] == json!(artifact)
                                    && reply.get("content").is_some()
                            });
                        if !has_read {
                            return Ok(tool_sse(
                                messages,
                                "read_document",
                                json!({"artifact_id":artifact}),
                            ));
                        }
                        let correction = child.agent_id == run.profiles[2].id
                            && child.rework_of.is_none()
                            && self.rework_first.swap(false, Ordering::AcqRel);
                        return Ok(tool_sse(
                            messages,
                            "review_child",
                            json!({"assignment_id":child.id,"verdict":if correction {"rework"} else {"accepted"},
                            "feedback":if correction {"Include concrete source evidence in the corrected report"} else {"Document read and reviewed against expected output"},"artifact_ids":[artifact]}),
                        ));
                    }
                    if children.iter().any(|w| {
                        !matches!(
                            w.status,
                            AssignmentStatus::Completed | AssignmentStatus::Superseded
                        )
                    }) {
                        return Ok(tool_sse(
                            messages,
                            "await_children",
                            json!({"assignment_ids":children.iter().map(|w|w.id).collect::<Vec<_>>()}),
                        ));
                    }
                    let has_read = messages.iter().filter(|m| m["role"] == "tool").any(|m| {
                        m["content"]
                            .as_str()
                            .is_some_and(|s| s.contains("truncated"))
                    });
                    if !has_read {
                        return Ok(tool_sse(
                            messages,
                            "read_document",
                            json!({"artifact_id":children[0].result.as_ref().unwrap().artifact_ids[0]}),
                        ));
                    }
                }
                Ok(tool_sse(
                    messages,
                    "save_document",
                    json!({"name":"Review.md","format":"md","content":"# Reviewed evidence\nActual child results reviewed."}),
                ))
            },
        )
    }
}
struct Fixture {
    dir: PathBuf,
    runtime: Arc<TeamRuntime>,
    run: TeamRunSnapshot,
    script: Arc<Scripted>,
}
impl Fixture {
    fn new(limit: u16) -> Self {
        Self::with_budget(limit, Some(2_000_000))
    }
    fn with_budget(limit: u16, max_tokens: Option<u64>) -> Self {
        Self::build(limit, max_tokens, false)
    }
    fn build(limit: u16, max_tokens: Option<u64>, mixed: bool) -> Self {
        let dir = std::env::temp_dir().join(format!("ion-team-runtime-{}", Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("project")).unwrap();
        let control = ControlStore::new(dir.join("control.db"));
        let coordinator = Arc::new(TeamCoordinator::new(control.path()));
        let agents: [Uuid; 3] = std::array::from_fn(|_| Uuid::new_v4());
        for (i, id) in agents.iter().enumerate() {
            control
                .save(
                    control.load().unwrap().revision,
                    ControlRecord::Agent(AgentProfile {
                        id: *id,
                        name: format!("Agent {i}"),
                        expertise: "Research".into(),
                        instructions: format!("private instructions {i}"),
                        provider: if mixed && i == 1 {
                            ProviderKind::Claude
                        } else {
                            ProviderKind::Nebius
                        },
                        model: Some(format!("model-{i}")),
                        expected_output: "Document".into(),
                        execution: Default::default(),
                        access: Default::default(),
                        max_parallel_tasks: 1,
                    }),
                )
                .unwrap();
        }
        let team = Uuid::new_v4();
        control
            .save(
                control.load().unwrap().revision,
                ControlRecord::Team(Team {
                    id: team,
                    name: "Engineering".into(),
                    objective: "Review project".into(),
                    limits: None,
                    members: agents
                        .iter()
                        .enumerate()
                        .map(|(i, id)| TeamMember {
                            agent_id: *id,
                            parent_id: if i == 0 { None } else { Some(agents[i - 1]) },
                        })
                        .collect(),
                }),
            )
            .unwrap();
        let store = Arc::new(StateStore::new(dir.join("tasks.db")));
        let mut state = store.load().unwrap();
        let project = Uuid::new_v4();
        state.projects.push(Project {
            id: project,
            name: "Project".into(),
            path: dir.join("project"),
            created_at: unix_time(),
        });
        store.save(&mut state).unwrap();
        let run = coordinator
            .queue_task(
                PrepareRun {
                    id: Uuid::new_v4(),
                    root_task_id: Uuid::new_v4(),
                    project_id: project,
                    team_id: team,
                    expected_definition_revision: control.load().unwrap().revision,
                    objective: "Review".into(),
                    limits: TeamRunLimits {
                        max_parallel_tasks: 1,
                        max_model_calls: limit,
                        max_tokens,
                        ..Default::default()
                    },
                    access: Default::default(),
                    source_prompt: Some("@Engineering Review".into()),
                    origin: None,
                },
                unix_time(),
            )
            .unwrap();
        let script = Arc::new(Scripted {
            ready: AtomicBool::new(true),
            leaf_ready: AtomicBool::new(true),
            rework_first: AtomicBool::new(false),
            starts: Mutex::new(vec![]),
            models: Mutex::new(vec![]),
        });
        let runtime = TeamRuntime::with_executor(
            coordinator,
            store,
            Arc::new(Mutex::new(state)),
            Arc::new(DaemonSettingsStore::open(dir.join("settings.json")).unwrap()),
            Arc::new(Mutex::new(HashSet::new())),
            script.clone(),
        );
        *runtime.publisher.lock() = Some(RuntimePublisher::for_test());
        Self {
            dir,
            runtime,
            run,
            script,
        }
    }
    fn until(&self, condition: impl Fn(&TeamRunSnapshot) -> bool) -> TeamRunSnapshot {
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut after = None;
        loop {
            self.runtime.tick(&mut after).unwrap();
            let run = self.runtime.coordinator.snapshot(self.run.id).unwrap();
            if condition(&run) && self.runtime.jobs.lock().is_empty() {
                return run;
            }
            assert!(
                Instant::now() < deadline,
                "Dispatcher stalled: {:?}",
                run.work
                    .iter()
                    .map(|w| (w.status, w.reason.clone()))
                    .collect::<Vec<_>>()
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.runtime.shutdown();
        assert!(
            self.dir.starts_with(std::env::temp_dir())
                && self
                    .dir
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("ion-team-runtime-")
        );
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn team_dispatch_executes_three_levels_and_resumes_with_persisted_transcripts() {
    let f = Fixture::new(64);
    let run = f.until(|run| run.status == TeamRunStatus::Completed);
    assert_eq!(run.work.len(), 3);
    assert!(run.work.iter().filter(|w| w.parent_id.is_some()).all(|w| {
        w.review
            .as_ref()
            .is_some_and(|r| r.verdict == ReviewVerdict::Accepted)
    }));
    assert!(
        run.work
            .iter()
            .all(|w| w.status == AssignmentStatus::Completed && w.attempts.len() == 1)
    );
    let starts = f.script.starts.lock();
    assert_eq!(starts.len(), 5);
    for work in &run.work {
        let attempts: HashSet<_> = starts
            .iter()
            .filter(|(id, _)| *id == work.id)
            .map(|(_, id)| *id)
            .collect();
        assert_eq!(attempts.len(), 1);
    }
    let mut persisted = f.runtime.store.load().unwrap();
    assert_eq!(persisted.sessions.len(), 3);
    for task in &mut persisted.sessions {
        f.runtime.store.hydrate(task).unwrap();
        assert_eq!(task.turns.len(), 1);
        assert_eq!(task.turns[0].status, TurnStatus::Completed);
        assert_eq!(task.status, SessionStatus::Idle);
        assert!(task.runtime_event_cursor.is_some());
        assert!(!task.transcript_blocks.is_empty());
        assert_eq!(
            task.messages
                .iter()
                .filter(|m| m.role == MessageRole::User)
                .count(),
            1
        );
    }
    assert!(f.runtime.streams.lock().is_empty());
}
#[test]
fn team_setup_blocker_consumes_no_attempt_and_recovers_same_task() {
    let f = Fixture::new(64);
    f.script.ready.store(false, Ordering::Release);
    let mut after = None;
    f.runtime.tick(&mut after).unwrap();
    let blocked = f.runtime.coordinator.snapshot(f.run.id).unwrap();
    assert_eq!(blocked.work[0].status, AssignmentStatus::Queued);
    assert!(blocked.work[0].attempts.is_empty());
    let revision = blocked.revision;
    after = None;
    f.runtime.tick(&mut after).unwrap();
    assert_eq!(
        f.runtime.coordinator.snapshot(f.run.id).unwrap().revision,
        revision
    );
    f.script.ready.store(true, Ordering::Release);
    let run = f.until(|r| r.status == TeamRunStatus::Completed);
    assert_eq!(run.root_task_id, f.run.root_task_id);
}
#[test]
fn team_model_call_quota_stops_before_next_completion() {
    let f = Fixture::new(1);
    let run = f.until(|r| r.work[0].status == AssignmentStatus::Blocked);
    assert_eq!(f.script.models.lock().len(), 1);
    assert!(run.work[0].reason.as_deref().unwrap().contains("budget"));
    assert_eq!(run.work[0].attempts.len(), 1);
}
struct HeldWorker {
    entered: AtomicUsize,
    release: AtomicBool,
    cancel_seen: AtomicBool,
}
impl Executor for HeldWorker {
    fn ready(&self, _: &AgentProfile) -> Result<DispatchHost> {
        Ok(DispatchHost::Local)
    }
    fn run(
        &self,
        _: &TeamCoordinator,
        _: NativeTeamStart,
        _: &Path,
        events: &DriverEventSender,
        cancel: &AtomicBool,
    ) -> Result<NativeTeamExit> {
        self.entered.fetch_add(1, Ordering::Release);
        events.send(DriverEvent::TurnStarted)?;
        while !self.release.load(Ordering::Acquire) {
            if cancel.load(Ordering::Acquire) {
                self.cancel_seen.store(true, Ordering::Release);
            }
            thread::sleep(Duration::from_millis(2));
        }
        anyhow::bail!("Worker stopped after cancellation")
    }
}
#[test]
fn team_cancel_holds_capacity_until_actual_worker_join() {
    let f = Fixture::new(64);
    let held = Arc::new(HeldWorker {
        entered: AtomicUsize::new(0),
        release: AtomicBool::new(false),
        cancel_seen: AtomicBool::new(false),
    });
    let runtime = TeamRuntime::with_executor(
        f.runtime.coordinator.clone(),
        f.runtime.store.clone(),
        f.runtime.state.clone(),
        f.runtime.settings.clone(),
        f.runtime.removed.clone(),
        held.clone(),
    );
    *runtime.publisher.lock() = Some(RuntimePublisher::for_test());
    let mut after = None;
    runtime.tick(&mut after).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while held.entered.load(Ordering::Acquire) == 0 {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(2));
    }
    let run = runtime.coordinator.snapshot(f.run.id).unwrap();
    runtime
        .coordinator
        .stop(run.id, run.revision, unix_time())
        .unwrap();
    runtime.cancel_run(run.id);
    let canceling = runtime.coordinator.snapshot(run.id).unwrap();
    assert_eq!(canceling.work[0].status, AssignmentStatus::Canceling);
    assert!(canceling.work[0].status.holds_capacity());
    while !held.cancel_seen.load(Ordering::Acquire) {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(held.entered.load(Ordering::Acquire), 1);
    held.release.store(true, Ordering::Release);
    while !runtime.jobs.lock().is_empty() {
        runtime.tick(&mut after).unwrap();
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(2));
    }
    let canceled = runtime.coordinator.snapshot(run.id).unwrap();
    assert_eq!(canceled.status, TeamRunStatus::Canceled);
    assert_eq!(canceled.work[0].status, AssignmentStatus::Canceled);
    assert!(runtime.runtime(run.root_task_id).is_none());
    runtime.shutdown();
}
#[test]
fn team_projection_preserves_reasoning_text_tool_order_and_activity_identity() {
    let mut session = AgentSession::new(Uuid::new_v4(), ProviderKind::Nebius);
    session.push_message(MessageRole::User, "Prompt");
    let mut projection = projection::Projection::new(&mut session);
    projection.apply(&mut session, &DriverEvent::TurnStarted);
    projection.apply(&mut session, &DriverEvent::ReasoningDelta("Thought".into()));
    projection.apply(&mut session, &DriverEvent::TextDelta("Before".into()));
    let activity = DriverEvent::Activity {
        id: Some("stable".into()),
        kind: ActivityKind::FileRead,
        title: "Read".into(),
        detail: None,
        complete: false,
    };
    projection.apply(&mut session, &activity);
    let id = session.transcript_blocks[1].activities[0].id;
    projection.apply(&mut session, &DriverEvent::TextDelta("After".into()));
    projection.apply(
        &mut session,
        &DriverEvent::Activity {
            id: Some("stable".into()),
            kind: ActivityKind::FileRead,
            title: "Read".into(),
            detail: Some("Evidence".into()),
            complete: true,
        },
    );
    assert_eq!(session.transcript_blocks.len(), 2);
    assert_eq!(session.transcript_blocks[0].after_message, 1);
    assert_eq!(session.transcript_blocks[1].after_message, 2);
    assert_eq!(session.transcript_blocks[1].activities[0].id, id);
    projection.apply(&mut session, &DriverEvent::TurnParked);
    assert_eq!(session.turns.len(), 1);
    assert!(session.active_turn_id().is_some());
    projection.apply(&mut session, &DriverEvent::TurnStarted);
    projection.apply(
        &mut session,
        &DriverEvent::TurnFinished {
            success: true,
            summary: None,
        },
    );
    assert_eq!(session.turns.len(), 1);
    assert_eq!(session.messages.len(), 3);
    assert!(session.messages.iter().all(|m| !m.streaming));
}

#[test]
fn team_dispatch_resumes_waiting_chain_after_owner_restart_without_new_attempts() {
    let mut f = Fixture::new(64);
    f.script.leaf_ready.store(false, Ordering::Release);
    let waiting = f.until(|run| {
        run.work.len() == 3
            && run
                .work
                .iter()
                .filter(|w| w.status == AssignmentStatus::Waiting)
                .count()
                == 2
    });
    assert_eq!(f.runtime.streams.lock().len(), 2);
    f.runtime.shutdown();
    let recovered = f
        .runtime
        .coordinator
        .reconcile_owner_restart(unix_time())
        .unwrap();
    let _ = recovered;
    let state = f.runtime.store.load().unwrap();
    let next = TeamRuntime::with_executor(
        f.runtime.coordinator.clone(),
        f.runtime.store.clone(),
        Arc::new(Mutex::new(state)),
        f.runtime.settings.clone(),
        Arc::new(Mutex::new(HashSet::new())),
        f.script.clone(),
    );
    *next.publisher.lock() = Some(RuntimePublisher::for_test());
    f.runtime = next;
    f.script.leaf_ready.store(true, Ordering::Release);
    let completed = f.until(|run| run.status == TeamRunStatus::Completed);
    for work in waiting
        .work
        .iter()
        .filter(|w| w.status == AssignmentStatus::Waiting)
    {
        let resumed = completed.work.iter().find(|w| w.id == work.id).unwrap();
        assert_eq!(resumed.attempts.len(), 1);
        assert_eq!(resumed.attempts[0].id, work.attempts[0].id);
    }
    let mut state = f.runtime.store.load().unwrap();
    for session in &mut state.sessions {
        f.runtime.store.hydrate(session).unwrap();
        assert_eq!(session.turns.len(), 1);
        assert_eq!(session.turns[0].status, TurnStatus::Completed);
    }
}

#[test]
fn team_restart_unknown_host_is_visible_and_does_not_redispatch() {
    let f = Fixture::new(64);
    {
        let mut state = f.runtime.state.lock();
        f.runtime
            .coordinator
            .materialize_run(f.run.id, &f.runtime.store, &mut state, unix_time())
            .unwrap();
    }
    f.runtime
        .coordinator
        .claim(
            f.run.id,
            f.run.work[0].id,
            DispatchHost::Local,
            unix_time(),
            180,
        )
        .unwrap();
    f.runtime
        .coordinator
        .reconcile_owner_restart(unix_time())
        .unwrap();
    let mut after = None;
    f.runtime.tick(&mut after).unwrap();
    let run = f.runtime.coordinator.snapshot(f.run.id).unwrap();
    assert_eq!(run.work[0].status, AssignmentStatus::Uncertain);
    assert!(run.work[0].status.holds_capacity());
    assert!(f.script.starts.lock().is_empty());
    let mut state = f.runtime.store.load().unwrap();
    let session = &mut state.sessions[0];
    f.runtime.store.hydrate(session).unwrap();
    assert_eq!(session.status, SessionStatus::Failed);
    assert_eq!(
        session.team_assignment_status,
        Some(AssignmentStatus::Uncertain)
    );
    let messages = session.messages.len();
    after = None;
    f.runtime.tick(&mut after).unwrap();
    assert_eq!(f.runtime.state.lock().sessions[0].messages.len(), messages);
}
#[test]
fn team_stop_projects_waiting_and_unstarted_children_without_new_turns() {
    let f = Fixture::new(64);
    f.script.leaf_ready.store(false, Ordering::Release);
    let waiting = f.until(|run| {
        run.work.len() == 3
            && run
                .work
                .iter()
                .filter(|w| w.status == AssignmentStatus::Waiting)
                .count()
                == 2
    });
    f.runtime
        .coordinator
        .stop(waiting.id, waiting.revision, unix_time())
        .unwrap();
    f.runtime.cancel_run(waiting.id);
    let canceled = f.until(|run| run.status == TeamRunStatus::Canceled);
    assert!(f.runtime.streams.lock().is_empty());
    let mut state = f.runtime.store.load().unwrap();
    for session in &mut state.sessions {
        f.runtime.store.hydrate(session).unwrap();
        assert_eq!(
            session.team_assignment_status,
            Some(AssignmentStatus::Canceled)
        );
        assert_eq!(session.status, SessionStatus::Idle);
        let work = canceled
            .work
            .iter()
            .find(|w| w.task_id == session.id)
            .unwrap();
        if work.attempts.is_empty() {
            assert!(session.turns.is_empty());
        } else {
            assert_eq!(session.turns[0].status, TurnStatus::Interrupted);
        }
    }
}
#[test]
fn team_background_scheduler_runs_saved_queue_and_starts_only_once() {
    let f = Fixture::new(64);
    f.runtime.start(RuntimePublisher::for_test()).unwrap();
    assert!(f.runtime.start(RuntimePublisher::for_test()).is_err());
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let run = f.runtime.coordinator.snapshot(f.run.id).unwrap();
        if run.status == TeamRunStatus::Completed && f.runtime.jobs.lock().is_empty() {
            break;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(f.script.starts.lock().len(), 5);
    f.runtime.shutdown();
    let scheduler = f.runtime.scheduler.lock().take().unwrap();
    scheduler.join().unwrap();
}

#[test]
fn team_dispatch_executes_bounded_rework_with_new_task_and_persisted_review_status() {
    let f = Fixture::new(64);
    f.script.rework_first.store(true, Ordering::Release);
    let run = f.until(|r| r.status == TeamRunStatus::Completed);
    assert_eq!(run.work.len(), 4);
    let original = run
        .work
        .iter()
        .find(|w| w.status == AssignmentStatus::Superseded)
        .unwrap();
    let correction = run
        .work
        .iter()
        .find(|w| w.rework_of == Some(original.id))
        .unwrap();
    assert_ne!(original.task_id, correction.task_id);
    assert_eq!(original.agent_id, correction.agent_id);
    assert_eq!(original.output_contract, correction.output_contract);
    assert_eq!(original.access, correction.access);
    assert_eq!(original.result.as_ref().unwrap().artifact_ids.len(), 1);
    assert_eq!(correction.status, AssignmentStatus::Completed);
    assert_eq!(
        correction.review.as_ref().unwrap().verdict,
        ReviewVerdict::Accepted
    );
    let mut restored = f.runtime.store.load().unwrap();
    for session in &mut restored.sessions {
        f.runtime.store.hydrate(session).unwrap();
        let work = run.work.iter().find(|w| w.task_id == session.id).unwrap();
        assert_eq!(session.team_assignment_status, Some(work.status));
        assert_eq!(session.turns.len(), 1);
    }
    assert!(f.runtime.streams.lock().is_empty());
}

#[test]
fn team_review_metadata_persists_when_catalog_projection_matches_but_detail_is_stale() {
    let f = Fixture::new(64);
    let run = f.until(|r| r.status == TeamRunStatus::Completed);
    let child = run.work.iter().find(|w| w.parent_id.is_some()).unwrap();
    {
        let mut state = f.runtime.state.lock();
        state
            .session_mut(child.task_id)
            .unwrap()
            .team_assignment_status = Some(AssignmentStatus::InReview);
        f.runtime.store.save(&mut state).unwrap();
    }
    let mut catalog = f.runtime.store.load().unwrap();
    let child_catalog = catalog
        .sessions
        .iter_mut()
        .find(|s| s.id == child.task_id)
        .unwrap();
    assert!(!child_catalog.detail_loaded);
    child_catalog.team_assignment_status = Some(AssignmentStatus::Completed);
    *f.runtime.state.lock() = catalog;
    f.runtime.sync_review_statuses(&run).unwrap();
    let mut restored = f.runtime.store.load().unwrap();
    let session = restored
        .sessions
        .iter_mut()
        .find(|s| s.id == child.task_id)
        .unwrap();
    f.runtime.store.hydrate(session).unwrap();
    assert_eq!(
        session.team_assignment_status,
        Some(AssignmentStatus::Completed)
    );
    assert!(!session.transcript_blocks.is_empty());
    assert_eq!(session.turns.len(), 1);
}

#[test]
fn team_token_budget_projects_blocker_without_sending_provider_request() {
    let f = Fixture::with_budget(64, Some(100));
    let run = f.until(|run| run.work[0].status == AssignmentStatus::Blocked);
    assert!(
        run.work[0]
            .reason
            .as_deref()
            .unwrap()
            .contains("token budget")
    );
    assert!(f.script.models.lock().is_empty());
    assert_eq!(run.usage.model_calls, 0);
    let mut state = f.runtime.store.load().unwrap();
    let session = &mut state.sessions[0];
    f.runtime.store.hydrate(session).unwrap();
    assert_eq!(
        session.team_assignment_status,
        Some(AssignmentStatus::Blocked)
    );
    assert!(
        session
            .messages
            .iter()
            .any(|m| m.content.contains("token budget"))
    );
}

#[test]
fn team_dispatch_mixed_harnesses_preserve_selected_provider_on_real_child_tasks() {
    let f = Fixture::build(64, Some(2_000_000), true);
    let run = f.until(|run| run.status == TeamRunStatus::Completed);
    assert_eq!(run.work.len(), 3);
    let mut state = f.runtime.store.load().unwrap();
    for session in &mut state.sessions {
        f.runtime.store.hydrate(session).unwrap();
        let work = run.work.iter().find(|w| w.task_id == session.id).unwrap();
        let profile = run.profiles.iter().find(|p| p.id == work.agent_id).unwrap();
        assert_eq!(session.provider, profile.provider);
        assert_eq!(session.model, profile.model);
        assert!(!session.transcript_blocks.is_empty());
        assert_eq!(work.attempts.len(), 1);
    }
    assert_eq!(
        state
            .sessions
            .iter()
            .filter(|s| s.provider == ProviderKind::Claude)
            .count(),
        1
    );
    assert!(
        run.work
            .iter()
            .all(|work| !work.result.as_ref().unwrap().artifact_ids.is_empty())
    );
}

#[test]
fn team_runtime_checkpoint_resume_waits_for_join_then_completes_existing_task() {
    let f = Fixture::new(64);
    f.runtime
        .coordinator
        .materialize_run(
            f.run.id,
            &f.runtime.store,
            &mut f.runtime.state.lock(),
            unix_time(),
        )
        .unwrap();
    let root = f
        .runtime
        .coordinator
        .claim(
            f.run.id,
            f.run.work[0].id,
            DispatchHost::Local,
            unix_time(),
            180,
        )
        .unwrap();
    let lease = root.lease.clone();
    let (events, _) = driver::test_event_channel();
    assert!(
        driver::run_scripted_team_assignment(
            &f.runtime.coordinator,
            NativeTeamStart::Fresh(root),
            &f.dir,
            &events,
            &AtomicBool::new(false),
            |_, _, _, _| anyhow::bail!("Fixture request interrupted")
        )
        .is_err()
    );
    f.runtime
        .confirm_stopped(&lease, Some("Fixture request interrupted".into()))
        .unwrap();
    let blocked = f.runtime.coordinator.snapshot(f.run.id).unwrap();
    assert!(blocked.work[0].can_resume);
    let release = Arc::new(AtomicBool::new(false));
    let release_worker = release.clone();
    let thread = thread::spawn(move || {
        while !release_worker.load(Ordering::Acquire) {
            thread::sleep(Duration::from_millis(1));
        }
    });
    f.runtime.jobs.lock().insert(
        lease.work_id(),
        Job {
            run: f.run.id,
            cancelled: Arc::new(AtomicBool::new(false)),
            thread,
        },
    );
    assert!(
        f.runtime
            .resume_checkpoint(f.run.id, lease.work_id(), blocked.revision)
            .unwrap_err()
            .to_string()
            .contains("still finishing")
    );
    release.store(true, Ordering::Release);
    f.runtime
        .jobs
        .lock()
        .remove(&lease.work_id())
        .unwrap()
        .thread
        .join()
        .unwrap();
    let queued = f
        .runtime
        .resume_checkpoint(f.run.id, lease.work_id(), blocked.revision)
        .unwrap();
    assert_eq!(queued.work[0].status, AssignmentStatus::Waiting);
    let completed = f.until(|run| run.status == TeamRunStatus::Completed);
    assert_eq!(completed.work[0].task_id, f.run.root_task_id);
    assert_eq!(completed.work[0].attempts.len(), 1);
    assert_eq!(completed.work[0].attempts[0].id, lease.attempt_id());
    assert!(
        completed
            .work
            .iter()
            .all(|w| w.status == AssignmentStatus::Completed)
    );
    assert!(completed.usage.reserved_tokens >= blocked.usage.reserved_tokens);
    let mut persisted = f.runtime.store.load().unwrap();
    let task = persisted.session_mut(f.run.root_task_id).unwrap();
    f.runtime.store.hydrate(task).unwrap();
    assert_eq!(task.status, SessionStatus::Idle);
    assert_eq!(
        task.team_assignment_status,
        Some(AssignmentStatus::Completed)
    );
}
