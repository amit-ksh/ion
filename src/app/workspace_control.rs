//! Native workspace navigation and definition editor. All RPC, credential,
//! model and document work runs on workers; render reads in-memory snapshots.
use super::*;
use waku_client::control::*;
use waku_client::teams::{
    AgentAccess, AgentExecution, AssignmentStatus, ExecutionEnvironment, TeamRunSnapshot,
    TeamRunStatus, TeamRunSummary,
};
mod automation_ui;
mod inbox;
mod routine_ui;
mod team_activity;
mod team_apps;
mod team_integration;
mod team_limits;
mod team_recovery;
mod usage_ui;
mod workflow_ui;
use waku_client::{Command as Rpc, ResponsePayload};

fn app_name(slug: &str) -> String {
    match slug {
        "gmail" => "Gmail",
        "github" => "GitHub",
        "googledrive" | "google_drive" => "Google Drive",
        "outlook" => "Outlook",
        "slack" => "Slack",
        "notion" => "Notion",
        "linear" => "Linear",
        "supabase" => "Supabase",
        _ => return slug.replace('_', " "),
    }
    .into()
}
fn account_is_active(this: &Waku, config: &str) -> bool {
    this.control_ui
        .app_configs
        .iter()
        .find(|a| a.id == config)
        .is_some_and(|config| {
            this.control_ui
                .app_accounts
                .iter()
                .any(|a| a.toolkit == config.toolkit && a.status == "ACTIVE")
        })
}

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
    pub(super) snapshot_loaded: bool,
    pub(super) team_index: Rc<Vec<Team>>,
    pub(super) team_names: HashSet<String>,
    pub(super) duplicate_team_names: HashSet<String>,
    pub busy: bool,
    inbox: inbox::InboxUi,
    usage: usage_ui::UsageUi,
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
    routine: routine_ui::RoutineUi,
    provider: ProviderKind,
    execution: AgentExecution,
    access: AgentAccess,
    expected_output: Entity<TextInput>,
    concurrency: Entity<TextInput>,
    sandbox_target: Entity<TextInput>,
    project: Option<Uuid>,
    members: Vec<TeamMember>,
    limit_inputs: Vec<(team_limits::LimitField, Entity<TextInput>)>,
    child_review: bool,
    stages: Vec<(
        Entity<TextInput>,
        Entity<TextInput>,
        bool,
        Option<Uuid>,
        Vec<String>,
        workflow_ui::TransitionUi,
    )>,
    pending_stages: Option<Vec<WorkflowStage>>,
    routine_team: Option<Uuid>,
    routine_workflow: Option<Uuid>,
    automation_runs: Rc<Vec<AutomationRun>>,
    automation_detail: Option<Rc<AutomationRun>>,
    automation_selected: Option<Uuid>,
    automation_definition: Option<Uuid>,
    automation_history: bool,
    automation_after: Option<Uuid>,
    automation_cursor: Option<Uuid>,
    automation_previous: Vec<Option<Uuid>>,
    automation_pending: u32,
    automation_requests: HashMap<Uuid, (Uuid, u64)>,
    focus: FocusHandle,
    rows_key: u64,
    list: ListState,
    scrollbar: Rc<ScrollbarState>,
    rows: Rc<Vec<Row>>,
    filter_project: Option<Uuid>,
    filter_task: Option<Uuid>,
    filter_kind: NoticeKind,
    filter_status: NoticeStatus,
    accepted_proposal: Option<WorkspaceAction>,
    app_configs: Vec<crate::credentials::ComposioAuthConfig>,
    app_accounts: Vec<crate::credentials::GatewayAccount>,
    apps_busy: bool,
    apps_generation: u64,
    pub(super) app_search: Entity<TextInput>,
    apps_connected_only: bool,
    app_action_account: Option<String>,
    app_action_search: Entity<TextInput>,
    app_action_tools: Vec<crate::credentials::composio::ComposioTool>,
    app_action_next: Option<String>,
    app_action_busy: bool,
    app_action_generation: u64,
    team_decisions: Rc<Vec<waku_client::teams::TeamDecision>>,
    decisions_after: Option<Uuid>,
    decisions_cursor: Option<Uuid>,
    decisions_previous: Vec<Option<Uuid>>,
    pending_team_decisions: u32,
    team_runs: Rc<Vec<TeamRunSummary>>,
    team_run: Option<Rc<TeamRunSnapshot>>,
    recovery: team_recovery::RecoveryUi,
    recovery_evidence: Entity<TextInput>,
    recovery_operation: Entity<TextInput>,
    integration: Option<std::sync::Arc<waku_client::teams::TeamIntegration>>,
    integration_busy: bool,
    integration_generation: u64,
    integration_error: Option<String>,
    integration_expanded: HashSet<usize>,
    integration_rows: Rc<Vec<std::sync::Arc<team_integration::SourceChangeRow>>>,
    selected_run: Option<Uuid>,
    runs_tab: bool,
    runs_team: Option<Uuid>,
    runs_cursor: Option<Uuid>,
    runs_after: Option<Uuid>,
    runs_previous: Vec<Option<Uuid>>,
    activity_busy: bool,
    activity_loaded: bool,
    activity_generation: u64,
    activity_revision: u64,
    activity_polling: bool,
    activity_open_generation: u64,
    activity_error: Option<String>,
}
#[derive(Clone)]
enum Row {
    Inbox(Rc<inbox::InboxRow>),
    InboxEvent(Rc<waku_client::inbox::InboxEntry>),
    Usage(Rc<usage_ui::UsageRow>),
    Task(Uuid),
    Project(Uuid),
    Record(ControlRecord),
    AgentInTeam(AgentProfile, usize, Option<String>),
    TeamRun(Rc<TeamRunSummary>),
    Assignment(Rc<team_activity::AssignmentRow>),
    TeamFollowUp(Rc<waku_client::teams::TeamFollowUp>),
    RecoveryFile(usize, std::sync::Arc<team_recovery::FileRow>),
    RecoveryApp(std::sync::Arc<team_recovery::AppRow>),
    RecoveryRemote(std::sync::Arc<team_recovery::RemoteRow>),
    SourceChange(usize, std::sync::Arc<team_integration::SourceChangeRow>),
    SourceDiffLine(usize, std::sync::Arc<crate::review_diff::Line>),
    SourceConflict(String),
    TeamDecision(Rc<waku_client::teams::TeamDecision>),
    Account(String, String, String),
    Empty(&'static str),
    Notice(Notice),
    AppPair(Vec<crate::credentials::ComposioAuthConfig>),
    Automation(Rc<AutomationRun>),
    AutomationStage(Rc<AutomationRun>, usize),
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
enum NoticeKind {
    #[default]
    All,
    Proposal,
    Approval,
    Question,
    Issue,
    Completed,
    Artifact,
    Decision,
}
impl NoticeKind {
    fn label(self) -> &'static str {
        match self {
            Self::All => "All types",
            Self::Proposal => "Proposed tasks",
            Self::Approval => "Approvals",
            Self::Question => "Questions",
            Self::Issue => "Issues",
            Self::Completed => "Completed work",
            Self::Artifact => "Documents",
            Self::Decision => "Decisions",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
enum NoticeStatus {
    #[default]
    All,
    Pending,
    Resolved,
}
impl NoticeStatus {
    fn label(self) -> &'static str {
        match self {
            Self::All => "All statuses",
            Self::Pending => "Needs action",
            Self::Resolved => "Resolved",
        }
    }
}
#[derive(Clone)]
struct Notice {
    project: Option<Uuid>,
    task: Option<Uuid>,
    title: String,
    detail: String,
    kind: NoticeKind,
    pending: bool,
    timestamp: u64,
    record: Option<ControlRecord>,
    run: Option<Uuid>,
    decision: Option<Rc<waku_client::teams::TeamDecision>>,
}
impl ControlUi {
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        Self {
            page: Some(Page::Dashboard),
            snapshot: ControlSnapshot::default(),
            snapshot_loaded: false,
            team_index: Rc::new(Vec::new()),
            team_names: HashSet::new(),
            duplicate_team_names: HashSet::new(),
            busy: false,
            inbox: inbox::InboxUi::default(),
            usage: usage_ui::UsageUi::new(window, cx),
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
            routine: routine_ui::RoutineUi::new(window, cx),
            provider: ProviderKind::Nebius,
            execution: AgentExecution::default(),
            access: AgentAccess::default(),
            expected_output: cx.new(|cx| {
                TextInput::new(window, cx).placeholder("Document, patch and validation evidence")
            }),
            concurrency: cx.new(|cx| TextInput::new(window, cx).placeholder("1")),
            sandbox_target: cx
                .new(|cx| TextInput::new(window, cx).placeholder("Configured sandbox target")),
            project: None,
            members: Vec::new(),
            limit_inputs: team_limits::inputs(window, cx),
            child_review: true,
            stages: Vec::new(),
            pending_stages: None,
            routine_team: None,
            routine_workflow: None,
            automation_runs: Rc::new(Vec::new()),
            automation_detail: None,
            automation_selected: None,
            automation_definition: None,
            automation_history: false,
            automation_after: None,
            automation_cursor: None,
            automation_previous: Vec::new(),
            automation_pending: 0,
            automation_requests: HashMap::new(),
            focus: cx.focus_handle(),
            rows_key: 0,
            list: ListState::new(0, ListAlignment::Top, px(72.0)),
            scrollbar: ScrollbarState::new(),
            rows: Rc::new(Vec::new()),
            filter_project: None,
            filter_task: None,
            filter_kind: NoticeKind::All,
            filter_status: NoticeStatus::All,
            accepted_proposal: None,
            app_configs: Vec::new(),
            app_accounts: Vec::new(),
            apps_busy: false,
            apps_generation: 0,
            app_search: cx.new(|cx| TextInput::new(window, cx).placeholder("Search apps")),
            apps_connected_only: false,
            app_action_account: None,
            app_action_search: cx
                .new(|cx| TextInput::new(window, cx).placeholder("Search actions")),
            app_action_tools: Vec::new(),
            app_action_next: None,
            app_action_busy: false,
            app_action_generation: 0,
            team_decisions: Rc::new(Vec::new()),
            decisions_after: None,
            decisions_cursor: None,
            decisions_previous: Vec::new(),
            pending_team_decisions: 0,
            team_runs: Rc::new(Vec::new()),
            team_run: None,
            recovery: Default::default(),
            recovery_operation: cx
                .new(|cx| TextInput::new(window, cx).placeholder("Sandbox operation UUID")),
            recovery_evidence: cx.new(|cx| {
                TextInput::new(window, cx).placeholder("Provider record or outcome evidence")
            }),
            integration: None,
            integration_busy: false,
            integration_generation: 0,
            integration_error: None,
            integration_expanded: HashSet::new(),
            integration_rows: Rc::new(vec![]),
            selected_run: None,
            runs_tab: false,
            runs_team: None,
            runs_cursor: None,
            runs_after: None,
            runs_previous: Vec::new(),
            activity_busy: false,
            activity_loaded: false,
            activity_generation: 0,
            activity_revision: 0,
            activity_polling: false,
            activity_open_generation: 0,
            activity_error: None,
        }
    }
}
impl Waku {
    pub(super) fn invalidate_composio_apps(&mut self) {
        self.control_ui.apps_generation = self.control_ui.apps_generation.wrapping_add(1);
        self.control_ui.apps_busy = false;
        self.control_ui.app_configs.clear();
        self.control_ui.app_accounts.clear();
    }
    fn refresh_composio_apps(&mut self, cx: &mut Context<Self>) {
        if self.control_ui.apps_busy {
            return;
        }
        self.control_ui.apps_busy = true;
        self.control_ui.apps_generation += 1;
        let generation = self.control_ui.apps_generation;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async {
                    smol::unblock(|| {
                        crate::credentials::CredentialService::global().composio_apps()
                    })
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.control_ui.apps_generation != generation {
                    return;
                }
                this.control_ui.apps_busy = false;
                match result {
                    Ok((configs, accounts)) => {
                        this.control_ui.app_configs = configs;
                        this.control_ui.app_accounts = accounts;
                        this.control_ui.error = None;
                    }
                    Err(error) => {
                        this.control_ui.app_configs.clear();
                        this.control_ui.app_accounts.clear();
                        this.control_ui.error = Some(error.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn connect_composio_app(&mut self, config_id: String, cx: &mut Context<Self>) {
        if config_id.starts_with("available:") {
            return;
        }
        if config_id.starts_with("configure:") {
            cx.open_url("https://dashboard.composio.dev");
            return;
        }
        if self.control_ui.apps_busy {
            return;
        }
        self.control_ui.apps_busy = true;
        self.control_ui.apps_generation += 1;
        let generation = self.control_ui.apps_generation;
        // Stable per-install identity, independent of model/task identity.
        let user_id = self.state.composio_user_id();
        self.save();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || {
                        crate::credentials::CredentialService::global()
                            .composio_connect(&config_id, &user_id)
                    })
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.control_ui.apps_generation != generation {
                    return;
                }
                this.control_ui.apps_busy = false;
                match result {
                    Ok(url) => {
                        cx.open_url(&url);
                        this.show_toast("Complete app sign-in, then refresh Apps.");
                    }
                    Err(error) => this.control_ui.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn workspace_notices(&self) -> Vec<Notice> {
        let mut notices = Vec::new();
        for session in self
            .state
            .sessions
            .iter()
            .filter(|s| s.has_started() && s.team_assignment.is_none())
        {
            let runtime = self.runtimes.get(&session.id);
            let mut add = |kind, pending, title: String, detail: String| {
                notices.push(Notice {
                    project: Some(session.project_id),
                    task: Some(session.id),
                    title,
                    detail,
                    kind,
                    pending,
                    timestamp: session.updated_at,
                    record: None,
                    run: None,
                    decision: None,
                })
            };
            if let Some(permission) = runtime.and_then(|r| r.pending_permission.as_ref()) {
                add(
                    if permission.title.starts_with("Decision: ") {
                        NoticeKind::Decision
                    } else {
                        NoticeKind::Approval
                    },
                    true,
                    permission.title.clone(),
                    permission.detail.clone(),
                );
            }
            if let Some(input) = runtime.and_then(|r| r.pending_user_input.as_ref()) {
                let question = input
                    .current_question()
                    .map(|q| q.question.clone())
                    .unwrap_or_else(|| "Agent needs your answer".into());
                add(
                    NoticeKind::Question,
                    true,
                    question,
                    session.display_title().into(),
                );
            }
            if runtime.is_some_and(|r| r.pending_computer_approval.is_some()) {
                add(
                    NoticeKind::Approval,
                    true,
                    "Computer action needs approval".into(),
                    session.display_title().into(),
                );
            }
            if session.status == SessionStatus::Waiting
                && runtime.is_none_or(|r| {
                    r.pending_permission.is_none()
                        && r.pending_user_input.is_none()
                        && r.pending_computer_approval.is_none()
                })
            {
                add(
                    NoticeKind::Decision,
                    true,
                    "Agent needs your decision".into(),
                    session.display_title().into(),
                );
            }
            if session.status == SessionStatus::Failed {
                add(
                    NoticeKind::Issue,
                    true,
                    "Task needs attention".into(),
                    runtime
                        .and_then(|r| r.last_driver_error.clone())
                        .unwrap_or_else(|| session.display_title().into()),
                );
            } else if session.status == SessionStatus::Idle && session.last_reply_at.is_some() {
                let completed = session
                    .turns
                    .last()
                    .is_some_and(|t| t.status == TurnStatus::Completed);
                add(
                    NoticeKind::Completed,
                    false,
                    if completed {
                        "Task completed"
                    } else {
                        "Task updated"
                    }
                    .into(),
                    session.display_title().into(),
                );
            }
        }
        notices.extend(self.team_activity_notices());
        for record in &self.control_ui.snapshot.records {
            match record {
                ControlRecord::Action(action) => notices.push(Notice {
                    project: Some(action.project_id),
                    task: Some(action.task_id),
                    title: action.title.clone(),
                    detail: action.detail.clone(),
                    kind: if action.action_kind == WorkspaceActionKind::ProposedTask {
                        NoticeKind::Proposal
                    } else {
                        NoticeKind::Decision
                    },
                    pending: action.status == WorkspaceActionStatus::Pending,
                    timestamp: action.created_at,
                    record: Some(record.clone()),
                    run: None,
                    decision: None,
                }),
                ControlRecord::Artifact(artifact) => notices.push(Notice {
                    project: artifact.project_id,
                    task: artifact.task_id,
                    title: artifact.name.clone(),
                    detail: format!(
                        "{} document created by agent",
                        artifact.format.to_uppercase()
                    ),
                    kind: NoticeKind::Artifact,
                    pending: false,
                    timestamp: artifact.created_at,
                    record: Some(record.clone()),
                    run: None,
                    decision: None,
                }),
                _ => {}
            }
        }
        notices.sort_by_key(|n| std::cmp::Reverse(n.timestamp));
        notices
    }
    fn resolve_workspace_action(
        &mut self,
        mut action: WorkspaceAction,
        accept: bool,
        cx: &mut Context<Self>,
    ) {
        if self.control_ui.busy || action.status != WorkspaceActionStatus::Pending {
            return;
        }
        if accept
            && !self
                .state
                .projects
                .iter()
                .any(|p| p.id == action.project_id)
        {
            self.control_ui.error = Some(
                "The proposal's project was removed. Restore the project before accepting.".into(),
            );
            cx.notify();
            return;
        }
        action.status = if accept {
            WorkspaceActionStatus::Accepted
        } else {
            WorkspaceActionStatus::Declined
        };
        if accept && action.action_kind == WorkspaceActionKind::ProposedTask {
            self.control_ui.accepted_proposal = Some(action.clone());
        }
        self.control_request(
            Rpc::SaveWorkspaceRecord {
                expected_revision: self.control_ui.snapshot.revision,
                record: ControlRecord::Action(action),
            },
            cx,
        );
    }
    fn render_workspace_filters(&self, cx: &mut Context<Self>) -> Div {
        #[derive(Clone, Copy)]
        enum Filter {
            Project(Option<Uuid>),
            Task(Option<Uuid>),
            Kind(NoticeKind),
            Status(NoticeStatus),
        }
        let labels = [
            self.control_ui
                .filter_project
                .and_then(|id| self.state.projects.iter().find(|p| p.id == id))
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "All projects".into()),
            self.control_ui
                .filter_task
                .and_then(|id| self.state.sessions.iter().find(|s| s.id == id))
                .map(|s| s.display_title().to_owned())
                .unwrap_or_else(|| "All tasks".into()),
            self.control_ui.filter_kind.label().into(),
            self.control_ui.filter_status.label().into(),
        ];
        let mut bar = div().flex().flex_wrap().gap(px(8.0));
        let count = if self.control_ui.page == Some(Page::Inbox) {
            4
        } else {
            3
        };
        for (index, label) in labels.into_iter().take(count).enumerate() {
            let weak = cx.entity().downgrade();
            let id = format!("workspace-filter-{index}");
            let menu = self.menu_handle(&id, cx);
            bar = bar.child(dropdown_menu(
                self.control_button(id.clone(), label, cx, |_, _, _| {})
                    .child(icon(
                        "icons/chevron-down.svg",
                        12.0,
                        Theme::current(cx).text_secondary,
                    )),
                SharedString::from(id),
                &menu,
                MenuAlign::BelowLeft,
                move |cx| {
                    // Build large project/task option lists only when the menu opens.
                    let Some(entity) = weak.upgrade() else {
                        return Vec::new();
                    };
                    let this = entity.read(cx);
                    let project = this.control_ui.filter_project;
                    let task = this.control_ui.filter_task;
                    let mut options = Vec::new();
                    match index {
                        0 => {
                            options.push((
                                "All projects".to_owned(),
                                Filter::Project(None),
                                project.is_none(),
                            ));
                            options.extend(this.state.projects.iter().map(|p| {
                                (
                                    p.name.clone(),
                                    Filter::Project(Some(p.id)),
                                    project == Some(p.id),
                                )
                            }));
                        }
                        1 => {
                            options.push((
                                "All tasks".to_owned(),
                                Filter::Task(None),
                                task.is_none(),
                            ));
                            options.extend(
                                this.state
                                    .sessions
                                    .iter()
                                    .filter(|s| project.is_none_or(|p| p == s.project_id))
                                    .map(|s| {
                                        (
                                            s.display_title().to_owned(),
                                            Filter::Task(Some(s.id)),
                                            task == Some(s.id),
                                        )
                                    }),
                            );
                        }
                        2 => options.extend(
                            [
                                NoticeKind::All,
                                NoticeKind::Proposal,
                                NoticeKind::Approval,
                                NoticeKind::Question,
                                NoticeKind::Decision,
                                NoticeKind::Issue,
                                NoticeKind::Completed,
                                NoticeKind::Artifact,
                            ]
                            .into_iter()
                            .map(|kind| {
                                (
                                    kind.label().into(),
                                    Filter::Kind(kind),
                                    kind == this.control_ui.filter_kind,
                                )
                            }),
                        ),
                        _ => options.extend(
                            [
                                NoticeStatus::All,
                                NoticeStatus::Pending,
                                NoticeStatus::Resolved,
                            ]
                            .into_iter()
                            .map(|status| {
                                (
                                    status.label().into(),
                                    Filter::Status(status),
                                    status == this.control_ui.filter_status,
                                )
                            }),
                        ),
                    }
                    options
                        .into_iter()
                        .map(|(name, filter, selected)| {
                            let weak = weak.clone();
                            MenuItem::new(name, move |_, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    match filter {
                                        Filter::Project(id) => {
                                            this.control_ui.filter_project = id;
                                            this.control_ui.filter_task = None;
                                            this.control_ui.runs_after = None;
                                            this.control_ui.runs_previous.clear();
                                            this.control_ui.decisions_after = None;
                                            this.control_ui.decisions_previous.clear();
                                            this.control_ui.automation_after = None;
                                            this.control_ui.automation_previous.clear();
                                            this.refresh_team_activity(true, cx);
                                        }
                                        Filter::Task(id) => this.control_ui.filter_task = id,
                                        Filter::Kind(kind) => this.control_ui.filter_kind = kind,
                                        Filter::Status(status) => {
                                            this.control_ui.filter_status = status;
                                            this.control_ui.decisions_after = None;
                                            this.control_ui.decisions_previous.clear();
                                            this.control_ui.automation_after = None;
                                            this.control_ui.automation_previous.clear();
                                            this.refresh_team_activity(true, cx);
                                        }
                                    }
                                    if this.control_ui.page == Some(Page::Inbox) {
                                        this.reset_inbox_history(cx);
                                    }
                                    cx.notify();
                                });
                            })
                            .selected(selected)
                        })
                        .collect()
                },
            ));
        }
        bar
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
    pub(super) fn control_button(
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
            .focus_visible(|style| style.border_1().border_color(theme.accent))
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
        if self.control_ui.page != Some(page) {
            self.control_ui.activity_generation += 1;
            self.control_ui.activity_busy = false;
            self.control_ui.automation_selected = None;
            self.control_ui.automation_detail = None;
            self.control_ui.automation_history = false;
            self.control_ui.automation_definition = None;
            self.control_ui.automation_after = None;
            self.control_ui.automation_previous.clear();
            self.control_ui.selected_run = None;
            self.clear_team_integration();
            self.clear_team_recovery();
            self.control_ui.team_run = None;
            self.control_ui.runs_after = None;
            self.control_ui.runs_previous.clear();
            self.control_ui.decisions_after = None;
            self.control_ui.decisions_previous.clear();
            self.control_ui.runs_team = None;
        }
        if self.control_ui.page != Some(page)
            && matches!(page, Page::Inbox | Page::Decisions | Page::Teams)
        {
            self.control_ui.filter_project = None;
            self.control_ui.filter_task = None;
            self.control_ui.filter_kind = NoticeKind::All;
            self.control_ui.filter_status = NoticeStatus::All;
            self.control_ui.inbox.view = inbox::InboxView::Inbox;
            self.reset_inbox_history(cx);
        }
        self.control_ui.editor_generation += 1;
        self.control_ui.app_action_generation += 1;
        self.control_ui.app_action_account = None;
        self.control_ui.app_action_tools.clear();
        self.control_ui.app_action_next = None;
        self.control_ui.app_action_busy = false;
        self.control_ui
            .app_action_search
            .update(cx, |input, cx| input.set_content("", cx));
        self.control_ui.page = Some(page);
        self.control_ui.form = None;
        self.control_ui.error = None;
        self.control_ui.list.scroll_to_reveal_item(0);
        self.refresh_workspace_control(cx);
        if page == Page::Apps {
            self.refresh_composio_apps(cx);
        }
        if page == Page::Usage {
            self.reset_usage_ledger(cx);
            self.start_usage_poll(cx);
        }
        if page == Page::Harnesses {
            self.refresh_provider_detection(None);
            self.refresh_credential_statuses(cx);
        }
        cx.notify();
    }
    pub(super) fn refresh_workspace_control(&mut self, cx: &mut Context<Self>) {
        self.refresh_inbox(cx);
        self.control_request(Rpc::LoadWorkspaceControl, cx);
        self.refresh_team_activity(true, cx);
        self.start_team_activity_poll(cx);
    }
    fn control_request(&mut self, command: Rpc, cx: &mut Context<Self>) {
        if self.control_ui.busy {
            return;
        }
        self.control_ui.generation += 1;
        let generation = self.control_ui.generation;
        let editor_generation = self.control_ui.editor_generation;
        let saving = matches!(&command, Rpc::SaveWorkspaceRecord { .. });
        let starting = if let Rpc::StartAutomation { id, .. } = &command {
            Some(*id)
        } else {
            None
        };
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
                    smol::unblock(move || {
                        let response=client.request(Uuid::nil(),Uuid::nil(),command);
                        let rejected=response.as_ref().err().is_some_and(|e|e.downcast_ref::<waku_client::DaemonRejection>().is_some());
                        if response.is_err() && let Some(id)=starting {
                            match client.request(Uuid::nil(),Uuid::nil(),Rpc::GetAutomationRun{id}) {
                                Ok(ResponsePayload::AutomationRun{run})=>return (Ok(ResponsePayload::AutomationRun{run}),false),
                                Ok(ResponsePayload::AutomationMissing{id:missing}) if missing==id=>return (response,rejected),
                                _=>{},
                            }
                        }
                        (response,false)
                    }).await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.control_ui.generation != generation {
                    return;
                }
                this.control_ui.busy = false;
                let (response,confirmed_missing)=response;
                if confirmed_missing {if let Some(id)=starting{this.control_ui.automation_requests.retain(|_,(request,_)|*request!=id);}}
                match response {
                    Ok(ResponsePayload::WorkspaceControl { snapshot }) => {
                        this.install_control_snapshot(snapshot);
                        if saving && this.control_ui.editor_generation==editor_generation {this.control_ui.form = None;}
                        if saving && let Some(proposal) = this.control_ui.accepted_proposal.take() {
                            if this.control_ui.snapshot.records.iter().any(|r| matches!(r, ControlRecord::Action(a) if a.id==proposal.id && a.status==WorkspaceActionStatus::Accepted)) {
                                this.prepare_control_task(proposal.project_id, ProviderKind::Nebius, format!("{}\n\n{}", proposal.title, proposal.detail), cx);
                            }
                        }
                    }
                    Ok(ResponsePayload::AutomationRun {run}) => {
                        let page=if run.routine_id.is_some() { Page::Routines } else { Page::Workflows };
                        if this.control_ui.page!=Some(page) { this.open_workspace_page(page,cx); }
                        this.control_ui.automation_requests.retain(|_,(id,_)|*id!=run.id);
                        this.control_ui.automation_selected=Some(run.id);
                        this.control_ui.automation_detail=Some(Rc::new(run));
                        this.control_ui.automation_history=true;
                        this.control_ui.activity_revision+=1;
                        this.refresh_team_activity(true,cx);
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
                    Err(error) => { this.control_ui.accepted_proposal = None; this.control_ui.error = Some(error.to_string()); },
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
        self.control_ui.routine_team = None;
        self.control_ui.routine_workflow = None;
        self.control_ui.members.clear();
        self.set_team_limit_inputs(waku_client::teams::TeamRunLimits::default(), cx);
        self.control_ui.stages.clear();
        self.control_ui.pending_stages = None;
        self.control_ui.project = self.state.selected_project;
        self.control_ui.provider = ProviderKind::Nebius;
        self.control_ui.execution = AgentExecution::default();
        self.control_ui.access = AgentAccess::default();
        for input in [
            &self.control_ui.name,
            &self.control_ui.instructions,
            &self.control_ui.expertise,
            &self.control_ui.model,
            &self.control_ui.interval,
            &self.control_ui.expected_output,
            &self.control_ui.concurrency,
            &self.control_ui.sandbox_target,
        ] {
            input.update(cx, |input, cx| input.set_content("", cx));
        }
        self.reset_routine_editor(cx);
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
        self.control_ui.stages.push((
            title_input,
            instructions_input,
            gate,
            None,
            vec![],
            workflow_ui::TransitionUi::new(WorkflowTransition::Next, window, cx),
        ));
        cx.notify();
    }
    fn edit_control_record(&mut self, record: ControlRecord, cx: &mut Context<Self>) {
        let (kind, instructions) = match &record {
            ControlRecord::Agent(v) => (FormKind::Agent, v.instructions.clone()),
            ControlRecord::Team(v) => (FormKind::Team, v.objective.clone()),
            ControlRecord::Workflow(v) => (FormKind::Workflow, v.prompt.clone()),
            ControlRecord::Routine(v) => (FormKind::Routine, v.instructions.clone()),
            ControlRecord::Artifact(_) | ControlRecord::Action(_) => return,
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
                self.control_ui.execution = v.execution;
                self.control_ui.access = v.access;
                self.control_ui
                    .expected_output
                    .update(cx, |input, cx| input.set_content(v.expected_output, cx));
                self.control_ui.concurrency.update(cx, |input, cx| {
                    input.set_content(v.max_parallel_tasks.to_string(), cx)
                });
                self.control_ui.sandbox_target.update(cx, |input, cx| {
                    input.set_content(
                        self.control_ui
                            .execution
                            .sandbox_target
                            .clone()
                            .unwrap_or_default(),
                        cx,
                    )
                });
                self.control_ui
                    .expertise
                    .update(cx, |input, cx| input.set_content(v.expertise, cx));
                self.control_ui.model.update(cx, |input, cx| {
                    input.set_content(v.model.unwrap_or_default(), cx)
                });
            }
            ControlRecord::Team(v) => {
                self.set_team_limit_inputs(v.limits.unwrap_or_default(), cx);
                self.control_ui.members = v.members;
            }
            ControlRecord::Routine(v) => {
                self.load_routine_editor(&v, cx);
                self.control_ui.routine_team = v.team_id;
                self.control_ui.routine_workflow = v.workflow_id;
                self.control_ui.project = Some(v.project_id);
                self.control_ui.provider = v.provider;
                self.control_ui.interval.update(cx, |input, cx| {
                    input.set_content(v.interval_minutes.to_string(), cx)
                });
            }
            ControlRecord::Workflow(v) => {
                self.set_team_limit_inputs(v.limits.clone().unwrap_or_default(), cx);
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
            FormKind::Agent => {
                let concurrency = self
                    .control_ui
                    .concurrency
                    .read(cx)
                    .content()
                    .trim()
                    .to_owned();
                let max_parallel_tasks = if concurrency.is_empty() {
                    1
                } else if let Ok(value) = concurrency.parse::<u16>() {
                    value
                } else {
                    self.control_ui.error = Some("Enter the maximum parallel tasks (1–16).".into());
                    cx.notify();
                    return;
                };
                let mut execution = self.control_ui.execution.clone();
                execution.sandbox_target = Some(
                    self.control_ui
                        .sandbox_target
                        .read(cx)
                        .content()
                        .trim()
                        .to_owned(),
                )
                .filter(|v| !v.is_empty());
                ControlRecord::Agent(AgentProfile {
                    id,
                    name,
                    expertise: self.control_ui.expertise.read(cx).content().to_owned(),
                    instructions,
                    provider: self.control_ui.provider,
                    model: Some(self.control_ui.model.read(cx).content().trim().to_owned())
                        .filter(|v| !v.is_empty()),
                    expected_output: self
                        .control_ui
                        .expected_output
                        .read(cx)
                        .content()
                        .trim()
                        .to_owned(),
                    execution,
                    access: self.control_ui.access.clone(),
                    max_parallel_tasks,
                })
            }
            FormKind::Team => {
                let limits = match self.read_team_limit_inputs(cx) {
                    Ok(limits) => limits,
                    Err(error) => {
                        self.control_ui.error = Some(error);
                        cx.notify();
                        return;
                    }
                };
                ControlRecord::Team(Team {
                    id,
                    name,
                    objective: instructions,
                    limits: Some(limits),
                    members: self.control_ui.members.clone(),
                })
            }
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
                    limits: Some(match self.read_team_limit_inputs(cx) {
                        Ok(limits) => limits,
                        Err(error) => {
                            self.control_ui.error = Some(error);
                            cx.notify();
                            return;
                        }
                    }),
                    stages: match self.workflow_stage_drafts(cx) {
                        Ok(stages) => stages,
                        Err(error) => {
                            self.control_ui.error = Some(error);
                            cx.notify();
                            return;
                        }
                    },
                })
            }
            FormKind::Routine => {
                let Some(project_id) = self.control_ui.project else {
                    self.control_ui.error = Some("Choose a project.".into());
                    cx.notify();
                    return;
                };
                let schedule = match self.routine_schedule_draft(cx) {
                    Ok(schedule) => schedule,
                    Err(error) => {
                        self.control_ui.error = Some(error);
                        cx.notify();
                        return;
                    }
                };
                let (missed_run_policy, overlap_policy) = self.routine_policies();
                ControlRecord::Routine(Routine {
                    id,
                    name,
                    project_id,
                    instructions,
                    interval_minutes: schedule.interval_minutes,
                    calendar: schedule.calendar,
                    missed_run_policy,
                    overlap_policy,
                    paused: true,
                    next_run: 0,
                    provider: self.control_ui.provider,
                    team_id: self.control_ui.routine_team,
                    workflow_id: self.control_ui.routine_workflow,
                    last_error: None,
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
                if agent.execution.environment == ExecutionEnvironment::NebiusSandbox {
                    self.show_toast("Sandbox agent execution requires the Teams runtime adapter.");
                    return;
                }
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
                    let mention =
                        waku_client::teams::mentions::mention(&team, &self.control_ui.team_index);
                    // Keep New task; picking a team only inserts an explicit address.
                    // Never combine every member's instructions into a solo prompt.
                    self.prepare_control_task(
                        project,
                        self.state.last_provider,
                        format!("{mention} {}", team.objective),
                        cx,
                    );
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
            ControlRecord::Artifact(_) | ControlRecord::Action(_) => {}
        }
    }
    pub(super) fn render_workspace_nav(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = Theme::current(cx);
        let mut nav = div()
            .id("workspace-navigation")
            .px(px(10.0))
            .flex_none()
            .max_h(gpui::relative(0.58))
            .overflow_y_scroll()
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
            let mut button = self
                .control_button(
                    format!("workspace-nav-{label}"),
                    "",
                    cx,
                    move |this, _, cx| this.open_workspace_page(page, cx),
                )
                .w_full()
                .justify_start()
                .px(px(4.0))
                .gap(px(10.0))
                .h(px(32.0))
                .border_0()
                .bg(if selected {
                    theme.overlay_strong
                } else {
                    gpui::transparent_black()
                })
                .child(
                    div()
                        .size(px(20.0))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon(
                            page.icon(),
                            14.0,
                            if selected {
                                theme.accent
                            } else {
                                theme.text_secondary
                            },
                        )),
                )
                .child(div().min_w_0().truncate().child(label));
            if page == Page::Inbox && self.inbox_unread_count() > 0 {
                button = button.child(
                    div()
                        .ml_auto()
                        .text_size(sp(12.0))
                        .text_color(theme.text_secondary)
                        .child(self.inbox_unread_count().to_string()),
                );
            }
            nav = nav.child(button);
        }
        nav
    }
    pub(super) fn render_workspace_header(&self, page: Page, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        let mut actions = div().flex().items_center().gap(px(8.0));
        let busy =
            self.control_ui.busy || self.control_ui.apps_busy || self.control_ui.activity_busy;
        actions = actions.child(
            self.control_button("workspace-refresh", "", cx, |this, _, cx| {
                this.refresh_workspace_control(cx);
                this.refresh_usage_ledger(cx);
                if this.control_ui.page == Some(Page::Apps) {
                    this.refresh_composio_apps(cx);
                }
            })
            .size(px(32.0))
            .p_0()
            .justify_center()
            .border_0()
            .opacity(if busy { 0.45 } else { 1.0 })
            .tooltip(Tooltip::text(if busy { "Refreshing" } else { "Refresh" }))
            .child(icon("icons/rotate-cw.svg", 16.0, theme.text_secondary)),
        );
        let form = match page {
            Page::Teams => Some((FormKind::Team, "New team")),
            Page::Routines => Some((FormKind::Routine, "New routine")),
            Page::Workflows => Some((FormKind::Workflow, "New workflow")),
            _ => None,
        };
        if matches!(page, Page::Workflows | Page::Routines) {
            actions = actions.child(self.control_button(
                "automation-definitions",
                "Definitions",
                cx,
                |this, _, cx| {
                    this.control_ui.automation_history = false;
                    this.control_ui.automation_selected = None;
                    this.control_ui.activity_revision += 1;
                    cx.notify();
                },
            ));
            actions =
                actions.child(
                    self.control_button("automation-runs", "Runs", cx, |this, _, cx| {
                        this.control_ui.automation_history = true;
                        this.control_ui.automation_selected = None;
                        this.control_ui.automation_definition = None;
                        this.control_ui.automation_after = None;
                        this.control_ui.automation_previous.clear();
                        this.refresh_team_activity(true, cx);
                        cx.notify();
                    }),
                );
        }
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
        div()
            .h(px(48.0))
            .flex_none()
            .px(px(24.0))
            .border_b_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .justify_end()
            .child(actions)
    }
    fn control_rows(&self, page: Page, cx: &App) -> Vec<Row> {
        if page == Page::Usage {
            let rows = self.usage_rows();
            return if rows.is_empty() {
                vec![Row::Empty("No usage matches these filters")]
            } else {
                rows
            };
        }
        if page == Page::Inbox {
            let mut rows = self.inbox_history_rows();
            if rows.is_empty() {
                rows.push(Row::Empty("No notifications match these filters."));
            }
            return rows;
        }
        let mut rows = Vec::new();
        match page {
            Page::Inbox | Page::Decisions => rows.extend(
                self.workspace_notices()
                    .into_iter()
                    .filter(|n| {
                        page != Page::Decisions
                            || (n.pending
                                && matches!(
                                    n.kind,
                                    NoticeKind::Decision
                                        | NoticeKind::Approval
                                        | NoticeKind::Question
                                        | NoticeKind::Proposal
                                ))
                    })
                    .filter(|n| {
                        self.control_ui
                            .filter_project
                            .is_none_or(|id| n.project == Some(id))
                    })
                    .filter(|n| {
                        self.control_ui
                            .filter_task
                            .is_none_or(|id| n.task == Some(id))
                    })
                    .filter(|n| {
                        self.control_ui.filter_kind == NoticeKind::All
                            || self.control_ui.filter_kind == n.kind
                    })
                    .filter(|n| match self.control_ui.filter_status {
                        NoticeStatus::All => true,
                        NoticeStatus::Pending => n.pending,
                        NoticeStatus::Resolved => !n.pending,
                    })
                    .map(|notice| match &notice.decision {
                        Some(decision) => Row::TeamDecision(decision.clone()),
                        None => Row::Notice(notice),
                    }),
            ),
            Page::Dashboard | Page::Tasks => {
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
                if page == Page::Dashboard {
                    rows.extend(
                        self.control_ui
                            .team_runs
                            .iter()
                            .cloned()
                            .map(|run| Row::TeamRun(Rc::new(run))),
                    );
                    sessions.retain(|s| s.team_assignment.is_none());
                }
                rows.extend(sessions.into_iter().map(|s| Row::Task(s.id)));
            }
            Page::Teams if self.control_ui.runs_tab => rows.extend(self.team_activity_rows()),
            Page::Projects => rows.extend(
                self.state
                    .projects
                    .iter()
                    .map(|project| Row::Project(project.id)),
            ),
            Page::Apps => {
                let query = self
                    .control_ui
                    .app_search
                    .read(cx)
                    .content()
                    .trim()
                    .to_lowercase();
                if self.control_ui.apps_connected_only {
                    rows.extend(
                        self.control_ui
                            .app_accounts
                            .iter()
                            .filter(|a| a.toolkit.to_lowercase().contains(&query))
                            .map(|a| {
                                Row::Account(a.toolkit.clone(), a.id.clone(), a.status.clone())
                            }),
                    );
                } else {
                    let configs = self
                        .control_ui
                        .app_configs
                        .iter()
                        .filter(|a| {
                            a.toolkit.to_lowercase().contains(&query)
                                || a.name.to_lowercase().contains(&query)
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    rows.extend(configs.chunks(2).map(|pair| Row::AppPair(pair.to_vec())));
                }
            }
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
                            let profiles: HashMap<_, _> = self
                                .control_ui
                                .snapshot
                                .records
                                .iter()
                                .filter_map(|r| match r {
                                    ControlRecord::Agent(a) => Some((a.id, a)),
                                    _ => None,
                                })
                                .collect();
                            for (id, depth) in team_activity::hierarchy(
                                &team
                                    .members
                                    .iter()
                                    .map(|m| (m.agent_id, m.parent_id))
                                    .collect::<Vec<_>>(),
                            ) {
                                if let Some(agent) = profiles.get(&id) {
                                    let parent = team
                                        .members
                                        .iter()
                                        .find(|m| m.agent_id == id)
                                        .and_then(|m| m.parent_id)
                                        .and_then(|id| profiles.get(&id))
                                        .map(|a| a.name.clone());
                                    rows.push(Row::AgentInTeam((*agent).clone(), depth, parent));
                                }
                            }
                        }
                    }
                }
            }
        }
        if matches!(page, Page::Workflows | Page::Routines) && self.control_ui.automation_history {
            return self.automation_rows();
        }
        if matches!(page, Page::Dashboard | Page::Inbox | Page::Decisions) {
            rows.extend(
                self.control_ui
                    .automation_runs
                    .iter()
                    .filter(|r| {
                        self.control_ui
                            .filter_project
                            .is_none_or(|p| r.project_id == p)
                            && self
                                .control_ui
                                .filter_task
                                .is_none_or(|t| r.stages.iter().any(|s| s.task_id == t))
                            && (page != Page::Decisions
                                || r.status == AutomationStatus::WaitingDecision)
                            && (page != Page::Inbox
                                || ((self.control_ui.filter_kind == NoticeKind::All
                                    || self.control_ui.filter_kind == NoticeKind::Decision
                                        && r.status == AutomationStatus::WaitingDecision
                                    || self.control_ui.filter_kind == NoticeKind::Issue
                                        && r.status == AutomationStatus::Blocked
                                    || self.control_ui.filter_kind == NoticeKind::Completed
                                        && r.status == AutomationStatus::Completed)
                                    && match self.control_ui.filter_status {
                                        NoticeStatus::All => true,
                                        NoticeStatus::Pending => matches!(
                                            r.status,
                                            AutomationStatus::WaitingDecision
                                                | AutomationStatus::Blocked
                                        ),
                                        NoticeStatus::Resolved => matches!(
                                            r.status,
                                            AutomationStatus::Completed
                                                | AutomationStatus::Canceled
                                                | AutomationStatus::Skipped
                                        ),
                                    }))
                    })
                    .cloned()
                    .map(|r| Row::Automation(Rc::new(r))),
            );
        }
        if page == Page::Inbox {
            rows = self.inbox_rows(rows);
        }
        if rows.is_empty() {
            rows.push(Row::Empty(match page {
                Page::Dashboard | Page::Tasks => "Your tasks will appear here.",
                Page::Inbox => "No actions match these filters.",
                Page::Decisions => "No decisions waiting for you.",
                Page::Teams => "Create an agent, then build a team.",
                Page::Workflows => "Describe a workflow to create its stages.",
                Page::Routines => "Create a routine for recurring agent work.",
                Page::Artifacts => "Documents created by your agents will appear here.",
                Page::Apps if self.control_ui.apps_busy => "Loading apps…",
                Page::Apps
                    if !self
                        .control_ui
                        .app_search
                        .read(cx)
                        .content()
                        .trim()
                        .is_empty() =>
                {
                    "No apps match your search."
                }
                Page::Apps if self.control_ui.apps_connected_only => "No connected accounts yet.",
                Page::Apps => "Save your Composio project key in Settings to load available apps.",
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
        if let Some(stages) = self.control_ui.pending_stages.take() {
            for stage in stages {
                let team_id = stage.team_id;
                let formats = stage.required_formats;
                let transition = workflow_ui::TransitionUi::new(stage.transition, window, cx);
                self.add_control_stage(
                    stage.title,
                    stage.instructions,
                    stage.requires_decision,
                    window,
                    cx,
                );
                if let Some(stage) = self.control_ui.stages.last_mut() {
                    stage.3 = team_id;
                    stage.4 = formats;
                    stage.5 = transition;
                }
            }
        }
        use std::hash::{Hash, Hasher};
        let mut key = std::collections::hash_map::DefaultHasher::new();
        page.hash(&mut key);
        self.control_ui.snapshot.revision.hash(&mut key);
        self.control_ui.activity_revision.hash(&mut key);
        self.control_ui.runs_tab.hash(&mut key);
        self.control_ui.selected_run.hash(&mut key);
        self.control_ui.automation_history.hash(&mut key);
        self.control_ui.automation_selected.hash(&mut key);
        self.control_ui.automation_definition.hash(&mut key);
        self.control_ui.automation_after.hash(&mut key);
        self.control_ui.filter_project.hash(&mut key);
        self.control_ui.filter_task.hash(&mut key);
        self.control_ui.filter_kind.hash(&mut key);
        self.control_ui.filter_status.hash(&mut key);
        self.control_ui.inbox.snapshot.revision.hash(&mut key);
        self.control_ui.inbox.loaded.hash(&mut key);
        self.control_ui.inbox.view.hash(&mut key);
        self.control_ui.apps_generation.hash(&mut key);
        self.control_ui.apps_busy.hash(&mut key);
        self.control_ui.apps_connected_only.hash(&mut key);
        self.control_ui.app_search.read(cx).content().hash(&mut key);
        for session in &self.state.sessions {
            (session.id, session.updated_at, session.status as u8).hash(&mut key);
        }
        for project in &self.state.projects {
            project.id.hash(&mut key);
        }
        for account in &self.gateway_accounts {
            (&account.id, &account.status).hash(&mut key);
        }
        for (id, runtime) in &self.runtimes {
            id.hash(&mut key);
            runtime
                .pending_permission
                .as_ref()
                .map(|p| &p.request_id)
                .hash(&mut key);
            runtime
                .pending_user_input
                .as_ref()
                .map(|p| &p.request_id)
                .hash(&mut key);
            runtime.pending_computer_approval.is_some().hash(&mut key);
            runtime.last_driver_error.hash(&mut key);
        }
        let key = key.finish();
        if self.control_ui.rows_key != key {
            let rows = Rc::new(self.control_rows(page, cx));
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
        if page == Page::Usage {
            content = content.child(self.usage_toolbar(cx));
        }
        if page == Page::Inbox {
            content = content.child(self.render_inbox_toolbar(cx));
            if let Some(error) = &self.control_ui.inbox.error {
                content = content.child(
                    div()
                        .text_size(sp(13.0))
                        .text_color(theme.danger)
                        .child(error.clone()),
                );
            }
        }
        if let Some(error) = &self.control_ui.error {
            content = content.child(
                div()
                    .text_size(sp(13.0))
                    .text_color(theme.danger)
                    .child(error.clone()),
            );
        }
        if let Some(error) = &self.control_ui.integration_error {
            content = content.child(
                div()
                    .text_size(sp(13.0))
                    .text_color(theme.danger)
                    .child(error.clone()),
            );
        }
        if let Some(error) = &self.control_ui.activity_error {
            content = content.child(
                div()
                    .text_size(sp(13.0))
                    .text_color(theme.danger)
                    .child(error.clone()),
            );
        }
        if page == Page::Teams {
            content = content.child(self.render_team_activity_header(cx));
        }
        if matches!(page, Page::Workflows | Page::Routines) && self.control_ui.automation_history {
            content = content.child(self.render_automation_header(cx));
        }
        if page == Page::Dashboard {
            let active = self.state.sessions.iter().filter(|s| s.is_busy()).count();
            let decisions = self
                .state
                .sessions
                .iter()
                .filter(|s| s.status == SessionStatus::Waiting && s.team_assignment.is_none())
                .count()
                + self.control_ui.pending_team_decisions as usize
                + self.control_ui.automation_pending as usize;
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
                    .flex()
                    .items_center()
                    .gap(px(16.0))
                    .child(icon("icons/ion.svg", 28.0, theme.accent))
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
            content = content.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(div().flex_1().min_w_0().child(
                        TextField::new("apps-search", self.control_ui.app_search.clone()).w_full(),
                    ))
                    .child(
                        self.control_button(
                            "manage-app-connections",
                            "",
                            cx,
                            |this, window, cx| {
                                this.open_settings_action(&OpenSettings, window, cx);
                                this.open_settings_page(SettingsPage::Providers, cx);
                            },
                        )
                        .size(px(32.0))
                        .p_0()
                        .border_0()
                        .justify_center()
                        .tooltip(Tooltip::text("App settings"))
                        .child(icon(
                            "icons/settings.svg",
                            16.0,
                            theme.text_secondary,
                        )),
                    ),
            );
            let connected = self.control_ui.apps_connected_only;
            content = content.child(
                div()
                    .flex()
                    .gap(px(8.0))
                    .child(
                        self.control_button("apps-all", "All apps", cx, |this, _, cx| {
                            this.control_ui.apps_connected_only = false;
                            cx.notify();
                        })
                        .border_0()
                        .bg(if !connected {
                            theme.overlay_strong
                        } else {
                            gpui::transparent_black()
                        }),
                    )
                    .child(
                        self.control_button("apps-connected", "Connected", cx, |this, _, cx| {
                            this.control_ui.apps_connected_only = true;
                            cx.notify();
                        })
                        .border_0()
                        .bg(if connected {
                            theme.overlay_strong
                        } else {
                            gpui::transparent_black()
                        }),
                    ),
            );
        }
        if matches!(page, Page::Inbox | Page::Decisions) {
            content = content.child(self.render_workspace_filters(cx));
            content = content.child(self.render_team_decision_pages(cx));
            if self.control_ui.automation_cursor.is_some()
                || !self.control_ui.automation_previous.is_empty()
            {
                content = content.child(
                    div()
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .text_size(sp(12.5))
                                .text_color(theme.text_secondary)
                                .child("Workflow runs"),
                        )
                        .child(self.render_automation_header(cx)),
                );
            }
        }
        if matches!(page, Page::Inbox | Page::Dashboard) {
            content = content.child(self.render_team_collection_pages(cx));
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
            Row::Inbox(row) => return self.render_inbox_row(row, index, cx),
            Row::InboxEvent(entry) => return self.render_inbox_history_row(entry, cx),
            Row::Usage(entry) => return self.render_usage_row(entry, cx),
            Row::Automation(run) => return self.render_automation_row(run, None, cx),
            Row::AutomationStage(run, index) => {
                return self.render_automation_row(run, Some(index), cx);
            }
            Row::TeamDecision(decision) => return self.render_team_decision_row(decision, cx),
            Row::RecoveryFile(index, row) => return self.render_team_recovery_file(index, row, cx),
            Row::RecoveryApp(row) => return self.render_team_recovery_app(row, cx),
            Row::RecoveryRemote(row) => return self.render_team_recovery_remote(row, cx),
            Row::TeamRun(run) => return self.render_team_run_row(run, cx),
            Row::Assignment(row) => return self.render_team_assignment_row(row, cx),
            Row::SourceChange(index, row) => return self.render_team_source_change(index, row, cx),
            Row::SourceDiffLine(index, line) => {
                return self.render_team_source_diff_line(index, line, cx);
            }
            Row::SourceConflict(conflict) => {
                return div()
                    .py(px(10.0))
                    .text_size(sp(13.0))
                    .text_color(theme.danger)
                    .child(conflict)
                    .into_any_element();
            }
            Row::AppPair(configs) => {
                let mut pair = div().flex().gap(px(28.0)).py(px(12.0));
                let count = configs.len();
                for config in configs {
                    let account = self
                        .control_ui
                        .app_accounts
                        .iter()
                        .find(|a| a.toolkit == config.toolkit && a.status == "ACTIVE");
                    let id = config.id.clone();
                    let mut tile = div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap(px(14.0))
                        .py(px(12.0));
                    tile = tile
                        .child(
                            div()
                                .size(px(38.0))
                                .flex_none()
                                .rounded(px(10.0))
                                .border_1()
                                .border_color(theme.border)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(icon("icons/package.svg", 20.0, theme.accent)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .text_size(sp(14.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.text)
                                        .truncate()
                                        .child(config.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(sp(12.5))
                                        .text_color(theme.text_secondary)
                                        .truncate()
                                        .child(if account.is_some() {
                                            "Connected".into()
                                        } else {
                                            config.description.clone()
                                        }),
                                ),
                        )
                        .child(self.control_button(
                            format!("app-connect-{id}"),
                            if account.is_some() {
                                "Connected"
                            } else if id.starts_with("configure:") {
                                "Configure"
                            } else if id.starts_with("available:") {
                                "Available"
                            } else {
                                "Connect"
                            },
                            cx,
                            move |this, _, cx| {
                                if account_is_active(this, &id) {
                                    return;
                                }
                                this.connect_composio_app(id.clone(), cx);
                            },
                        ));
                    pair = pair.child(tile);
                }
                if count == 1 {
                    pair = pair.child(div().flex_1());
                }
                return pair.into_any_element();
            }
            Row::Notice(notice) => {
                let project = notice
                    .project
                    .and_then(|id| self.state.projects.iter().find(|p| p.id == id))
                    .map(|p| p.name.as_str())
                    .unwrap_or("Workspace");
                let task = notice
                    .task
                    .and_then(|id| self.state.sessions.iter().find(|s| s.id == id))
                    .map(|s| s.display_title())
                    .unwrap_or("");
                let mut status = if notice.pending {
                    "Needs action"
                } else {
                    "Resolved"
                };
                if let Some(ControlRecord::Action(action)) = &notice.record {
                    status = match action.status {
                        WorkspaceActionStatus::Pending => "Needs action",
                        WorkspaceActionStatus::Accepted => "Accepted",
                        WorkspaceActionStatus::Declined => "Declined",
                    };
                    if action.status == WorkspaceActionStatus::Pending {
                        let accept = action.clone();
                        let decline = action.clone();
                        actions = actions
                            .child(self.control_button(
                                format!("action-accept-{}", action.id),
                                "Accept",
                                cx,
                                move |this, _, cx| {
                                    this.resolve_workspace_action(accept.clone(), true, cx)
                                },
                            ))
                            .child(self.control_button(
                                format!("action-decline-{}", action.id),
                                "Decline",
                                cx,
                                move |this, _, cx| {
                                    this.resolve_workspace_action(decline.clone(), false, cx)
                                },
                            ));
                    }
                }
                if let Some(run_id) = notice.run {
                    actions = actions.child(self.control_button(
                        format!("notice-run-{index}"),
                        "Open run",
                        cx,
                        move |this, _, cx| this.open_workspace_run(run_id, cx),
                    ));
                }
                if let Some(id) = notice.task {
                    actions = actions.child(self.control_button(
                        format!("notice-open-{index}"),
                        if notice.pending {
                            "Review"
                        } else {
                            "Open task"
                        },
                        cx,
                        move |this, _, cx| this.open_team_activity_task(id, cx),
                    ));
                }
                if let Some(record @ ControlRecord::Artifact(_)) = notice.record {
                    actions = actions.child(self.control_button(
                        format!("notice-document-{index}"),
                        "Save document",
                        cx,
                        move |this, _, cx| this.export_control_artifact(record.clone(), cx),
                    ));
                }
                let path = match notice.kind {
                    NoticeKind::Approval | NoticeKind::Decision => "icons/check.svg",
                    NoticeKind::Question => "icons/info.svg",
                    NoticeKind::Issue => "icons/alert.svg",
                    NoticeKind::Artifact => "icons/file.svg",
                    NoticeKind::Proposal => "icons/compose.svg",
                    _ => "icons/check.svg",
                };
                (
                    notice.title,
                    format!("{project} · {task} · {status} · {}", notice.detail),
                    path,
                )
            }
            Row::TeamFollowUp(followup) => {
                let status = match followup.status {
                    waku_client::teams::TeamFollowUpStatus::Queued => "Queued for lead",
                    waku_client::teams::TeamFollowUpStatus::Delivered => "Delivered to lead",
                    waku_client::teams::TeamFollowUpStatus::Canceled => "Canceled",
                };
                return div()
                    .w_full()
                    .px(px(16.0))
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_size(sp(12.0))
                            .text_color(theme.text_secondary)
                            .child(format!("Follow-up · {status}")),
                    )
                    .child(
                        div()
                            .mt(px(6.0))
                            .text_size(sp(13.0))
                            .text_color(theme.text)
                            .whitespace_normal()
                            .child(followup.message.content.clone()),
                    )
                    .into_any_element();
            }
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
            Row::Account(toolkit, id, status) => (
                app_name(&toolkit),
                format!("{status} · {id}"),
                "icons/package.svg",
            ),
            Row::AgentInTeam(agent, depth, parent) => {
                body = body.pl(px(22.0 + depth as f32 * 24.0));
                (
                    agent.name,
                    format!(
                        "{} · {} · {} · {}",
                        parent
                            .map(|name| format!("Reports to {name}"))
                            .unwrap_or_else(|| "Team lead".into()),
                        agent.expertise,
                        agent.provider.display_name(),
                        agent.execution.environment.label()
                    ),
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
                        format!(
                            "{} · {} · {}",
                            agent.expertise,
                            agent.provider.display_name(),
                            agent.execution.environment.label()
                        ),
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
                            "{} · {}{}",
                            if routine.paused {
                                "Paused"
                            } else {
                                "Scheduled"
                            },
                            routine_ui::schedule_label(
                                routine.interval_minutes,
                                routine.calendar.as_ref()
                            ),
                            routine
                                .last_error
                                .as_ref()
                                .map(|e| format!(" · {e}"))
                                .unwrap_or_else(|| if routine.next_run > 0 {
                                    format!(
                                        " · next {}",
                                        automation_ui::next_fire(routine.next_run)
                                    )
                                } else {
                                    String::new()
                                })
                        ),
                        "icons/rotate-cw.svg",
                    ),
                    ControlRecord::Artifact(artifact) => {
                        let detail = match &artifact.evidence {
                            Some(evidence) => format!(
                                "{} · {} bytes · {}",
                                artifact.format.to_uppercase(),
                                evidence.bytes,
                                match evidence.producer.origin {
                                    ArtifactOrigin::AgentGenerated => "Generated by agent",
                                    ArtifactOrigin::AgentEdit => "Captured agent edit",
                                    ArtifactOrigin::TeamAssignment => "Generated by team",
                                }
                            ),
                            None => format!(
                                "{} · Legacy document · integrity unavailable",
                                artifact.format.to_uppercase()
                            ),
                        };
                        (detail, "icons/file.svg")
                    }
                    ControlRecord::Action(action) => (action.detail.clone(), "icons/check.svg"),
                };
                if let ControlRecord::Team(team) = &record {
                    let id = team.id;
                    actions = actions.child(self.control_button(
                        format!("team-runs-{id}"),
                        "Runs",
                        cx,
                        move |this, _, cx| {
                            this.control_ui.runs_team = Some(id);
                            this.control_ui.runs_tab = true;
                            this.clear_team_recovery();
                            this.control_ui.selected_run = None;
                            this.control_ui.team_run = None;
                            this.control_ui.runs_after = None;
                            this.control_ui.runs_previous.clear();
                            this.refresh_team_activity(true, cx);
                            cx.notify();
                        },
                    ));
                }
                if matches!(
                    record,
                    ControlRecord::Workflow(_) | ControlRecord::Routine(_)
                ) {
                    actions = actions.child(self.control_button(
                        format!("automation-start-{id}"),
                        if self.control_ui.automation_requests.contains_key(&id) {
                            "Retry run"
                        } else {
                            "Run now"
                        },
                        cx,
                        move |this, _, cx| this.start_control_automation(id, cx),
                    ));
                    actions = actions.child(self.control_button(
                        format!("automation-history-{id}"),
                        "Runs",
                        cx,
                        move |this, _, cx| {
                            this.control_ui.automation_history = true;
                            this.control_ui.automation_selected = None;
                            this.control_ui.automation_definition = Some(id);
                            this.control_ui.automation_after = None;
                            this.control_ui.automation_previous.clear();
                            this.refresh_team_activity(true, cx);
                            cx.notify();
                        },
                    ));
                    if let ControlRecord::Routine(r) = &record {
                        let paused = r.paused;
                        actions = actions.child(self.control_button(
                            format!("routine-schedule-{id}"),
                            if paused { "Enable schedule" } else { "Pause" },
                            cx,
                            move |this, _, cx| {
                                this.control_request(
                                    Rpc::SetRoutinePaused {
                                        id,
                                        expected_revision: this.control_ui.snapshot.revision,
                                        paused: !paused,
                                    },
                                    cx,
                                )
                            },
                        ));
                    }
                }
                if matches!(record, ControlRecord::Artifact(_)) {
                    actions = actions.child(self.control_button(
                        format!("document-export-{id}"),
                        "Save document",
                        cx,
                        move |this, _, cx| this.export_control_artifact(run.clone(), cx),
                    ));
                } else {
                    if !matches!(
                        record,
                        ControlRecord::Workflow(_) | ControlRecord::Routine(_)
                    ) {
                        actions = actions.child(self.control_button(
                            format!("record-run-{id}"),
                            "New task",
                            cx,
                            move |this, _, cx| this.prepare_record_task(run.clone(), cx),
                        ));
                    }
                    actions = actions.child(self.control_button(
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
                            .truncate()
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
        if kind == FormKind::Agent {
            let weak = cx.entity().downgrade();
            let provider = self.control_ui.provider;
            let menu = self.menu_handle("control-provider-menu", cx);
            form = form
                .child(
                    div()
                        .text_size(sp(12.5))
                        .text_color(theme.text_secondary)
                        .child("Harness"),
                )
                .child(dropdown_menu(
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
                                        if this.control_ui.provider != kind {
                                            this.control_ui
                                                .model
                                                .update(cx, |input, cx| input.set_content("", cx));
                                        }
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
            let environment = self.control_ui.execution.environment;
            let weak = cx.entity().downgrade();
            let menu = self.menu_handle("control-environment-menu", cx);
            form = form
                .child(control_field(
                    "Model",
                    self.control_ui.model.clone(),
                    "control-model",
                ))
                .child(control_field(
                    "Expected output",
                    self.control_ui.expected_output.clone(),
                    "control-output",
                ))
                .child(
                    div()
                        .text_size(sp(12.5))
                        .text_color(theme.text_secondary)
                        .child("Execution environment"),
                )
                .child(dropdown_menu(
                    self.control_button(
                        "control-environment",
                        environment.label(),
                        cx,
                        |_, _, _| {},
                    ),
                    "control-environment-menu",
                    &menu,
                    MenuAlign::BelowLeft,
                    move |_| {
                        [
                            ExecutionEnvironment::Local,
                            ExecutionEnvironment::NebiusSandbox,
                        ]
                        .into_iter()
                        .map(|value| {
                            let weak = weak.clone();
                            MenuItem::new(value.label(), move |_, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    this.control_ui.execution.environment = value;
                                    cx.notify();
                                });
                            })
                            .selected(value == environment)
                        })
                        .collect()
                    },
                ))
                .child(control_field(
                    "Maximum parallel tasks",
                    self.control_ui.concurrency.clone(),
                    "control-concurrency",
                ));
            if environment == ExecutionEnvironment::NebiusSandbox {
                form=form.child(control_field("Sandbox target",self.control_ui.sandbox_target.clone(),"control-sandbox-target"))
                    .child(div().text_size(sp(12.5)).text_color(theme.text_secondary).child(if self.control_ui.execution.local_fallback {
                        "If the sandbox cannot start, use the same harness locally when its access and requirements can be satisfied."
                    } else {"Local fallback is disabled for this saved profile."}));
            }
            let access = self.control_ui.access.clone();
            form = form.child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child("Team task access"),
            );
            for (index, label, enabled) in [
                (0, "Read workspace", access.workspace_read),
                (1, "Edit workspace", access.workspace_write),
                (2, "Run commands", access.shell),
                (3, "Network", access.network),
            ] {
                form = form.child(self.control_button(
                    format!("control-access-{index}"),
                    format!("{}  {label}", if enabled { "✓" } else { "○" }),
                    cx,
                    move |this, _, cx| {
                        let access = &mut this.control_ui.access;
                        match index {
                            0 => {
                                access.workspace_read = !access.workspace_read;
                                if !access.workspace_read {
                                    access.workspace_write = false;
                                }
                            }
                            1 => {
                                access.workspace_write = !access.workspace_write;
                                if access.workspace_write {
                                    access.workspace_read = true;
                                }
                            }
                            2 => access.shell = !access.shell,
                            _ => access.network = !access.network,
                        }
                        cx.notify();
                    },
                ));
            }
        }
        if kind == FormKind::Agent {
            form = form.child(self.render_agent_app_grants(cx));
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
                .child(self.render_routine_target(cx))
                .child(self.render_routine_schedule(cx));
        }
        if kind == FormKind::Team {
            form = form.child(self.render_team_limit_inputs(cx));
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
            form = form.child(self.render_team_limit_inputs(cx));
            for (i, (title, instructions, gate, _team, _formats, _transition)) in
                self.control_ui.stages.iter().enumerate()
            {
                let gate = *gate;
                form = form.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .child(self.render_stage_team(i, cx))
                        .child(self.render_stage_documents(i, cx))
                        .child(self.render_workflow_route(i, cx))
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
                                    format!("stage-up-{i}"),
                                    "Move up",
                                    cx,
                                    move |this, _, cx| {
                                        if i > 0 {
                                            this.move_workflow_stage(i, i - 1, cx);
                                        }
                                    },
                                ))
                                .child(self.control_button(
                                    format!("stage-down-{i}"),
                                    "Move down",
                                    cx,
                                    move |this, _, cx| this.move_workflow_stage(i, i + 1, cx),
                                ))
                                .child(self.control_button(
                                    format!("stage-remove-{i}"),
                                    "Remove",
                                    cx,
                                    move |this, _, cx| {
                                        this.remove_workflow_stage(i, cx);
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
