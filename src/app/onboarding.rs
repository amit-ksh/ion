//! Skippable first-launch setup. Credential access stays on existing workers.
use super::settings::{CredentialAction, GatewayAction};
use super::*;
use crate::credentials::{CredentialKind as Kind, CredentialStatus};
use waku_client::onboarding::{Progress, Step};

pub(super) struct OnboardingUi {
    pub open: bool,
    step: Step,
    focus: FocusHandle,
    indicator_focus: FocusHandle,
    scroll: ScrollHandle,
    pub(super) failed_tests: HashSet<Kind>,
}
impl OnboardingUi {
    pub fn new(open: bool, step: Step, cx: &mut App) -> Self {
        Self {
            open,
            step,
            focus: cx.focus_handle(),
            indicator_focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            failed_tests: HashSet::new(),
        }
    }
}
fn connection(step: Step) -> Option<Kind> {
    match step {
        Step::Welcome => None,
        Step::Models => Some(Kind::NebiusTokenFactoryApiKey),
        Step::Apps => Some(Kind::ComposioScopedProjectKey),
        Step::Speech => None,
        Step::Gateway => Some(Kind::GatewayPairingToken),
    }
}
fn title(step: Step) -> &'static str {
    match step {
        Step::Welcome => "Welcome to Ion",
        Step::Models => "AI providers",
        Step::Apps => "Connected apps",
        Step::Speech => "Speech",
        Step::Gateway => "Remote access",
    }
}
fn index(step: Step) -> usize {
    match step {
        Step::Welcome => 0,
        Step::Models => 1,
        Step::Apps => 2,
        Step::Speech => 3,
        Step::Gateway => 4,
    }
}
fn step_icon(step: Step) -> &'static str {
    match step {
        Step::Welcome => "icons/ion.svg",
        Step::Models => "icons/bot.svg",
        Step::Apps => "icons/package.svg",
        Step::Speech => "icons/mic.svg",
        Step::Gateway => "icons/server.svg",
    }
}
impl Waku {
    pub(super) fn initialize_onboarding(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.onboarding_ui.open {
            self.state
                .set_onboarding_progress(Progress::new(self.onboarding_ui.step));
            self.save();
            self.onboarding_ui.focus.focus(window, cx);
        }
        // One bounded vault snapshot; frames only read these four cached states.
        self.refresh_credential_statuses(cx);
    }
    fn setup_needs_attention(&self, step: Step) -> bool {
        if step == Step::Models && self.shared_inference_enabled() { return false; }
        if step == Step::Gateway && self.cloud_ui.status.connected {
            return false;
        }
        let Some(kind) = connection(step) else {
            return false;
        };
        self.onboarding_ui.failed_tests.contains(&kind)
            || !matches!(
                self.credential_statuses.get(&kind),
                Some(CredentialStatus::StoredInVault | CredentialStatus::ManagedFile)
            )
    }
    fn setup_pending_count(&self) -> usize {
        Step::ALL
            .iter()
            .copied()
            .filter(|step| self.setup_needs_attention(*step))
            .count()
    }
    fn setup_status(&self, kind: Kind) -> &'static str {
        if kind == Kind::NebiusTokenFactoryApiKey && self.shared_inference_enabled() {
            return "Shared Token Factory is enabled. A personal key is optional.";
        }
        if kind == Kind::GatewayPairingToken && self.cloud_ui.status.connected {
            return "This computer is linked to Ion Cloud.";
        }
        if self.credential_busy.contains(&kind) {
            return "Connection request in progress…";
        }
        if self.onboarding_ui.failed_tests.contains(&kind) {
            return "Connection test failed. Update the details and test again.";
        }
        match self.credential_statuses.get(&kind) {
            None => "Checking saved details…",
            Some(CredentialStatus::Missing) => "Not configured",
            Some(CredentialStatus::StoredInVault) => {
                "Saved in your OS credential vault. Test to verify the connection."
            }
            Some(CredentialStatus::ManagedFile) => {
                "Configured by your private service file. Test to verify the connection."
            }
            Some(CredentialStatus::MemoryOnly) => {
                "Saved for this app session only. An available OS vault is needed to keep these details and share keys with the execution service."
            }
        }
    }
    pub(super) fn open_onboarding(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let step = Step::ALL
            .iter()
            .copied()
            .find(|step| self.setup_needs_attention(*step))
            .unwrap_or(Step::Welcome);
        self.settings_page = None;
        self.close_command_palette(window, cx);
        self.onboarding_ui.open = true;
        self.set_setup_step(step, window, cx);
        self.refresh_credential_statuses(cx);
    }
    fn set_setup_step(&mut self, step: Step, window: &mut Window, cx: &mut Context<Self>) {
        self.onboarding_ui.step = step;
        self.onboarding_ui.scroll.set_offset(gpui::Point::default());
        self.state.set_onboarding_progress(Progress::new(step));
        self.save();
        self.onboarding_ui.focus.focus(window, cx);
        if step == Step::Speech {
            self.refresh_speech_runtime_status(cx);
        }
        cx.notify();
    }
    fn leave_setup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.onboarding_ui.open = false;
        self.state
            .set_onboarding_progress(Progress::new(self.onboarding_ui.step).with_dismissed(true));
        // Unsaved secrets never survive skipping the form in a hidden input.
        for input in self.credential_inputs.values() {
            input.update(cx, |input, cx| input.clear(cx));
        }
        self.gateway_invite_input
            .update(cx, |input, cx| input.clear(cx));
        self.save();
        if self.setup_pending_count() > 0 {
            self.onboarding_ui.indicator_focus.focus(window, cx);
        } else {
            self.composer.read(cx).focus().focus(window, cx);
        }
        cx.notify();
    }
    fn skip_setup_step(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(kind) = connection(self.onboarding_ui.step)
            && let Some(input) = self.credential_inputs.get(&kind)
        {
            input.update(cx, |input, cx| input.clear(cx));
        }
        if self.onboarding_ui.step == Step::Gateway {
            self.gateway_invite_input
                .update(cx, |input, cx| input.clear(cx));
        }
        self.next_setup_step(window, cx);
    }
    fn next_setup_step(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.onboarding_ui.step.next() {
            Some(step) => self.set_setup_step(step, window, cx),
            None => self.leave_setup(window, cx),
        }
    }
    fn setup_settings(&mut self, page: SettingsPage, window: &mut Window, cx: &mut Context<Self>) {
        self.leave_setup(window, cx);
        self.open_settings_page(page, cx);
    }
    fn setup_apps(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.leave_setup(window, cx);
        self.open_workspace_page(workspace_control::Page::Apps, cx);
    }
    pub(super) fn render_setup_indicator(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.onboarding_ui.open {
            return None;
        }
        let pending = self.setup_pending_count();
        if pending == 0 {
            return None;
        }
        let theme = Theme::current(cx);
        let checking = Step::ALL
            .into_iter()
            .filter_map(connection)
            .any(|kind| !self.credential_statuses.contains_key(&kind));
        let label = if checking {
            "Finish setup".to_owned()
        } else {
            format!("Finish setup · {pending}")
        };
        Some(
            div()
                .id("ion-setup-indicator")
                .track_focus(&self.onboarding_ui.indicator_focus)
                .tab_index(0)
                .tab_stop(true)
                .h(px(28.0))
                .flex_none()
                .px(px(10.0))
                .rounded(px(7.0))
                .flex()
                .items_center()
                .gap(px(8.0))
                .text_size(sp(12.5))
                .text_color(theme.text)
                .bg(theme.overlay)
                .hover(|s| s.bg(theme.overlay_strong))
                .focus_visible(|s| s.border_1().border_color(theme.accent))
                .child(icon("icons/settings.svg", 15.0, theme.accent))
                .child(label)
                .tooltip(Tooltip::text(
                    "Add missing connection details. Every setup step is optional.",
                ))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(|this, _, window, cx| this.open_onboarding(window, cx)))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    if !event.keystroke.modifiers.modified()
                        && matches!(event.keystroke.key.as_str(), "enter" | "space")
                    {
                        this.open_onboarding(window, cx);
                        cx.stop_propagation();
                    }
                }))
                .into_any_element(),
        )
    }
    pub(super) fn render_setup_launcher(&self, cx: &mut Context<Self>) -> AnyElement {
        self.control_button(
            "open-ion-setup",
            "Connection setup",
            cx,
            |this, window, cx| this.open_onboarding(window, cx),
        )
        .into_any_element()
    }
    fn setup_credential_form(
        &self,
        kind: Kind,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = Theme::current(cx);
        let mut form = div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(
                div()
                    .text_size(sp(13.0))
                    .font_weight(FontWeight::MEDIUM)
                    .child(label),
            )
            .child(
                TextField::new(
                    SharedString::from(format!("setup-credential-{}", kind.account())),
                    self.credential_inputs[&kind].clone(),
                )
                .comfortable()
                .icon("icons/lock.svg", 16.0)
                .w_full(),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.0))
                    .child(self.credential_button(kind, CredentialAction::Set, "Save key", cx))
                    .child(self.credential_button(
                        kind,
                        CredentialAction::Test,
                        "Test connection",
                        cx,
                    )),
            )
            .child(
                div()
                    .text_size(sp(13.0))
                    .text_color(theme.text_secondary)
                    .child(self.setup_status(kind)),
            );
        if let Some(feedback) = self.credential_feedback.get(&kind) {
            form = form.child(
                div()
                    .text_size(sp(13.0))
                    .text_color(theme.text_secondary)
                    .child(feedback.clone()),
            );
        }
        form
    }
    fn setup_field(
        &self,
        label: &'static str,
        id: &'static str,
        input: Entity<TextInput>,
        image: &'static str,
        cx: &App,
    ) -> Div {
        let theme = Theme::current(cx);
        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(sp(13.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child(label),
            )
            .child(
                TextField::new(id, input)
                    .comfortable()
                    .icon(image, 16.0)
                    .w_full(),
            )
    }
    fn setup_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.modifiers.modified() {
            return;
        }
        if event.keystroke.key == "escape" {
            self.leave_setup(window, cx);
            cx.stop_propagation();
            return;
        }
        // Keep text-editing keys with the field; page navigation also works
        // when a button is focused.
        let editing = self
            .credential_inputs
            .values()
            .any(|input| input.read(cx).focus().is_focused(window))
            || [
                &self.speech_url_input,
                &self.gateway_url_input,
                &self.gateway_invite_input,
            ]
            .into_iter()
            .any(|input| input.read(cx).focus().is_focused(window));
        if editing {
            return;
        }
        let scroll = &self.onboarding_ui.scroll;
        let offset = scroll.offset();
        let max = scroll.max_offset().y;
        let next = match event.keystroke.key.as_str() {
            "up" => offset.y + px(40.0),
            "down" => offset.y - px(40.0),
            "home" => Pixels::ZERO,
            "end" => -max,
            "pageup" => offset.y + scroll.bounds().size.height * 0.9,
            "pagedown" => offset.y - scroll.bounds().size.height * 0.9,
            _ => return,
        }
        .clamp(-max, Pixels::ZERO);
        scroll.set_offset(point(offset.x, next));
        cx.stop_propagation();
        cx.notify();
    }
    pub(super) fn render_onboarding(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let step = self.onboarding_ui.step;
        let client_corners = match window.window_decorations() {
            gpui::Decorations::Client { tiling } => Some(tiling),
            _ => None,
        };
        let header = div()
            .flex()
            .items_center()
            .h(px(48.0))
            .gap(px(10.0))
            .px(px(16.0))
            .when(cfg!(target_os = "macos"), |s| s.pl(px(80.0)))
            .children(self.render_client_window_controls(
                super::window_chrome::WindowControlSide::Left,
                window,
                cx,
            ))
            .child(
                self.window_drag_region(
                    div()
                        .id("setup-window-drag-region")
                        .flex_1()
                        .h_full()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .child(icon("icons/ion.svg", 20.0, theme.accent))
                        .child(div().text_size(sp(14.0)).child("Set up Ion")),
                    cx,
                ),
            )
            .children(self.render_client_window_controls(
                super::window_chrome::WindowControlSide::Right,
                window,
                cx,
            ));
        let mut form = div()
            .w_full()
            .max_w(px(680.0))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(20.0))
            .p(px(28.0))
            .rounded(px(13.0))
            .bg(theme.raised)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .text_size(sp(24.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(icon(step_icon(step), 24.0, theme.accent))
                    .child(title(step)),
            );
        if step != Step::Welcome {
            let mut navigation = div().flex().flex_wrap().gap(px(8.0));
            for next in Step::ALL.into_iter().filter(|s| *s != Step::Welcome) {
                let label = title(next);
                navigation = navigation.child(
                    self.control_button(
                        format!("setup-step-{}", index(next)),
                        label,
                        cx,
                        move |this, window, cx| this.set_setup_step(next, window, cx),
                    )
                    .h(px(36.0))
                    .text_size(sp(12.0))
                    .child(icon(
                        if !self.setup_needs_attention(next) {
                            "icons/check.svg"
                        } else {
                            step_icon(next)
                        },
                        14.0,
                        if next == step {
                            theme.accent
                        } else {
                            theme.text_secondary
                        },
                    ))
                    .bg(if next == step {
                        theme.overlay_strong
                    } else {
                        gpui::transparent_black()
                    })
                    .border_color(if next == step {
                        theme.accent
                    } else {
                        theme.border_strong
                    }),
                );
            }
            form = form.child(navigation);
        }
        match step {
            Step::Welcome => {
                form=form.child(div().text_size(sp(15.0)).text_color(theme.text_secondary).child("Connect your AI provider, apps and devices. Skip any step and add the details later."));
                for (image, label, detail) in [
                    (
                        "icons/bot.svg",
                        "AI providers",
                        "Nebius models or a coding harness you already use",
                    ),
                    (
                        "icons/package.svg",
                        "Connected apps",
                        "App access for your tasks and agent teams",
                    ),
                    (
                        "icons/mic.svg",
                        "Speech",
                        "Local Parakeet dictation for desktop and paired phones",
                    ),
                    (
                        "icons/server.svg",
                        "Remote access",
                        "Sign in to Ion Cloud or connect your own gateway",
                    ),
                ] {
                    form = form.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(14.0))
                            .child(icon(image, 20.0, theme.text_secondary))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.0))
                                    .child(div().text_size(sp(15.0)).child(label))
                                    .child(
                                        div()
                                            .text_size(sp(13.0))
                                            .text_color(theme.text_secondary)
                                            .child(detail),
                                    ),
                            ),
                    );
                }
            }
            Step::Models => {
                form=form.child(div().text_size(sp(15.0)).text_color(theme.text_secondary).child("Use shared Token Factory when enabled by the owner, or add your own key for personal billing. Installed coding harnesses use their own sign-in."))
                    .child(self.setup_credential_form(Kind::NebiusTokenFactoryApiKey,"Nebius API key",cx))
                    .child(self.control_button("setup-harnesses","Configure harnesses",cx,|this,window,cx|this.setup_settings(SettingsPage::Providers,window,cx)))
                    .child(div().text_size(sp(13.0)).text_color(theme.text_secondary).child("Cloud execution targets, permissions and budgets are configured separately for each agent. Saving a model key does not enable cloud spending."));
            }
            Step::Apps => {
                form=form.child(div().text_size(sp(15.0)).text_color(theme.text_secondary).child("Add your Composio project key so agents can work with your apps. Choose apps and sign in from Apps after saving the key."))
                    .child(self.setup_credential_form(Kind::ComposioScopedProjectKey,"Composio scoped project key",cx))
                    .child(self.control_button("setup-connect-apps","Connect apps",cx,|this,window,cx|this.setup_apps(window,cx)));
            }
            Step::Speech => {
                form = form
                    .child(div().text_size(sp(14.0)).line_height(sp(21.0)).text_color(theme.text_secondary)
                        .child("Dictation uses NVIDIA Parakeet TDT-CTC 110M on this computer. Download the model now, or let Ion download it when you first use the microphone. The download is about 146 MB."))
                    .child(div().text_size(sp(13.0)).text_color(theme.text_secondary)
                        .child("After download, speech recognition runs on this computer. Phone dictation requires this paired desktop to be online. English dictation only."))
                    .child(self.control_button("setup-download-speech", if self.speech_busy { "Cancel download" } else { "Download / verify Parakeet" }, cx, |this, _, cx| {
                        if this.speech_busy { this.cancel_speech_recording(cx); } else { this.prepare_local_speech_model(cx); }
                    }));
                if let Some(feedback) = &self.speech_feedback {
                    form = form.child(div().text_size(sp(13.0)).text_color(theme.text_secondary).child(feedback.clone()));
                }
                if let Some(status) = &self.speech_runtime_status {
                    form = form.child(
                        div()
                            .text_size(sp(13.0))
                            .text_color(theme.text_secondary)
                            .child(status.clone()),
                    );
                }
            }
            Step::Gateway => {
                form=form.child(self.render_cloud_account(cx))
                    .child(div().text_size(sp(14.0)).font_weight(FontWeight::MEDIUM).child("Your own gateway"))
                    .child(div().text_size(sp(13.0)).line_height(sp(20.0)).text_color(theme.text_secondary).child("Optional: connect a self-hosted gateway using its HTTPS URL and owner invite. Disconnect Ion Cloud before pairing it."))
                    .child(self.setup_field("Gateway URL", "setup-gateway-url", self.gateway_url_input.clone(), "icons/server.svg", cx))
                    .child(self.setup_field("One-use pairing invite", "setup-gateway-invite", self.gateway_invite_input.clone(), "icons/lock.svg", cx))
                    .child(div().flex().flex_wrap().gap(px(8.0))
                        .child(self.gateway_button(GatewayAction::Pair,"Pair gateway",cx))
                        .child(self.credential_button(Kind::GatewayPairingToken,CredentialAction::Test,"Test pairing",cx)))
                    .child(div().text_size(sp(13.0)).text_color(theme.text_secondary).child(self.setup_status(Kind::GatewayPairingToken)));
                if let Some(feedback) = self.credential_feedback.get(&Kind::GatewayPairingToken) {
                    form = form.child(
                        div()
                            .text_size(sp(13.0))
                            .text_color(theme.text_secondary)
                            .child(feedback.clone()),
                    );
                }
            }
        }
        let mut actions = div()
            .pt(px(20.0))
            .mt(px(4.0))
            .border_t_1()
            .border_color(theme.border)
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(10.0));
        if let Some(previous) = step.previous() {
            actions = actions.child(self.control_button(
                "setup-back",
                "Back",
                cx,
                move |this, window, cx| this.set_setup_step(previous, window, cx),
            ));
        }
        actions = actions.child(
            self.control_button(
                "setup-next",
                if step == Step::Welcome {
                    "Set up Ion"
                } else if step == Step::Gateway {
                    "Open Ion"
                } else {
                    "Next"
                },
                cx,
                |this, window, cx| this.next_setup_step(window, cx),
            )
            .h(px(38.0))
            .border_color(theme.accent)
            .bg(theme.overlay_strong)
            .child(icon("icons/arrow-right.svg", 14.0, theme.accent)),
        );
        if step != Step::Welcome && step != Step::Gateway {
            actions = actions.child(self.control_button(
                "setup-skip-step",
                "Skip this step",
                cx,
                |this, window, cx| this.skip_setup_step(window, cx),
            ));
        }
        actions = actions.child(self.control_button(
            "setup-skip-all",
            "Skip for now",
            cx,
            |this, window, cx| this.leave_setup(window, cx),
        ));
        form=form.child(actions).child(div().flex().items_start().gap(px(8.0)).text_size(sp(12.0)).line_height(sp(18.0)).text_color(theme.text_secondary).child(icon("icons/lock.svg", 14.0, theme.text_secondary)).child("Keys stay in your OS credential vault. You can finish skipped steps from the top bar."));
        div()
            .id("ion-onboarding")
            .track_focus(&self.onboarding_ui.focus)
            .tab_group()
            .tab_index(0)
            .tab_stop(true)
            .focus_visible(|s| s.border_1().border_color(theme.accent))
            .size_full()
            .flex()
            .flex_col()
            .text_color(theme.text)
            .font_family(".SystemUIFont")
            .when_some(client_corners, |root, tiling| {
                let radius = px((super::window_chrome::CLIENT_FRAME_ROUNDING - 1.0).max(0.0));
                root.when(!(tiling.top || tiling.left), |r| r.rounded_tl(radius))
                    .when(!(tiling.top || tiling.right), |r| r.rounded_tr(radius))
                    .when(!(tiling.bottom || tiling.left), |r| r.rounded_bl(radius))
                    .when(!(tiling.bottom || tiling.right), |r| r.rounded_br(radius))
            })
            .bg(theme.surface)
            .on_key_down(cx.listener(|this, event, window, cx| this.setup_key(event, window, cx)))
            .child(header)
            .child(
                div()
                    .id("onboarding-scroll")
                    .track_scroll(&self.onboarding_ui.scroll)
                    .overflow_y_scroll()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .p(px(28.0))
                    .child(form),
            )
            .into_any_element()
    }
}
