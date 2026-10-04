//! Native workspace navigation and definition editor. All RPC, credential,
//! model and document work runs on workers; render reads in-memory snapshots.
use super::*;
use waku_client::control::*;
use waku_client::{Command as Rpc, ResponsePayload};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(super) enum Page {
    Dashboard,
    Inbox,
    Decisions,
    Tasks,
    Projects,
    Teams,
    Routines,
    Workflows,
    Artifacts,
    Apps,
    Harnesses,
    Usage,
}
impl Page {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Inbox => "Inbox",
            Self::Decisions => "Decisions",
            Self::Tasks => "Tasks",
            Self::Projects => "Projects",
            Self::Teams => "Teams",
            Self::Routines => "Routines",
            Self::Workflows => "Workflows",
            Self::Artifacts => "Artifacts",
            Self::Apps => "Apps",
            Self::Harnesses => "Harnesses",
            Self::Usage => "Usage",
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Self::Dashboard => "icons/gauge.svg",
            Self::Inbox => "icons/queue.svg",
            Self::Decisions => "icons/check.svg",
            Self::Tasks => "icons/list.svg",
            Self::Projects => "icons/folder.svg",
            Self::Teams => "icons/fork.svg",
            Self::Routines => "icons/rotate-cw.svg",
            Self::Workflows => "icons/git-branch.svg",
            Self::Artifacts => "icons/file.svg",
            Self::Apps => "icons/package.svg",
            Self::Harnesses => "icons/bot.svg",
            Self::Usage => "icons/chart-column.svg",
        }
    }
}
#[derive(Clone, Copy, PartialEq)]
enum FormKind {
    Agent,
    Team,
    Workflow,
    Routine,
}
pub(super) struct ControlUi {
    pub page: Option<Page>,
    pub snapshot: ControlSnapshot,
    pub busy: bool,
    generation: u64,
    editor_generation: u64,
    error: Option<String>,
    form: Option<FormKind>,
    edit_id: Option<Uuid>,
    name: Entity<TextInput>,
    instructions: Entity<TextInput>,
    expertise: Entity<TextInput>,
    model: Entity<TextInput>,
    interval: Entity<TextInput>,
    provider: ProviderKind,
    project: Option<Uuid>,
    members: Vec<TeamMember>,
    stages: Vec<(Entity<TextInput>, Entity<TextInput>, bool)>,
    pending_stages: Option<Vec<WorkflowStage>>,
    focus: FocusHandle,
    rows_key: u64,
    list: ListState,
    scrollbar: Rc<ScrollbarState>,
    rows: Rc<Vec<Row>>,
}
#[derive(Clone)]
enum Row {
    Task(Uuid),
    Project(Uuid),
    Record(ControlRecord),
    AgentInTeam(AgentProfile, usize),
    Account(String, String, String),
    Empty(&'static str),
}
impl ControlUi {
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        Self {
            page: Some(Page::Dashboard),
            snapshot: ControlSnapshot::default(),
            busy: false,
            generation: 0,
            editor_generation: 0,
            error: None,
            form: None,
            edit_id: None,
            name: cx.new(|cx| TextInput::new(window, cx).placeholder("Name")),
            instructions: cx.new(|cx| {
                TextInput::new(window, cx)
                    .multi_line()
                    .placeholder("Instructions")
            }),
            expertise: cx.new(|cx| TextInput::new(window, cx).placeholder("Expertise")),
            model: cx.new(|cx| TextInput::new(window, cx).placeholder("Use harness default")),
            interval: cx.new(|cx| TextInput::new(window, cx).placeholder("Interval in minutes")),
            provider: ProviderKind::Nebius,
            project: None,
            members: Vec::new(),
            stages: Vec::new(),
            pending_stages: None,
            focus: cx.focus_handle(),
            rows_key: 0,
            list: ListState::new(0, ListAlignment::Top, px(72.0)),
            scrollbar: ScrollbarState::new(),
            rows: Rc::new(Vec::new()),
        }
    }
}
impl Waku {
    fn import_control_artifact(&mut self, cx: &mut Context<Self>) {
        if self.control_ui.busy {
            return;
        }
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Add document".into()),
        });
        let revision = self.control_ui.snapshot.revision;
        let project_id = self.state.selected_project;
        let client = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let result = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || -> anyhow::Result<ResponsePayload> {
                        anyhow::ensure!(
                            std::fs::metadata(&path)?.len() <= 16 * 1024 * 1024,
                            "Choose a document smaller than 16 MB"
                        );
                        let name = path
                            .file_name()
                            .and_then(|v| v.to_str())
                            .unwrap_or("Document")
                            .to_owned();
                        let format = path
                            .extension()
                            .and_then(|v| v.to_str())
                            .unwrap_or("")
                            .to_lowercase();
                        client.request(
                            Uuid::nil(),
                            Uuid::nil(),
                            Rpc::ImportWorkspaceArtifact {
                                expected_revision: revision,
                                name,
                                format,
                                project_id,
                                task_id: None,
                                bytes: std::fs::read(path)?,
                            },
                        )
                    })
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(ResponsePayload::WorkspaceControl { snapshot }) => {
                        if snapshot.revision >= this.control_ui.snapshot.revision {
                            this.control_ui.snapshot = snapshot;
                        }
                    }
                    Err(error) => this.control_ui.error = Some(error.to_string()),
                    _ => this.control_ui.error = Some("Could not import document".into()),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn export_control_artifact(&mut self, record: ControlRecord, cx: &mut Context<Self>) {
        let ControlRecord::Artifact(artifact) = record else {
            return;
        };
        let receiver = cx.prompt_for_new_path(&PathBuf::from("."), Some(&artifact.name));
        let client = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = receiver.await else {
                return;
            };
            let result = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || -> anyhow::Result<()> {
                        let ResponsePayload::BlobData { bytes } = client.request(
                            Uuid::nil(),
                            Uuid::nil(),
                            Rpc::ReadWorkspaceArtifact { id: artifact.id },
                        )?
                        else {
                            anyhow::bail!("Could not read document")
                        };
                        std::fs::write(path, bytes)?;
                        Ok(())
                    })
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(()) => this.show_toast("Document saved"),
                    Err(error) => this.control_ui.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn control_button(
        &self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        let theme = Theme::current(cx);
        let action = Rc::new(action);
        let click = action.clone();
        div()
            .id(id.into())
            .tab_index(0)
            .tab_stop(true)
            .h(px(32.0))
            .px(px(12.0))
            .rounded(px(7.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .items_center()
            .gap(px(7.0))
            .text_size(sp(13.0))
            .text_color(theme.text)
            .cursor_default()
            .hover(|style| style.bg(theme.overlay_strong))
            .focus_visible(|style| style.border_color(theme.accent))
            .child(label.into())
            .on_click(cx.listener(move |this, _, window, cx| click(this, window, cx)))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    action(this, window, cx);
                    cx.stop_propagation();
                }
            }))
    }
    pub(super) fn open_workspace_page(&mut self, page: Page, cx: &mut Context<Self>) {
        self.settings_page = None;
        self.control_ui.editor_generation += 1;
        self.control_ui.page = Some(page);
        self.control_ui.form = None;
        self.control_ui.error = None;
        self.control_ui.list.scroll_to_reveal_item(0);
        self.refresh_workspace_control(cx);
        if page == Page::Apps {
            self.refresh_gateway_accounts(cx);
        }
        if page == Page::Usage {
            self.ensure_usage_history(false, cx);
        }
        if page == Page::Harnesses {
            self.refresh_provider_detection(None);
            self.refresh_credential_statuses(cx);
        }
        cx.notify();
    }
    pub(super) fn refresh_workspace_control(&mut self, cx: &mut Context<Self>) {
        self.control_request(Rpc::LoadWorkspaceControl, cx);
    }
    fn control_request(&mut self, command: Rpc, cx: &mut Context<Self>) {
        if self.control_ui.busy {
            return;
        }
        self.control_ui.generation += 1;
        let generation = self.control_ui.generation;
        let editor_generation = self.control_ui.editor_generation;
        let saving = matches!(&command, Rpc::SaveWorkspaceRecord { .. });
        let requested_edit = self.control_ui.edit_id;
        let requested_name = self.control_ui.name.read(cx).content().to_owned();
        let requested_prompt = if let Rpc::GenerateWorkflow { prompt, .. } = &command {
            Some(prompt.clone())
        } else {
            None
        };
        self.control_ui.busy = true;
        self.control_ui.error = None;
        let client = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let response = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || client.request(Uuid::nil(), Uuid::nil(), command)).await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.control_ui.generation != generation {
                    return;
                }
                this.control_ui.busy = false;
                match response {
                    Ok(ResponsePayload::WorkspaceControl { snapshot }) => {
                        if snapshot.revision>=this.control_ui.snapshot.revision {this.control_ui.snapshot = snapshot;}
                        if saving && this.control_ui.editor_generation==editor_generation {this.control_ui.form = None;}
                    }
                    Ok(ResponsePayload::WorkflowDraft {mut workflow}) => {
                        if this.control_ui.editor_generation==editor_generation && this.control_ui.page==Some(Page::Workflows) && requested_prompt.as_deref().is_none_or(|prompt|this.control_ui.instructions.read(cx).content()==prompt) {
                            if let Some(id)=requested_edit {workflow.id=id;}
                            if !requested_name.trim().is_empty(){workflow.name=requested_name;}
                            this.edit_control_record(ControlRecord::Workflow(workflow), cx);
                        }else {this.control_ui.error=Some("Workflow editor changed. Generate again from the current instructions.".into());}
                    }
                    Ok(_) => {
                        this.control_ui.error =
                            Some("Unexpected workspace response. Refresh and try again.".into())
                    }
                    Err(error) => this.control_ui.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn begin_control_form(&mut self, kind: FormKind, cx: &mut Context<Self>) {
        self.control_ui.editor_generation += 1;
        self.control_ui.form = Some(kind);
        self.control_ui.edit_id = None;
        self.control_ui.error = None;
        self.control_ui.members.clear();
        self.control_ui.stages.clear();
        self.control_ui.pending_stages = None;
        self.control_ui.project = self.state.selected_project;
        self.control_ui.provider = ProviderKind::Nebius;
        for input in [
            &self.control_ui.name,
            &self.control_ui.instructions,
            &self.control_ui.expertise,
            &self.control_ui.model,
            &self.control_ui.interval,
        ] {
            input.update(cx, |input, cx| input.set_content("", cx));
        }
        cx.notify();
    }
    fn add_control_stage(
        &mut self,
        title: String,
        instructions: String,
        gate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let title_input = cx.new(|cx| TextInput::new(window, cx).placeholder("Stage title"));
        title_input.update(cx, |input, cx| input.set_content(title, cx));
        let instructions_input =
            cx.new(|cx| TextInput::new(window, cx).placeholder("Stage instructions"));
        instructions_input.update(cx, |input, cx| input.set_content(instructions, cx));
        self.control_ui
            .stages
            .push((title_input, instructions_input, gate));
        cx.notify();
    }
    fn edit_control_record(&mut self, record: ControlRecord, cx: &mut Context<Self>) {
        let (kind, instructions) = match &record {
            ControlRecord::Agent(v) => (FormKind::Agent, v.instructions.clone()),
            ControlRecord::Team(v) => (FormKind::Team, v.objective.clone()),
            ControlRecord::Workflow(v) => (FormKind::Workflow, v.prompt.clone()),
            ControlRecord::Routine(v) => (FormKind::Routine, v.instructions.clone()),
            ControlRecord::Artifact(_) => return,
        };
        self.begin_control_form(kind, cx);
        self.control_ui.edit_id = Some(record.id());
        self.control_ui.name.update(cx, |input, cx| {
            input.set_content(record.name().to_owned(), cx)
        });
        self.control_ui
            .instructions
            .update(cx, |input, cx| input.set_content(instructions, cx));
        match record {
            ControlRecord::Agent(v) => {
                self.control_ui.provider = v.provider;
                self.control_ui
                    .expertise
                    .update(cx, |input, cx| input.set_content(v.expertise, cx));
                self.control_ui.model.update(cx, |input, cx| {
                    input.set_content(v.model.unwrap_or_default(), cx)
                });
            }
            ControlRecord::Team(v) => self.control_ui.members = v.members,
            ControlRecord::Routine(v) => {
                self.control_ui.project = Some(v.project_id);
                self.control_ui.provider = v.provider;
                self.control_ui.interval.update(cx, |input, cx| {
                    input.set_content(v.interval_minutes.to_string(), cx)
                });
            }
            ControlRecord::Workflow(v) => {
                self.control_ui.project = Some(v.project_id);
                // Window entities must be constructed in update_window, never a worker.
                self.control_ui.pending_stages = Some(v.stages);
            }
            _ => {}
        }
        cx.notify();
    }
    fn save_control_form(&mut self, cx: &mut Context<Self>) {
        let id = self.control_ui.edit_id.unwrap_or_else(Uuid::new_v4);
        let name = self.control_ui.name.read(cx).content().trim().to_owned();
        let instructions = self
            .control_ui
            .instructions
            .read(cx)
            .content()
            .trim()
            .to_owned();
        let Some(kind) = self.control_ui.form else {
            return;
        };
        let record = match kind {
            FormKind::Agent => ControlRecord::Agent(AgentProfile {
                id,
                name,
                expertise: self.control_ui.expertise.read(cx).content().to_owned(),
                instructions,
                provider: self.control_ui.provider,
                model: Some(self.control_ui.model.read(cx).content().trim().to_owned())
                    .filter(|v| !v.is_empty()),
            }),
            FormKind::Team => ControlRecord::Team(Team {
                id,
                name,
                objective: instructions,
                members: self.control_ui.members.clone(),
            }),
            FormKind::Workflow => {
                let Some(project_id) = self.control_ui.project else {
                    self.control_ui.error = Some("Choose a project.".into());
                    cx.notify();
                    return;
                };
                ControlRecord::Workflow(Workflow {
                    id,
                    name,
                    project_id,
                    prompt: instructions,
                    stages: self
                        .control_ui
                        .stages
                        .iter()
                        .map(|(title, instructions, gate)| WorkflowStage {
                            title: title.read(cx).content().to_owned(),
                            instructions: instructions.read(cx).content().to_owned(),
                            requires_decision: *gate,
                        })
                        .collect(),
                })
            }
            FormKind::Routine => {
                let Some(project_id) = self.control_ui.project else {
                    self.control_ui.error = Some("Choose a project.".into());
                    cx.notify();
                    return;
                };
                let Ok(interval_minutes) =
                    self.control_ui.interval.read(cx).content().trim().parse()
                else {
                    self.control_ui.error = Some("Enter the schedule interval in minutes.".into());
                    cx.notify();
                    return;
                };
                ControlRecord::Routine(Routine {
                    id,
                    name,
                    project_id,
                    instructions,
                    interval_minutes,
                    paused: true,
                    next_run: 0,
                    provider: self.control_ui.provider,
                })
            }
        };
        self.control_request(
            Rpc::SaveWorkspaceRecord {
                expected_revision: self.control_ui.snapshot.revision,
                record,
            },
            cx,
        );
    }
    fn prepare_control_task(
        &mut self,
        project_id: Uuid,
        provider: ProviderKind,
        instructions: String,
        cx: &mut Context<Self>,
    ) {
        self.create_session_for(project_id, provider, cx);
        self.control_ui.page = None;
        if let Some(session) = self.selected_session_mut() {
            session.provider = provider;
            session.model = None;
        }
        self.composer
            .update(cx, |input, cx| input.set_content(instructions, cx));
        self.save();
        cx.notify();
    }
    fn prepare_record_task(&mut self, record: ControlRecord, cx: &mut Context<Self>) {
        let project = self.state.selected_project;
        if project.is_none() && matches!(&record, ControlRecord::Agent(_) | ControlRecord::Team(_))
        {
            self.show_toast("Choose a project before creating an agent task");
            self.open_workspace_page(Page::Projects, cx);
            return;
        }
        match record {
            ControlRecord::Agent(agent) => {
                if let Some(project) = project {
                    self.prepare_control_task(
                        project,
                        agent.provider,
                        format!(
                            "You are {}. Expertise: {}.\n{}",
                            agent.name, agent.expertise, agent.instructions
                        ),
                        cx,
                    );
                    if let Some(session) = self.selected_session_mut() {
                        session.model = agent.model;
                    }
                    self.save();
                }
            }
            ControlRecord::Team(team) => {
                if let Some(project) = project {
                    let profiles: HashMap<_, _> = self
                        .control_ui
                        .snapshot
                        .records
                        .iter()
                        .filter_map(|v| {
                            if let ControlRecord::Agent(a) = v {
                                Some((a.id, a))
                            } else {
                                None
                            }
                        })
                        .collect();
                    let lead = team
                        .members
                        .iter()
                        .find(|m| m.parent_id.is_none())
                        .and_then(|m| profiles.get(&m.agent_id));
                    let provider = lead
                        .map(|agent| agent.provider)
                        .unwrap_or(ProviderKind::Nebius);
                    let mut prompt = format!(
                        "Team: {}\nObjective: {}\nHigher agents delegate to their direct reports. Respect the current task's approval policy and project scope.\n",
                        team.name, team.objective
                    );
                    for member in team.members {
                        if let Some(agent) = profiles.get(&member.agent_id) {
                            let parent = member
                                .parent_id
                                .and_then(|id| profiles.get(&id))
                                .map(|p| p.name.as_str())
                                .unwrap_or("User");
                            prompt.push_str(&format!(
                                "\n{} — {}. Reports to {}. Instructions: {}\n",
                                agent.name, agent.expertise, parent, agent.instructions
                            ));
                        }
                    }
                    self.prepare_control_task(project, provider, prompt, cx);
                }
            }
            ControlRecord::Workflow(workflow) => {
                let mut prompt = format!(
                    "Workflow: {}\n{}\nExecute the following stages in order. Require the user's explicit decision at each marked gate. Show progress and keep all generated documents as artifacts.\n",
                    workflow.name, workflow.prompt
                );
                for (i, stage) in workflow.stages.iter().enumerate() {
                    prompt.push_str(&format!(
                        "\n{}. {}{}\n{}\n",
                        i + 1,
                        stage.title,
                        if stage.requires_decision {
                            " [User decision required]"
                        } else {
                            ""
                        },
                        stage.instructions
                    ));
                }
                self.prepare_control_task(workflow.project_id, ProviderKind::Nebius, prompt, cx);
            }
            ControlRecord::Routine(routine) => self.prepare_control_task(
                routine.project_id,
                routine.provider,
                routine.instructions,
                cx,
            ),
            ControlRecord::Artifact(_) => {}
        }
    }
    pub(super) fn render_workspace_nav(&self, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        let mut nav = div()
            .px(px(10.0))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(1.0));
        for (index, page) in [
            Page::Dashboard,
            Page::Inbox,
            Page::Decisions,
            Page::Tasks,
            Page::Projects,
            Page::Teams,
            Page::Routines,
            Page::Workflows,
            Page::Artifacts,
            Page::Apps,
            Page::Harnesses,
            Page::Usage,
        ]
        .into_iter()
        .enumerate()
        {
            if index == 3 || index == 9 {
                nav = nav.child(
                    div()
                        .mt(px(10.0))
                        .mb(px(4.0))
                        .px(px(8.0))
                        .text_size(sp(10.5))
                        .text_color(theme.text_tertiary)
                        .child(if index == 3 { "WORK" } else { "WORKSPACE" }),
                );
            }
            let selected = self.control_ui.page == Some(page);
            let label = page.label();
            let button = self
                .control_button(
                    format!("workspace-nav-{label}"),
                    label,
                    cx,
                    move |this, _, cx| this.open_workspace_page(page, cx),
                )
                .flex_row_reverse()
                .justify_end()
                .h(px(29.0))
                .border_0()
                .bg(if selected {
                    theme.overlay_strong
                } else {
                    gpui::transparent_black()
                })
                .child(icon(
                    page.icon(),
                    13.0,
                    if selected {
                        theme.accent
                    } else {
                        theme.text_secondary
                    },
                ));
            nav = nav.child(button);
        }
        nav
    }
    pub(super) fn render_workspace_header(&self, page: Page, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        let mut actions = div().flex().items_center().gap(px(8.0));
        actions = actions.child(self.control_button(
            "workspace-refresh",
            if self.control_ui.busy {
                "Refreshing…"
            } else {
                "Refresh"
            },
            cx,
            |this, _, cx| this.refresh_workspace_control(cx),
        ));
        let form = match page {
            Page::Teams => Some((FormKind::Team, "New team")),
            Page::Routines => Some((FormKind::Routine, "New routine")),
            Page::Workflows => Some((FormKind::Workflow, "New workflow")),
            _ => None,
        };
        if let Some((kind, label)) = form {
            actions = actions.child(self.control_button(
                "workspace-create",
                label,
                cx,
                move |this, _, cx| this.begin_control_form(kind, cx),
            ));
        }
        if page == Page::Teams {
            actions = actions.child(self.control_button(
                "workspace-new-agent",
                "New agent",
                cx,
                |this, _, cx| this.begin_control_form(FormKind::Agent, cx),
            ));
        }
        if page == Page::Artifacts {
            actions = actions.child(self.control_button(
                "workspace-import-artifact",
                "Add document",
                cx,
                |this, _, cx| this.import_control_artifact(cx),
            ));
        }
        div()
            .h(px(58.0))
            .flex_none()
            .px(px(24.0))
            .border_b_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .justify_end()
            .child(actions)
    }
    fn control_rows(&self, page: Page) -> Vec<Row> {
        let mut rows = Vec::new();
        match page {
            Page::Dashboard | Page::Inbox | Page::Decisions | Page::Tasks => {
                let mut sessions = self
                    .state
                    .sessions
                    .iter()
                    .filter(|session| session.has_started())
                    .filter(|session| {
                        page != Page::Decisions || session.status == SessionStatus::Waiting
                    })
                    .collect::<Vec<_>>();
                sessions.sort_by_key(|session| std::cmp::Reverse(session.updated_at));
                rows.extend(sessions.into_iter().map(|s| Row::Task(s.id)));
            }
            Page::Projects => rows.extend(
                self.state
                    .projects
                    .iter()
                    .map(|project| Row::Project(project.id)),
            ),
            Page::Apps => rows.extend(
                self.gateway_accounts
                    .iter()
                    .map(|a| Row::Account(a.toolkit.clone(), a.id.clone(), a.status.clone())),
            ),
            _ => {
                for record in &self.control_ui.snapshot.records {
                    if matches!(
                        (page, record),
                        (Page::Teams, ControlRecord::Team(_))
                            | (Page::Teams, ControlRecord::Agent(_))
                            | (Page::Workflows, ControlRecord::Workflow(_))
                            | (Page::Routines, ControlRecord::Routine(_))
                            | (Page::Artifacts, ControlRecord::Artifact(_))
                    ) {
                        rows.push(Row::Record(record.clone()));
                        if let ControlRecord::Team(team) = record {
                            let parents: HashMap<_, _> = team
                                .members
                                .iter()
                                .map(|m| (m.agent_id, m.parent_id))
                                .collect();
                            let mut members = Vec::new();
                            for member in &team.members {
                                if let Some(ControlRecord::Agent(agent)) = self
                                    .control_ui
                                    .snapshot
                                    .records
                                    .iter()
                                    .find(|r| r.id() == member.agent_id)
                                {
                                    let mut depth = 0;
                                    let mut parent = member.parent_id;
                                    while let Some(id) = parent {
                                        depth += 1;
                                        if depth > 32 {
                                            break;
                                        }
                                        parent = parents.get(&id).copied().flatten();
                                    }
                                    members.push((depth, agent.clone()));
                                }
                            }
                            members.sort_by_key(|(depth, _)| *depth);
                            rows.extend(
                                members
                                    .into_iter()
                                    .map(|(depth, agent)| Row::AgentInTeam(agent, depth)),
                            );
                        }
                    }
                }
            }
        }
        if rows.is_empty() {
            rows.push(Row::Empty(match page {
                Page::Dashboard | Page::Tasks => "Your tasks will appear here.",
                Page::Inbox => "No task updates yet.",
                Page::Decisions => "No decisions waiting for you.",
                Page::Teams => "Create an agent, then build a team.",
                Page::Workflows => "Describe a workflow to create its stages.",
                Page::Routines => "Create a routine for recurring agent work.",
                Page::Artifacts => "Add HTML, PDF, Markdown, and other documents.",
                Page::Apps => "Connect your personal gateway to see app accounts.",
                _ => "Nothing here yet.",
            }));
        }
        rows
    }
    pub(super) fn render_workspace_page(
        &mut self,
        page: Page,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        if page == Page::Harnesses {
            return div()
                .id("workspace-harnesses")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p(px(28.0))
                .child(self.render_providers_settings(cx))
                .child(self.render_models_speech_settings(cx))
                .into_any_element();
        }
        if page == Page::Usage {
            return div()
                .id("workspace-usage")
                .flex_1()
                .overflow_y_scroll()
                .p(px(24.0))
                .child(self.render_usage_settings(cx))
                .into_any_element();
        }
        if let Some(stages) = self.control_ui.pending_stages.take() {
            for stage in stages {
                self.add_control_stage(
                    stage.title,
                    stage.instructions,
                    stage.requires_decision,
                    window,
                    cx,
                );
            }
        }
        use std::hash::{Hash, Hasher};
        let mut key = std::collections::hash_map::DefaultHasher::new();
        page.hash(&mut key);
        self.control_ui.snapshot.revision.hash(&mut key);
        for session in &self.state.sessions {
            (session.id, session.updated_at, session.status as u8).hash(&mut key);
        }
        for project in &self.state.projects {
            project.id.hash(&mut key);
        }
        for account in &self.gateway_accounts {
            (&account.id, &account.status).hash(&mut key);
        }
        let key = key.finish();
        if self.control_ui.rows_key != key {
            let rows = Rc::new(self.control_rows(page));
            self.control_ui.list.reset(rows.len());
            self.control_ui.rows = rows;
            self.control_ui.rows_key = key;
        }
        let rows = self.control_ui.rows.clone();
        let weak = cx.entity().downgrade();
        let mut content = div()
            .track_focus(&self.control_ui.focus)
            .tab_group()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(14.0))
            .p(px(24.0));
        if let Some(error) = &self.control_ui.error {
            content = content.child(
                div()
                    .text_size(sp(13.0))
                    .text_color(theme.danger)
                    .child(error.clone()),
            );
        }
        if page == Page::Dashboard {
            let active = self.state.sessions.iter().filter(|s| s.is_busy()).count();
            let decisions = self
                .state
                .sessions
                .iter()
                .filter(|s| s.status == SessionStatus::Waiting)
                .count();
            let agents = self
                .control_ui
                .snapshot
                .records
                .iter()
                .filter(|r| matches!(r, ControlRecord::Agent(_)))
                .count();
            content = content.child(
                div()
                    .p(px(20.0))
                    .rounded(px(12.0))
                    .bg(linear_gradient(
                        135.0,
                        linear_color_stop(theme.raised, 0.0),
                        linear_color_stop(theme.sidebar_drag_background, 1.0),
                    ))
                    .text_color(theme.text)
                    .text_size(sp(15.0))
                    .child(format!(
                        "{} projects   ·   {} active tasks   ·   {} agents   ·   {} decisions",
                        self.state.projects.len(),
                        active,
                        agents,
                        decisions
                    )),
            );
        }
        if page == Page::Apps {
            content = content.child(self.control_button(
                "manage-app-connections",
                "Manage connections",
                cx,
                |this, window, cx| {
                    this.open_settings_action(&OpenSettings, window, cx);
                    this.open_settings_page(SettingsPage::Providers, cx);
                },
            ));
        }
        let collection = div()
            .id("workspace-records")
            .flex_1()
            .min_h_0()
            .relative()
            .child(
                list(self.control_ui.list.clone(), move |index, _, cx| {
                    weak.upgrade()
                        .map(|entity| {
                            entity.update(cx, |this, cx| {
                                this.render_control_row(rows[index].clone(), index, cx)
                            })
                        })
                        .unwrap_or_else(|| div().into_any_element())
                })
                .size_full(),
            )
            .child(scrollbar::vertical(
                &self.control_ui.list,
                &self.control_ui.scrollbar,
            ));
        if self.control_ui.form.is_some() {
            content = content.child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .gap(px(24.0))
                    .child(div().flex_1().min_w_0().child(collection))
                    .child(self.render_control_form(window, cx)),
            );
        } else {
            content = content.child(collection);
        }
        content.into_any_element()
    }
    fn render_control_row(&self, row: Row, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let mut body = div()
            .min_h(px(68.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .gap(px(14.0));
        let mut actions = div().flex_none().flex().gap(px(8.0));
        let (title, detail, icon_path) = match row {
            Row::Empty(message) => {
                return div()
                    .p(px(28.0))
                    .text_size(sp(14.0))
                    .text_color(theme.text_secondary)
                    .child(message)
                    .into_any_element();
            }
            Row::Task(id) => {
                let Some(session) = self.state.sessions.iter().find(|s| s.id == id) else {
                    return div().into_any_element();
                };
                actions = actions.child(self.control_button(
                    format!("task-open-{id}"),
                    if session.status == SessionStatus::Waiting {
                        "Review"
                    } else {
                        "Open"
                    },
                    cx,
                    move |this, _, cx| this.select_session(id, cx),
                ));
                let status = match session.status {
                    SessionStatus::Idle => "Idle",
                    SessionStatus::Connecting => "Starting",
                    SessionStatus::Working => "Working",
                    SessionStatus::Waiting => "Needs decision",
                    SessionStatus::Background => "Background",
                    SessionStatus::Failed => "Failed",
                };
                (
                    session.title.clone(),
                    format!("{} · {}", session.provider.display_name(), status),
                    "icons/compose.svg",
                )
            }
            Row::Project(id) => {
                let Some(project) = self.state.projects.iter().find(|p| p.id == id) else {
                    return div().into_any_element();
                };
                actions = actions.child(self.control_button(
                    format!("project-open-{id}"),
                    "New task",
                    cx,
                    move |this, _, cx| {
                        this.control_ui.page = None;
                        this.select_project(id, cx);
                    },
                ));
                (
                    project.name.clone(),
                    project.path.display().to_string(),
                    "icons/folder.svg",
                )
            }
            Row::Account(toolkit, id, status) => {
                (toolkit, format!("{status} · {id}"), "icons/package.svg")
            }
            Row::AgentInTeam(agent, depth) => {
                body = body.pl(px(22.0 + depth as f32 * 24.0));
                (
                    agent.name,
                    format!("{} · {}", agent.expertise, agent.provider.display_name()),
                    if depth == 0 {
                        "icons/target.svg"
                    } else {
                        "icons/corner-down-right.svg"
                    },
                )
            }
            Row::Record(record) => {
                let id = record.id();
                let name = record.name().to_owned();
                let edit = record.clone();
                let run = record.clone();
                let (detail, icon_path) = match &record {
                    ControlRecord::Agent(agent) => (
                        format!("{} · {}", agent.expertise, agent.provider.display_name()),
                        "icons/bot.svg",
                    ),
                    ControlRecord::Team(team) => (
                        format!("{} agents · {}", team.members.len(), team.objective),
                        "icons/fork.svg",
                    ),
                    ControlRecord::Workflow(workflow) => (
                        format!("{} stages · {}", workflow.stages.len(), workflow.prompt),
                        "icons/git-branch.svg",
                    ),
                    ControlRecord::Routine(routine) => (
                        format!(
                            "{} · every {} minutes",
                            if routine.paused {
                                "Paused"
                            } else {
                                "Scheduled"
                            },
                            routine.interval_minutes
                        ),
                        "icons/rotate-cw.svg",
                    ),
                    ControlRecord::Artifact(artifact) => {
                        (artifact.format.to_uppercase(), "icons/file.svg")
                    }
                };
                if matches!(record, ControlRecord::Artifact(_)) {
                    actions = actions.child(self.control_button(
                        format!("document-export-{id}"),
                        "Save document",
                        cx,
                        move |this, _, cx| this.export_control_artifact(run.clone(), cx),
                    ));
                } else {
                    actions = actions
                        .child(self.control_button(
                            format!("record-run-{id}"),
                            "New task",
                            cx,
                            move |this, _, cx| this.prepare_record_task(run.clone(), cx),
                        ))
                        .child(self.control_button(
                            format!("record-edit-{id}"),
                            "Edit",
                            cx,
                            move |this, _, cx| this.edit_control_record(edit.clone(), cx),
                        ));
                }
                (name, detail, icon_path)
            }
        };
        body = body
            .child(icon(icon_path, 18.0, theme.accent))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(5.0))
                    .child(
                        div()
                            .text_size(sp(14.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(sp(12.5))
                            .text_color(theme.text_secondary)
                            .truncate()
                            .child(detail),
                    ),
            )
            .child(actions);
        div()
            .id(SharedString::from(format!("control-row-{index}")))
            .child(body)
            .into_any_element()
    }
    fn render_control_form(&self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let kind = self.control_ui.form.unwrap();
        let mut form = div()
            .id("workspace-definition-editor")
            .w(px(420.0))
            .min_w(px(280.0))
            .h_full()
            .overflow_y_scroll()
            .p(px(20.0))
            .rounded(px(12.0))
            .bg(theme.raised)
            .flex()
            .flex_col()
            .gap(px(14.0));
        form = form
            .child(
                div()
                    .text_size(sp(17.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(match kind {
                        FormKind::Agent => "Agent",
                        FormKind::Team => "Team",
                        FormKind::Workflow => "Workflow",
                        FormKind::Routine => "Routine",
                    }),
            )
            .child(control_field(
                "Name",
                self.control_ui.name.clone(),
                "control-name",
            ));
        if kind == FormKind::Agent {
            form = form.child(control_field(
                "Expertise",
                self.control_ui.expertise.clone(),
                "control-expertise",
            ));
        }
        form = form
            .child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(if kind == FormKind::Workflow {
                        "Describe the workflow"
                    } else {
                        "Instructions"
                    }),
            )
            .child(
                div()
                    .h(px(120.0))
                    .p(px(10.0))
                    .rounded(px(8.0))
                    .bg(theme.inset)
                    .child(self.control_ui.instructions.clone()),
            );
        if matches!(kind, FormKind::Agent | FormKind::Routine) {
            let weak = cx.entity().downgrade();
            let provider = self.control_ui.provider;
            let menu = self.menu_handle("control-provider-menu", cx);
            form = form.child(dropdown_menu(
                self.control_button(
                    "control-provider",
                    provider.display_name(),
                    cx,
                    |_, _, _| {},
                ),
                "control-provider-menu",
                &menu,
                MenuAlign::BelowLeft,
                move |_| {
                    ProviderKind::ALL
                        .into_iter()
                        .map(|kind| {
                            let weak = weak.clone();
                            MenuItem::new(kind.display_name(), move |_, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    this.control_ui.provider = kind;
                                    cx.notify();
                                });
                            })
                            .selected(kind == provider)
                        })
                        .collect()
                },
            ));
        }
        if kind == FormKind::Agent {
            form = form.child(control_field(
                "Model",
                self.control_ui.model.clone(),
                "control-model",
            ));
        }
        if matches!(kind, FormKind::Workflow | FormKind::Routine) {
            let weak = cx.entity().downgrade();
            let projects = self.state.projects.clone();
            let selected = self.control_ui.project;
            let label = projects
                .iter()
                .find(|p| Some(p.id) == selected)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "Choose project".into());
            let menu = self.menu_handle("control-project-menu", cx);
            form = form.child(dropdown_menu(
                self.control_button("control-project", label, cx, |_, _, _| {}),
                "control-project-menu",
                &menu,
                MenuAlign::BelowLeft,
                move |_| {
                    projects
                        .iter()
                        .map(|p| {
                            let weak = weak.clone();
                            let id = p.id;
                            MenuItem::new(p.name.clone(), move |_, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    this.control_ui.project = Some(id);
                                    cx.notify();
                                });
                            })
                            .selected(Some(id) == selected)
                        })
                        .collect()
                },
            ));
        }
        if kind == FormKind::Routine {
            form = form
                .child(control_field(
                    "Every (minutes)",
                    self.control_ui.interval.clone(),
                    "control-interval",
                ))
                .child(
                    div()
                        .text_size(sp(12.5))
                        .text_color(theme.text_secondary)
                        .child("New routines are paused until you activate their schedule."),
                );
        }
        if kind == FormKind::Team {
            form = form.child(
                div()
                    .text_size(sp(14.0))
                    .font_weight(FontWeight::MEDIUM)
                    .child("Hierarchy"),
            );
            let agents = self
                .control_ui
                .snapshot
                .records
                .iter()
                .filter_map(|r| {
                    if let ControlRecord::Agent(a) = r {
                        Some(a.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            for agent in &agents {
                let id = agent.id;
                let member = self.control_ui.members.iter().find(|m| m.agent_id == id);
                let included = member.is_some();
                let parent = member.and_then(|m| m.parent_id);
                let label = if included {
                    format!("Remove {}", agent.name)
                } else {
                    format!("Add {}", agent.name)
                };
                let mut row = div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(self.control_button(
                        format!("team-member-{id}"),
                        label,
                        cx,
                        move |this, _, cx| {
                            if included {
                                this.control_ui.members.retain(|m| m.agent_id != id);
                            } else {
                                let parent_id = this.control_ui.members.first().map(|m| m.agent_id);
                                this.control_ui.members.push(TeamMember {
                                    agent_id: id,
                                    parent_id,
                                });
                            }
                            cx.notify();
                        },
                    ));
                if included {
                    let parents = agents
                        .iter()
                        .filter(|a| {
                            a.id != id && self.control_ui.members.iter().any(|m| m.agent_id == a.id)
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    let weak = cx.entity().downgrade();
                    let menu =
                        self.menu_handle(SharedString::from(format!("parent-menu-{id}")), cx);
                    let label = parent
                        .and_then(|id| agents.iter().find(|a| a.id == id))
                        .map(|a| format!("Reports to {}", a.name))
                        .unwrap_or_else(|| "Team lead".into());
                    row = row.child(dropdown_menu(
                        self.control_button(format!("parent-{id}"), label, cx, |_, _, _| {}),
                        SharedString::from(format!("parent-menu-{id}")),
                        &menu,
                        MenuAlign::BelowLeft,
                        move |_| {
                            let mut options = vec![(None, "Team lead".to_owned())];
                            options.extend(parents.iter().map(|a| (Some(a.id), a.name.clone())));
                            options
                                .into_iter()
                                .map(|(parent, label)| {
                                    let weak = weak.clone();
                                    MenuItem::new(label, move |_, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            if let Some(m) = this
                                                .control_ui
                                                .members
                                                .iter_mut()
                                                .find(|m| m.agent_id == id)
                                            {
                                                m.parent_id = parent;
                                            }
                                            cx.notify();
                                        });
                                    })
                                })
                                .collect()
                        },
                    ));
                }
                form = form.child(row);
            }
        }
        if kind == FormKind::Workflow {
            form = form.child(self.control_button(
                "generate-workflow",
                if self.control_ui.busy {
                    "Generating…"
                } else {
                    "Generate stages"
                },
                cx,
                |this, _, cx| {
                    if let Some(project_id) = this.control_ui.project {
                        let prompt = this.control_ui.instructions.read(cx).content().to_owned();
                        this.control_request(Rpc::GenerateWorkflow { project_id, prompt }, cx);
                    } else {
                        this.control_ui.error = Some("Choose a project first.".into());
                        cx.notify();
                    }
                },
            ));
            for (i, (title, instructions, gate)) in self.control_ui.stages.iter().enumerate() {
                let gate = *gate;
                form = form.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .child(control_field(
                            "Stage",
                            title.clone(),
                            SharedString::from(format!("stage-title-{i}")),
                        ))
                        .child(control_field(
                            "Instructions",
                            instructions.clone(),
                            SharedString::from(format!("stage-instructions-{i}")),
                        ))
                        .child(
                            div()
                                .flex()
                                .gap(px(8.0))
                                .child(self.control_button(
                                    format!("stage-gate-{i}"),
                                    if gate {
                                        "User decision required"
                                    } else {
                                        "No decision gate"
                                    },
                                    cx,
                                    move |this, _, cx| {
                                        this.control_ui.stages[i].2 = !gate;
                                        cx.notify();
                                    },
                                ))
                                .child(self.control_button(
                                    format!("stage-remove-{i}"),
                                    "Remove",
                                    cx,
                                    move |this, _, cx| {
                                        this.control_ui.stages.remove(i);
                                        cx.notify();
                                    },
                                )),
                        ),
                );
            }
            form = form.child(self.control_button(
                "add-workflow-stage",
                "Add stage",
                cx,
                |this, window, cx| {
                    this.add_control_stage(String::new(), String::new(), false, window, cx)
                },
            ));
        }
        form = form.child(
            div()
                .flex()
                .gap(px(8.0))
                .child(self.control_button(
                    "save-control-definition",
                    if self.control_ui.busy {
                        "Saving…"
                    } else {
                        "Save"
                    },
                    cx,
                    |this, _, cx| this.save_control_form(cx),
                ))
                .child(
                    self.control_button("cancel-control-editor", "Close", cx, |this, _, cx| {
                        this.control_ui.editor_generation += 1;
                        this.control_ui.form = None;
                        cx.notify();
                    }),
                ),
        );
        form.into_any_element()
    }
}
fn control_field(
    label: &'static str,
    input: Entity<TextInput>,
    id: impl Into<SharedString>,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(div().text_size(sp(12.5)).child(label))
        .child(TextField::new(id.into(), input))
}
