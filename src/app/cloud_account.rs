use super::*;
use crate::credentials::cloud;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) struct CloudUiState {
    focus: [FocusHandle; 8],
    services: Option<cloud::services::ServiceControls>,
    shared_usage: Option<serde_json::Value>,
    status_error: bool,
    checked: bool,
    checking: bool,
    verified: bool,
    require_sign_in: bool,
    sign_in_pending: bool,
    pub status: cloud::AccountStatus,
    pub busy: bool,
    pub generation: u64,
    pub cancel: Option<Arc<AtomicBool>>,
    pub feedback: Option<String>,
}
impl CloudUiState {
    pub(super) fn new(cx: &mut App) -> Self {
        Self {
            focus: std::array::from_fn(|_| cx.focus_handle()),
            services: None,
            shared_usage: None,
            status_error: false,
            checked: false,
            checking: false,
            verified: false,
            require_sign_in: false,
            sign_in_pending: false,
            status: cloud::AccountStatus::default(),
            busy: false,
            generation: 0,
            cancel: None,
            feedback: None,
        }
    }
}
impl Drop for CloudUiState {
    fn drop(&mut self) {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Release);
        }
    }
}
#[derive(Clone, Copy)]
pub(super) enum CloudAction {
    SignIn,
    Connect,
    Disconnect,
    SignOut,
    Refresh,
    Cancel,
    ToggleCloud,
    ToggleTokenFactory,
}

