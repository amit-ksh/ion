use super::*;
use crate::{
    control::ControlStore,
    teams::{DispatchHost, PrepareRun},
};
use waku_protocol::{control::*, model::QueuedMessage};

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
        Self::with_budget(read, Some(2_000_000))
    }
    fn with_budget(read: bool, max_tokens: Option<u64>) -> Self {
        Self::with_access(
            AgentAccess {
                workspace_read: read,
                ..Default::default()
            },
            max_tokens,
        )
    }
    fn with_access(access: AgentAccess, max_tokens: Option<u64>) -> Self {
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
                        max_tokens,
                        ..Default::default()
                    },
                    access,
                    input_data: vec![],
                    source_attachments: vec![],
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
        self.worker_apps(start, completion, &ComposioApps)
    }
    fn worker_apps(
        &self,
        start: NativeTeamStart,
        mut completion: impl FnMut(&str, &str, &[Value], Value) -> Result<(String, Vec<ToolCall>)>,
        apps: &dyn TeamApps,
    ) -> Result<NativeTeamExit> {
        let work_id = match &start {
            NativeTeamStart::Fresh(c) => c.lease.work_id(),
            NativeTeamStart::Resumed(c) => c.claimed.lease.work_id(),
        };
        let (events, receiver) = crate::driver::test_event_channel();
        let outcome = run_with_harness_and_apps(
            &self.store,
            start,
            &self.workspace,
            &events,
            &AtomicBool::new(false),
            ProviderKind::Nebius,
            |model| {
                Ok(ModelCallAllowance {
                    model: model.to_owned(),
                    max_input_tokens: 32768,
                    max_output_tokens: 4096,
                })
            },
            |model, system, messages, tools| {
                let (text, calls) = completion(model, system, messages, tools)?;
                Ok(StreamCompletion {
                    text,
                    calls,
                    usage: None,
                })
            },
            apps,
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
    fn execute(
        &self,
        start: NativeTeamStart,
        completion: impl FnMut(&str, &str, &[Value], Value) -> Result<String>,
    ) -> Result<NativeTeamExit> {
        let (events, _) = crate::driver::test_event_channel();
        run_scripted_team_assignment(
            &self.store,
            start,
            &self.workspace,
            &events,
            &AtomicBool::new(false),
            completion,
        )
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

#[test]
fn native_team_budget_blocks_before_billable_callback() {
    let f = Fixture::with_budget(true, Some(100));
    let outcome = f.execute(
        NativeTeamStart::Fresh(f.claim(f.run.work[0].id)),
        |_, _, _, _| panic!("Budgeted-out request must never be sent"),
    );
    assert!(outcome.unwrap_err().to_string().contains("token budget"));
    assert_eq!(f.store.snapshot(f.run.id).unwrap().usage.model_calls, 0);
}
#[test]
fn native_team_provider_usage_releases_allowance_before_next_request() {
    let f = Fixture::with_budget(true, Some(36890));
    let mut calls = 0;
    let outcome=f.execute(NativeTeamStart::Fresh(f.claim(f.run.work[0].id)),|_,_,_,_| {
        calls+=1;
        let delta=if calls==1 {json!({"content":"Working"})} else {
            json!({"tool_calls":[{"index":0,"id":"blocked","function":{"name":"report_blocker","arguments":json!({"reason":"Further setup required"}).to_string()}}]})
        };
        Ok(format!("data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
            json!({"choices":[{"delta":delta}]}),json!({"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":10,"total_tokens":20}})))
    });
    assert_eq!(outcome.unwrap(), NativeTeamExit::Settled);
    assert_eq!(calls, 2);
    let usage = f.store.snapshot(f.run.id).unwrap().usage;
    assert_eq!(usage.model_calls, 2);
    assert_eq!(usage.consumed_tokens, 40);
    assert_eq!(usage.reserved_tokens, 0);
    assert_eq!(usage.unknown_calls, 0);
}
#[test]
fn native_team_missing_usage_keeps_allowance_and_prevents_next_request() {
    let f = Fixture::with_budget(true, Some(36890));
    let mut calls = 0;
    let outcome = f.execute(
        NativeTeamStart::Fresh(f.claim(f.run.work[0].id)),
        |_, _, _, _| {
            calls += 1;
            Ok(
                "data: {\"choices\":[{\"delta\":{\"content\":\"Working\"}}]}\n\ndata: [DONE]\n\n"
                    .into(),
            )
        },
    );
    assert!(outcome.unwrap_err().to_string().contains("token budget"));
    assert_eq!(calls, 1);
    let usage = f.store.snapshot(f.run.id).unwrap().usage;
    assert_eq!(usage.reserved_tokens, 36864);
    assert_eq!(usage.consumed_tokens, 0);
}
#[test]
fn native_team_model_metadata_requires_exact_unique_context_bound() {
    let valid = json!({"data":[{"id":"other","context_length":100000},{"id":"model-0","context_length":32768}]});
    let quote = allowance_from_catalog(&valid, "model-0").unwrap();
    assert_eq!(quote.max_input_tokens, 32768);
    assert_eq!(quote.max_output_tokens, 4096);
    assert_eq!(quote.model, "model-0");
    for catalog in [
        json!({"data":[{"id":"other","context_length":1}]}),
        json!({"data":[{"id":"model-0"}]}),
        json!({"data":[{"id":"model-0","context_length":0}]}),
        json!({"data":[{"id":"model-0","context_length":-1}]}),
        json!({"data":[{"id":"model-0","context_length":100},{"id":"model-0","context_length":100}]}),
    ] {
        assert!(allowance_from_catalog(&catalog, "model-0").is_err());
    }
}

#[test]
fn native_team_decision_parks_and_delivers_only_the_pinned_human_receipt() {
    for approved in [true, false] {
        let f = Fixture::new(false);
        let root = f.claim(f.run.work[0].id);
        let mut calls = 0;
        assert_eq!(
            f.worker(NativeTeamStart::Fresh(root), |_, _, _, _| {
                calls += 1;
                Ok(call(
                    "request_decision",
                    json!({"title":"Release scope","proposal":"Include the documented migration?"}),
                ))
            })
            .unwrap(),
            NativeTeamExit::Yielded
        );
        assert_eq!(calls, 1);
        let snapshot = f.store.snapshot(f.run.id).unwrap();
        let d = &snapshot.decisions[0];
        assert_eq!(snapshot.work[0].status, AssignmentStatus::Waiting);
        assert!(
            f.store
                .resume_after_children(f.run.id, f.run.work[0].id, crate::model::unix_time(), 180)
                .is_err()
        );
        f.store
            .resolve_decision(d.id, d.revision, approved, crate::model::unix_time())
            .unwrap();
        let continuation = f
            .store
            .resume_after_children(f.run.id, f.run.work[0].id, crate::model::unix_time(), 180)
            .unwrap();
        assert!(!continuation.claimed.assignment.access.workspace_write);
        assert_eq!(
            f.worker(
                NativeTeamStart::Resumed(continuation),
                |model, _, messages, tools| {
                    assert_eq!(model, "model-0");
                    // Approval cannot add actual execution tools to this readless,
                    // shellless assignment. Check the contract, not prompt wording.
                    for tool in tools.as_array().unwrap() {
                        assert!(!matches!(
                            tool["function"]["name"].as_str(),
                            Some("write_file" | "run_remote_command" | "await_gpu_job")
                        ));
                    }
                    let receipt = messages.last().unwrap()["content"].as_str().unwrap();
                    assert!(receipt.contains(&d.id.to_string()));
                    assert!(receipt.contains(&d.proposal));
                    assert!(receipt.contains(if approved { "approved" } else { "declined" }));
                    assert!(receipt.contains("does not enlarge"));
                    Ok(call(
                        "report_blocker",
                        json!({"reason":"No additional execution grants"}),
                    ))
                }
            )
            .unwrap(),
            NativeTeamExit::Settled
        );
    }
}
#[test]
fn native_team_decision_rejects_identity_overrides_and_nonfinal_yield() {
    let f = Fixture::new(false);
    let root = f.claim(f.run.work[0].id);
    let invalid = call(
        "request_decision",
        json!({"title":"Scope","proposal":"Plan","host":"other"}),
    )
    .1
    .remove(0);
    let mut journal = TeamJournal::new("Test");
    journal.stage(String::new(), vec![invalid]).unwrap();
    assert!(
        execute(
            &f.store,
            &root,
            &f.workspace,
            &journal.pending[0],
            true,
            &ComposioApps
        )
        .is_err()
    );
    let valid = call(
        "request_decision",
        json!({"title":"Scope","proposal":"Plan"}),
    )
    .1
    .remove(0);
    let mut journal = TeamJournal::new("Test");
    journal.stage(String::new(), vec![valid]).unwrap();
    assert!(
        execute(
            &f.store,
            &root,
            &f.workspace,
            &journal.pending[0],
            false,
            &ComposioApps
        )
        .is_err()
    );
    assert!(f.store.snapshot(f.run.id).unwrap().decisions.is_empty());
}

#[test]
fn team_harness_seam_cannot_substitute_the_pinned_provider() {
    let f = Fixture::new(false);
    let mut claimed = f.claim(f.run.work[0].id);
    claimed.profile.provider = ProviderKind::Claude;
    let (events, _) = crate::driver::test_event_channel();
    let mut called = false;
    assert!(
        run_with_harness(
            &f.store,
            NativeTeamStart::Fresh(claimed),
            &f.workspace,
            &events,
            &AtomicBool::new(false),
            ProviderKind::Claude,
            |_| { panic!("no allowance before harness validation") },
            |_, _, _, _| {
                called = true;
                panic!("no completion before harness validation")
            }
        )
        .is_err()
    );
    assert!(!called);
    assert_eq!(f.store.snapshot(f.run.id).unwrap().usage.model_calls, 0);
}

struct FakeApps {
    calls: std::rc::Rc<std::cell::Cell<usize>>,
    unavailable: bool,
    missing_ack: bool,
}
impl TeamApps for FakeApps {
    fn describe(&self, action: &TeamAppAction) -> Result<Value> {
        Ok(
            json!({"action":action.grant.action,"version":action.grant.version,"input_parameters":{"type":"object"}}),
        )
    }
    fn prepare(&self, action: &TeamAppAction) -> Result<crate::driver::team_apps::PreparedApp> {
        anyhow::ensure!(!self.unavailable, "Not connected");
        assert_eq!(action.grant.account_id, "ca_github_1");
        assert_eq!(action.grant.action, "GITHUB_CREATE_ISSUE");
        assert_eq!(action.grant.version, "20250905_00");
        assert_eq!(
            action.arguments_json,
            json!({"title":"Exact issue"}).to_string()
        );
        let calls = self.calls.clone();
        let missing = self.missing_ack;
        Ok(Box::new(move || {
            calls.set(calls.get() + 1);
            anyhow::ensure!(!missing, "No acknowledgement");
            Ok(json!({"successful":true,"data":{"issue":123}}).to_string())
        }))
    }
}
fn app_agent_fixture() -> Fixture {
    Fixture::with_access(
        AgentAccess {
            workspace_read: false,
            app_actions: vec![AppActionGrant {
                account_id: "ca_github_1".into(),
                action: "GITHUB_CREATE_ISSUE".into(),
                version: "20250905_00".into(),
            }],
            ..Default::default()
        },
        Some(2_000_000),
    )
}
fn request_app(f: &Fixture, apps: &FakeApps) -> TeamDecision {
    let claimed = f.claim(f.run.work[0].id);
    assert_eq!(f.worker_apps(NativeTeamStart::Fresh(claimed),|_,_,_,tools| {
        assert!(tools.as_array().unwrap().iter().any(|t|t["function"]["name"]=="request_app_action"));
        Ok(call("request_app_action",json!({"account_id":"ca_github_1","action":"GITHUB_CREATE_ISSUE","arguments":{"title":"Exact issue"}})))
    },apps).unwrap(),NativeTeamExit::Yielded);
    assert_eq!(apps.calls.get(), 0);
    f.store.snapshot(f.run.id).unwrap().decisions[0].clone()
}
#[test]
fn native_team_app_approval_executes_exact_payload_before_next_inference_and_publishes_receipt() {
    let f = app_agent_fixture();
    let apps = FakeApps {
        calls: Default::default(),
        unavailable: false,
        missing_ack: false,
    };
    let decision = request_app(&f, &apps);
    f.store
        .resolve_decision(
            decision.id,
            decision.revision,
            true,
            crate::model::unix_time(),
        )
        .unwrap();
    let resumed = f
        .store
        .resume_after_children(f.run.id, f.run.work[0].id, crate::model::unix_time(), 180)
        .unwrap();
    assert_eq!(f.worker_apps(NativeTeamStart::Resumed(resumed),|_,_,messages,_| {
        assert_eq!(apps.calls.get(),1);
        let receipt:Value=messages.iter().filter_map(|m|m["content"].as_str()).filter_map(|v|serde_json::from_str::<Value>(v).ok()).find(|v|v["app_action_receipt"]==decision.id.to_string()).unwrap();
        assert_eq!(receipt["successful"],true);
        Ok(call("report_result",json!({"summary":"Created approved issue","artifact_ids":[receipt["artifact_id"]]})))
    },&apps).unwrap(),NativeTeamExit::Settled);
    assert_eq!(apps.calls.get(), 1);
    let artifact = f
        .artifacts()
        .into_iter()
        .find(|a| a.name.starts_with("App-action-"))
        .unwrap();
    assert_eq!(artifact.task_id, Some(f.run.root_task_id));
    assert_eq!(
        f.store.snapshot(f.run.id).unwrap().status,
        TeamRunStatus::Completed
    );
}
#[test]
fn native_team_app_decline_and_preflight_failure_send_no_mutation_request() {
    for approve in [false, true] {
        let f = app_agent_fixture();
        let apps = FakeApps {
            calls: Default::default(),
            unavailable: approve,
            missing_ack: false,
        };
        let d = request_app(&f, &apps);
        f.store
            .resolve_decision(d.id, 1, approve, crate::model::unix_time())
            .unwrap();
        let resumed = f
            .store
            .resume_after_children(f.run.id, f.run.work[0].id, crate::model::unix_time(), 180)
            .unwrap();
        assert_eq!(
            f.worker_apps(
                NativeTeamStart::Resumed(resumed),
                |_, _, messages, _| {
                    assert_eq!(apps.calls.get(), 0);
                    assert!(
                        messages
                            .to_vec()
                            .iter()
                            .filter_map(|m| m["content"].as_str())
                            .any(|s| s.contains(if approve {
                                "was not started"
                            } else {
                                "declined"
                            }))
                    );
                    Ok(call(
                        "report_blocker",
                        json!({"reason":"No app action executed"}),
                    ))
                },
                &apps
            )
            .unwrap(),
            NativeTeamExit::Settled
        );
        assert!(!f.store.has_pending_effect(f.run.id).unwrap());
    }
}
#[test]
fn native_team_app_missing_ack_stops_before_another_model_call_or_effect() {
    let f = app_agent_fixture();
    let apps = FakeApps {
        calls: Default::default(),
        unavailable: false,
        missing_ack: true,
    };
    let d = request_app(&f, &apps);
    f.store
        .resolve_decision(d.id, 1, true, crate::model::unix_time())
        .unwrap();
    let resumed = f
        .store
        .resume_after_children(f.run.id, f.run.work[0].id, crate::model::unix_time(), 180)
        .unwrap();
    let lease = resumed.claimed.lease.clone();
    assert!(
        f.worker_apps(
            NativeTeamStart::Resumed(resumed),
            |_, _, _, _| panic!("No inference after uncertain app outcome"),
            &apps
        )
        .is_err()
    );
    assert_eq!(apps.calls.get(), 1);
    assert!(f.store.has_pending_effect(f.run.id).unwrap());

    let now = crate::model::unix_time();
    let blocked = f
        .store
        .report(
            &lease,
            AssignmentOutcome::Blocked("Lost app acknowledgement".into()),
            now,
        )
        .unwrap();
    let (_, settled) = f
        .store
        .confirm_recovery(
            f.run.id,
            lease.work_id(),
            lease.attempt_id(),
            blocked.revision,
            TeamRecoveryConfirmation::AppSucceeded {
                decision_id: d.id,
                evidence: "User verified provider item 123".into(),
            },
            now,
        )
        .unwrap();
    let (_, prepared) = f
        .store
        .confirm_recovery(
            f.run.id,
            lease.work_id(),
            lease.attempt_id(),
            settled.revision,
            TeamRecoveryConfirmation::PrepareContinuation,
            now,
        )
        .unwrap();
    f.store
        .queue_checkpoint_resume(f.run.id, lease.work_id(), prepared.revision, now)
        .unwrap();
    let resumed = f
        .store
        .resume_after_children(f.run.id, lease.work_id(), now, 180)
        .unwrap();
    assert_eq!(f.worker_apps(NativeTeamStart::Resumed(resumed),|_,_,messages,_|{
        assert_eq!(apps.calls.get(),1);
        let receipt=messages.iter().filter_map(|m|serde_json::from_str::<Value>(m["content"].as_str()?).ok()).find(|v|v["app_action_receipt"]==d.id.to_string()).unwrap();
        assert_eq!(receipt["confirmed_by"],"user");
        Ok(call("report_result",json!({"summary":"Recovered approved app outcome","artifact_ids":[receipt["artifact_id"]]})))
    },&apps).unwrap(),NativeTeamExit::Settled);
    assert_eq!(apps.calls.get(), 1);
    assert_eq!(
        f.store.snapshot(f.run.id).unwrap().status,
        TeamRunStatus::Completed
    );
}

#[test]
fn native_team_app_schema_is_grant_scoped_and_cannot_execute_or_override_identity() {
    let f = app_agent_fixture();
    let apps = FakeApps {
        calls: Default::default(),
        unavailable: false,
        missing_ack: false,
    };
    let claimed = f.claim(f.run.work[0].id);
    let mut journal = TeamJournal::new("App metadata");
    for args in [
        json!({"account_id":"ca_other","action":"GITHUB_CREATE_ISSUE"}),
        json!({"account_id":"ca_github_1","action":"GITHUB_DELETE_REPOSITORY"}),
        json!({"account_id":"ca_github_1","action":"GITHUB_CREATE_ISSUE","version":"latest"}),
    ] {
        let (text, calls) = call("describe_app_action", args);
        journal.stage(text, calls).unwrap();
        assert!(
            execute(
                &f.store,
                &claimed,
                &f.workspace,
                &journal.pending[0],
                true,
                &apps
            )
            .is_err()
        );
        journal.pending.clear();
    }
    let (text, calls) = call(
        "describe_app_action",
        json!({"account_id":"ca_github_1","action":"GITHUB_CREATE_ISSUE"}),
    );
    journal.stage(text, calls).unwrap();
    let body = match execute(
        &f.store,
        &claimed,
        &f.workspace,
        &journal.pending[0],
        true,
        &apps,
    )
    .unwrap()
    {
        ToolOutcome::Reply(body) => body,
        _ => panic!(),
    };
    let body: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(body["version"], "20250905_00");
    assert_eq!(apps.calls.get(), 0);
    let mut legacy = claimed.assignment.access.clone();
    legacy.app_actions[0].version.clear();
    assert!(!tools(&legacy).to_string().contains("request_app_action"));
    assert!(
        !tools(&AgentAccess::default())
            .to_string()
            .contains("describe_app_action")
    );
}

#[test]
fn native_team_checkpoint_resume_after_transport_failure_keeps_context_and_reserved_usage() {
    let f = Fixture::new(true);
    let root = f.claim(f.run.work[0].id);
    let lease = root.lease.clone();
    let error = f
        .worker(NativeTeamStart::Fresh(root), |_, _, _, _| {
            bail!("Fixture connection failed")
        })
        .unwrap_err();
    let blocked = f
        .store
        .report(
            &lease,
            AssignmentOutcome::Blocked(error.to_string()),
            crate::model::unix_time(),
        )
        .unwrap();
    assert!(blocked.work[0].can_resume);
    assert_eq!(blocked.usage.unknown_calls, 1);
    let waiting = f
        .store
        .queue_checkpoint_resume(
            f.run.id,
            lease.work_id(),
            blocked.revision,
            crate::model::unix_time(),
        )
        .unwrap();
    let continuation = f
        .store
        .resume_after_children(waiting.id, lease.work_id(), crate::model::unix_time(), 180)
        .unwrap();
    let mut calls = 0;
    assert_eq!(f.worker(NativeTeamStart::Resumed(continuation), |model, _, messages, _| {
        calls += 1;
        assert_eq!(model, "model-0");
        assert!(messages.iter().any(|m| m["content"].as_str().is_some_and(|s| s.contains("safe durable checkpoint"))));
        if calls == 1 {
            Ok(call("save_document", json!({"name":"Recovered.md","format":"md","content":"# Verified recovered result"})))
        } else {
            let artifact = f.artifacts().into_iter().find(|a| a.name == "Recovered.md").unwrap();
            Ok(call("report_result", json!({"summary":"Recovered with evidence","artifact_ids":[artifact.id]})))
        }
    }).unwrap(), NativeTeamExit::Settled);
    let done = f.store.snapshot(f.run.id).unwrap();
    assert_eq!(done.status, TeamRunStatus::Completed);
    assert_eq!(done.work[0].attempts.len(), 1);
    assert_eq!(done.usage.model_calls, 3);
    assert_eq!(done.usage.unknown_calls, 3);
    assert!(done.usage.reserved_tokens >= blocked.usage.reserved_tokens);
    assert!(
        f.store
            .renew(&lease, crate::model::unix_time(), 180)
            .is_err()
    );
}

#[test]
fn native_team_reads_and_lists_exclude_private_edit_backups() {
    let f = Fixture::new(true);
    let workspace = fs::canonicalize(&f.workspace).unwrap();
    for name in [".ion-edit-test.deleted", ".ION-EDIT-TEST.pending"] {
        fs::write(f.workspace.join(name), "Private recovery content").unwrap();
        assert!(scoped_path(&workspace, name).is_err());
    }
    let mut inspected = 0;
    let mut files = vec![];
    list_files(&workspace, &workspace, 0, &mut inspected, &mut files).unwrap();
    assert!(files.contains(&"README.md".into()));
    assert!(
        !files
            .iter()
            .any(|n| n.to_ascii_lowercase().starts_with(".ion-edit-"))
    );
    let mut solo_files = vec![];
    visit_files(&workspace, &workspace, &mut solo_files).unwrap();
    assert!(
        !solo_files
            .iter()
            .any(|n| n.to_ascii_lowercase().starts_with(".ion-edit-"))
    );
}

#[test]
fn native_team_inputs_reach_real_child_tasks_and_survive_a_resumed_parent() {
    use crate::attachments::{AttachmentStore, AttachmentUpload};
    use base64::Engine as _;
    let mut f = Fixture::new(true);
    let store = AttachmentStore::new(f.directory.join("attachments"));
    let stored = store
        .import(
            "requirements.md",
            AttachmentUpload::File {
                data_base64: base64::engine::general_purpose::STANDARD
                    .encode(b"Actual user requirements\n"),
            },
        )
        .unwrap();
    let input = waku_protocol::model::MessageAttachment {
        path: stored.path,
        mention: stored.name.clone(),
        name: stored.name,
        is_dir: false,
        is_image: false,
        blob_reference: Some(stored.reference),
    };
    let input_data = f.store.snapshot_inputs(&store, &[input.clone()]).unwrap();
    f.run = f
        .store
        .prepare(
            PrepareRun {
                id: Uuid::new_v4(),
                root_task_id: Uuid::new_v4(),
                project_id: f.run.project_id,
                team_id: f.run.team.id,
                expected_definition_revision: f.control.load().unwrap().revision,
                objective: "Review attached requirements".into(),
                limits: f.run.limits.clone(),
                access: f.run.access.clone(),
                input_data,
                source_attachments: vec![input.clone()],
                source_prompt: None,
                origin: None,
            },
            crate::model::unix_time(),
        )
        .unwrap();
    // Changing the original managed file cannot change what either model reads.
    fs::write(&input.path, "Newer upload, not this run").unwrap();
    let root_id = f.run.work[0].id;
    let manager_id = delegate_and_wait(
        &f,
        f.claim(root_id),
        f.agents[1],
        "model-0",
        "private-instructions-0",
    );
    let mut step = 0;
    let mut key = String::new();
    let mut artifact = Uuid::nil();
    assert_eq!(f.worker(NativeTeamStart::Fresh(f.claim(manager_id)),|model,system,messages,tools| {
        assert_eq!(model,"model-1"); assert!(system.contains("immutable run inputs"));
        assert!(tools.as_array().unwrap().iter().any(|t|t["function"]["name"]=="read_input"));
        step+=1;
        Ok(match step {
            1=>call("list_inputs",json!({})),
            2=>{let reply=last_reply(messages); key=reply["inputs"][0]["key"].as_str().unwrap().to_owned(); assert_eq!(reply["next_offset"],Value::Null); call("read_input",json!({"key":key}))},
            3=>{assert_eq!(last_reply(messages)["content"],"Actual user requirements\n"); call("save_document",json!({"name":"review.md","format":"md","content":"Reviewed actual attached requirements."}))},
            _=>{artifact=Uuid::parse_str(last_reply(messages)["artifact_id"].as_str().unwrap()).unwrap();call("report_result",json!({"summary":"Reviewed input","artifact_ids":[artifact]}))},
        })
    }).unwrap(),NativeTeamExit::Settled);
    let continuation = f
        .store
        .resume_after_children(f.run.id, root_id, crate::model::unix_time(), 180)
        .unwrap();
    assert_eq!(continuation.children[0].artifact_ids, vec![artifact]);
    let mut step = 0;
    assert_eq!(f.worker(NativeTeamStart::Resumed(continuation),|_,_,messages,_| {
        step+=1;
        Ok(match step {
            1=>call("read_input",json!({"key":key})),
            2=>{assert_eq!(last_reply(messages)["content"],"Actual user requirements\n"); call("read_document",json!({"artifact_id":artifact}))},
            _=>{assert_eq!(last_reply(messages)["content"],"Reviewed actual attached requirements.");call("report_result",json!({"summary":"Accepted child result against user input","artifact_ids":[artifact]}))},
        })
    }).unwrap(),NativeTeamExit::Settled);
    assert_eq!(
        f.store.snapshot(f.run.id).unwrap().status,
        TeamRunStatus::Completed
    );
    assert_eq!(f.artifacts().len(), 1);
}

#[test]
fn native_team_followup_racing_completion_reaches_next_model_request_before_result() {
    let f = Fixture::new(false);
    let root = f.claim(f.run.work[0].id);
    let attempt = root.lease.attempt_id();
    let message = QueuedMessage::new("Also include the API evidence");
    let mut step = 0;
    let mut artifact = Uuid::nil();
    let exit=f.worker(NativeTeamStart::Fresh(root),|model,_,messages,_| {
        assert_eq!(model,"model-0"); step+=1;
        Ok(match step {
            1 => call("save_document",json!({"name":"review.md","format":"md","content":"Initial review"})),
            2 => {
                artifact=Uuid::parse_str(last_reply(messages)["artifact_id"].as_str().unwrap()).unwrap();
                // The user sends new instructions while this old completion request is in flight.
                f.store.queue_followup(f.run.id,f.run.root_task_id,message.clone(),crate::model::unix_time()).unwrap();
                call("report_result",json!({"summary":"Old result","artifact_ids":[artifact]}))
            },
            3 => {
                let received:Vec<Value>=messages.iter().filter(|m|m["role"]=="user")
                    .filter_map(|m|serde_json::from_str(m["content"].as_str()?).ok()).collect();
                assert!(received.iter().any(|m|m["user_follow_up"]==json!(message.id) && m["message"]==message.content));
                let error=last_reply(messages).to_string(); assert!(error.contains("queued user follow-up"));
                call("save_document",json!({"name":"api-evidence.md","format":"md","content":"API evidence included"}))
            },
            4 => {
                let evidence=last_reply(messages)["artifact_id"].clone();
                call("report_result",json!({"summary":"User follow-up addressed","artifact_ids":[artifact,evidence]}))
            },
            _ => panic!("Unexpected follow-up inference")
        })
    }).unwrap();
    assert_eq!(exit, NativeTeamExit::Settled);
    assert_eq!(step, 4);
    let done = f.store.snapshot(f.run.id).unwrap();
    assert_eq!(done.status, TeamRunStatus::Completed);
    assert_eq!(done.follow_ups[0].status, TeamFollowUpStatus::Delivered);
    assert_eq!(done.follow_ups[0].attempt_id, Some(attempt));
    assert_eq!(done.usage.model_calls, 4);
    assert_eq!(done.work[0].attempts.len(), 1);
}

#[test]
fn native_team_followup_queued_before_start_is_present_once_in_first_request() {
    let f = Fixture::new(false);
    let message = QueuedMessage::new("Use the supplied format");
    f.store
        .queue_followup(
            f.run.id,
            f.run.root_task_id,
            message.clone(),
            crate::model::unix_time(),
        )
        .unwrap();
    let root = f.claim(f.run.work[0].id);
    let mut step = 0;
    f.worker(NativeTeamStart::Fresh(root),|_,_,messages,_| {
        let count=messages.iter().filter(|m|m["role"]=="user").filter_map(|m|serde_json::from_str::<Value>(m["content"].as_str()?).ok()).filter(|m|m["user_follow_up"]==json!(message.id)).count();
        assert_eq!(count,1); step+=1;
        Ok(if step==1 {call("save_document",json!({"name":"format.md","format":"md","content":"Requested format"}))}
        else {call("report_result",json!({"summary":"Format included","artifact_ids":[last_reply(messages)["artifact_id"]]}))})
    }).unwrap();
    assert_eq!(step, 2);
    assert_eq!(f.store.snapshot(f.run.id).unwrap().follow_ups.len(), 1);
}

#[test]
fn native_team_command_check_contract_survives_delegation_and_denies_forged_local_receipt() {
    let f = Fixture::new(true);
    let root = f.claim(f.run.work[0].id);
    let mut step = 0;
    let mut child = Uuid::nil();
    let command = json!({"name":"Unit check","argv":["/usr/bin/python3","check.py"],"inputPaths":[],"outputPaths":[],"timeoutSeconds":10});
    let exit=f.worker(NativeTeamStart::Fresh(root),|_,system,messages,tools| {
        assert!(system.contains("requiredCommands need verified execution receipts"));
        assert!(tools.as_array().unwrap().iter().any(|tool|tool["function"]["name"]=="delegate" && tool["function"]["parameters"]["properties"]["output_contract"]["properties"].get("requiredCommands").is_some()));
        step+=1;
        Ok(if step==1 {call("delegate",json!({"agent_id":f.agents[1],"objective":"Run a check","expected_output":"Actual receipt",
            "output_contract":{"requiredCommands":[command.clone()]}}))}
        else {child=Uuid::parse_str(last_reply(messages)["assignment_id"].as_str().unwrap()).unwrap();call("await_children",json!({"assignment_ids":[child]}))})
    }).unwrap();
    assert_eq!(exit, NativeTeamExit::Yielded);
    let worker = f.claim(child);
    assert_eq!(
        serde_json::to_value(&worker.assignment.output_contract.required_commands[0]).unwrap(),
        command
    );
    let mut step = 0;
    let mut artifact = Uuid::nil();
    let exit=f.worker(NativeTeamStart::Fresh(worker),|_,_,messages,_| {
        step+=1;
        Ok(match step {
            1=>call("save_document",json!({"name":"report.md","format":"md","content":"A proposed check is not an execution receipt."})),
            2=>{artifact=Uuid::parse_str(last_reply(messages)["artifact_id"].as_str().unwrap()).unwrap();call("report_result",json!({"summary":"Claimed test","artifact_ids":[artifact],"checks":["Unit check"],
                "executed_checks":[{"name":"Unit check","requestId":Uuid::new_v4()}]}))},
            _=>{assert!(last_reply(messages).to_string().contains("Local/fallback attempt"));call("report_blocker",json!({"reason":"This check requires configured remote execution"}))}
        })
    }).unwrap();
    assert_eq!(exit, NativeTeamExit::Settled);
    let snapshot = f.store.snapshot(f.run.id).unwrap();
    let work = snapshot.work.iter().find(|w| w.id == child).unwrap();
    assert_eq!(work.status, AssignmentStatus::Blocked);
    assert!(work.result.is_none());
}
