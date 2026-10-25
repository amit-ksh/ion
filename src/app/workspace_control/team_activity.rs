//! Background run collection and cached native activity projections.
use super::*;

#[derive(Clone)]
pub(super) struct AssignmentRow {
    id: Uuid,
    task_id: Uuid,
    resume: Option<(Uuid, u64)>,
    recovery: Option<Uuid>,
    parent_task: Option<Uuid>,
    depth: usize,
    title: String,
    detail: String,
    reason: Option<String>,
    summary: Option<String>,
    review: Option<String>,
    documents: Vec<Artifact>,
}

/// Preorder keeps each manager's reports together. Bound malformed legacy graphs.
pub(super) fn hierarchy(items: &[(Uuid, Option<Uuid>)]) -> Vec<(Uuid, usize)> {
    let mut children: HashMap<Option<Uuid>, Vec<Uuid>> = HashMap::new();
    for (id, parent) in items {
        children.entry(*parent).or_default().push(*id);
    }
    let mut seen = HashSet::new();
    let mut output = Vec::with_capacity(items.len());
    let mut stack: Vec<_> = children
        .get(&None)
        .into_iter()
        .flatten()
        .rev()
        .map(|id| (*id, 0))
        .collect();
    while let Some((id, depth)) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        output.push((id, depth.min(32)));
        stack.extend(
            children
                .get(&Some(id))
                .into_iter()
                .flatten()
                .rev()
                .map(|id| (*id, depth + 1)),
        );
    }
    output.extend(
        items
            .iter()
            .filter(|(id, _)| seen.insert(*id))
            .map(|(id, _)| (*id, 0)),
    );
    output
}
fn status(status: AssignmentStatus) -> &'static str {
    match status {
        AssignmentStatus::Queued => "Queued",
        AssignmentStatus::Running => "Running",
        AssignmentStatus::Waiting => "Waiting for children",
        AssignmentStatus::InReview => "Awaiting parent review",
        AssignmentStatus::Completed => "Accepted",
        AssignmentStatus::Superseded => "Replaced by correction",
        AssignmentStatus::Blocked => "Blocked",
        AssignmentStatus::Failed => "Failed",
        AssignmentStatus::Uncertain => "Host outcome needs confirmation",
        AssignmentStatus::Canceling => "Stopping",
        AssignmentStatus::Canceled => "Stopped",
    }
}
fn run_status(status: TeamRunStatus) -> &'static str {
    match status {
        TeamRunStatus::Active => "Active",
        TeamRunStatus::Stopping => "Stopping",
        TeamRunStatus::Canceled => "Stopped",
        TeamRunStatus::Completed => "Completed",
    }
}
fn relevant(page: Option<Page>) -> bool {
    matches!(
        page,
        Some(
            Page::Teams
                | Page::Dashboard
                | Page::Inbox
                | Page::Decisions
                | Page::Artifacts
                | Page::Workflows
                | Page::Routines
        )
    )
}
impl Waku {
    pub(super) fn install_control_snapshot(&mut self, snapshot: ControlSnapshot) {
        if snapshot.revision < self.control_ui.snapshot.revision {
            return;
        }
        self.control_ui.team_index = Rc::new(
            snapshot
                .records
                .iter()
                .filter_map(|r| match r {
                    ControlRecord::Team(t) => Some(t.clone()),
                    _ => None,
                })
                .collect(),
        );
        let mut seen = HashSet::new();
        self.control_ui.duplicate_team_names = self
            .control_ui
            .team_index
            .iter()
            .filter_map(|t| {
                let name = t.name.trim().to_lowercase();
                (!seen.insert(name.clone())).then_some(name)
            })
            .collect();
        self.control_ui.team_names = seen;
        self.control_ui.snapshot = snapshot;
        self.control_ui.snapshot_loaded = true;
        if self.control_ui.form == Some(FormKind::Team) {
            // Refresh cached labels without replacing the unsaved membership draft.
            self.refresh_team_graph();
        }
    }
    pub(super) fn start_team_activity_poll(&mut self, cx: &mut Context<Self>) {
        if self.control_ui.activity_polling || !relevant(self.control_ui.page) {
            return;
        }
        self.control_ui.activity_polling = true;
        cx.spawn(async move |this, cx| {
            loop {
                smol::Timer::after(Duration::from_secs(3)).await;
                let keep = this
                    .update(cx, |this, cx| {
                        if !relevant(this.control_ui.page) {
                            this.control_ui.activity_polling = false;
                            return false;
                        }
                        this.refresh_team_activity(false, cx);
                        true
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        })
        .detach();
    }
    pub(super) fn refresh_team_activity(&mut self, replace_pending: bool, cx: &mut Context<Self>) {
        self.refresh_inbox(cx);
        if !relevant(self.control_ui.page) || (self.control_ui.activity_busy && !replace_pending) {
            return;
        }
        self.control_ui.activity_generation += 1;
        let generation = self.control_ui.activity_generation;
        let selected = self.control_ui.selected_run;
        let automation_selected = self.control_ui.automation_selected;
        let automation_definition =
            if matches!(self.control_ui.page, Some(Page::Workflows | Page::Routines)) {
                self.control_ui.automation_definition
            } else {
                None
            };
        let automation_after = self.control_ui.automation_after;
        let team = if self.control_ui.page == Some(Page::Teams) {
            self.control_ui.runs_team
        } else {
            None
        };
        let project = if matches!(
            self.control_ui.page,
            Some(Page::Teams | Page::Inbox | Page::Decisions)
        ) {
            self.control_ui.filter_project
        } else {
            None
        };
        let after = self.control_ui.runs_after;
        let decisions_after = self.control_ui.decisions_after;
        let pending_only = self.control_ui.page == Some(Page::Decisions)
            || (self.control_ui.page == Some(Page::Inbox)
                && self.control_ui.filter_status == NoticeStatus::Pending);
        let client = self.daemon.client();
        self.control_ui.activity_busy = true;
        cx.spawn(async move |this, cx| {
            let response = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || -> anyhow::Result<_> {
                        let ResponsePayload::TeamRuns { page } = client.request(
                            Uuid::nil(),
                            Uuid::nil(),
                            Rpc::ListTeamRuns {
                                project_id: project,
                                team_id: team,
                                after,
                                limit: 32,
                            },
                        )?
                        else {
                            anyhow::bail!("Unexpected team collection response");
                        };
                        let detail = if let Some(run_id) = selected {
                            match client.request(
                                Uuid::nil(),
                                Uuid::nil(),
                                Rpc::GetTeamRun { run_id },
                            )? {
                                ResponsePayload::TeamRun { snapshot } => Some(snapshot),
                                _ => anyhow::bail!("Unexpected team run response"),
                            }
                        } else {
                            None
                        };
                        let ResponsePayload::WorkspaceControl { snapshot } =
                            client.request(Uuid::nil(), Uuid::nil(), Rpc::LoadWorkspaceControl)?
                        else {
                            anyhow::bail!("Unexpected document response");
                        };
                        let ResponsePayload::TeamDecisions { page: decisions } = client.request(
                            Uuid::nil(),
                            Uuid::nil(),
                            Rpc::ListTeamDecisions {
                                project_id: project,
                                pending_only,
                                after: decisions_after,
                                limit: 64,
                            },
                        )?
                        else {
                            anyhow::bail!("Unexpected decision collection response");
                        };
                        let ResponsePayload::AutomationRuns { page: automations } = client
                            .request(
                                Uuid::nil(),
                                Uuid::nil(),
                                Rpc::ListAutomationRuns {
                                    pending_only,
                                    project_id: project,
                                    definition_id: automation_definition,
                                    after: automation_after,
                                    limit: 64,
                                },
                            )?
                        else {
                            anyhow::bail!("Unexpected automation collection response");
                        };
                        let automation_detail = if let Some(id) = automation_selected {
                            let ResponsePayload::AutomationRun { run } = client.request(
                                Uuid::nil(),
                                Uuid::nil(),
                                Rpc::GetAutomationRun { id },
                            )?
                            else {
                                anyhow::bail!("Unexpected automation detail response");
                            };
                            Some(run)
                        } else {
                            None
                        };
                        Ok((
                            page,
                            detail,
                            snapshot,
                            decisions,
                            automations,
                            automation_detail,
                        ))
                    })
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.control_ui.activity_generation != generation {
                    return;
                }
                this.control_ui.activity_busy = false;
                match response {
                    Ok((page, detail, control, decisions, automations, automation_detail)) => {
                        // No notifications for an unchanged poll; streaming keeps its own cadence.
                        let automation_changed = automations.next_cursor
                            != this.control_ui.automation_cursor
                            || automations.pending_count != this.control_ui.automation_pending
                            || automations
                                .runs
                                .iter()
                                .map(|r| (r.id, r.revision))
                                .collect::<Vec<_>>()
                                != this
                                    .control_ui
                                    .automation_runs
                                    .iter()
                                    .map(|r| (r.id, r.revision))
                                    .collect::<Vec<_>>()
                            || automation_detail.as_ref().map(|r| (r.id, r.revision))
                                != this
                                    .control_ui
                                    .automation_detail
                                    .as_ref()
                                    .map(|r| (r.id, r.revision));
                        this.control_ui.automation_cursor = automations.next_cursor;
                        this.control_ui.automation_pending = automations.pending_count;
                        this.control_ui.automation_runs = Rc::new(automations.runs);
                        this.control_ui.automation_detail = automation_detail.map(Rc::new);
                        let changed = automation_changed
                            || !this.control_ui.activity_loaded
                            || page.next_cursor != this.control_ui.runs_cursor
                            || page
                                .runs
                                .iter()
                                .map(|r| (r.id, r.revision))
                                .collect::<Vec<_>>()
                                != this
                                    .control_ui
                                    .team_runs
                                    .iter()
                                    .map(|r| (r.id, r.revision))
                                    .collect::<Vec<_>>()
                            || detail.as_ref().map(|r| (r.id, r.revision))
                                != this
                                    .control_ui
                                    .team_run
                                    .as_ref()
                                    .map(|r| (r.id, r.revision))
                            || control.revision > this.control_ui.snapshot.revision
                            || this.control_ui.activity_error.is_some()
                            || this.control_ui.pending_team_decisions != decisions.pending_count
                            || this.control_ui.decisions_cursor != decisions.next_cursor
                            || this
                                .control_ui
                                .team_decisions
                                .iter()
                                .map(|d| (d.id, d.revision))
                                .collect::<Vec<_>>()
                                != decisions
                                    .decisions
                                    .iter()
                                    .map(|d| (d.id, d.revision))
                                    .collect::<Vec<_>>();
                        this.control_ui.activity_error = None;
                        this.control_ui.activity_loaded = true;
                        this.control_ui.runs_cursor = page.next_cursor;
                        this.control_ui.team_runs = Rc::new(page.runs);
                        this.control_ui.pending_team_decisions = decisions.pending_count;
                        this.control_ui.decisions_cursor = decisions.next_cursor;
                        this.control_ui.team_decisions = Rc::new(decisions.decisions);
                        if this.control_ui.selected_run == selected {
                            this.control_ui.team_run = detail.map(Rc::new);
                        }
                        this.install_control_snapshot(control);
                        if changed {
                            this.control_ui.activity_revision += 1;
                            cx.notify();
                        }
                    }
                    Err(error) => {
                        let message = format!(
                            "Could not refresh team activity: {error}. Use Refresh to retry."
                        );
                        if this.control_ui.activity_error.as_ref() != Some(&message) {
                            this.control_ui.activity_error = Some(message);
                            cx.notify();
                        }
                    }
                }
            });
        })
        .detach();
    }
    pub(in crate::app) fn open_workspace_run(&mut self, run_id: Uuid, cx: &mut Context<Self>) {
        self.open_workspace_page(Page::Teams, cx);
        self.control_ui.runs_tab = true;
        self.control_ui.selected_run = Some(run_id);
        self.clear_team_integration();
        self.clear_team_recovery();
        self.control_ui.team_run = None;
        self.control_ui.activity_revision += 1;
        self.refresh_team_activity(true, cx);
        cx.notify();
    }
    fn stop_workspace_run(&mut self, run_id: Uuid, expected_revision: u64, cx: &mut Context<Self>) {
        self.control_ui.activity_generation += 1;
        let generation = self.control_ui.activity_generation;
        self.control_ui.activity_busy = true;
        let client = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || {
                        client.request(
                            Uuid::nil(),
                            Uuid::nil(),
                            Rpc::StopTeamRun {
                                run_id,
                                expected_revision,
                            },
                        )
                    })
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.control_ui.activity_generation != generation {
                    return;
                }
                this.control_ui.activity_busy = false;
                match result {
                    Ok(ResponsePayload::TeamRun { snapshot }) => {
                        if this.control_ui.selected_run == Some(run_id) {
                            this.control_ui.team_run = Some(Rc::new(snapshot));
                        }
                        this.control_ui.activity_revision += 1;
                    }
                    Ok(_) => {
                        this.control_ui.activity_error =
                            Some("Unexpected Stop response. Refresh the run.".into())
                    }
                    Err(error) => {
                        this.control_ui.activity_error = Some(format!(
                            "Could not stop the team: {error}. Refresh before retrying."
                        ))
                    }
                }
                this.refresh_team_activity(true, cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn resume_workspace_assignment(
        &mut self,
        run_id: Uuid,
        assignment_id: Uuid,
        expected_revision: u64,
        cx: &mut Context<Self>,
    ) {
        if self.control_ui.activity_busy {
            return;
        }
        self.control_ui.activity_error = None;
        self.control_ui.activity_generation += 1;
        let generation = self.control_ui.activity_generation;
        self.control_ui.activity_busy = true;
        let client = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || {
                        client.request(
                            Uuid::nil(),
                            Uuid::nil(),
                            Rpc::ResumeTeamAssignment {
                                assignment_id,
                                run_id,
                                expected_revision,
                            },
                        )
                    })
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.control_ui.activity_generation != generation {
                    return;
                }
                this.control_ui.activity_busy = false;
                match result {
                    Ok(ResponsePayload::TeamRun { snapshot }) => {
                        this.control_ui.activity_error = None;
                        if this.control_ui.selected_run == Some(run_id) {
                            this.control_ui.team_run = Some(Rc::new(snapshot));
                        }
                        this.control_ui.activity_revision += 1;
                    }
                    Ok(_) => {
                        this.control_ui.activity_error =
                            Some("Unexpected continuation response. Refresh the run.".into())
                    }
                    Err(error) => {
                        this.control_ui.activity_error = Some(format!(
                            "Could not continue the assignment: {error}. Refresh before retrying."
                        ))
                    }
                }
                if this.control_ui.activity_error.is_none() {
                    this.refresh_team_activity(true, cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn open_team_activity_task(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if self.state.sessions.iter().any(|s| s.id == id) {
            self.select_session(id, cx);
            return;
        }
        self.control_ui.activity_open_generation += 1;
        let generation = self.control_ui.activity_open_generation;
        let page = self.control_ui.page;
        let selected = self.control_ui.selected_run;
        let prior_session = self.state.selected_session;
        let daemon = self.daemon.clone();
        cx.spawn(async move |this, cx| {
            let response = cx
                .background_executor()
                .spawn(async move { waku_client::persistence::hydrate_session(&daemon, id) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.control_ui.activity_open_generation != generation
                    || this.control_ui.page != page
                    || this.control_ui.selected_run != selected
                    || this.state.selected_session != prior_session
                {
                    return;
                }
                match response {
                    Ok(Some(session)) => {
                        if !this.state.sessions.iter().any(|s| s.id == id) {
                            this.state.sessions.push(session);
                        }
                        this.select_session(id, cx);
                    }
                    Ok(None) => {
                        this.show_toast("The child task is being created. Refresh and try again.")
                    }
                    Err(error) => this.show_toast(format!("Could not open team task: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn team_activity_notices(&self) -> Vec<Notice> {
        let mut notices = self
            .control_ui
            .team_decisions
            .iter()
            .map(|decision| Notice {
                project: Some(decision.project_id),
                task: Some(decision.task_id),
                title: decision.title.clone(),
                detail: decision.proposal.clone(),
                kind: NoticeKind::Decision,
                pending: decision.status == waku_client::teams::TeamDecisionStatus::Pending,
                timestamp: decision.created_at,
                record: None,
                run: Some(decision.run_id),
                decision: Some(Rc::new(decision.clone())),
            })
            .collect::<Vec<_>>();
        notices.extend(self.control_ui.team_runs.iter().filter_map(|run| {
            let (kind, pending, title) = if run.needs_attention > 0 {
                (
                    NoticeKind::Issue,
                    true,
                    format!("{} needs attention", run.team_name),
                )
            } else if run.status == TeamRunStatus::Completed {
                (
                    NoticeKind::Completed,
                    false,
                    format!("{} completed", run.team_name),
                )
            } else {
                return None;
            };
            Some(Notice {
                project: Some(run.project_id),
                task: Some(run.root_task_id),
                title,
                detail: format!(
                    "{} · {} assignments · {} need attention",
                    run.objective, run.assignments, run.needs_attention
                ),
                kind,
                pending,
                timestamp: run.updated_at,
                record: None,
                run: Some(run.id),
                decision: None,
            })
        }));
        notices
    }
    pub(super) fn team_activity_rows(&self) -> Vec<Row> {
        if self.control_ui.selected_run.is_some() {
            let Some(run) = &self.control_ui.team_run else {
                return vec![Row::Empty("Loading team assignments…")];
            };
            if self.control_ui.recovery.review.is_some() {
                return self.team_recovery_rows();
            }
            if let Some(review) = &self.control_ui.integration {
                let mut rows: Vec<Row> = review
                    .conflicts
                    .iter()
                    .cloned()
                    .map(Row::SourceConflict)
                    .collect();
                for (index, change) in self.control_ui.integration_rows.iter().enumerate() {
                    rows.push(Row::SourceChange(index, change.clone()));
                    if self.control_ui.integration_expanded.contains(&index) {
                        for line in &change.lines {
                            let index = rows.len();
                            rows.push(Row::SourceDiffLine(index, line.clone()));
                        }
                    }
                }
                return rows;
            }
            let profiles: HashMap<_, _> = run.profiles.iter().map(|a| (a.id, a)).collect();
            let work: HashMap<_, _> = run.work.iter().map(|w| (w.id, w)).collect();
            let artifacts: HashMap<_, _> = self
                .control_ui
                .snapshot
                .records
                .iter()
                .filter_map(|r| match r {
                    ControlRecord::Artifact(a) => Some((a.id, a)),
                    _ => None,
                })
                .collect();
            let mut rows: Vec<Row> = run
                .decisions
                .iter()
                .rev()
                .map(|d| Row::TeamDecision(Rc::new(d.clone())))
                .collect();
            rows.extend(
                run.follow_ups
                    .iter()
                    .rev()
                    .map(|f| Row::TeamFollowUp(Rc::new(f.clone()))),
            );
            rows.extend(
                hierarchy(
                    &run.work
                        .iter()
                        .map(|w| (w.id, w.parent_id))
                        .collect::<Vec<_>>(),
                )
                .into_iter()
                .filter_map(|(id, depth)| {
                    let w = work.get(&id)?;
                    let profile = profiles.get(&w.agent_id)?;
                    let host = w
                        .attempts
                        .last()
                        .map(|a| {
                            format!(
                                "{}{}",
                                host_label(&a.host),
                                a.fallback_reason
                                    .as_ref()
                                    .map(|r| format!(" · Local fallback: {r}"))
                                    .unwrap_or_default()
                            )
                        })
                        .unwrap_or_else(|| profile.execution.environment.label().into());
                    Some(Row::Assignment(Rc::new(AssignmentRow {
                        id,
                        task_id: w.task_id,
                        resume: w.can_resume.then_some((run.id, run.revision)),
                        recovery: w.can_reconcile.then_some(run.id),
                        parent_task: w.parent_id.and_then(|id| work.get(&id)).map(|w| w.task_id),
                        depth,
                        title: format!("{} · {}", profile.name, w.objective),
                        detail: format!(
                            "{} · {} · {}{}",
                            status(w.status),
                            profile.provider.display_name(),
                            host,
                            if w.rework_of.is_some() {
                                " · Correction task"
                            } else {
                                ""
                            }
                        ),
                        reason: w.reason.clone(),
                        summary: w.result.as_ref().map(|r| r.summary.clone()),
                        review: w
                            .review
                            .as_ref()
                            .map(|r| format!("Parent review: {}", r.feedback)),
                        documents: w
                            .result
                            .as_ref()
                            .into_iter()
                            .flat_map(|r| &r.artifact_ids)
                            .filter_map(|id| artifacts.get(id).map(|a| (*a).clone()))
                            .collect(),
                    })))
                })
                .collect::<Vec<_>>(),
            );
            return rows;
        }
        let mut runs = self.control_ui.team_runs.iter().collect::<Vec<_>>();
        runs.sort_by_key(|run| std::cmp::Reverse(run.updated_at));
        if runs.is_empty() {
            vec![Row::Empty(if self.control_ui.activity_loaded {
                "Start a task with @teamname to see its delegated work here."
            } else {
                "Loading team runs…"
            })]
        } else {
            runs.into_iter()
                .map(|r| Row::TeamRun(Rc::new(r.clone())))
                .collect()
        }
    }
    pub(super) fn render_team_collection_pages(&self, cx: &mut Context<Self>) -> Div {
        let mut bar = div().flex().items_center().gap(px(8.0));
        if !self.control_ui.runs_previous.is_empty() {
            bar = bar.child(self.control_button(
                "activity-previous",
                "Previous team runs",
                cx,
                |this, _, cx| {
                    if !this.control_ui.activity_busy {
                        this.control_ui.runs_after = this.control_ui.runs_previous.pop().flatten();
                        this.refresh_team_activity(true, cx);
                    }
                },
            ));
        }
        if let Some(cursor) = self.control_ui.runs_cursor {
            bar = bar.child(self.control_button(
                "activity-next",
                "Next team runs",
                cx,
                move |this, _, cx| {
                    if !this.control_ui.activity_busy {
                        this.control_ui
                            .runs_previous
                            .push(this.control_ui.runs_after);
                        this.control_ui.runs_after = Some(cursor);
                        this.refresh_team_activity(true, cx);
                    }
                },
            ));
        }
        bar
    }
    pub(super) fn render_team_decision_pages(&self, cx: &mut Context<Self>) -> Div {
        let mut bar = div().flex().items_center().gap(px(8.0));
        if !self.control_ui.decisions_previous.is_empty() {
            bar = bar.child(self.control_button(
                "decisions-previous",
                "Previous decisions",
                cx,
                |this, _, cx| {
                    if !this.control_ui.activity_busy {
                        this.control_ui.decisions_after =
                            this.control_ui.decisions_previous.pop().flatten();
                        this.refresh_team_activity(true, cx);
                    }
                },
            ));
        }
        if let Some(cursor) = self.control_ui.decisions_cursor {
            bar = bar.child(self.control_button(
                "decisions-next",
                "Next decisions",
                cx,
                move |this, _, cx| {
                    if !this.control_ui.activity_busy {
                        this.control_ui
                            .decisions_previous
                            .push(this.control_ui.decisions_after);
                        this.control_ui.decisions_after = Some(cursor);
                        this.refresh_team_activity(true, cx);
                    }
                },
            ));
        }
        bar
    }
    fn answer_team_decision(
        &mut self,
        decision: waku_client::teams::TeamDecision,
        approve: bool,
        cx: &mut Context<Self>,
    ) {
        let client = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let response = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || {
                        client.request(
                            Uuid::nil(),
                            Uuid::nil(),
                            Rpc::ResolveTeamDecision {
                                decision_id: decision.id,
                                expected_revision: decision.revision,
                                approve,
                            },
                        )
                    })
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                match response {
                    Ok(ResponsePayload::TeamDecision { .. }) => {
                        this.refresh_team_activity(true, cx)
                    }
                    Ok(_) => {
                        this.show_toast("Unexpected decision response. Refresh before retrying.")
                    }
                    Err(error) => this.show_toast(format!(
                        "Could not answer this decision: {error}. Refresh before retrying."
                    )),
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn render_team_decision_row(
        &self,
        decision: Rc<waku_client::teams::TeamDecision>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use waku_client::teams::TeamDecisionStatus;
        let theme = Theme::workspace(cx);
        let id = decision.id;
        let project = self
            .state
            .projects
            .iter()
            .find(|p| p.id == decision.project_id)
            .map(|p| p.name.as_str())
            .unwrap_or("Project");
        let state = match decision.status {
            TeamDecisionStatus::Pending => "Needs your decision",
            TeamDecisionStatus::Approved => "Approved",
            TeamDecisionStatus::Declined => "Declined",
            TeamDecisionStatus::Canceled => "Canceled",
        };
        let pending = decision.status == TeamDecisionStatus::Pending;
        let task = decision.task_id;
        let run = decision.run_id;
        // Navigation stays quiet on the left; the verdict sits on the right,
        // with lime reserved for sending the work out.
        let mut actions = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(6.0))
            .child(self.control_button_tone(
                format!("decision-task-{id}"),
                "Open task",
                ControlTone::Ghost,
                cx,
                move |this, _, cx| this.open_team_activity_task(task, cx),
            ))
            .child(self.control_button_tone(
                format!("decision-run-{id}"),
                "Open run",
                ControlTone::Ghost,
                cx,
                move |this, _, cx| this.open_workspace_run(run, cx),
            ))
            .child(div().flex_1());
        if pending {
            for (approve, label) in [
                (false, "Decline"),
                (
                    true,
                    if decision.app_action.is_some() {
                        "Approve action"
                    } else {
                        "Approve proposal"
                    },
                ),
            ] {
                let decision = (*decision).clone();
                actions = actions.child(self.control_button_tone(
                    format!("decision-answer-{id}-{approve}"),
                    label,
                    if approve {
                        ControlTone::Primary
                    } else {
                        ControlTone::Secondary
                    },
                    cx,
                    move |this, _, cx| this.answer_team_decision(decision.clone(), approve, cx),
                ));
            }
        }
        let pill = match decision.status {
            TeamDecisionStatus::Pending => {
                status_pill("icons/alert.svg", state, theme.accent, theme.accent_soft)
            }
            TeamDecisionStatus::Approved => {
                status_pill("icons/check.svg", state, theme.accent, theme.accent_soft)
            }
            TeamDecisionStatus::Declined => {
                status_pill("icons/x.svg", state, theme.danger, theme.danger_soft)
            }
            TeamDecisionStatus::Canceled => {
                status_pill("icons/stop.svg", state, theme.text_secondary, theme.overlay)
            }
        };
        let short_id = id.simple().to_string()[..4].to_uppercase();
        div()
            .id(SharedString::from(format!("team-decision-{id}")))
            .mx(px(2.0))
            .mb(px(12.0))
            .rounded(px(14.0))
            .border_1()
            .border_color(if pending {
                theme.primary
            } else {
                theme.ticket_border
            })
            .bg(theme.ticket)
            .shadow(elevation(&theme, Elevation::Rest))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(38.0))
                    .px(px(16.0))
                    .rounded_t(px(13.0))
                    .when(pending, |stub| stub.bg(theme.accent_soft))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        div()
                            .font_family(crate::md::render::MONO_FAMILY)
                            .text_size(sp(11.5))
                            .text_color(theme.text_secondary)
                            .child(format!("#{short_id}")),
                    )
                    .child(pill)
                    .child(div().flex_1())
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(sp(12.0))
                            .text_color(theme.text_tertiary)
                            .child(format!("{project} · {}", decision.host)),
                    ),
            )
            .child(super::super::home::perforation(theme.border_strong))
            .child(
                div()
                    .px(px(16.0))
                    .pt(px(12.0))
                    .pb(px(12.0))
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(sp(15.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child(decision.title.clone()),
                    )
                    .child(
                        div()
                            .text_size(sp(13.5))
                            .line_height(sp(20.0))
                            .text_color(theme.text)
                            .child(decision.proposal.clone()),
                    )
                    .child(
                        div()
                            .font_family(crate::md::render::MONO_FAMILY)
                            .text_size(sp(11.0))
                            .text_color(theme.text_tertiary)
                            .child(format!(
                                "decision r{} · team definition r{}",
                                decision.revision, decision.definition_revision
                            )),
                    )
                    .child(div().mt(px(4.0)).child(actions)),
            )
            .into_any_element()
    }
    pub(super) fn render_team_view_tabs(&self, cx: &mut Context<Self>) -> Div {
        let theme = Theme::workspace(cx);
        let mut tabs = crate::ui::segmented_track(&theme);
        // Org chart and list are two views of the same definitions.
        for (id, label, runs, list) in [
            ("teams-chart", "Org chart", false, false),
            ("teams-definitions", "List", false, true),
            ("teams-runs", "Runs", true, false),
        ] {
            let active =
                self.control_ui.runs_tab == runs && (runs || self.control_ui.team_list == list);
            tabs = tabs.child(crate::ui::segment(id, label, active, &theme).on_activation(
                cx,
                move |this, _, cx| {
                    this.control_ui.runs_tab = runs;
                    if !runs {
                        this.control_ui.team_list = list;
                    }
                    this.control_ui.selected_run = None;
                    this.clear_team_integration();
                    this.clear_team_recovery();
                    this.control_ui.team_run = None;
                    this.control_ui.runs_team = None;
                    this.control_ui.runs_after = None;
                    this.control_ui.runs_previous.clear();
                    this.refresh_team_activity(true, cx);
                    cx.notify();
                },
            ));
        }
        tabs
    }
    pub(super) fn render_team_activity_header(&self, cx: &mut Context<Self>) -> Div {
        let theme = Theme::workspace(cx);
        let mut header = div().flex().flex_col().gap(px(12.0));
        let mut tabs = div().flex().flex_wrap().items_center().gap(px(8.0));
        if self.control_ui.runs_tab && self.control_ui.selected_run.is_none() {
            if self.control_ui.runs_team.is_some() {
                tabs = tabs.child(self.control_button(
                    "team-runs-all",
                    "All teams",
                    cx,
                    |this, _, cx| {
                        this.control_ui.runs_team = None;
                        this.control_ui.runs_after = None;
                        this.control_ui.runs_previous.clear();
                        this.refresh_team_activity(true, cx);
                    },
                ));
            }
            if !self.control_ui.runs_previous.is_empty() {
                tabs = tabs.child(self.control_button(
                    "team-runs-previous",
                    "Previous",
                    cx,
                    |this, _, cx| {
                        if !this.control_ui.activity_busy {
                            this.control_ui.runs_after =
                                this.control_ui.runs_previous.pop().flatten();
                            this.refresh_team_activity(true, cx);
                        }
                    },
                ));
            }
            if let Some(cursor) = self.control_ui.runs_cursor {
                tabs = tabs.child(self.control_button(
                    "team-runs-next",
                    "Next",
                    cx,
                    move |this, _, cx| {
                        if !this.control_ui.activity_busy {
                            this.control_ui
                                .runs_previous
                                .push(this.control_ui.runs_after);
                            this.control_ui.runs_after = Some(cursor);
                            this.refresh_team_activity(true, cx);
                        }
                    },
                ));
            }
        }
        if self.control_ui.runs_tab && self.control_ui.selected_run.is_none() {
            let projects = self.state.projects.clone();
            let selected = self.control_ui.filter_project;
            let label = projects
                .iter()
                .find(|p| Some(p.id) == selected)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "All projects".into());
            let menu = self.menu_handle("team-runs-project", cx);
            let weak = cx.entity().downgrade();
            tabs = tabs.child(dropdown_menu(
                self.control_button("team-runs-project-button", label, cx, |_, _, _| {}),
                "team-runs-project",
                &menu,
                MenuAlign::BelowLeft,
                move |_| {
                    std::iter::once((None, "All projects".to_owned()))
                        .chain(projects.iter().map(|p| (Some(p.id), p.name.clone())))
                        .map(|(id, label)| {
                            let weak = weak.clone();
                            MenuItem::new(label, move |_, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    this.control_ui.filter_project = id;
                                    this.control_ui.runs_after = None;
                                    this.control_ui.runs_previous.clear();
                                    this.refresh_team_activity(true, cx);
                                    cx.notify();
                                });
                            })
                            .selected(id == selected)
                        })
                        .collect()
                },
            ));
        }
        header = header.child(tabs);
        if let Some(run) = &self.control_ui.team_run {
            let id = run.id;
            let revision = run.revision;
            let mut actions = div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    self.control_button("team-root-task", "Open lead task", cx, {
                        let task = run.root_task_id;
                        move |this, _, cx| this.open_team_activity_task(task, cx)
                    }),
                );
            if run.status == TeamRunStatus::Active {
                // Background polls set activity_busy; Stop works regardless.
                actions = actions.child(self.control_button(
                    "team-stop",
                    "Stop team",
                    cx,
                    move |this, _, cx| this.stop_workspace_run(id, revision, cx),
                ));
            }
            if let Some(workspace) = &run.workspace {
                let path = workspace.path.clone();
                actions = actions.child(self.control_button(
                    "team-workspace-open",
                    "Open workspace",
                    cx,
                    move |_, _, cx| crate::platform::reveal_in_file_manager(&path, cx),
                ));
                header = header.child(
                    div()
                        .text_size(sp(12.5))
                        .text_color(theme.text_secondary)
                        .child(format!(
                            "{} · base {} · isolated team workspace",
                            workspace.branch, workspace.base_commit
                        )),
                );
            }
            if run.status == TeamRunStatus::Completed
                && run.workspace.is_some()
                && self.control_ui.integration.is_none()
            {
                actions = actions.child(self.control_button(
                    "team-source-review",
                    "Review changes",
                    cx,
                    move |this, _, cx| {
                        this.request_team_integration(
                            Rpc::PrepareTeamIntegration {
                                run_id: id,
                                expected_revision: revision,
                            },
                            cx,
                        )
                    },
                ));
            }
            if self.control_ui.recovery.review.is_some() {
                header = header.child(self.render_team_recovery_header(cx));
            }
            if self.control_ui.recovery.review.is_none() {
                if self.control_ui.recovery.busy {
                    header = header.child(div().text_size(sp(12.5)).child("Loading recovery…"));
                }
                if let Some(error) = &self.control_ui.recovery.error {
                    header = header.child(
                        div()
                            .text_size(sp(13.0))
                            .text_color(theme.danger)
                            .child(error.clone()),
                    );
                }
            }
            if self.control_ui.integration.is_some() {
                header = header.child(self.render_team_integration_header(cx));
            }
            let cap = run
                .limits
                .max_tokens
                .map(|n| super::super::usage_page::format_tokens_compact(n as f64))
                .unwrap_or_else(|| "No cap configured".into());
            header=header.child(div().flex().items_center().gap(px(12.0))
                .child(div().flex_1().min_w_0().text_size(sp(14.0)).font_weight(FontWeight::MEDIUM).child(format!("{} · {}",run.team.name,run_status(run.status))))
                .child(actions))
                .child(div().text_size(sp(12.5)).text_color(theme.text_secondary)
                    .child(format!("{} tokens used · {} reserved · {} without usage receipts · limit {} · {} / {} model calls",run.usage.consumed_tokens,run.usage.reserved_tokens,run.usage.unknown_calls,cap,run.usage.model_calls,run.limits.max_model_calls)));
        }
        header
    }
    pub(super) fn render_team_run_row(
        &self,
        run: Rc<TeamRunSummary>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::workspace(cx);
        let id = run.id;
        div().id(SharedString::from(format!("team-run-{id}"))).min_h(px(76.0)).py(px(14.0)).border_b_1().border_color(theme.border)
            .flex().items_center().gap(px(16.0)).child(icon("icons/fork.svg",18.0,theme.accent))
            .child(div().flex_1().min_w_0().flex().flex_col().gap(px(5.0))
                .child(div().text_size(sp(14.0)).font_weight(FontWeight::MEDIUM).truncate().child(format!("{} · {}",run.team_name,run.objective)))
                .child(div().text_size(sp(12.5)).text_color(theme.text_secondary).child(format!("{} · {} running · {} waiting / review · {} accepted · {} need attention",run_status(run.status),run.running,run.waiting,run.completed,run.needs_attention))))
            .child(self.control_button(format!("team-run-open-{id}"),"Open run",cx,move |this,_,cx|this.open_workspace_run(id,cx))).into_any_element()
    }
    pub(super) fn render_team_assignment_row(
        &self,
        row: Rc<AssignmentRow>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::workspace(cx);
        let task = row.task_id;
        let mut body = div()
            .id(SharedString::from(format!("team-work-{}", row.id)))
            .py(px(14.0))
            .pl(px((row.depth.min(8) * 20) as f32))
            .border_b_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .gap(px(7.0));
        let mut actions = div().flex().gap(px(8.0)).child(self.control_button(
            format!("assignment-open-{task}"),
            "Open task",
            cx,
            move |this, _, cx| this.open_team_activity_task(task, cx),
        ));
        if let Some(run) = row.recovery {
            let work = row.id;
            actions = actions.child(self.control_button(
                format!("assignment-recovery-{work}"),
                "Review recovery",
                cx,
                move |this, _, cx| {
                    this.request_team_recovery(
                        Rpc::GetTeamRecovery {
                            run_id: run,
                            assignment_id: work,
                        },
                        cx,
                    )
                },
            ));
        }
        if let Some((run, revision)) = row.resume {
            let work = row.id;
            actions = actions.child(self.control_button(
                format!("assignment-resume-{work}"),
                "Continue from checkpoint",
                cx,
                move |this, _, cx| this.resume_workspace_assignment(run, work, revision, cx),
            ));
        }
        if let Some(parent) = row.parent_task {
            actions = actions.child(self.control_button(
                format!("assignment-parent-{task}"),
                "Parent task",
                cx,
                move |this, _, cx| this.open_team_activity_task(parent, cx),
            ));
        }
        body = body
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(icon(
                        if row.depth == 0 {
                            "icons/target.svg"
                        } else {
                            "icons/corner-down-right.svg"
                        },
                        16.0,
                        theme.text_secondary,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(sp(14.0))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(row.title.clone()),
                    )
                    .child(actions),
            )
            .child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(row.detail.clone()),
            );
        if let Some(reason) = &row.reason {
            body = body.child(
                div()
                    .text_size(sp(13.0))
                    .text_color(theme.danger)
                    .child(reason.clone()),
            );
        }
        if let Some(summary) = &row.summary {
            body = body.child(
                div()
                    .text_size(sp(13.0))
                    .text_color(theme.text)
                    .child(summary.clone()),
            );
        }
        if let Some(review) = &row.review {
            body = body.child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(review.clone()),
            );
        }
        for document in &row.documents {
            let record = ControlRecord::Artifact(document.clone());
            body = body.child(
                self.control_button(
                    format!("assignment-document-{}-{}", row.id, document.id),
                    format!("{} · {}", document.name, document.format.to_uppercase()),
                    cx,
                    move |this, window, cx| {
                        if let ControlRecord::Artifact(artifact) = &record {
                            this.open_document_preview(artifact.clone(), window, cx);
                        }
                    },
                )
                .child(icon("icons/file.svg", 14.0, theme.text_secondary)),
            );
        }
        body.into_any_element()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hierarchy_keeps_direct_reports_with_their_manager() {
        let [lead, a, b, x, y] = std::array::from_fn(|_| Uuid::new_v4());
        assert_eq!(
            hierarchy(&[
                (x, Some(a)),
                (lead, None),
                (a, Some(lead)),
                (b, Some(lead)),
                (y, Some(b))
            ]),
            vec![(lead, 0), (a, 1), (x, 2), (b, 1), (y, 2)]
        );
        assert_eq!(hierarchy(&[(a, Some(b)), (b, Some(a))]).len(), 2);
    }
}

/// Attempt hosts are wire ids; people read the environment's name.
fn host_label(host: &str) -> String {
    match host {
        "local" => ExecutionEnvironment::Local.label().into(),
        "nebiusSandbox" | "nebius-sandbox" | "nebius_sandbox" => {
            ExecutionEnvironment::NebiusSandbox.label().into()
        }
        other => other.into(),
    }
}
