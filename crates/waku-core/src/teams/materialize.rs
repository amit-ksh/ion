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
    session.team_assignment = Some(task.binding.clone());
    session.team_assignment_status = Some(AssignmentStatus::Queued);
    // A materialized assignment is an idle record, not an execution authorization.
    session.runtime_mode = RuntimeMode::Ask;
    session.title = task.assignment.objective.chars().take(120).collect();
    session.messages.push(Message::new(
        MessageRole::User,
        task.source_prompt.clone().unwrap_or_else(|| {
            format!(
                "You are {}. Expertise: {}.\n{}\n\nAssignment:\n{}\n\nExpected output:\n{}",
                task.profile.name,
                task.profile.expertise,
                task.profile.instructions,
                task.assignment.objective,
                task.assignment.expected_output,
            )
        }),
    ));
    session.messages.last_mut().unwrap().attachments = task.source_attachments.clone();
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
        self.materialize(self.pending_materializations(128)?, task_store, state, now)
    }
    pub fn materialize_run(
        &self,
        run: Uuid,
        task_store: &StateStore,
        state: &mut PersistedState,
        now: u64,
    ) -> Result<usize> {
        self.materialize(
            self.pending_materializations_for_run(Some(run), 128)?,
            task_store,
            state,
            now,
        )
    }
    fn materialize(
        &self,
        pending: Vec<TaskMaterialization>,
        task_store: &StateStore,
        state: &mut PersistedState,
        now: u64,
    ) -> Result<usize> {
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
                .iter_mut()
                .find(|s| s.id == task.assignment.task_id)
            {
                // Binding lives in session detail. Hydrate before adopting it so
                // replay persists the binding without losing an older transcript.
                task_store.hydrate(existing)?;
                if existing.detail_loaded
                    && !existing.has_started()
                    && existing.turns.is_empty()
                    && existing.queued_messages.is_empty()
                    && existing.team_assignment.is_none()
                {
                    ensure!(
                        existing.project_id == task.project_id,
                        "Draft belongs to another project"
                    );
                    new_sessions.push(task_session(task));
                    continue;
                }
                ensure!(
                    existing.project_id == task.project_id
                        && existing.provider == task.profile.provider
                        && existing.model == task.profile.model
                        && existing
                            .team_assignment
                            .as_ref()
                            .is_none_or(|b| b == &task.binding),
                    "Materialized task no longer matches its pinned agent settings"
                );
            } else {
                new_sessions.push(task_session(task));
            }
        }
        for session in new_sessions {
            if let Some(existing) = state.sessions.iter_mut().find(|s| s.id == session.id) {
                let id = session.id;
                *existing = session;
                state.mark_session_dirty(id);
            } else {
                state.push_session(session);
            }
        }
        for task in &pending {
            if let Some(session) = state
                .sessions
                .iter_mut()
                .find(|s| s.id == task.assignment.task_id)
            {
                session.team_assignment = Some(task.binding.clone());
                state.mark_session_dirty(task.assignment.task_id);
            }
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
