//! Bridge the durable control outbox to the daemon's separate task database.
use super::*;
use crate::{
    model::{AgentSession, Message, MessageRole, RuntimeMode},
    persistence::{PersistedState, StateStore},
};

pub(super) fn task_session(task: &TaskMaterialization) -> AgentSession {
    let mut session = AgentSession::new(task.project_id, task.profile.provider);
    session.id = task.assignment.task_id;
    session.model = task.profile.model.clone();
    // A materialized assignment is an idle record, not an execution authorization.
    session.runtime_mode = RuntimeMode::Ask;
    session.title = task.assignment.objective.chars().take(120).collect();
    session.messages.push(Message::new(
        MessageRole::User,
        format!(
            "You are {}. Expertise: {}.\n{}\n\nAssignment:\n{}\n\nExpected output:\n{}",
            task.profile.name,
            task.profile.expertise,
            task.profile.instructions,
            task.assignment.objective,
            task.assignment.expected_output,
        ),
    ));
    session
}
impl TeamCoordinator {
    /// The owner daemon holds its task-state lock during this operation. Save
    /// tasks first, then ack the outbox. Reopening after either crash window
    /// reuses the allocated UUID and never replaces an existing transcript.
    pub fn materialize_pending(
        &self,
        task_store: &StateStore,
        state: &mut PersistedState,
        now: u64,
    ) -> Result<usize> {
        let pending = self.pending_materializations(128)?;
        if pending.is_empty() {
            return Ok(0);
        }
        let mut new_sessions = Vec::new();
        for task in &pending {
            ensure!(
                state.projects.iter().any(|p| p.id == task.project_id),
                "Team project no longer exists"
            );
            if let Some(existing) = state
                .sessions
                .iter()
                .find(|s| s.id == task.assignment.task_id)
            {
                ensure!(
                    existing.project_id == task.project_id
                        && existing.provider == task.profile.provider
                        && existing.model == task.profile.model,
                    "Materialized task no longer matches its pinned agent settings"
                );
            } else {
                new_sessions.push(task_session(task));
            }
        }
        for session in new_sessions {
            state.push_session(session);
        }
        task_store
            .save(state)
            .context("Could not persist team child tasks")?;
        for task in &pending {
            ensure!(
                task_store.contains_session(task.assignment.task_id)?,
                "Child task did not persist; leave its outbox pending"
            );
            self.acknowledge_materialized(
                task.run_id,
                task.assignment.id,
                task.assignment.task_id,
                now,
            )?;
        }
        Ok(pending.len())
    }
}