impl Waku {
    pub(super) fn shared_inference_enabled(&self) -> bool {
        self.cloud_ui.status.signed_in
            && self
                .cloud_ui
                .services
                .as_ref()
                .is_some_and(|s| s.token_factory && s.token_factory_available)
    }
    pub(super) fn ide_account_unlocked(&self) -> bool {
        self.cloud_ui.checked
            && self.cloud_ui.verified
            && !self.cloud_ui.status_error
            && !self.cloud_ui.require_sign_in
            && self.cloud_ui.status.signed_in
    }
    pub(super) fn start_account_gate(&mut self, cx: &mut Context<Self>) {
        self.refresh_cloud_account(cx);
        cx.spawn(async move |this, cx| {
            loop {
                smol::Timer::after(Duration::from_secs(60)).await;
                if this
                    .update(cx, |this, cx| this.refresh_cloud_account(cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }
    pub(super) fn refresh_cloud_account(&mut self, cx: &mut Context<Self>) {
        if self.cloud_ui.busy || self.cloud_ui.checking {
            return;
        }
        self.cloud_ui.generation = self.cloud_ui.generation.wrapping_add(1);
        let generation = self.cloud_ui.generation;
        self.cloud_ui.checking = true;
        cx.spawn(async move |view, cx| {
            let status = cx
                .background_executor()
                .spawn(async {
                    let status = cloud::verified_status()?;
                    let services = if status.signed_in {
                        cloud::services::controls().ok()
                    } else {
                        None
                    };
                    let usage = if status.signed_in {
                        cloud::services::request("GET", "/v1/services/usage", None).ok()
                    } else {
                        None
                    };
                    anyhow::Ok((status, services, usage))
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                if this.cloud_ui.generation != generation {
                    return;
                }
                this.cloud_ui.checking = false;
                this.cloud_ui.checked = true;
                match status {
                    Ok((status, services, usage)) => {
                        let refresh_models = this.cloud_ui.status.workspace_id
                            != status.workspace_id
                            || this.cloud_ui.services.as_ref().map(|s| s.token_factory)
                                != services.as_ref().map(|s| s.token_factory);
                        this.cloud_ui.shared_usage = usage;
                        this.cloud_ui.services = services;
                        if refresh_models {
                            this.refresh_provider_model_discovery(ProviderKind::Nebius);
                        }
                        if this.cloud_ui.status.workspace_id != status.workspace_id {
                            this.control_ui.clear_hosting_account(cx);
                        }
                        this.cloud_ui.status = status;
                        this.cloud_ui.status_error = false;
                        this.cloud_ui.verified = this.cloud_ui.status.signed_in;
                    }
                    Err(error) => {
                        this.cloud_ui.services = None;
                        this.cloud_ui.shared_usage = None;
                        this.cloud_ui.status_error = true;
                        this.cloud_ui.verified = false;
                        this.cloud_ui.status.configured = true;
                        this.cloud_ui.feedback = Some(error.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn finish_cloud_action(
        &mut self,
        generation: u64,
        result: anyhow::Result<(cloud::AccountStatus, String)>,
        cx: &mut Context<Self>,
    ) {
        if self.cloud_ui.generation != generation {
            return;
        }
        let signing_in = self.cloud_ui.sign_in_pending;
        self.cloud_ui.sign_in_pending = false;
        self.cloud_ui.busy = false;
        self.cloud_ui.checking = false;
        self.cloud_ui.cancel = None;
        self.credential_busy
            .remove(&crate::credentials::CredentialKind::GatewayPairingToken);
        match result {
            Ok((status, message)) => {
                if signing_in {
                    self.cloud_ui.require_sign_in = false;
                }
                if self.cloud_ui.status.workspace_id != status.workspace_id {
                    self.control_ui.clear_hosting_account(cx);
                    self.cloud_ui.services = None;
                    self.cloud_ui.shared_usage = None;
                }
                if !status.signed_in {
                    self.cancel_speech_recording(cx);
                }
                self.cloud_ui.status = status;
                self.cloud_ui.status_error = false;
                self.cloud_ui.feedback = Some(message);
                self.republish_pending_gateway_interactions();
            }
            Err(error) => {
                self.cloud_ui.feedback = Some(error.to_string());
                self.refresh_cloud_account(cx);
            }
        }
        self.refresh_cloud_account(cx);
        cx.notify();
    }
    pub(super) fn run_cloud_action(&mut self, action: CloudAction, cx: &mut Context<Self>) {
        if matches!(
            action,
            CloudAction::ToggleCloud | CloudAction::ToggleTokenFactory
        ) {
            let Some(services) = self.cloud_ui.services.as_ref().filter(|s| s.can_manage) else {
                return;
            };
            if self.cloud_ui.busy || self.cloud_ui.checking {
                return;
            }
            let (service, enabled) = if matches!(action, CloudAction::ToggleCloud) {
                ("cloud", !services.cloud)
            } else {
                ("tokenFactory", !services.token_factory)
            };
            self.cloud_ui.busy = true;
            self.cloud_ui.generation = self.cloud_ui.generation.wrapping_add(1);
            let generation = self.cloud_ui.generation;
            cx.spawn(async move |view, cx| {
                let result = cx.background_executor().spawn(async move { cloud::services::set_control(service, enabled) }).await;
                let _ = view.update(cx, |this, cx| {
                    if this.cloud_ui.generation != generation { return; }
                    this.cloud_ui.busy = false;
                    match result {
                        Ok(services) => {
                            this.cloud_ui.services = Some(services);
                            this.cloud_ui.feedback = Some(if service == "cloud" && !enabled { "Cloud work is blocked. VM shutdown still needs confirmation from the controller." } else { "Shared service permission updated." }.into());
                        }
                        Err(error) => { this.cloud_ui.feedback = Some(error.to_string()); }
                    }
                    cx.notify();
                });
            }).detach();
            cx.notify();
            return;
        }
        if matches!(action, CloudAction::Cancel) {
            if let Some(cancel) = &self.cloud_ui.cancel {
                cancel.store(true, Ordering::Release);
                self.cloud_ui.feedback = Some("Cancelling sign-in…".into());
            }
            cx.notify();
            return;
        }
        let kind = crate::credentials::CredentialKind::GatewayPairingToken;
        if self.cloud_ui.busy || !self.credential_busy.insert(kind) {
            return;
        }
        self.cloud_ui.busy = true;
        self.cloud_ui.checking = false;
        if matches!(action, CloudAction::SignOut) {
            self.cloud_ui.require_sign_in = true;
            self.cloud_ui.verified = false;
            self.cloud_ui.services = None;
            self.cancel_speech_recording(cx);
        }
        self.cloud_ui.sign_in_pending = matches!(action, CloudAction::SignIn);
        if matches!(action, CloudAction::SignIn) {
            self.cloud_ui.require_sign_in = true;
            self.cloud_ui.verified = false;
        }
        self.cloud_ui.generation = self.cloud_ui.generation.wrapping_add(1);
        let generation = self.cloud_ui.generation;
        self.cloud_ui.feedback = Some(
            if matches!(action, CloudAction::SignIn) {
                "Opening secure sign-in in your browser…"
            } else {
                "Updating cloud connection…"
            }
            .into(),
        );
        cx.notify();
        if matches!(action, CloudAction::SignIn) {
            cx.spawn(async move |view, cx| {
                let attempt = cx.background_executor().spawn(async { cloud::begin_sign_in() }).await;
                let _ = view.update(cx, |this, cx| {
                    if this.cloud_ui.generation != generation { return; }
                    match attempt {
                        Err(error) => this.finish_cloud_action(generation, Err(error), cx),
                        Ok(attempt) => {
                            this.cloud_ui.cancel = Some(attempt.cancellation());
                            this.cloud_ui.feedback = Some("Finish sign-in in your browser, then return to Ion.".into());
                            cx.open_url(attempt.authorize_url());
                            cx.notify();
                            cx.spawn(async move |view, cx| {
                                let result = cx.background_executor().spawn(async move {
                                    attempt.finish().map(|status| (status, "Signed in. Connect this computer to share its tasks.".into()))
                                }).await;
                                let _ = view.update(cx, |this, cx| this.finish_cloud_action(generation, result, cx));
                            }).detach();
                        }
                    }
                });
            }).detach();
        } else {
            cx.spawn(async move |view, cx| {
                let result = cx.background_executor().spawn(async move {
                    let message = match action {
                        CloudAction::Connect => {
                            cloud::connect_computer()?;
                            "Computer linked. Tasks and review requests can now reach your account.".to_owned()
                        }
                        CloudAction::Disconnect => {
                            if cloud::disconnect_computer()? { "Computer disconnected.".to_owned() }
                            else { "Cloud publishing stopped here. Remote revocation could not be confirmed; review this device when online.".to_owned() }
                        }
                        CloudAction::SignOut => {
                            if cloud::sign_out()? { "Signed out. This computer is no longer linked.".to_owned() }
                            else { "Signed out on this computer. Remote revocation could not be confirmed; review this device when online.".to_owned() }
                        }
                        CloudAction::Refresh => {
                            if cloud::verify_connection()? { "Cloud connection verified.".to_owned() }
                            else { "Account status refreshed.".to_owned() }
                        }
                        _ => unreachable!(),
                    };
                    anyhow::Ok((cloud::status()?, message))
                }).await;
                let _ = view.update(cx, |this, cx| this.finish_cloud_action(generation, result, cx));
            }).detach();
        }
    }
    fn cloud_button(
        &self,
        label: &'static str,
        action: CloudAction,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        let unavailable = match action {
            CloudAction::ToggleCloud => self
                .cloud_ui
                .services
                .as_ref()
                .is_none_or(|s| !s.cloud_available && !s.cloud),
            CloudAction::ToggleTokenFactory => self
                .cloud_ui
                .services
                .as_ref()
                .is_none_or(|s| !s.token_factory_available && !s.token_factory),
            _ => false,
        };
        let disabled =
            unavailable || (self.cloud_ui.busy && !matches!(action, CloudAction::Cancel));
        div()
            .id(SharedString::from(format!("cloud-{label}")))
            .track_focus(&self.cloud_ui.focus[action as usize])
            .tab_group()
            .tab_index(0)
            .tab_stop(true)
            .focus_visible(|style| style.border_color(theme.accent))
            .min_h(px(34.0))
            .px(px(12.0))
            .rounded(px(7.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .items_center()
            .text_size(sp(12.5))
            .text_color(theme.text)
            .opacity(if disabled { 0.5 } else { 1.0 })
            .hover(|style| style.bg(theme.overlay))
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                if !disabled {
                    this.run_cloud_action(action, cx);
                }
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                if !disabled
                    && !event.keystroke.modifiers.modified()
                    && matches!(event.keystroke.key.as_str(), "enter" | "space")
                {
                    this.run_cloud_action(action, cx);
                    cx.stop_propagation();
                }
            }))
            .into_any_element()
    }
    pub(super) fn render_account_gate(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        let mut actions = div().flex().flex_wrap().gap(px(8.0));
        if self.cloud_ui.status.configured || self.cloud_ui.status_error {
            actions = actions.child(self.cloud_button("Sign in", CloudAction::SignIn, cx));
        }
        if self.cloud_ui.cancel.is_some() {
            actions = actions.child(self.cloud_button("Cancel sign-in", CloudAction::Cancel, cx));
        }
        if self.cloud_ui.checked && !self.cloud_ui.busy {
            actions = actions.child(self.cloud_button("Retry", CloudAction::Refresh, cx));
        }
        let copy = if !self.cloud_ui.checked || self.cloud_ui.checking {
            "Checking your account…"
        } else if !self.cloud_ui.status.configured {
            "Sign-in is not configured in this build. Install an updated Ion build connected to Ion Cloud."
        } else {
            "Sign in securely in your browser to open your workspace."
        };
        let header =
            div()
                .h(px(48.0))
                .flex_none()
                .flex()
                .items_center()
                .px(px(16.0))
                .when(cfg!(target_os = "macos"), |style| style.pl(px(80.0)))
                .children(self.render_client_window_controls(
                    super::window_chrome::WindowControlSide::Left,
                    window,
                    cx,
                ))
                .child(self.window_drag_region(
                    div().id("account-window-drag-region").flex_1().h_full(),
                    cx,
                ))
                .children(self.render_client_window_controls(
                    super::window_chrome::WindowControlSide::Right,
                    window,
                    cx,
                ));
        div()
            .size_full()
            .bg(theme.surface)
            .text_color(theme.text)
            .font_family(".SystemUIFont")
            .flex()
            .flex_col()
            .child(header)
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .p(px(28.0))
                    .child(
                        div()
                            .w_full()
                            .max_w(px(440.0))
                            .flex()
                            .flex_col()
                            .gap(px(18.0))
                            .child(icon("icons/ion.svg", 36.0, theme.accent))
                            .child(
                                div()
                                    .text_size(sp(26.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text)
                                    .child("Sign in to Ion"),
                            )
                            .child(
                                div()
                                    .text_size(sp(14.0))
                                    .text_color(theme.text_secondary)
                                    .child(copy),
                            )
                            .child(actions)
                            .when_some(self.cloud_ui.feedback.clone(), |view, feedback| {
                                view.child(
                                    div()
                                        .text_size(sp(13.0))
                                        .text_color(theme.text_secondary)
                                        .child(feedback),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }
    pub(super) fn render_shared_cloud_usage(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let mut content = div().flex().flex_col().gap(px(6.0));
        if let Some(usage) = &self.cloud_ui.shared_usage {
            let spent = usage["tokenFactory"]["account"]["spent"]
                .as_u64()
                .unwrap_or(0);
            let held = usage["tokenFactory"]["account"]["held"]
                .as_u64()
                .unwrap_or(0);
            let vm = usage["compute"]["state"].as_str().unwrap_or("UNKNOWN");
            content = content.child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(format!(
                        "Shared Token Factory: ${:.4} used · ${:.4} reserved · Cloud: {vm}",
                        spent as f64 / 1_000_000.0,
                        held as f64 / 1_000_000.0
                    )),
            );
        }
        if let Some(note) = self
            .cloud_ui
            .shared_usage
            .as_ref()
            .and_then(|v| v["compute"]["note"].as_str())
        {
            content = content.child(
                div()
                    .text_size(sp(12.0))
                    .text_color(theme.text_secondary)
                    .child(note.to_owned()),
            );
        }
        content.into_any_element()
    }
    pub(super) fn render_cloud_account(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let status = &self.cloud_ui.status;
        let label = if !status.configured {
            "Unavailable in this build"
        } else if status.connected {
            "Computer linked"
        } else if status.signed_in {
            "Signed in"
        } else {
            "Signed out"
        };
        let mut content = div()
            .w_full()
            .p(px(18.0))
            .rounded(px(12.0))
            .bg(theme.raised)
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(
                div()
                    .text_size(sp(14.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child("Ion Cloud"),
            )
            .child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(label),
            );
        if status.configured {
            content = content.child(div().text_size(sp(12.5)).text_color(theme.text_secondary)
                .child("Sign in, then choose whether this computer shares tasks and review requests with your account."));
            let mut actions = div().flex().flex_wrap().gap(px(8.0));
            if !status.signed_in {
                actions = actions.child(self.cloud_button("Sign in", CloudAction::SignIn, cx));
                if self.cloud_ui.status_error {
                    actions = actions.child(self.cloud_button(
                        "Clear saved sign-in",
                        CloudAction::SignOut,
                        cx,
                    ));
                }
            } else {
                actions = if status.connected {
                    actions.child(self.cloud_button(
                        "Disconnect computer",
                        CloudAction::Disconnect,
                        cx,
                    ))
                } else {
                    actions.child(self.cloud_button(
                        "Connect this computer",
                        CloudAction::Connect,
                        cx,
                    ))
                };
                actions = actions.child(self.cloud_button("Sign out", CloudAction::SignOut, cx));
            }
            actions = actions.child(self.cloud_button("Refresh", CloudAction::Refresh, cx));
            if self.cloud_ui.cancel.is_some() {
                actions =
                    actions.child(self.cloud_button("Cancel sign-in", CloudAction::Cancel, cx));
            }
            if status.signed_in {
                actions = actions.child(self.control_button(
                    "account-deployments",
                    "Deployments",
                    cx,
                    |this, _, cx| {
                        this.settings_page = None;
                        this.open_hosting_library(cx);
                    },
                ));
            }
            content = content.child(actions);
            if status.signed_in {
                if let Some(services) = &self.cloud_ui.services {
                    for (label, enabled, action) in [
                        (
                            "Shared Nebius Cloud",
                            services.cloud,
                            CloudAction::ToggleCloud,
                        ),
                        (
                            "Shared Token Factory",
                            services.token_factory,
                            CloudAction::ToggleTokenFactory,
                        ),
                    ] {
                        let mut row =
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap(px(12.0))
                                .child(div().text_color(theme.text).text_size(sp(12.5)).child(
                                    format!("{label}: {}", if enabled { "ON" } else { "OFF" }),
                                ));
                        if services.can_manage {
                            row = row.child(self.cloud_button(
                                if enabled {
                                    match action {
                                        CloudAction::ToggleCloud => "Turn Cloud off",
                                        _ => "Turn Token Factory off",
                                    }
                                } else {
                                    match action {
                                        CloudAction::ToggleCloud => "Turn Cloud on",
                                        _ => "Turn Token Factory on",
                                    }
                                },
                                action,
                                cx,
                            ));
                        }
                        content = content.child(row);
                    }
                    content = content.child(div().text_size(sp(12.0)).text_color(theme.text_secondary).child("Shared services use owner-configured spending limits. Cloud ON permits on-demand work while the operator controller is online. A configured personal provider key is used first, with its own billing."));
                    if !services.can_manage {
                        content = content.child(
                            div()
                                .text_size(sp(12.0))
                                .text_color(theme.text_secondary)
                                .child(
                                    "These shared service permissions are managed by the owner.",
                                ),
                        );
                    }
                } else {
                    content = content.child(div().text_size(sp(12.0)).text_color(theme.text_secondary).child("Shared service status is unavailable. Refresh after the gateway is configured."));
                }
            }
        } else {
            content = content.child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(
                        "Sign-in is required. Install an updated Ion build connected to Ion Cloud.",
                    ),
            );
        }
        if let Some(feedback) = &self.cloud_ui.feedback {
            content = content.child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text)
                    .child(feedback.clone()),
            );
        }
        content.into_any_element()
    }
}
