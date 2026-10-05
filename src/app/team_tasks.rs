//! Explicit team invocation in the existing New task composer.
use super::*;
use waku_client::{Command as Rpc, ResponsePayload, teams::mentions};

impl Waku {
    /// Button availability uses the cached name set, never a catalog scan on a frame.
    pub(super) fn composer_has_team_address(&self, cx: &App) -> bool {
        let text = self.composer.read(cx).content(cx).trim_start();
        let Some(address) = text.strip_prefix('@') else {
            return false;
        };
        if address.starts_with("team:") {
            return true;
        }
        let name = if let Some(braced) = address.strip_prefix('{') {
            braced.split('}').next().unwrap_or_default()
        } else {
            address.split_whitespace().next().unwrap_or_default()
        };
        if !address.starts_with('{') && name.contains(['/', '\\']) {
            return false;
        }
        self.control_ui
            .team_names
            .contains(&name.trim().to_lowercase())
    }

    /// Return true when this submission belongs to Teams or needs a Teams error.
    /// Keep the input until the durable queue acknowledges it. Enter clears the
    /// field before emitting Submit, so every rejected path restores its text.
    pub(super) fn handle_team_composer_submission(
        &mut self,
        prompt: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let restore_error = |this: &mut Self, message: &str, cx: &mut Context<Self>| {
            this.composer
                .update(cx, |input, cx| input.set_content(prompt, cx));
            this.show_toast(message);
            cx.notify();
        };
        if self
            .selected_session()
            .is_some_and(|s| s.team_assignment.is_some())
        {
            restore_error(
                self,
                "Team execution is not available yet. This task remains in the team queue.",
                cx,
            );
            return true;
        }
        if !prompt.trim_start().starts_with('@') {
            return false;
        }
        if !self.control_ui.snapshot_loaded {
            self.refresh_workspace_control(cx);
            restore_error(
                self,
                "Loading teams. Send again when suggestions appear.",
                cx,
            );
            return true;
        }
        let invocation = match mentions::resolve(prompt, &self.control_ui.team_index) {
            Ok(None) => return false,
            Ok(Some(invocation)) => invocation,
            Err(message) => {
                restore_error(self, &message, cx);
                return true;
            }
        };
        let Some(session) = self.selected_session().cloned() else {
            return true;
        };
        if !session.detail_loaded
            || session.has_started()
            || !session.turns.is_empty()
            || session.is_busy()
            || self.submission_preparations.contains(&session.id)
        {
            restore_error(
                self,
                "Use New task to invoke a team. Agents can invoke teams through their supported tools.",
                cx,
            );
            return true;
        }
        if !self.composer_attachments.is_empty() {
            restore_error(
                self,
                "Team attachment handoffs are not available yet. Keep the files in the project and describe them in the task.",
                cx,
            );
            return true;
        }
        if !prompt.trim_start().starts_with("@team:") && self.mention_file_index_loading {
            restore_error(
                self,
                "Checking file mentions. Choose the team from @ suggestions to address it explicitly.",
                cx,
            );
            return true;
        }
        // A bare name that is also a workspace path must be qualified explicitly.
        if !prompt.trim_start().starts_with("@team:")
            && self.mention_file_index.iter().any(|f| {
                prompt
                    .trim_start()
                    .strip_prefix('@')
                    .is_some_and(|s| s.split_whitespace().next() == Some(f.path.as_str()))
            })
        {
            restore_error(
                self,
                "This mention matches a team and a file. Choose the team from @ suggestions.",
                cx,
            );
            return true;
        }
        let Some(project) = self
            .state
            .projects
            .iter()
            .find(|p| p.id == session.project_id)
            .cloned()
        else {
            restore_error(self, "Choose a project before invoking a team.", cx);
            return true;
        };
        let session_id = session.id;
        let revision = self.control_ui.snapshot.revision;
        let source_prompt = prompt.to_owned();
        let draft_key = self.selected_composer_draft_key();
        self.composer
            .update(cx, |input, cx| input.set_content(prompt, cx));
        self.submission_preparations.insert(session_id);
        let client = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let source = source_prompt.clone();
            let response = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || -> anyhow::Result<_> {
                        // Blank sessions are deliberately absent from task storage.
                        // Persist the project before the daemon creates this root.
                        client.request(
                            Uuid::nil(),
                            Uuid::nil(),
                            Rpc::SaveTaskState {
                                projects: vec![project.clone()],
                                live_session_ids: vec![],
                                sessions: vec![],
                            },
                        )?;
                        client.request(
                            session_id,
                            Uuid::nil(),
                            Rpc::QueueTeamTask {
                                run_id: session_id,
                                project_id: project.id,
                                expected_definition_revision: revision,
                                team_id: invocation.team_id,
                                source_prompt: source,
                            },
                        )
                    })
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.submission_preparations.remove(&session_id);
                match response {
                    Ok(ResponsePayload::TeamTaskQueued { session, .. }) => {
                        let same_input = this.state.selected_session == Some(session_id)
                            && this.composer.read(cx).content(cx) == source_prompt;
                        if let Some(key) = draft_key
                            && (same_input
                                || (this.state.selected_session != Some(session_id)
                                    && this.composer_drafts.get(key).is_some_and(|draft| {
                                        draft.text.trim() == source_prompt.trim()
                                    })))
                        {
                            this.remove_composer_draft(key, cx);
                        }
                        if same_input {
                            this.composer.update(cx, |input, cx| input.clear(cx));
                        }
                        if let Some(existing) = this.state.session_mut(session_id) {
                            *existing = session;
                        }
                        // A response arriving after local removal must not resurrect the task.
                        if this.state.selected_session == Some(session_id) {
                            this.reset_visible_state();
                            this.reset_transcript_rows(this.transcript_row_count());
                        }
                        this.show_toast("Team task queued. Team execution is not available yet.");
                    }
                    Ok(_) => this
                        .show_toast("Unexpected Teams response. Refresh the task before retrying."),
                    Err(error) => this.show_toast(format!(
                        "Could not queue team: {error}. Refresh Tasks before retrying."
                    )),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
        true
    }

    pub(super) fn render_team_task_status(&self, cx: &App) -> Option<AnyElement> {
        let binding = self.selected_session()?.team_assignment.as_ref()?;
        let theme = Theme::current(cx);
        Some(
            div()
                .px(px(24.0))
                .py(px(10.0))
                .flex()
                .gap(px(8.0))
                .items_center()
                .text_size(sp(12.5))
                .text_color(theme.text_secondary)
                .child(icon("icons/fork.svg", 14.0, theme.text_secondary))
                .child(format!(
                    "{} · Team task · Execution not available yet",
                    binding.team_name
                ))
                .into_any_element(),
        )
    }
}
