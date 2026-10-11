//! Outbound personal-gateway projection. Task prompts, local paths, agent access,
//! credentials and document bodies never enter this bounded mirror. A pending
//! stage includes its review instructions only.
use crate::control::{AutomationRun, AutomationStatus, ControlRecord, ControlSnapshot};
use crate::model::Project;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

fn label(value: &str) -> String {
    value.chars().take(80).collect()
}

pub fn automation_projection(
    snapshot: &ControlSnapshot,
    projects: &[Project],
    runs: &[AutomationRun],
    more_runs: bool,
) -> Value {
    let mut projection = json!({"version":1,"revision":snapshot.revision,"projects":[],"definitions":[],"runs":[],"truncated":more_runs});
    let names: HashMap<_, _> = projects.iter().map(|p| (p.id, label(&p.name))).collect();
    let teams: HashSet<_> = snapshot
        .records
        .iter()
        .filter_map(|r| match r {
            ControlRecord::Team(t) => Some(t.id),
            _ => None,
        })
        .collect();
    let workflows: HashMap<_, _> = snapshot
        .records
        .iter()
        .filter_map(|r| match r {
            ControlRecord::Workflow(w) => Some((w.id, w)),
            _ => None,
        })
        .collect();
    let ready = |w: &crate::control::Workflow| {
        !w.stages.is_empty()
            && w.stages.len() <= 24
            && w.stages
                .iter()
                .all(|s| s.team_id.is_some_and(|id| teams.contains(&id)))
    };
    let mut definitions = Vec::new();
    for record in &snapshot.records {
        let definition = match record {
            ControlRecord::Workflow(w) => {
                json!({"id":w.id,"projectId":w.project_id,"name":label(&w.name),"kind":"workflow","revision":snapshot.revision,"ready":ready(w),"paused":null,"nextRun":null,"schedule":null,"stageCount":w.stages.len()})
            }
            ControlRecord::Routine(r) => {
                let workflow = r.workflow_id.and_then(|id| workflows.get(&id).copied());
                let valid = r.team_id.is_some() != r.workflow_id.is_some()
                    && (r.team_id.is_some_and(|id| teams.contains(&id))
                        || workflow.is_some_and(|w| w.project_id == r.project_id && ready(w)));
                let schedule = if let Some(calendar) = &r.calendar {
                    {
                        let recurrence = match &calendar.recurrence {
                            crate::control::CalendarRecurrence::Daily => "Daily".to_owned(),
                            crate::control::CalendarRecurrence::Weekly { days } => days
                                .iter()
                                .filter_map(|d| {
                                    ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
                                        .get(usize::from(*d))
                                })
                                .copied()
                                .collect::<Vec<_>>()
                                .join(", "),
                            crate::control::CalendarRecurrence::Monthly { days } => format!(
                                "Monthly on {}",
                                days.iter()
                                    .map(u8::to_string)
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                        };
                        format!(
                            "{recurrence} at {:02}:{:02} · {}",
                            calendar.hour, calendar.minute, calendar.time_zone
                        )
                    }
                } else {
                    format!("Every {} minutes", r.interval_minutes)
                };
                json!({"id":r.id,"projectId":r.project_id,"name":label(&r.name),"kind":"routine","revision":snapshot.revision,"ready":valid,"paused":r.paused,"nextRun":r.next_run,"schedule":schedule,"stageCount":workflow.map_or(1,|w|w.stages.len())})
            }
            _ => continue,
        };
        if !names.contains_key(&match record {
            ControlRecord::Workflow(w) => w.project_id,
            ControlRecord::Routine(r) => r.project_id,
            _ => unreachable!(),
        }) || definition["stageCount"].as_u64().unwrap_or(25) > 24
            || definitions.len() >= 64
        {
            projection["truncated"] = json!(true);
            continue;
        }
        definitions.push(definition);
    }
    let mut summaries = Vec::new();
    for run in runs {
        if !names.contains_key(&run.project_id)
            || summaries.len() >= 64
            || run.stages.is_empty()
            || run.stages.len() > 24
        {
            projection["truncated"] = json!(true);
            continue;
        }
        let gate = if run.status == AutomationStatus::WaitingDecision {
            run.stages.get(usize::from(run.current_stage)).map(|stage| {
                // A partial instruction view cannot authorize the stage. The
                // user can still decline or stop it, or review on the desktop.
                json!({"index":run.current_stage,"title":label(&stage.title),"instructions":stage.instructions.chars().take(1000).collect::<String>(),"reviewComplete":stage.instructions.chars().count()<=1000})
            })
        } else {
            None
        };
        if run.status == AutomationStatus::WaitingDecision && gate.is_none() {
            projection["truncated"] = json!(true);
            continue;
        }
        summaries.push(json!({"id":run.id,"projectId":run.project_id,"name":label(&run.name),"definitionId":run.routine_id.or(run.workflow_id).unwrap_or(run.id),"revision":run.revision,"status":run.status,"currentStage":run.current_stage,"stageCount":run.stages.len(),"gate":gate}));
    }
    projection["definitions"] = json!(definitions);
    projection["runs"] = json!(summaries);
    // Names are included only for projects present in this page. Keep the whole
    // serialized request inside the gateway's 70 KB request boundary, including
    // UTF-8 expansion. Drop the last row, never parts of a review instruction.
    loop {
        let ids: HashSet<_> = projection["definitions"]
            .as_array()
            .unwrap()
            .iter()
            .chain(projection["runs"].as_array().unwrap())
            .filter_map(|v| v["projectId"].as_str())
            .collect();
        let mut selected: Vec<_> = projects
            .iter()
            .filter(|p| ids.contains(p.id.to_string().as_str()))
            .map(|p| json!({"id":p.id,"name":label(&p.name)}))
            .collect();
        selected.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
        projection["projects"] = json!(selected);
        if serde_json::to_vec(&projection).unwrap().len() <= 60_000
            && projection["projects"].as_array().unwrap().len() <= 64
        {
            break;
        }
        projection["truncated"] = json!(true);
        if !projection["runs"].as_array().unwrap().is_empty() {
            projection["runs"].as_array_mut().unwrap().pop();
        } else if !projection["definitions"].as_array().unwrap().is_empty() {
            projection["definitions"].as_array_mut().unwrap().pop();
        } else {
            break;
        }
    }
    projection
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::{Team, Workflow, WorkflowStage, WorkflowTransition};
    use uuid::Uuid;
    fn fixture() -> (ControlSnapshot, Vec<Project>, AutomationRun) {
        let project = Project {
            id: Uuid::new_v4(),
            name: "Ion".into(),
            path: "C:/private/owner-checkout".into(),
            created_at: 1,
        };
        let team = Team {
            id: Uuid::new_v4(),
            name: "Release team".into(),
            objective: "PRIVATE_TEAM_OBJECTIVE".into(),
            limits: None,
            members: vec![],
        };
        let workflow = Workflow {
            id: Uuid::new_v4(),
            name: "Release".into(),
            project_id: project.id,
            prompt: "PRIVATE_WORKFLOW_PROMPT".into(),
            limits: None,
            stages: vec![WorkflowStage {
                title: "Deploy".into(),
                instructions: "Exact stage request".into(),
                requires_decision: true,
                team_id: Some(team.id),
                required_formats: vec![],
                transition: WorkflowTransition::default(),
                inherit_workspace: false,
                retry: None,
            }],
        };
        let run: AutomationRun = serde_json::from_value(json!({"id":Uuid::new_v4(),"name":"Release","projectId":project.id,"workflowId":workflow.id,"routineId":null,"definitionRevision":3,"revision":4,"createdAt":1,"scheduledAt":null,"deadline":100,"status":"waitingDecision","reason":"PRIVATE_REASON","currentStage":0,"stages":[{"title":"Deploy","instructions":"Exact stage request","teamId":team.id,"teamRunId":Uuid::new_v4(),"taskId":Uuid::new_v4(),"requiresDecision":true,"approved":false,"started":false,"completed":false}]})).unwrap();
        (
            ControlSnapshot {
                revision: 3,
                records: vec![ControlRecord::Team(team), ControlRecord::Workflow(workflow)],
            },
            vec![project],
            run,
        )
    }
    #[test]
    fn mirror_keeps_only_explicit_review_content_and_metadata() {
        let (snapshot, projects, mut run) = fixture();
        let projection = automation_projection(&snapshot, &projects, &[run.clone()], false);
        assert_eq!(projection["definitions"][0]["ready"], true);
        assert_eq!(
            projection["runs"][0]["gate"]["instructions"],
            "Exact stage request"
        );
        let serialized = projection.to_string();
        for secret in [
            "PRIVATE_WORKFLOW_PROMPT",
            "PRIVATE_TEAM_OBJECTIVE",
            "PRIVATE_REASON",
            "owner-checkout",
            "teamRunId",
            "taskId",
        ] {
            assert!(!serialized.contains(secret), "{secret} leaked");
        }
        run.status = AutomationStatus::Running;
        let projection = automation_projection(&snapshot, &projects, &[run], false);
        assert!(projection["runs"][0]["gate"].is_null());
        assert!(!projection.to_string().contains("Exact stage request"));
    }
    #[test]
    fn partial_unicode_review_cannot_authorize_a_stage() {
        let (snapshot, projects, mut run) = fixture();
        run.stages[0].instructions = "😀".repeat(1001);
        let projection = automation_projection(&snapshot, &projects, &[run], false);
        let gate = &projection["runs"][0]["gate"];
        assert_eq!(gate["reviewComplete"], false);
        assert_eq!(gate["instructions"].as_str().unwrap().chars().count(), 1000);
        assert_eq!(
            gate["instructions"]
                .as_str()
                .unwrap()
                .encode_utf16()
                .count(),
            2000
        );
    }
    #[test]
    fn utf8_byte_cap_preserves_whole_rows_and_marks_missing_history() {
        let (snapshot, projects, run) = fixture();
        let runs: Vec<_> = (0..80)
            .map(|_| {
                let mut r = run.clone();
                r.id = Uuid::new_v4();
                r.stages[0].instructions = "😀".repeat(1000);
                r
            })
            .collect();
        let projection = automation_projection(&snapshot, &projects, &runs, true);
        assert!(serde_json::to_vec(&projection).unwrap().len() <= 60_000);
        assert_eq!(projection["truncated"], true);
        assert!(projection["runs"].as_array().unwrap().len() < 64);
        for r in projection["runs"].as_array().unwrap() {
            assert_eq!(r["gate"]["reviewComplete"], true);
            assert_eq!(
                r["gate"]["instructions"].as_str().unwrap().chars().count(),
                1000
            );
        }
    }
    #[test]
    fn deleted_projects_and_missing_teams_cannot_be_started_from_a_projection() {
        let (mut snapshot, projects, run) = fixture();
        snapshot
            .records
            .retain(|r| !matches!(r, ControlRecord::Team(_)));
        let projection = automation_projection(&snapshot, &projects, &[run.clone()], false);
        assert_eq!(projection["definitions"][0]["ready"], false);
        if let Some(ControlRecord::Workflow(workflow)) = snapshot.records.first_mut() {
            let stage = workflow.stages[0].clone();
            workflow.stages = vec![stage; 25];
        }
        let unsupported = automation_projection(&snapshot, &projects, &[], false);
        assert!(unsupported["definitions"].as_array().unwrap().is_empty());
        assert_eq!(unsupported["truncated"], true);
        let projection = automation_projection(&snapshot, &[], &[run], false);
        assert!(projection["definitions"].as_array().unwrap().is_empty());
        assert!(projection["runs"].as_array().unwrap().is_empty());
        assert_eq!(projection["truncated"], true);
    }
}
