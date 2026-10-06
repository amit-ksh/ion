//! Human recovery uses cached RPC data; file probes and receipt parsing stay off frames.
use super::*;
use anyhow::Context as _;
use std::sync::Arc;
use waku_client::teams::{
    TeamFileRecovery, TeamRecoveryConfirmation as Confirmation, TeamRecoveryReview,
    TeamSourceChange,
};
#[derive(Default)]
pub(super) struct RecoveryUi {
    pub(super) review: Option<Arc<TeamRecoveryReview>>,
    files: Rc<Vec<Arc<FileRow>>>,
    apps: Rc<Vec<Arc<AppRow>>>,
    remote: Rc<Vec<Arc<RemoteRow>>>,
    expanded: HashSet<usize>,
    pub(super) busy: bool,
    generation: u64,
    pub(super) error: Option<String>,
    title: String,
    task: Option<Uuid>,
    host_uncertain: bool,
}
pub(super) struct FileRow {
    effect: TeamFileRecovery,
    diff: Arc<team_integration::SourceChangeRow>,
    stages: String,
}
pub(super) struct AppRow {
    id: Uuid,
    title: String,
    identity: String,
    arguments: String,
}
pub(super) struct RemoteRow {
    request: Uuid,
    identity: String,
    operation: String,
    status: String,
    can_bind: bool,
}
fn presentation(
    review: &TeamRecoveryReview,
) -> (Vec<Arc<FileRow>>, Vec<Arc<AppRow>>, Vec<Arc<RemoteRow>>) {
    let changes: Vec<_> = review
        .files
        .iter()
        .map(|f| TeamSourceChange {
            path: f.path.clone(),
            directory: f.operation == "createDirectory",
            before: f.before.clone(),
            after: f.after.clone(),
            edit_ids: vec![f.id],
        })
        .collect();
    let diffs = team_integration::presentation(&changes);
    let files = review
        .files
        .iter()
        .cloned()
        .zip(diffs)
        .map(|(effect, diff)| {
            let stages = effect.private_stages.join(", ");
            Arc::new(FileRow {
                effect,
                diff,
                stages,
            })
        })
        .collect();
    let apps = review
        .apps
        .iter()
        .map(|decision| {
            let (identity, arguments) = decision
                .app_action
                .as_ref()
                .map(|a| {
                    (
                        format!(
                            "{} · account {} · version {}",
                            a.grant.action, a.grant.account_id, a.grant.version
                        ),
                        a.arguments_json.clone(),
                    )
                })
                .unwrap_or(("App approval is missing its action".into(), String::new()));
            Arc::new(AppRow {
                id: decision.id,
                title: decision.title.clone(),
                identity,
                arguments,
            })
        })
        .collect();
    let remote = review
        .remote_calls
        .iter()
        .map(|c| {
            Arc::new(RemoteRow {
                request: c.request_id,
                identity: format!(
                    "Request {} · image {} · project {}",
                    c.request_id, c.image_id, c.project
                ),
                operation: c
                    .operation_id
                    .map(|id| format!("Operation {id}"))
                    .unwrap_or("Spawn acknowledgement missing".into()),
                status: match c.state.as_str() {
                    "submitting" => "Submission outcome unknown",
                    "running" => "Remote stop not verified",
                    "SUCCESS" => "Completed",
                    "FAILED" => "Failed",
                    "CANCELLED" => "Canceled",
                    "rejected" => "Rejected before launch",
                    _ => "Unknown status",
                }
                .into(),
                can_bind: c.can_bind,
            })
        })
        .collect();
    (files, apps, remote)
}
impl Waku {
    pub(super) fn clear_team_recovery(&mut self) {
        let generation = self.control_ui.recovery.generation.wrapping_add(1);
        self.control_ui.recovery = RecoveryUi {
            generation,
            ..Default::default()
        };
        self.control_ui.activity_revision += 1;
    }
    pub(super) fn request_team_recovery(&mut self, command: Rpc, cx: &mut Context<Self>) {
        if self.control_ui.recovery.busy {
            return;
        }
        self.clear_team_integration();
        let run = self.control_ui.selected_run;
        self.control_ui.recovery.generation = self.control_ui.recovery.generation.wrapping_add(1);
        let generation = self.control_ui.recovery.generation;
        self.control_ui.recovery.busy = true;
        self.control_ui.recovery.error = None;
        let client = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || -> anyhow::Result<_> {
                        let ResponsePayload::TeamRecovery { review, snapshot } =
                            client.request(Uuid::nil(), Uuid::nil(), command)?
                        else {
                            anyhow::bail!("Unexpected recovery response")
                        };
                        anyhow::ensure!(
                            review.run_id == snapshot.id && review.revision == snapshot.revision,
                            "Recovery snapshot identity changed"
                        );
                        let work = snapshot
                            .work
                            .iter()
                            .find(|w| w.id == review.assignment_id)
                            .context("Recovery assignment missing")?;
                        anyhow::ensure!(
                            work.attempts
                                .last()
                                .is_some_and(|a| a.id == review.attempt_id),
                            "Recovery attempt differs from snapshot"
                        );
                        let agent = snapshot
                            .profiles
                            .iter()
                            .find(|p| p.id == work.agent_id)
                            .context("Recovery agent missing")?;
                        let title = format!("{} · {}", agent.name, work.objective);
                        let task = work.task_id;
                        let host_uncertain = work.status == AssignmentStatus::Uncertain;
                        let (files, apps, remote) = presentation(&review);
                        Ok((
                            review,
                            snapshot,
                            files,
                            apps,
                            remote,
                            title,
                            task,
                            host_uncertain,
                        ))
                    })
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.control_ui.recovery.generation != generation
                    || this.control_ui.selected_run != run
                {
                    return;
                }
                this.control_ui.recovery.busy = false;
                match result {
                    Ok((review, snapshot, files, apps, remote, title, task, host_uncertain))
                        if Some(review.run_id) == run =>
                    {
                        this.control_ui.recovery.title = title;
                        this.control_ui.recovery.task = Some(task);
                        this.control_ui.recovery.host_uncertain = host_uncertain;
                        this.control_ui.recovery.review = Some(Arc::new(review));
                        this.control_ui.recovery.files = Rc::new(files);
                        this.control_ui.recovery.apps = Rc::new(apps);
                        this.control_ui.recovery.remote = Rc::new(remote);
                        this.control_ui.recovery.expanded.clear();
                        if this
                            .control_ui
                            .team_run
                            .as_ref()
                            .is_none_or(|old| old.revision <= snapshot.revision)
                        {
                            this.control_ui.team_run = Some(Rc::new(snapshot));
                        }
                        this.control_ui.activity_revision += 1;
                        this.refresh_team_activity(true, cx);
                    }
                    Ok(_) => {
                        this.control_ui.recovery.error =
                            Some("Recovery belongs to another run. Refresh the review.".into())
                    }
                    Err(error) => this.control_ui.recovery.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn confirm_team_recovery(&mut self, confirmation: Confirmation, cx: &mut Context<Self>) {
        let Some(review) = &self.control_ui.recovery.review else {
            return;
        };
        self.request_team_recovery(
            Rpc::ConfirmTeamRecovery {
                run_id: review.run_id,
                assignment_id: review.assignment_id,
                attempt_id: review.attempt_id,
                expected_revision: review.revision,
                confirmation,
            },
            cx,
        );
    }
    pub(super) fn team_recovery_rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        for (index, row) in self.control_ui.recovery.files.iter().enumerate() {
            rows.push(Row::RecoveryFile(index, row.clone()));
            if self.control_ui.recovery.expanded.contains(&index) {
                for line in &row.diff.lines {
                    rows.push(Row::SourceDiffLine(rows.len(), line.clone()));
                }
            }
        }
        rows.extend(
            self.control_ui
                .recovery
                .apps
                .iter()
                .cloned()
                .map(Row::RecoveryApp),
        );
        rows.extend(
            self.control_ui
                .recovery
                .remote
                .iter()
                .cloned()
                .map(Row::RecoveryRemote),
        );
        if rows.is_empty() {
            rows.push(Row::Empty(
                "No unresolved effect receipts for this attempt.",
            ));
        }
        rows
    }
    pub(super) fn render_team_recovery_header(&self, cx: &mut Context<Self>) -> Div {
        let r = self.control_ui.recovery.review.as_ref().unwrap();
        let theme = Theme::current(cx);
        let mut actions = div()
            .flex()
            .flex_wrap()
            .gap(px(8.0))
            .child(self.control_button(
                "team-recovery-back",
                "Back to assignments",
                cx,
                |this, _, cx| {
                    this.clear_team_recovery();
                    cx.notify();
                },
            ));
        let run = r.run_id;
        let work = r.assignment_id;
        if !self.control_ui.recovery.busy {
            actions = actions.child(self.control_button(
                "team-recovery-refresh",
                "Refresh recovery",
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
            if !r.host_stopped && r.environment == ExecutionEnvironment::NebiusSandbox {
                actions = actions.child(self.control_button(
                    "team-recovery-remote-stop",
                    "Verify sandbox stopped",
                    cx,
                    |this, _, cx| this.confirm_team_recovery(Confirmation::VerifyRemoteStopped, cx),
                ));
            } else if !r.host_stopped && self.control_ui.recovery.host_uncertain {
                actions = actions.child(self.control_button(
                    "team-recovery-host",
                    "Confirm previous host stopped",
                    cx,
                    |this, _, cx| this.confirm_team_recovery(Confirmation::HostStopped, cx),
                ));
            } else if r.can_prepare_continuation {
                actions = actions.child(self.control_button(
                    "team-recovery-prepare",
                    "Prepare continuation",
                    cx,
                    |this, _, cx| this.confirm_team_recovery(Confirmation::PrepareContinuation, cx),
                ));
            } else if let Some(snapshot) = &self.control_ui.team_run {
                if snapshot.work.iter().any(|w| w.id == work && w.can_resume) {
                    let revision = snapshot.revision;
                    actions = actions.child(self.control_button(
                        "team-recovery-continue",
                        "Continue from checkpoint",
                        cx,
                        move |this, _, cx| {
                            if !this.control_ui.activity_busy {
                                this.clear_team_recovery();
                                this.resume_workspace_assignment(run, work, revision, cx);
                            }
                        },
                    ));
                }
            }
        }
        if let Some(task) = self.control_ui.recovery.task {
            actions = actions.child(self.control_button(
                "team-recovery-task",
                "Open task",
                cx,
                move |this, _, cx| this.open_team_activity_task(task, cx),
            ));
        }
        if let Some(path) = r.workspace_path.clone() {
            actions = actions.child(self.control_button(
                "team-recovery-workspace",
                "Open workspace",
                cx,
                move |_, _, cx| crate::platform::reveal_in_file_manager(&path, cx),
            ));
        }
        let mut header = div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .py(px(12.0))
            .child(
                div()
                    .text_size(sp(14.0))
                    .font_weight(FontWeight::MEDIUM)
                    .child("Assignment recovery"),
            )
            .child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(format!(
                        "{} · {}",
                        r.host,
                        if r.host_stopped {
                            "Previous worker stopped"
                        } else if r.environment == ExecutionEnvironment::NebiusSandbox {
                            "Bind missing operations, then verify their terminal status"
                        } else if self.control_ui.recovery.host_uncertain {
                            "Stop the previous worker, then confirm its host has stopped"
                        } else {
                            "Recovery confirmation is unavailable in the current worker state"
                        }
                    )),
            )
            .child(
                div()
                    .text_size(sp(13.0))
                    .truncate()
                    .child(self.control_ui.recovery.title.clone()),
            )
            .child(actions);
        if self
            .control_ui
            .recovery
            .remote
            .iter()
            .any(|row| row.can_bind)
        {
            header = header.child(control_field(
                "Operation ID from Nebius",
                self.control_ui.recovery_operation.clone(),
                "team-recovery-operation",
            ));
        }
        if !r.apps.is_empty() && r.host_stopped {
            header = header.child(control_field(
                "Provider evidence for app confirmation",
                self.control_ui.recovery_evidence.clone(),
                "team-recovery-evidence",
            ));
        }
        if self.control_ui.recovery.busy {
            header = header.child(div().text_size(sp(12.5)).child("Updating recovery…"));
        }
        if let Some(error) = &self.control_ui.recovery.error {
            header = header.child(
                div()
                    .text_size(sp(13.0))
                    .text_color(theme.danger)
                    .child(error.clone()),
            );
        }
        header
    }
    pub(super) fn render_team_recovery_remote(
        &self,
        row: Arc<RemoteRow>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        let mut body = div()
            .id(SharedString::from(format!(
                "recovery-remote-{}",
                row.request
            )))
            .py(px(14.0))
            .border_b_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .gap(px(9.0))
            .child(
                div()
                    .text_size(sp(14.0))
                    .font_weight(FontWeight::MEDIUM)
                    .child(row.status.clone()),
            )
            .child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(row.identity.clone()),
            )
            .child(div().text_size(sp(12.5)).child(row.operation.clone()));
        if row.can_bind && !self.control_ui.recovery.busy {
            let request_id = row.request;
            body=body.child(self.control_button(format!("recovery-bind-{request_id}"),"Verify and bind operation",cx,move |this,_,cx| {
                let value=this.control_ui.recovery_operation.read(cx).content().trim().to_owned();
                match Uuid::parse_str(&value) {
                    Ok(operation_id) if !operation_id.is_nil()=>this.confirm_team_recovery(Confirmation::BindRemoteOperation{request_id,operation_id},cx),
                    _=>{this.control_ui.recovery.error=Some("Enter the operation UUID from Nebius, then verify and bind it.".into());cx.notify();}
                }
            })).child(div().text_size(sp(12.5)).text_color(theme.text_secondary)
                .child("Ion verifies this operation against the original input and image. Binding submits no work and does not confirm that it has stopped."));
        }
        body.into_any_element()
    }
    pub(super) fn render_team_recovery_file(
        &self,
        index: usize,
        row: Arc<FileRow>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        let effect = &row.effect;
        let id = effect.id;
        let mut actions = div().flex().flex_wrap().gap(px(8.0));
        if effect.operation != "createDirectory" {
            let expanded = self.control_ui.recovery.expanded.contains(&index);
            actions = actions.child(self.control_button(
                format!("recovery-diff-{index}"),
                if expanded {
                    "Hide changes"
                } else {
                    "Show changes"
                },
                cx,
                move |this, _, cx| {
                    if !this.control_ui.recovery.expanded.remove(&index) {
                        this.control_ui.recovery.expanded.insert(index);
                    }
                    this.control_ui.activity_revision += 1;
                    cx.notify();
                },
            ));
        }
        if self
            .control_ui
            .recovery
            .review
            .as_ref()
            .is_some_and(|r| r.host_stopped)
            && !self.control_ui.recovery.busy
        {
            actions = actions
                .child(self.control_button(
                    format!("recovery-applied-{id}"),
                    "Confirm applied",
                    cx,
                    move |this, _, cx| {
                        this.confirm_team_recovery(Confirmation::FileApplied { id }, cx)
                    },
                ))
                .child(self.control_button(
                    format!("recovery-restored-{id}"),
                    "Confirm restored",
                    cx,
                    move |this, _, cx| {
                        this.confirm_team_recovery(Confirmation::FileRestored { id }, cx)
                    },
                ));
        }
        let mut body = div()
            .id(SharedString::from(format!("recovery-file-{id}")))
            .py(px(14.0))
            .border_b_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .gap(px(9.0))
            .child(
                div()
                    .text_size(sp(14.0))
                    .font_weight(FontWeight::MEDIUM)
                    .child(effect.path.clone()),
            )
            .child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(format!("{} · unresolved receipt", effect.operation)),
            )
            .child(actions);
        if !row.stages.is_empty() {
            body=body.child(div().text_size(sp(12.5)).text_color(theme.text_secondary)
            .child(format!("Recovery stage paths: {}. Inspect and preserve any remaining bytes before clearing a stage and confirming.",row.stages)));
        }
        body.into_any_element()
    }
    pub(super) fn render_team_recovery_app(
        &self,
        row: Arc<AppRow>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        let id = row.id;
        let mut body=div().id(SharedString::from(format!("recovery-app-{id}"))).py(px(14.0)).border_b_1().border_color(theme.border).flex().flex_col().gap(px(9.0))
            .child(div().text_size(sp(14.0)).font_weight(FontWeight::MEDIUM).child(row.title.clone()))
            .child(div().text_size(sp(12.5)).text_color(theme.text_secondary).child(row.identity.clone()))
            .child(div().text_size(sp(12.5)).font_family(crate::md::render::MONO_FAMILY).child(row.arguments.clone()))
            .child(div().text_size(sp(12.5)).text_color(theme.text_secondary).child("Verify the outcome in the connected app. Your confirmation is recorded as user evidence; no app request is repeated."));
        if self
            .control_ui
            .recovery
            .review
            .as_ref()
            .is_some_and(|r| r.host_stopped)
            && !self.control_ui.recovery.busy
        {
            body = body.child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.0))
                    .child(self.control_button(
                        format!("recovery-app-success-{id}"),
                        "Confirm succeeded",
                        cx,
                        move |this, _, cx| {
                            let evidence = this
                                .control_ui
                                .recovery_evidence
                                .read(cx)
                                .content()
                                .to_owned();
                            this.confirm_team_recovery(
                                Confirmation::AppSucceeded {
                                    decision_id: id,
                                    evidence,
                                },
                                cx,
                            );
                        },
                    ))
                    .child(self.control_button(
                        format!("recovery-app-failure-{id}"),
                        "Confirm failed",
                        cx,
                        move |this, _, cx| {
                            let evidence = this
                                .control_ui
                                .recovery_evidence
                                .read(cx)
                                .content()
                                .to_owned();
                            this.confirm_team_recovery(
                                Confirmation::AppFailed {
                                    decision_id: id,
                                    evidence,
                                },
                                cx,
                            );
                        },
                    )),
            );
        }
        body.into_any_element()
    }
}
