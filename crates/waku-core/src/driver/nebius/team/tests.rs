use super::*;
use crate::{
    control::ControlStore,
    teams::{DispatchHost, PrepareRun},
};
use waku_protocol::control::*;

struct Fixture {
    directory: PathBuf,
    workspace: PathBuf,
    store: TeamCoordinator,
    control: ControlStore,
    agents: [Uuid; 3],
    run: TeamRunSnapshot,
}
impl Fixture {
    fn new(read: bool) -> Self {
        let directory =
            std::env::temp_dir().join(format!("ion-native-team-tests-{}", Uuid::new_v4()));
        let workspace = directory.join("project");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(
            workspace.join("README.md"),
            "# Actual project\nEvidence to review\n",
        )
        .unwrap();
        fs::write(workspace.join(".env"), "PRIVATE=do-not-read").unwrap();
        let control = ControlStore::new(directory.join("control.db"));
        let store = TeamCoordinator::new(control.path());
        let agents = std::array::from_fn(|_| Uuid::new_v4());
        let access = AgentAccess {
            workspace_read: read,
            ..Default::default()
        };
        for (i, id) in agents.iter().enumerate() {
            control
                .save(
                    control.load().unwrap().revision,
                    ControlRecord::Agent(AgentProfile {
                        id: *id,
                        name: format!("Agent {i}"),
                        expertise: "Research".into(),
                        instructions: format!("private-instructions-{i}"),
                        provider: ProviderKind::Nebius,
                        model: Some(format!("model-{i}")),
                        expected_output: "Reviewed document".into(),
                        execution: Default::default(),
                        access: access.clone(),
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
                    name: "Native team".into(),
                    objective: "Produce evidence".into(),
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
                    ],
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
                    objective: "Review the project".into(),
                    limits: TeamRunLimits {
                        max_parallel_tasks: 1,
                        require_child_review: false,
                        ..Default::default()
                    },
                    access,
                    source_prompt: None,
                    origin: None,
                },
                crate::model::unix_time(),
            )
            .unwrap();
        Self {
            directory,
            workspace,
            store,
            control,
            agents,
            run,
        }
    }
    fn claim(&self, work: Uuid) -> ClaimedAssignment {
        for pending in self.store.pending_materializations(128).unwrap() {
            self.store
                .acknowledge_materialized(
                    pending.run_id,
                    pending.assignment.id,
                    pending.assignment.task_id,
                    crate::model::unix_time(),
                )
                .unwrap();
        }
        self.store
            .claim(
                self.run.id,
                work,
                DispatchHost::Local,
                crate::model::unix_time(),
                180,
            )
            .unwrap()
    }
    fn worker(
        &self,
        start: NativeTeamStart,
        completion: impl FnMut(&str, &str, &[Value], Value) -> Result<(String, Vec<ToolCall>)>,
    ) -> Result<NativeTeamExit> {
        let work_id = match &start {
            NativeTeamStart::Fresh(c) => c.lease.work_id(),
            NativeTeamStart::Resumed(c) => c.claimed.lease.work_id(),
        };
        let (events, receiver) = crate::driver::test_event_channel();
        let outcome = run_with_completion(
            &self.store,
            start,
            &self.workspace,
            &events,
            &AtomicBool::new(false),
            completion,
        );
        let emitted: Vec<_> = receiver.try_iter().collect();
        if matches!(&outcome, Ok(NativeTeamExit::Yielded)) {
            assert!(emitted.iter().any(|e| matches!(e, DriverEvent::TurnParked)));
            assert!(
                !emitted
                    .iter()
                    .any(|e| matches!(e, DriverEvent::TurnFinished { .. }))
            );
        } else if matches!(&outcome, Ok(NativeTeamExit::Settled)) {
            let work = self
                .store
                .snapshot(self.run.id)
                .unwrap()
                .work
                .into_iter()
                .find(|w| w.id == work_id)
                .unwrap();
            let completed = work.status == AssignmentStatus::Completed;
            assert!(emitted.iter().any(
                |e| matches!(e, DriverEvent::TurnFinished {success, ..} if *success == completed)
            ));
        }
        outcome
    }
    fn artifacts(&self) -> Vec<Artifact> {
        self.control
            .load()
            .unwrap()
            .records
            .into_iter()
            .filter_map(|r| match r {
                ControlRecord::Artifact(a) => Some(a),
                _ => None,
            })
            .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert!(
            self.directory.starts_with(std::env::temp_dir())
                && self
                    .directory
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("ion-native-team-tests-")
        );
        let _ = fs::remove_dir_all(&self.directory);
    }
}
fn call(name: &str, args: Value) -> (String, Vec<ToolCall>) {
    (
        String::new(),
        vec![ToolCall {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            arguments: args.to_string(),
            activity_id: Uuid::new_v4().to_string(),
            announced: false,
        }],
    )
}
fn last_reply(messages: &[Value]) -> Value {
    serde_json::from_str(
        messages.iter().rev().find(|m| m["role"] == "tool").unwrap()["content"]
            .as_str()
            .unwrap(),
    )
    .unwrap()
}
fn delegate_and_wait(
    f: &Fixture,
    claimed: ClaimedAssignment,
    agent: Uuid,
    model: &str,
    instructions: &str,
) -> Uuid {
    let mut step = 0;
    let mut delegated = Uuid::nil();
    assert_eq!(f.worker(NativeTeamStart::Fresh(claimed), |actual,system,messages,_| {
        assert_eq!(actual,model);
        assert!(system.contains(instructions));
        assert!(!system.contains("private-instructions-2"));
        step+=1;
        Ok(if step==1 { call("delegate",json!({"agent_id":agent,"objective":"Read and review","expected_output":"Document evidence"})) }
        else { delegated=Uuid::parse_str(last_reply(messages)["assignment_id"].as_str().unwrap()).unwrap();call("await_children",json!({"assignment_ids":[delegated]})) })
    }).unwrap(),NativeTeamExit::Yielded);
    delegated
}

#[test]
fn native_team_three_level_execution_uses_pinned_models_private_context_and_real_documents() {
    let f = Fixture::new(true);
    let root = f.claim(f.run.work[0].id);
    let root_id = root.assignment.id;
    let manager_id = delegate_and_wait(&f, root, f.agents[1], "model-0", "private-instructions-0");
    let manager = f.claim(manager_id);
    let leaf_id = delegate_and_wait(
        &f,
        manager,
        f.agents[2],
        "model-1",
        "private-instructions-1",
    );
    let leaf = f.claim(leaf_id);
    let leaf_task = leaf.assignment.task_id;
    let mut step = 0;
    let mut artifact = Uuid::nil();
    assert_eq!(f.worker(NativeTeamStart::Fresh(leaf),|model,system,messages,_| {
        assert_eq!(model,"model-2");
        assert!(system.contains("private-instructions-2"));
        assert!(!system.contains("private-instructions-0"));
        step+=1;
        Ok(match step {
            1=>call("read_file",json!({"path":"README.md"})),
            2=>{assert!(last_reply(messages)["content"].as_str().unwrap().contains("Actual project"));call("save_document",json!({"name":"review.md","format":"md","content":"Reviewed actual project evidence."}))},
            _=>{artifact=Uuid::parse_str(last_reply(messages)["artifact_id"].as_str().unwrap()).unwrap();call("report_result",json!({"summary":"Reviewed project","artifact_ids":[artifact]}))},
        })
    }).unwrap(),NativeTeamExit::Settled);
    let document = f.artifacts().pop().unwrap();
    assert_eq!(document.task_id, Some(leaf_task));
    assert_eq!(
        f.control.read_document(artifact).unwrap(),
        b"Reviewed actual project evidence."
    );
    for (work, model) in [(manager_id, "model-1"), (root_id, "model-0")] {
        let continuation = f
            .store
            .resume_after_children(f.run.id, work, crate::model::unix_time(), 180)
            .unwrap();
        assert_eq!(continuation.children[0].artifact_ids, vec![artifact]);
        let mut step = 0;
        assert_eq!(
            f.worker(
                NativeTeamStart::Resumed(continuation),
                |actual, _, messages, _| {
                    assert_eq!(actual, model);
                    step += 1;
                    Ok(if step == 1 {
                        assert!(
                            messages.last().unwrap()["content"]
                                .as_str()
                                .unwrap()
                                .contains("Child work has settled")
                        );
                        call("read_document", json!({"artifact_id":artifact}))
                    } else {
                        assert_eq!(
                            last_reply(messages)["content"],
                            "Reviewed actual project evidence."
                        );
                        call(
                            "report_result",
                            json!({"summary":"Reviewed child document","artifact_ids":[artifact]}),
                        )
                    })
                }
            )
            .unwrap(),
            NativeTeamExit::Settled
        );
    }
    let completed = f.store.snapshot(f.run.id).unwrap();
    assert_eq!(completed.status, TeamRunStatus::Completed);
    assert_eq!(completed.work.len(), 3);
    assert_eq!(
        completed
            .work
            .iter()
            .map(|w| w.task_id)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        3
    );
    assert!(completed.work.iter().all(|w| w.attempts.len() == 1));
}

#[test]
fn native_team_tools_reject_identity_access_and_hierarchy_overrides_without_effects() {
    let f = Fixture::new(false);
    let root = f.claim(f.run.work[0].id);
    let task = root.assignment.task_id;
    let mut step = 0;
    assert_eq!(f.worker(NativeTeamStart::Fresh(root),|_,_,messages,tools| {
        assert!(!tools.as_array().unwrap().iter().any(|t|t["function"]["name"]=="read_file"));
        if step>0 {assert!(last_reply(messages)["error"].is_string());}
        step+=1;
        Ok(match step {
            1=>call("save_document",json!({"task_id":Uuid::new_v4(),"name":"spoof.md","format":"md","content":"spoof"})),
            2=>call("delegate",json!({"agent_id":f.agents[2],"objective":"Bypass manager","expected_output":"Document"})),
            3=>call("read_file",json!({"path":"README.md"})),
            4=>call("write_file",json!({"path":"pwned.txt","new_content":"bad"})),
            _=>call("report_blocker",json!({"reason":"Required capabilities are unavailable"})),
        })
    }).unwrap(),NativeTeamExit::Settled);
    assert!(f.artifacts().is_empty());
    assert!(!f.workspace.join("pwned.txt").exists());
    let run = f.store.snapshot(f.run.id).unwrap();
    assert_eq!(run.work.len(), 1);
    assert_eq!(run.work[0].task_id, task);
    assert_eq!(run.work[0].status, AssignmentStatus::Blocked);
}

#[test]
fn native_team_plain_text_does_not_claim_completion() {
    let f = Fixture::new(true);
    let root = f.claim(f.run.work[0].id);
    let result = f.worker(NativeTeamStart::Fresh(root), |_, _, _, _| {
        Ok(("All work is done".into(), vec![]))
    });
    assert!(result.unwrap_err().to_string().contains("model-call limit"));
    assert_eq!(
        f.store.snapshot(f.run.id).unwrap().work[0].status,
        AssignmentStatus::Running
    );
    assert!(f.artifacts().is_empty());
}

#[test]
fn native_team_stop_before_tool_execution_cannot_publish_an_artifact() {
    let f = Fixture::new(true);
    let root = f.claim(f.run.work[0].id);
    let lease = root.lease.clone();
    let result = f.worker(NativeTeamStart::Fresh(root), |_, _, _, _| {
        let run = f.store.snapshot(f.run.id)?;
        f.store
            .stop(run.id, run.revision, crate::model::unix_time())?;
        Ok(call(
            "save_document",
            json!({"name":"must-not-exist.md","format":"md","content":"stopped"}),
        ))
    });
    assert!(result.is_err());
    assert!(f.artifacts().is_empty());
    assert_eq!(
        f.store.snapshot(f.run.id).unwrap().work[0].status,
        AssignmentStatus::Canceling
    );
    f.store
        .report(
            &lease,
            AssignmentOutcome::Canceled,
            crate::model::unix_time(),
        )
        .unwrap();
    assert_eq!(
        f.store.snapshot(f.run.id).unwrap().status,
        TeamRunStatus::Canceled
    );
}

#[test]
fn native_team_reads_exclude_private_and_outside_paths_and_bound_escaped_replies() {
    let f = Fixture::new(true);
    fs::write(f.workspace.join("control.txt"), "\u{1}\"".repeat(4000)).unwrap();
    fs::write(f.directory.join("outside.txt"), "private outside").unwrap();
    let root = f.claim(f.run.work[0].id);
    let mut step = 0;
    assert_eq!(
        f.worker(NativeTeamStart::Fresh(root), |_, _, messages, _| {
            step += 1;
            if step == 2 || step == 3 {
                assert!(last_reply(messages)["error"].is_string());
            }
            if step == 4 {
                let reply = last_reply(messages);
                assert_eq!(reply["truncated"], true);
                assert!(reply.to_string().len() < MAX_REPLY_BYTES);
            }
            Ok(match step {
                1 => call("read_file", json!({"path":".env"})),
                2 => call("read_file", json!({"path":"../outside.txt"})),
                3 => call("read_file", json!({"path":"control.txt"})),
                _ => call("report_blocker", json!({"reason":"Read checks finished"})),
            })
        })
        .unwrap(),
        NativeTeamExit::Settled
    );
    let checkpoint = f.store.snapshot(f.run.id).unwrap();
    assert_eq!(checkpoint.work[0].status, AssignmentStatus::Blocked);
}

#[test]
fn native_team_invalid_wait_returns_feedback_and_unfinished_children_block_results() {
    let f = Fixture::new(true);
    let root = f.claim(f.run.work[0].id);
    let mut step = 0;
    let mut artifact = Uuid::nil();
    assert_eq!(f.worker(NativeTeamStart::Fresh(root),|_,_,messages,_| {
        step+=1;
        Ok(match step {
            1=>call("await_children",json!({"assignment_ids":[Uuid::new_v4()]})),
            2=>{assert!(last_reply(messages)["error"].is_string());call("delegate",json!({"agent_id":f.agents[1],"objective":"Pending child","expected_output":"Document"}))},
            3=>call("save_document",json!({"name":"parent.md","format":"md","content":"Work in progress"})),
            4=>{artifact=Uuid::parse_str(last_reply(messages)["artifact_id"].as_str().unwrap()).unwrap();call("report_result",json!({"summary":"Too early","artifact_ids":[artifact]}))},
            _=>{assert!(last_reply(messages)["error"].as_str().unwrap().contains("unfinished child"));call("report_blocker",json!({"reason":"Child requires setup"}))},
        })
    }).unwrap(),NativeTeamExit::Settled);
    let run = f.store.snapshot(f.run.id).unwrap();
    assert_eq!(run.work[0].status, AssignmentStatus::Blocked);
    assert_eq!(run.work[1].status, AssignmentStatus::Queued);
}

#[test]
fn native_team_lease_selects_authoritative_definitions_despite_changed_projections() {
    let f = Fixture::new(false);
    let mut root = f.claim(f.run.work[0].id);
    root.profile.model = Some("substituted-model".into());
    root.profile.instructions = "injected projection".into();
    root.assignment.access.workspace_read = true;
    root.assignment.objective = "injected objective".into();
    assert_eq!(
        f.worker(
            NativeTeamStart::Fresh(root),
            |model, system, messages, tools| {
                assert_eq!(model, "model-0");
                assert!(system.contains("private-instructions-0"));
                assert!(!system.contains("injected projection"));
                assert_eq!(messages[0]["content"], "Review the project");
                assert!(
                    !tools
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|t| t["function"]["name"] == "read_file")
                );
                Ok(call(
                    "report_blocker",
                    json!({"reason":"Pinned authorization verified"}),
                ))
            }
        )
        .unwrap(),
        NativeTeamExit::Settled
    );
}
