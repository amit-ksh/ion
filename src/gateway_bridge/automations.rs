use crate::credentials::{GatewayApi, GatewayAutomationAction};
use anyhow::{Result, bail};
use uuid::Uuid;
use waku_client::{Command, DaemonClient, DaemonRejection, ResponsePayload};

pub(super) fn sync(
    client: &DaemonClient,
    api: &GatewayApi,
    projects: &[crate::model::Project],
) -> Result<()> {
    let ResponsePayload::WorkspaceControl { snapshot } =
        client.request(Uuid::nil(), Uuid::nil(), Command::LoadWorkspaceControl)?
    else {
        bail!("Invalid gateway automation definitions");
    };
    let mut runs = Vec::new();
    let mut more = false;
    for pending_only in [true, false] {
        let ResponsePayload::AutomationRuns { page } = client.request(
            Uuid::nil(),
            Uuid::nil(),
            Command::ListAutomationRuns {
                pending_only,
                project_id: None,
                definition_id: None,
                after: None,
                limit: 32,
            },
        )?
        else {
            bail!("Invalid gateway automation history");
        };
        more |= page.next_cursor.is_some();
        for run in page.runs {
            if !runs
                .iter()
                .any(|known: &waku_protocol::control::AutomationRun| known.id == run.id)
            {
                runs.push(run);
            }
        }
    }
    api.publish_automations(waku_protocol::gateway::automation_projection(
        &snapshot, projects, &runs, more,
    ))?;
    for pending in api.pending_automations()? {
        if pending.status != "queued" || pending.publisher_id != api.identity().1 {
            continue;
        }
        let claimed = api.claim_automation(&pending.id)?;
        if claimed.status != "uncertain" || claimed.publisher_id != api.identity().1 {
            continue;
        }
        let command = match daemon_command(&claimed.action) {
            Ok(command) => command,
            Err(_) => {
                api.settle_automation(&claimed.id, false)?;
                continue;
            }
        };
        // The owner actor applies the revision check transactionally. A failed
        // transport acknowledgement is unknown, never an automatic replay.
        match client.request(Uuid::nil(), Uuid::nil(), command) {
            Ok(response) => {
                let applied = match (&claimed.action, response) {
                    (GatewayAutomationAction::Start { run_id, definition_id, expected_revision }, ResponsePayload::AutomationRun { run }) => {
                        run.id.to_string() == *run_id
                            && run.routine_id.or(run.workflow_id).is_some_and(|d| d.to_string() == *definition_id)
                            && run.definition_revision == *expected_revision
                    }
                    (GatewayAutomationAction::Stop { run_id, .. }, ResponsePayload::AutomationRun { run }) => {
                        run.id.to_string() == *run_id && run.status == waku_protocol::control::AutomationStatus::Canceled
                    }
                    (GatewayAutomationAction::Gate { run_id, stage, approved, .. }, ResponsePayload::AutomationRun { run }) => {
                        run.id.to_string() == *run_id && run.stages.get(usize::from(*stage)).is_some_and(|s|
                            if *approved { s.approved } else { run.current_stage == *stage && run.status == waku_protocol::control::AutomationStatus::Canceled })
                    }
                    (GatewayAutomationAction::Routine { definition_id, paused, expected_revision }, ResponsePayload::WorkspaceControl { snapshot }) => {
                        snapshot.revision > *expected_revision && snapshot.records.iter().any(|record|
                            matches!(record, waku_protocol::control::ControlRecord::Routine(r) if r.id.to_string() == *definition_id && r.paused == *paused))
                    }
                    _ => false,
                };
                if applied {
                    api.settle_automation(&claimed.id, true)?;
                }
                // An unexpected response also remains unknown; it is not evidence
                // that a possibly billable daemon effect failed.
            }
            Err(error) if error.downcast_ref::<DaemonRejection>().is_some() => {
                api.settle_automation(&claimed.id, false)?;
            }
            Err(_) => {}
        }
    }
    // Only a start has a unique durable daemon receipt (its supplied run UUID).
    // Reconcile that exact receipt after a lost acknowledgement or gateway
    // settlement, without issuing Start again. An absent receipt proves nothing.
    for uncertain in api.uncertain_automations()? {
        if uncertain.publisher_id != api.identity().1 || uncertain.status != "uncertain" {
            continue;
        }
        if let GatewayAutomationAction::Start {
            definition_id,
            run_id,
            expected_revision,
        } = &uncertain.action
        {
            if let Ok(id) = Uuid::parse_str(run_id)
                && let Ok(ResponsePayload::AutomationRun { run }) =
                    client.request(Uuid::nil(), Uuid::nil(), Command::GetAutomationRun { id })
                && run.id == id
                && run
                    .routine_id
                    .or(run.workflow_id)
                    .is_some_and(|d| d.to_string() == *definition_id)
                && run.definition_revision == *expected_revision
            {
                api.settle_automation(&uncertain.id, true)?;
            }
        }
    }
    Ok(())
}
fn daemon_command(action: &GatewayAutomationAction) -> Result<Command> {
    Ok(match action {
        GatewayAutomationAction::Start {
            definition_id,
            run_id,
            expected_revision,
        } => Command::StartAutomation {
            id: Uuid::parse_str(run_id)?,
            definition_id: Uuid::parse_str(definition_id)?,
            expected_definition_revision: *expected_revision,
        },
        GatewayAutomationAction::Stop {
            run_id,
            expected_revision,
        } => Command::StopAutomation {
            id: Uuid::parse_str(run_id)?,
            expected_revision: *expected_revision,
        },
        GatewayAutomationAction::Gate {
            run_id,
            expected_revision,
            stage,
            approved,
        } => Command::ResolveAutomationGate {
            id: Uuid::parse_str(run_id)?,
            expected_revision: *expected_revision,
            stage: *stage,
            approved: *approved,
        },
        GatewayAutomationAction::Routine {
            definition_id,
            expected_revision,
            paused,
        } => Command::SetRoutinePaused {
            id: Uuid::parse_str(definition_id)?,
            expected_revision: *expected_revision,
            paused: *paused,
        },
    })
}
