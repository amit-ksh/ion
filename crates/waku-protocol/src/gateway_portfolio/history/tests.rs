use super::*;
fn run() -> AutomationRun {
    AutomationRun {
        id: Uuid::new_v4(),
        trigger: None,
        name: "Release".into(),
        project_id: Uuid::new_v4(),
        workflow_id: Some(Uuid::new_v4()),
        routine_id: None,
        definition_revision: 1,
        revision: 2,
        created_at: 100,
        scheduled_at: None,
        deadline: 1000,
        status: AutomationStatus::Completed,
        reason: Some("/private/source and provider body".into()),
        current_stage: 1,
        stages: vec![AutomationStage {
            inherit_workspace: false,
            workspace_from: None,
            retry: None,
            route: None,
            skipped: false,
            transition: WorkflowTransition::default(),
            title: "Build".into(),
            instructions: "/private/instructions".into(),
            required_formats: vec![],
            team_id: Uuid::new_v4(),
            team_run_id: Uuid::new_v4(),
            task_id: Uuid::new_v4(),
            requires_decision: false,
            approved: true,
            started: true,
            completed: true,
        }],
    }
}
fn evidence(run: &AutomationRun, content: &[u8]) -> (TeamRunSnapshot, ControlSnapshot, Uuid) {
    let stage = &run.stages[0];
    let id = Uuid::new_v4();
    let work = Uuid::new_v4();
    let agent = Uuid::new_v4();
    let artifact = Artifact {
        id,
        name: "Report".into(),
        project_id: Some(run.project_id),
        task_id: Some(stage.task_id),
        reference: format!("ion-document:{id}"),
        format: "md".into(),
        created_at: 110,
        evidence: Some(ArtifactEvidence::from_bytes(
            content,
            ArtifactProvenance {
                origin: ArtifactOrigin::TeamAssignment,
                run_id: Some(stage.team_run_id),
                assignment_id: Some(work),
                agent_id: Some(agent),
                source_path: Some("/private/path".into()),
                ..Default::default()
            },
        )),
    };
    let team = TeamRunSnapshot {
        workspace: None,
        id: stage.team_run_id,
        root_task_id: stage.task_id,
        project_id: run.project_id,
        definition_revision: 1,
        revision: 2,
        status: TeamRunStatus::Completed,
        team: Team {
            id: stage.team_id,
            name: "Team".into(),
            objective: "/private/objective".into(),
            limits: None,
            members: vec![],
        },
        profiles: vec![],
        limits: TeamRunLimits::default(),
        usage: TeamTokenUsage::default(),
        access: AgentAccess::default(),
        created_at: 100,
        deadline: 1000,
        source_prompt: Some("/private/source".into()),
        inputs: vec![],
        follow_ups: vec![],
        origin: None,
        decisions: vec![],
        work: vec![TeamAssignment {
            can_resume: false,
            can_reconcile: false,
            id: work,
            task_id: stage.task_id,
            parent_id: None,
            agent_id: agent,
            objective: "Private agent prompt".into(),
            expected_output: "Private output contract".into(),
            output_contract: OutputContract::default(),
            requires_review: false,
            rework_of: None,
            review: None,
            dependencies: vec![],
            access: AgentAccess::default(),
            status: AssignmentStatus::Completed,
            reason: None,
            attempts: vec![],
            result: Some(AssignmentResult {
                summary: "Private report".into(),
                artifact_ids: vec![id],
                checks: vec![],
                executed_checks: vec![],
            }),
        }],
    };
    (
        team,
        ControlSnapshot {
            revision: 1,
            records: vec![ControlRecord::Artifact(artifact)],
        },
        id,
    )
}
#[test]
fn workflow_history_queries_reject_effects_extra_fields_bad_digest_and_bounds() {
    for value in [
        json!({"kind":"workflowRun","query":{"runId":Uuid::new_v4(),"approved":true}}),
        json!({"kind":"workflowDocument","query":{"runId":Uuid::new_v4(),"artifactId":Uuid::new_v4(),"sha256":"a".repeat(64),"offset":0,"limit":1024,"path":"/private"}}),
    ] {
        assert!(serde_json::from_value::<PortfolioQuery>(value).is_err());
    }
    for (sha, limit, offset) in [
        ("bad".to_owned(), 1024, 0),
        ("a".repeat(64), 0, 0),
        ("a".repeat(64), 16385, 0),
        ("a".repeat(64), 1024, 16 * 1024 * 1024),
    ] {
        let query:PortfolioQuery=serde_json::from_value(json!({"kind":"workflowDocument","query":{"runId":Uuid::new_v4(),"artifactId":Uuid::new_v4(),"sha256":sha,"offset":offset,"limit":limit}})).unwrap();
        assert!(query.validate().is_err());
    }
}
#[test]
fn historical_run_projection_keeps_stage_facts_without_profile_paths_or_inactive_instructions() {
    let mut run = run();
    let page = history_result(
        AutomationPage {
            runs: vec![run.clone()],
            next_cursor: Some(run.id),
            pending_count: 50,
        },
        100_000,
    )
    .unwrap();
    assert_eq!(page["page"]["pendingCount"], 50);
    assert_eq!(page["page"]["nextCursor"], run.id.to_string());
    let detail = run_result(&run, 100_000).unwrap();
    assert_eq!(detail["page"]["stages"][0]["completed"], true);
    assert_eq!(detail["page"]["stages"][0]["review"], Value::Null);
    assert!(!detail.to_string().contains("/private"));
    run.status = AutomationStatus::WaitingDecision;
    run.current_stage = 0;
    run.stages[0].instructions = "😀".repeat(1001);
    let detail = run_result(&run, 100_000).unwrap();
    assert_eq!(detail["page"]["stages"][0]["review"]["complete"], false);
    assert_eq!(
        detail["page"]["stages"][0]["review"]["instructions"]
            .as_str()
            .unwrap()
            .chars()
            .count(),
        1000
    );
}
#[test]
fn evidence_is_bound_to_the_exact_started_stage_and_accepted_producer_without_paths() {
    let run = run();
    let (team, snapshot, id) = evidence(&run, b"report");
    let query = WorkflowEvidenceQuery {
        run_id: run.id,
        after: None,
        limit: 32,
    };
    let page = evidence_result(&run, &[team.clone()], &snapshot, &query, 100_000).unwrap();
    assert_eq!(page["page"]["documents"][0]["id"], id.to_string());
    assert!(!page.to_string().contains("/private"));
    let mut changed = team.clone();
    changed.work[0].status = AssignmentStatus::InReview;
    assert!(
        accepted_artifacts(&run, &[changed], &snapshot)
            .unwrap()
            .is_empty()
    );
    let mut changed = team.clone();
    changed.work[0].status = AssignmentStatus::Superseded;
    assert!(
        accepted_artifacts(&run, &[changed], &snapshot)
            .unwrap()
            .is_empty()
    );
    let mut changed = team.clone();
    changed.project_id = Uuid::new_v4();
    assert!(accepted_artifacts(&run, &[changed], &snapshot).is_err());
    let mut changed = team.clone();
    changed.id = Uuid::new_v4();
    assert!(accepted_artifacts(&run, &[changed], &snapshot).is_err());
    let mut other = run.clone();
    other.id = Uuid::new_v4();
    other.stages[0].team_run_id = Uuid::new_v4();
    assert!(accepted_artifacts(&other, &[team], &snapshot).is_err());
}
#[test]
fn binary_document_chunks_preserve_exact_bytes_and_digest_and_reject_substitution() {
    let run = run();
    let content: Vec<_> = (0..24 * 1024).map(|i| (i % 256) as u8).collect();
    let (team, snapshot, id) = evidence(&run, &content);
    let rows = accepted_artifacts(&run, &[team.clone()], &snapshot).unwrap();
    let mut query = WorkflowDocumentQuery {
        run_id: run.id,
        artifact_id: id,
        sha256: rows[0]["sha256"].as_str().unwrap().into(),
        offset: 0,
        limit: 16384,
    };
    let first =
        document_result(&run, &[team.clone()], &snapshot, &query, &content, 100_000).unwrap();
    let mut assembled = STANDARD
        .decode(first["page"]["data"].as_str().unwrap())
        .unwrap();
    assert_eq!(first["page"]["nextOffset"], 16384);
    query.offset = 16384;
    let second =
        document_result(&run, &[team.clone()], &snapshot, &query, &content, 100_000).unwrap();
    assembled.extend(
        STANDARD
            .decode(second["page"]["data"].as_str().unwrap())
            .unwrap(),
    );
    assert_eq!(assembled, content);
    assert_eq!(second["page"]["nextOffset"], Value::Null);
    let mut altered = content.clone();
    altered[0] ^= 1;
    assert!(document_result(&run, &[team.clone()], &snapshot, &query, &altered, 100_000).is_err());
    query.artifact_id = Uuid::new_v4();
    assert!(document_result(&run, &[team.clone()], &snapshot, &query, &content, 100_000).is_err());
}
