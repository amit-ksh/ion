use super::composer::next_picker_highlight;
use super::*;
use crate::ui::ActivationExt;
use anyhow::Context as _;

const SETTINGS_CONTENT_MAX_WIDTH: f32 = 760.0;

/// The Usage page is a dashboard, not a form; it mirrors T3 Code's wide
/// two-column layout and needs the extra room for the chart.
const SETTINGS_USAGE_MAX_WIDTH: f32 = 1024.0;

/// Key context the settings sidebar declares around its search field.
const SETTINGS_SIDEBAR_CONTEXT: &str = "SettingsSidebar";

/// The search field while focused inside the sidebar. The field holds real
/// focus the whole time — the sidebar's selection is only drawn — so `up` and
/// `down` have to be claimed from under it, and only a binding can do that:
/// they arrive as actions, which consume the keystroke before the field sees
/// it.
const SETTINGS_SEARCH_CONTEXT: &str = "SettingsSidebar > TextInput";

#[derive(Clone, Copy)]
pub(super) enum CredentialAction {
    Set,
    Test,
    Delete,
}

#[derive(Clone, Copy)]
pub(super) enum GatewayAction {
    Pair,
    Accounts,
    ToggleProposal,
    Propose,
    Inspect,
}

/// The sidebar's rows in display order, each with the keyword haystack the
/// search field filters against.
const SETTINGS_PAGES: [(SettingsPage, &str, &str, &str); 7] = [
    (
        SettingsPage::General,
        "settings.general",
        "icons/settings.svg",
        "settings.general_keywords",
    ),
    (
        SettingsPage::Appearance,
        "settings.appearance",
        "icons/appearance.svg",
        "settings.appearance_keywords",
    ),
    (
        SettingsPage::Providers,
        "settings.providers",
        "icons/bot.svg",
        "settings.providers_keywords",
    ),
    (
        SettingsPage::Skills,
        "settings.skills",
        "icons/package.svg",
        "settings.skills_keywords",
    ),
    (
        SettingsPage::Usage,
        "settings.usage",
        "icons/chart-column.svg",
        "settings.usage_keywords",
    ),
    (
        SettingsPage::Daemon,
        "settings.daemon",
        "icons/server.svg",
        "settings.daemon_keywords",
    ),
    (
        SettingsPage::ComputerUse,
        "settings.computer_use",
        "icons/cursor-spark.svg",
        "settings.computer_use_keywords",
    ),
];

/// Bind the search field's list-navigation keys. Called once at startup.
pub fn init(cx: &mut App) {
    use gpui::KeyBinding;
    cx.bind_keys([
        KeyBinding::new("down", SelectNextEntry, Some(SETTINGS_SEARCH_CONTEXT)),
        KeyBinding::new("up", SelectPreviousEntry, Some(SETTINGS_SEARCH_CONTEXT)),
    ]);
}

/// The sidebar rows the query leaves visible, in display order. `query` must
/// already be trimmed and lowercased; when it is empty every page matches.
pub(super) fn visible_settings_pages(
    query: &str,
) -> impl Iterator<Item = (SettingsPage, String, &'static str)> + '_ {
    SETTINGS_PAGES
        .into_iter()
        .filter(|(page, ..)| page.is_visible_in_navigation())
        .filter_map(move |(page, label_key, icon, keywords_key)| {
            let label = if page == SettingsPage::Providers
                && waku_protocol::distribution::DistributionProfile::current()
                    == waku_protocol::distribution::DistributionProfile::NebiusHackathon
            {
                "Models & Speech".to_owned()
            } else {
                crate::i18n::translate(label_key)
            };
            let keywords = if page == SettingsPage::Providers
                && waku_protocol::distribution::DistributionProfile::current()
                    == waku_protocol::distribution::DistributionProfile::NebiusHackathon
            {
                "models speech nebius nemotron api key byok credential catalog".to_owned()
            } else {
                crate::i18n::translate(keywords_key).to_lowercase()
            };
            (query.is_empty() || keywords.contains(query)).then_some((page, label, icon))
        })
}

impl Waku {
    pub(super) fn toggle_speech_recording(&mut self, cx: &mut Context<Self>) {
        if self.speech_recording {
            if let Some(stop) = self.speech_stop_tx.as_ref() {
                let _ = stop.send(crate::speech::RecordCommand::Stop);
            }
            self.speech_recording = false;
            self.speech_busy = true;
            self.speech_feedback = Some("Finalizing microphone transcription…".into());
            cx.notify();
            return;
        }
        if self.speech_busy {
            return;
        }
        self.speech_generation = self.speech_generation.wrapping_add(1);
        let generation = self.speech_generation;
        self.speech_busy = true;
        self.speech_transcript = None;
        self.speech_partial = None;
        self.speech_draft_owner = self.selected_composer_draft_key();
        self.speech_session_owner = self.state.selected_session;
        self.speech_feedback =
            Some("Preparing local Parakeet… First use downloads about 146 MB.".into());
        let (stop, commands) = crossbeam_channel::unbounded();
        self.speech_stop_tx = Some(stop);
        cx.notify();
        let (progress_tx, progress_rx) = crossbeam_channel::bounded(32);
        self.observe_speech_progress(generation, progress_rx, cx);
        cx.spawn(async move |waku, cx| {
            let preparing_commands = commands.clone();
            let preparing_progress = progress_tx.clone();
            let prepared = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || {
                        crate::speech::prepare_recording(
                            &preparing_commands,
                            Some(&preparing_progress),
                        )
                    })
                    .await
                })
                .await;
            if let Err(error) = prepared {
                let _ = waku.update(cx, |waku, cx| {
                    if waku.speech_generation == generation {
                        waku.speech_busy = false;
                        waku.speech_stop_tx = None;
                        waku.speech_feedback = Some(error.to_string());
                        cx.notify();
                    }
                });
                return;
            }
            let started = waku
                .update(cx, |waku, cx| {
                    if waku.speech_generation != generation {
                        return false;
                    }
                    let canceled = waku.speech_stop_tx.is_none();
                    let changed_context = waku.speech_draft_owner
                        != waku.selected_composer_draft_key()
                        || waku.speech_session_owner != waku.state.selected_session;
                    if canceled || changed_context {
                        waku.speech_busy = false;
                        waku.speech_recording = false;
                        waku.speech_stop_tx = None;
                        waku.speech_feedback = Some(
                            if canceled {
                                "Dictation canceled."
                            } else {
                                "Parakeet is ready. Start dictation from the chat you want to use."
                            }
                            .into(),
                        );
                        cx.notify();
                        return false;
                    }
                    waku.speech_busy = false;
                    waku.speech_recording = true;
                    waku.speech_feedback = Some("Starting the microphone…".into());
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if !started {
                return;
            }
            let result = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || {
                        crate::speech::record_and_transcribe(commands, progress_tx)
                    })
                    .await
                })
                .await;
            let _ = waku.update(cx, |waku, cx| {
                if waku.speech_generation != generation {
                    return;
                }
                // Cancel clears the sender immediately, including while finalization
                // is in flight. A late successful result must remain discarded.
                let result = if waku.speech_stop_tx.is_some() {
                    result
                } else {
                    Ok(None)
                };
                waku.speech_busy = false;
                waku.speech_recording = false;
                waku.speech_stop_tx = None;
                waku.speech_partial = None;
                match result {
                    Ok(Some(transcript)) => {
                        waku.speech_transcript = Some(transcript);
                        waku.speech_feedback =
                            Some("Dictation is ready to insert into the composer.".into());
                    }
                    Ok(None) => waku.speech_feedback = Some("Recording canceled.".into()),
                    Err(error) => waku.speech_feedback = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn observe_speech_progress(
        &self,
        generation: u64,
        progress_rx: crossbeam_channel::Receiver<crate::speech::SpeechProgress>,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |waku, cx| {
            loop {
                let receiver = progress_rx.clone();
                let event = cx.background_executor().spawn(async move {
                    smol::unblock(move || receiver.recv()).await
                }).await;
                let Ok(event) = event else { break; };
                let still_current = waku.update(cx, |waku, cx| {
                    if waku.speech_generation != generation || waku.speech_stop_tx.is_none() { return false; }
                    match event {
                        crate::speech::SpeechProgress::Ready => {
                            if waku.speech_recording {
                                waku.speech_feedback = Some("Recording on this computer. Press Stop to transcribe with Parakeet.".into());
                            }
                        }
                        crate::speech::SpeechProgress::Model(event) => {
                            use waku_credentials::speech::download::Progress;
                            waku.speech_feedback = Some(match event {
                                Progress::Waiting => "Waiting for another local model download. You can cancel or keep using Ion.".into(),
                                Progress::Verifying => "Verifying the saved Parakeet model…".into(),
                                Progress::Downloading { bytes, total } => format!("Downloading Parakeet: {} / {} MB. You can cancel or keep using Ion.", bytes / 1_000_000, total / 1_000_000),
                                Progress::Ready => "Parakeet is ready for local dictation.".into(),
                            });
                        }
                        crate::speech::SpeechProgress::Warning(message) => {
                            waku.speech_feedback = Some(message);
                        }
                    }
                    cx.notify();
                    true
                }).unwrap_or(false);
                if !still_current { break; }
            }
        }).detach();
    }

    pub(super) fn cancel_speech_recording(&mut self, cx: &mut Context<Self>) {
        if let Some(stop) = self.speech_stop_tx.take() {
            let _ = stop.send(crate::speech::RecordCommand::Cancel);
            self.speech_recording = false;
            self.speech_busy = true;
            self.speech_partial = None;
            self.speech_feedback = Some("Canceling dictation…".into());
            cx.notify();
        }
    }

    pub(super) fn prepare_local_speech_model(&mut self, cx: &mut Context<Self>) {
        if self.speech_busy || self.speech_recording {
            return;
        }
        self.speech_generation = self.speech_generation.wrapping_add(1);
        let generation = self.speech_generation;
        let (stop, commands) = crossbeam_channel::unbounded();
        self.speech_stop_tx = Some(stop);
        self.speech_busy = true;
        self.speech_feedback = Some("Downloading and verifying Parakeet… About 146 MB on first use. You can keep using Ion.".into());
        cx.notify();
        let (progress_tx, progress_rx) = crossbeam_channel::bounded(32);
        self.observe_speech_progress(generation, progress_rx, cx);
        cx.spawn(async move |waku, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || {
                        crate::speech::prepare_recording(&commands, Some(&progress_tx))
                    })
                    .await
                })
                .await;
            let _ = waku.update(cx, |waku, cx| {
                if waku.speech_generation != generation {
                    return;
                }
                waku.speech_busy = false;
                waku.speech_stop_tx = None;
                waku.speech_feedback = Some(match result {
                    Ok(()) => "Parakeet is ready for local dictation.".into(),
                    Err(error) => error.to_string(),
                });
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn refresh_speech_runtime_status(&mut self, cx: &mut Context<Self>) {
        let generation = self.speech_generation;
        cx.spawn(async move |waku, cx| {
            let status = cx
                .background_executor()
                .spawn(async { crate::speech::runtime_version() })
                .await;
            let _ = waku.update(cx, |waku, cx| {
                if waku.speech_generation == generation {
                    waku.speech_runtime_status = Some(match status {
                        Ok(version) => format!("Runtime installed: {version}"),
                        Err(error) => error.to_string(),
                    });
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn choose_speech_wav(&mut self, cx: &mut Context<Self>) {
        if self.speech_busy || self.speech_recording {
            return;
        }
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose a WAV recording".into()),
        });
        self.speech_generation = self.speech_generation.wrapping_add(1);
        let generation = self.speech_generation;
        self.speech_feedback = None;
        self.speech_transcript = None;
        self.speech_partial = None;
        self.speech_draft_owner = self.selected_composer_draft_key();
        self.speech_session_owner = self.state.selected_session;
        let (stop, commands) = crossbeam_channel::unbounded();
        self.speech_stop_tx = Some(stop);
        self.speech_busy = true;
        self.speech_feedback = Some(
            "Preparing local Parakeet transcription… First use may download about 146 MB.".into(),
        );
        cx.notify();
        cx.spawn(async move |waku, cx| {
            let path = match receiver.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                _ => None,
            };
            let result = if let Some(path) = path {
                Some(
                    cx.background_executor()
                        .spawn(async move { crate::speech::transcribe_wav(&path, &commands) })
                        .await,
                )
            } else {
                None
            };
            let _ = waku.update(cx, |waku, cx| {
                if waku.speech_generation != generation {
                    return;
                }
                let result = if waku.speech_stop_tx.is_some() {
                    result
                } else {
                    None
                };
                waku.speech_stop_tx = None;
                waku.speech_busy = false;
                match result {
                    Some(Ok(transcript)) => {
                        waku.speech_transcript = Some(transcript);
                        waku.speech_feedback = Some("Transcript is ready to insert.".into());
                    }
                    Some(Err(error)) => waku.speech_feedback = Some(error.to_string()),
                    None => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn insert_speech_transcript(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.speech_draft_owner != self.selected_composer_draft_key() {
            self.speech_feedback =
                Some("Return to the task where dictation started before inserting it.".into());
            cx.notify();
            return;
        }
        let Some(transcript) = self.speech_transcript.take() else {
            return;
        };
        let current = self.composer.read(cx).content(cx).to_owned();
        let content = if current.trim().is_empty() {
            transcript
        } else {
            format!("{current}\n{transcript}")
        };
        self.composer
            .update(cx, |composer, cx| composer.set_content(content, cx));
        self.schedule_composer_draft_save(cx);
        self.speech_feedback = None;
        self.settings_page = None;
        self.composer.read(cx).focus().focus(window, cx);
        cx.notify();
    }

    pub(super) fn refresh_credential_statuses(&mut self, cx: &mut Context<Self>) {
        self.refresh_cloud_account(cx);
        use crate::credentials::{CredentialKind as Kind, CredentialService};
        let kinds = [
            Kind::NebiusTokenFactoryApiKey,
            Kind::NebiusSandboxProject,
            Kind::ComposioScopedProjectKey,
            Kind::GatewayPairingToken,
            Kind::SpeechService,
        ];
        let kinds = kinds
            .into_iter()
            .filter(|kind| !self.credential_busy.contains(kind))
            .collect::<Vec<_>>();
        let generations = self.credential_generation.clone();
        let refresh_project = !self
            .credential_busy
            .contains(&Kind::NebiusTokenFactoryApiKey)
            && !self.credential_busy.contains(&Kind::NebiusSandboxProject);
        cx.spawn(async move |waku, cx| {
            let statuses = cx
                .background_executor()
                .spawn(async move {
                    let statuses = kinds
                        .into_iter()
                        .map(|kind| (kind, CredentialService::global().status(kind)))
                        .collect::<Vec<_>>();
                    let project = if refresh_project {
                        CredentialService::global()
                            .nebius_project_summary()
                            .ok()
                            .flatten()
                    } else {
                        None
                    };
                    (statuses, project)
                })
                .await;
            let _ = waku.update(cx, |waku, cx| {
                for (kind, status) in statuses.0 {
                    if waku.credential_generation.get(&kind) == generations.get(&kind) {
                        waku.credential_statuses.insert(kind, status);
                    }
                }
                if refresh_project
                    && [Kind::NebiusTokenFactoryApiKey, Kind::NebiusSandboxProject]
                        .iter()
                        .all(|kind| waku.credential_generation.get(kind) == generations.get(kind))
                {
                    waku.onboarding_ui.saved_nebius_project = statuses.1;
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn pair_gateway(&mut self, cx: &mut Context<Self>) {
        use crate::credentials::{CredentialKind as Kind, CredentialService};
        let kind = Kind::GatewayPairingToken;
        if !self.credential_busy.insert(kind) {
            return;
        }
        let url = self.gateway_url_input.read(cx).content().trim().to_owned();
        let invite = self
            .gateway_invite_input
            .read(cx)
            .content()
            .trim()
            .to_owned();
        if url.is_empty() || invite.is_empty() {
            self.credential_busy.remove(&kind);
            self.credential_feedback
                .insert(kind, "Enter the gateway URL and one-use invite.".into());
            cx.notify();
            return;
        }
        self.gateway_invite_input
            .update(cx, |input, cx| input.clear(cx));
        let generation = self.credential_generation.entry(kind).or_default();
        *generation = generation.wrapping_add(1);
        let generation = *generation;
        self.credential_feedback.remove(&kind);
        cx.notify();
        cx.spawn(async move |waku, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let service = CredentialService::global();
                    let paired = service.pair_gateway(&url, &invite, "ion desktop");
                    let scope = service.gateway_api().map(|api| api.interaction_scope_key());
                    (paired, service.status(kind), scope)
                })
                .await;
            let _ = waku.update(cx, |waku, cx| {
                if waku.credential_generation.get(&kind).copied() != Some(generation) {
                    return;
                }
                waku.credential_busy.remove(&kind);
                waku.credential_statuses.insert(kind, result.1);
                if result.0.is_ok() {
                    waku.onboarding_ui.failed_tests.remove(&kind);
                    waku.gateway_accounts.clear();
                    waku.gateway_last_proposal = None;
                    if let (Some(bridge), Ok(scope)) = (&mut waku._gateway_bridge, result.2) {
                        bridge.set_scope(Some(scope));
                    }
                    waku.republish_pending_gateway_interactions();
                }
                waku.credential_feedback.insert(
                    kind,
                    match result.0 {
                        Ok(crate::credentials::CredentialStatus::StoredInVault) => {
                            "Desktop paired. Device token saved in the OS vault.".into()
                        }
                        Ok(crate::credentials::CredentialStatus::ManagedFile) => {
                            "Gateway pairing is managed by a private service file.".into()
                        }
                        Ok(crate::credentials::CredentialStatus::MemoryOnly) => {
                            "Desktop paired for this app session only; OS vault unavailable.".into()
                        }
                        Ok(crate::credentials::CredentialStatus::Missing) => {
                            "Pairing did not save.".into()
                        }
                        Err(error) => error.to_string(),
                    },
                );
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn republish_pending_gateway_interactions(&self) {
        let Some(bridge) = &self._gateway_bridge else {
            return;
        };
        // Upsert this window's current state. A refresh must never reset another
        // window's pending requests or erase resolutions awaiting delivery.
        for (&task_id, runtime) in &self.runtimes {
            let Some(runtime_id) = runtime.driver.runtime_id() else {
                continue;
            };
            if let Some(permission) = &runtime.pending_permission {
                bridge.update_interaction(crate::gateway_bridge::InteractionUpdate::Publish(
                    crate::credentials::GatewayRelayInteraction {
                        task_id: task_id.to_string(),
                        runtime_id: runtime_id.to_string(),
                        request_id: permission.request_id.clone(),
                        kind: "permission".into(),
                        title: permission.title.clone(),
                        detail: permission.detail.clone(),
                        options: permission
                            .options
                            .iter()
                            .filter_map(|option| serde_json::to_value(option).ok())
                            .collect(),
                        questions: Vec::new(),
                    },
                ));
            }
            if let Some(input) = &runtime.pending_user_input {
                bridge.update_interaction(crate::gateway_bridge::InteractionUpdate::Publish(
                    crate::credentials::GatewayRelayInteraction {
                        task_id: task_id.to_string(),
                        runtime_id: runtime_id.to_string(),
                        request_id: input.request_id.clone(),
                        kind: "question".into(),
                        title: "Provider question".into(),
                        detail: String::new(),
                        options: Vec::new(),
                        questions: input
                            .questions
                            .iter()
                            .filter_map(|question| serde_json::to_value(question).ok())
                            .collect(),
                    },
                ));
            }
        }
    }

    pub(super) fn refresh_gateway_accounts(&mut self, cx: &mut Context<Self>) {
        use crate::credentials::{CredentialKind as Kind, CredentialService};
        let kind = Kind::GatewayPairingToken;
        if !self.credential_busy.insert(kind) {
            return;
        }
        let generation = self.credential_generation.entry(kind).or_default();
        *generation = generation.wrapping_add(1);
        let generation = *generation;
        cx.notify();
        cx.spawn(async move |waku, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { CredentialService::global().gateway_accounts() })
                .await;
            let _ = waku.update(cx, |waku, cx| {
                if waku.credential_generation.get(&kind).copied() != Some(generation) {
                    return;
                }
                waku.credential_busy.remove(&kind);
                match result {
                    Ok(accounts) => {
                        let count = accounts.len();
                        waku.gateway_accounts = accounts;
                        waku.credential_feedback
                            .insert(kind, format!("{count} connected accounts available."));
                    }
                    Err(error) => {
                        waku.credential_feedback.insert(kind, error.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn propose_gateway_action(&mut self, cx: &mut Context<Self>) {
        use crate::credentials::{CredentialKind as Kind, CredentialService, GatewayEffect};
        let kind = Kind::GatewayPairingToken;
        if !self.credential_busy.insert(kind) {
            return;
        }
        let field = |name: &'static str| {
            self.gateway_effect_inputs[name]
                .read(cx)
                .content()
                .trim()
                .to_owned()
        };
        let body = field("body");
        let parsed_body = serde_json::from_str::<serde_json::Value>(&body);
        if !matches!(parsed_body, Ok(serde_json::Value::Object(_))) {
            self.credential_busy.remove(&kind);
            self.credential_feedback
                .insert(kind, "Arguments must be a JSON object.".into());
            cx.notify();
            return;
        }
        let effect = GatewayEffect {
            toolkit: field("toolkit"),
            tool: field("tool"),
            version: field("version"),
            account_id: field("account"),
            target: field("target"),
            body,
            scopes: field("scopes")
                .split(',')
                .map(str::trim)
                .filter(|scope| !scope.is_empty())
                .map(str::to_owned)
                .collect(),
        };
        if effect.toolkit.is_empty()
            || effect.tool.is_empty()
            || effect.version.is_empty()
            || effect.account_id.is_empty()
            || effect.target.is_empty()
        {
            self.credential_busy.remove(&kind);
            self.credential_feedback.insert(
                kind,
                "Fill in the toolkit, tool, version, account, and target.".into(),
            );
            cx.notify();
            return;
        }
        let generation = self.credential_generation.entry(kind).or_default();
        *generation = generation.wrapping_add(1);
        let generation = *generation;
        self.credential_feedback.remove(&kind);
        cx.notify();
        cx.spawn(async move |waku, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { CredentialService::global().gateway_propose(effect) })
                .await;
            let _ = waku.update(cx, |waku, cx| {
                if waku.credential_generation.get(&kind).copied() != Some(generation) {
                    return;
                }
                waku.credential_busy.remove(&kind);
                match result {
                    Ok(proposal) => {
                        waku.credential_feedback.insert(
                            kind,
                            "Action submitted for owner review. It has not executed.".into(),
                        );
                        waku.gateway_last_proposal = Some(proposal);
                    }
                    Err(error) => {
                        waku.credential_feedback.insert(kind, error.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn inspect_gateway_action(&mut self, cx: &mut Context<Self>) {
        use crate::credentials::{CredentialKind as Kind, CredentialService};
        let kind = Kind::GatewayPairingToken;
        if !self.credential_busy.insert(kind) {
            return;
        }
        let Some(proposal_id) = self.gateway_last_proposal.as_ref().map(|p| p.id.clone()) else {
            self.credential_busy.remove(&kind);
            return;
        };
        let generation = self.credential_generation.entry(kind).or_default();
        *generation = generation.wrapping_add(1);
        let generation = *generation;
        cx.notify();
        cx.spawn(async move |waku, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { CredentialService::global().gateway_inspect(&proposal_id) })
                .await;
            let _ = waku.update(cx, |waku, cx| {
                if waku.credential_generation.get(&kind).copied() != Some(generation) {
                    return;
                }
                waku.credential_busy.remove(&kind);
                match result {
                    Ok(proposal) => waku.gateway_last_proposal = Some(proposal),
                    Err(error) => {
                        waku.credential_feedback.insert(kind, error.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn run_credential_action(
        &mut self,
        kind: crate::credentials::CredentialKind,
        action: CredentialAction,
        cx: &mut Context<Self>,
    ) {
        use crate::credentials::{CredentialKind as Kind, CredentialService, CredentialTest};
        let project = Kind::NebiusSandboxProject;
        let key = Kind::NebiusTokenFactoryApiKey;
        let other = if kind == project {
            Some(key)
        } else if kind == key {
            Some(project)
        } else {
            None
        };
        if other.is_some_and(|other| self.credential_busy.contains(&other)) {
            self.credential_feedback.insert(
                kind,
                "Wait for the current Nebius connection update.".into(),
            );
            cx.notify();
            return;
        }
        let expected_workspace = self.cloud_ui.status.workspace_id.clone();
        if !self.credential_busy.insert(kind) {
            return;
        }
        let value = if matches!(action, CredentialAction::Set) {
            let input = self.credential_inputs.get(&kind).unwrap();
            let value = input.read(cx).content().trim().to_owned();
            if value.is_empty() {
                self.credential_busy.remove(&kind);
                self.credential_feedback.insert(
                    kind,
                    if kind == project {
                        "Enter a Token Factory AI project ID first."
                    } else {
                        "Enter a key or token first."
                    }
                    .into(),
                );
                cx.notify();
                return;
            }
            input.update(cx, |input, cx| {
                // Project IDs are nonsecret drafts: keep them on validation/storage
                // failure, and while a key is being replaced. Secrets clear at once.
                if kind != project {
                    input.clear(cx);
                }
                input.set_secure(kind != project, cx);
            });
            Some(if kind == Kind::SpeechService {
                serde_json::json!({"baseUrl":self.speech_url_input.read(cx).content().trim(),"token":value}).to_string()
            } else {
                value
            })
        } else {
            None
        };
        if kind == key && matches!(action, CredentialAction::Set | CredentialAction::Delete) {
            self.invalidate_personal_nebius_project();
        }
        let generation = self.credential_generation.entry(kind).or_default();
        *generation = generation.wrapping_add(1);
        let generation = *generation;
        self.credential_feedback.remove(&kind);
        cx.notify();
        cx.spawn(async move |waku, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let service = CredentialService::global();
                    let result = (|| -> anyhow::Result<(String, Option<Vec<String>>)> {
                        match action {
                            CredentialAction::Set => {
                                let value = value.unwrap();
                                let status = if kind == Kind::NebiusSandboxProject {
                                    service.save_nebius_project(&value, expected_workspace.as_deref().context("Sign in to Ion first")?)?
                                } else { service.set(kind, value)? };
                                let note =
                                    if status == crate::credentials::CredentialStatus::MemoryOnly {
                                        "Saved for this app session only; OS vault unavailable."
                                            .to_owned()
                                    } else {
                                        "Saved in the OS credential vault.".to_owned()
                                    };
                                Ok((note, None))
                            }
                            CredentialAction::Test => match if kind == Kind::NebiusSandboxProject {
                                CredentialTest::NebiusProject(service.test_nebius_project_for(expected_workspace.as_deref().context("Sign in to Ion first")?)?)
                            } else { service.test(kind)? } {
                                CredentialTest::SpeechVerified => Ok((
                                    "Speech service connected; Nemotron 3.5 ASR streaming is ready.".into(),
                                    None,
                                )),
                                CredentialTest::NebiusModels(models) => Ok((
                                    format!(
                                        "Nebius key verified; {} models available.",
                                        models.len()
                                    ),
                                    Some(models),
                                )),
                                CredentialTest::NebiusProject(access) => Ok((
                                    if access.can_execute && access.can_list && access.can_cancel {
                                        format!("{}: sandbox permissions verified.", access.project_id)
                                    } else { format!("{}: execution, list or cancellation permission is missing.", access.project_id) }, None)),
                                CredentialTest::ComposioVerified => Ok((
                                    "Composio key verified with Tools read access.".into(),
                                    None,
                                )),
                                CredentialTest::GatewayConnected(session) => Ok((
                                    format!(
                                        "Gateway connected as {} ({}); grants: {}.",
                                        session.name,
                                        session.id,
                                        session.grants.join(", ")
                                    ),
                                    None,
                                )),
                            },
                            CredentialAction::Delete => {
                                if kind == Kind::NebiusSandboxProject {
                                    service.delete_nebius_project_for(expected_workspace.as_deref().context("Sign in to Ion first")?)?;
                                } else { service.delete(kind)?; }
                                Ok(("Credential deleted.".into(), None))
                            }
                        }
                    })();
                    (result, service.status(kind))
                })
                .await;
            let _ = waku.update(cx, |waku, cx| {
                if waku.credential_generation.get(&kind).copied() != Some(generation) {
                    return;
                }
                waku.credential_busy.remove(&kind);
                waku.credential_statuses.insert(kind, result.1);
                match result.0 {
                    Ok((message, catalog)) => {
                        waku.onboarding_ui.failed_tests.remove(&kind);
                        waku.credential_feedback.insert(kind, message);
                        if kind == Kind::ComposioScopedProjectKey
                            && matches!(action, CredentialAction::Set | CredentialAction::Delete)
                        {
                            // Discard summaries and any result started under the previous key.
                            waku.invalidate_composio_apps();
                        }
                        if kind == Kind::SpeechService {
                            waku.refresh_speech_runtime_status(cx);
                        }
                        if kind == Kind::NebiusTokenFactoryApiKey {
                            if let Some(catalog) = catalog {
                                waku.nebius_catalog = catalog;
                            }
                            if matches!(action, CredentialAction::Delete) {
                                waku.nebius_catalog.clear();
                            }
                            waku.refresh_provider_model_discovery(ProviderKind::Nebius);
                        }
                        if matches!(kind, Kind::NebiusTokenFactoryApiKey | Kind::NebiusSandboxProject)
                            && matches!(action, CredentialAction::Set | CredentialAction::Delete) {
                            waku.refresh_credential_statuses(cx);
                        }
                        // Entering the project is the whole setup: Ion builds
                        // its sandbox image there without another step.
                        if kind == Kind::NebiusSandboxProject {
                            match action {
                                CredentialAction::Set => waku.prepare_personal_sandbox(cx),
                                CredentialAction::Delete => {
                                    waku.cloud_ui.personal_sandbox = None;
                                    waku.cloud_ui.personal_sandbox_error = None;
                                    waku.refresh_cloud_account(cx);
                                }
                                CredentialAction::Test => {}
                            }
                        }
                        if kind == Kind::GatewayPairingToken
                            && matches!(action, CredentialAction::Delete)
                        {
                            waku.gateway_accounts.clear();
                            waku.gateway_last_proposal = None;
                        }
                    }
                    Err(error) => {
                        if matches!(action, CredentialAction::Test) {
                            waku.onboarding_ui.failed_tests.insert(kind);
                        }
                        waku.credential_feedback.insert(kind, error.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn credential_button(
        &self,
        kind: crate::credentials::CredentialKind,
        action: CredentialAction,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        let id = SharedString::from(format!("credential-{}-{label}", kind.account()));
        div()
            .id(id)
            .tab_group()
            .tab_index(0)
            .tab_stop(true)
            .focus_visible(|style| style.border_color(theme.accent))
            .h(px(34.0))
            .px(px(12.0))
            .rounded(px(7.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .items_center()
            .gap(px(7.0))
            .text_size(sp(12.5))
            .text_color(theme.text)
            .hover(|element| element.bg(theme.overlay))
            .opacity(if self.credential_busy.contains(&kind) {
                0.5
            } else {
                1.0
            })
            .child(icon(
                match action {
                    CredentialAction::Set => "icons/check.svg",
                    CredentialAction::Test => "icons/rotate-cw.svg",
                    CredentialAction::Delete => "icons/trash.svg",
                },
                14.0,
                theme.text_secondary,
            ))
            .child(label)
            .on_click(
                cx.listener(move |this, _, _, cx| this.run_credential_action(kind, action, cx)),
            )
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                if !event.keystroke.modifiers.modified()
                    && matches!(event.keystroke.key.as_str(), "enter" | "space")
                {
                    this.run_credential_action(kind, action, cx);
                    cx.stop_propagation();
                }
            }))
            .into_any_element()
    }

    pub(super) fn gateway_button(
        &self,
        action: GatewayAction,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        let busy = self
            .credential_busy
            .contains(&crate::credentials::CredentialKind::GatewayPairingToken);
        div()
            .id(SharedString::from(format!("gateway-{label}")))
            .tab_group()
            .tab_index(0)
            .tab_stop(true)
            .focus_visible(|style| style.border_color(theme.accent))
            .h(px(30.0))
            .px(px(12.0))
            .rounded(px(7.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .items_center()
            .text_size(sp(12.5))
            .text_color(theme.text)
            .hover(|element| element.bg(theme.overlay))
            .opacity(if busy { 0.5 } else { 1.0 })
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| match action {
                GatewayAction::Pair => this.pair_gateway(cx),
                GatewayAction::Accounts => this.refresh_gateway_accounts(cx),
                GatewayAction::ToggleProposal => {
                    this.gateway_effect_expanded = !this.gateway_effect_expanded;
                    cx.notify();
                }
                GatewayAction::Propose => this.propose_gateway_action(cx),
                GatewayAction::Inspect => this.inspect_gateway_action(cx),
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                if !event.keystroke.modifiers.modified()
                    && matches!(event.keystroke.key.as_str(), "enter" | "space")
                {
                    match action {
                        GatewayAction::Pair => this.pair_gateway(cx),
                        GatewayAction::Accounts => this.refresh_gateway_accounts(cx),
                        GatewayAction::ToggleProposal => {
                            this.gateway_effect_expanded = !this.gateway_effect_expanded;
                            cx.notify();
                        }
                        GatewayAction::Propose => this.propose_gateway_action(cx),
                        GatewayAction::Inspect => this.inspect_gateway_action(cx),
                    }
                    cx.stop_propagation();
                }
            }))
            .into_any_element()
    }

    pub(super) fn render_models_speech_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        use crate::credentials::{CredentialKind as Kind, CredentialStatus};
        let theme = Theme::current(cx);
        let mut content = div()
            .flex()
            .flex_col()
            .gap(px(14.0))
            .child(self.render_cloud_account(cx));
        for (kind, title, description) in [
            (
                Kind::NebiusTokenFactoryApiKey,
                "Nebius Token Factory",
                "Optional personal API key, used first when configured. Otherwise Ion uses account-based shared inference when the owner enables it.",
            ),
            (
                Kind::ComposioScopedProjectKey,
                "Composio project",
                "Connect your apps with Composio. Your project key stays in your OS vault; app sign-in happens in Composio.",
            ),
        ] {
            let status = match self
                .credential_statuses
                .get(&kind)
                .copied()
                .unwrap_or(CredentialStatus::Missing)
            {
                CredentialStatus::Missing
                    if kind == Kind::NebiusTokenFactoryApiKey
                        && self.shared_inference_enabled() =>
                {
                    "Shared inference enabled; personal key optional"
                }
                CredentialStatus::Missing => "Not configured",
                CredentialStatus::StoredInVault => "Stored in OS vault",
                CredentialStatus::ManagedFile => "Managed by a private service file",
                CredentialStatus::MemoryOnly => "Stored for this app session only",
            };
            let input = self.credential_inputs.get(&kind).unwrap().clone();
            let mut card = div()
                .w_full()
                .p(px(18.0))
                .rounded(px(12.0))
                .bg(theme.raised)
                .flex()
                .flex_col()
                .gap(px(8.0))
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
                        .child(description),
                )
                .when(kind == Kind::SpeechService, |card| {
                    card.child(
                        div()
                            .text_size(sp(12.5))
                            .text_color(theme.text_secondary)
                            .child("Speech service URL"),
                    )
                    .child(TextField::new(
                        "speech-service-url",
                        self.speech_url_input.clone(),
                    ))
                })
                .child(
                    TextField::new(
                        SharedString::from(format!("credential-input-{}", kind.account())),
                        input,
                    )
                    .w_full(),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(8.0))
                        .child(self.credential_button(kind, CredentialAction::Set, "Save", cx))
                        .child(self.credential_button(kind, CredentialAction::Test, "Test", cx))
                        .child(self.credential_button(
                            kind,
                            CredentialAction::Delete,
                            "Delete",
                            cx,
                        )),
                )
                .child(
                    div()
                        .text_size(sp(12.0))
                        .text_color(theme.text_secondary)
                        .child(status),
                );
            if let Some(feedback) = self.credential_feedback.get(&kind) {
                card = card.child(
                    div()
                        .text_size(sp(12.0))
                        .text_color(theme.text_secondary)
                        .child(feedback.clone()),
                );
            }
            if kind == Kind::NebiusTokenFactoryApiKey
                && matches!(
                    self.credential_statuses.get(&kind),
                    Some(
                        CredentialStatus::StoredInVault
                            | CredentialStatus::MemoryOnly
                            | CredentialStatus::ManagedFile
                    )
                )
            {
                card = card.child(self.setup_personal_project(cx));
            }
            if kind == Kind::NebiusTokenFactoryApiKey && !self.nebius_catalog.is_empty() {
                let models = self
                    .nebius_catalog
                    .iter()
                    .take(8)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" · ");
                card = card.child(
                    div()
                        .text_size(sp(12.0))
                        .text_color(theme.text_secondary)
                        .child(format!("Available models: {models}")),
                );
            }
            content = content.child(card);
        }
        let mut speech = div()
            .w_full()
            .p(px(18.0))
            .rounded(px(12.0))
            .bg(theme.raised)
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(div().text_size(sp(14.0)).font_weight(FontWeight::MEDIUM).text_color(theme.text).child("Speech"))
            .child(div().text_size(sp(12.5)).text_color(theme.text_secondary)
                .child("Dictation runs on this computer with NVIDIA Parakeet TDT-CTC 110M. The first use downloads about 146 MB; later recognition uses the saved model on this computer. Paired phones send recordings here. English dictation with punctuation. Other languages are unavailable."))
            .child(div().text_size(sp(12.0)).text_color(theme.text_secondary)
                .child(self.speech_runtime_status.clone().unwrap_or_else(|| "Checking local speech runtime…".into())))
            .child(self.control_button("speech-prepare-model", if self.speech_busy { "Cancel dictation setup" } else { "Download / verify Parakeet" }, cx, |this, _, cx| {
                if this.speech_busy { this.cancel_speech_recording(cx); } else { this.prepare_local_speech_model(cx); }
            }))
            .child(div()
                .id("speech-choose-wav")
                .tab_group()
                .tab_index(0)
                .tab_stop(true)
                .focus_visible(|style| style.border_color(theme.accent))
                .h(px(34.0))
                .px(px(12.0))
                .rounded(px(7.0))
                .border_1()
                .border_color(theme.border_strong)
                .flex()
                .items_center()
                .text_size(sp(12.5))
                .text_color(theme.text)
                .child(if self.speech_busy { "Transcribing…" } else { "Transcribe WAV…" })
                .on_click(cx.listener(|this, _, _, cx| this.choose_speech_wav(cx)))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if !event.keystroke.modifiers.modified()
                        && matches!(event.keystroke.key.as_str(), "enter" | "space")
                    {
                        this.choose_speech_wav(cx);
                        cx.stop_propagation();
                    }
                })));
        if let Some(feedback) = &self.speech_feedback {
            speech = speech.child(
                div()
                    .text_size(sp(12.0))
                    .text_color(theme.text_secondary)
                    .child(feedback.clone()),
            );
        }
        if let Some(transcript) = self
            .speech_transcript
            .as_ref()
            .or(self.speech_partial.as_ref())
        {
            speech = speech.child(
                div()
                    .id("speech-transcript")
                    .max_h(px(140.0))
                    .overflow_y_scroll()
                    .p(px(8.0))
                    .rounded(px(7.0))
                    .bg(theme.inset)
                    .text_size(sp(12.5))
                    .text_color(theme.text)
                    .child(transcript.clone()),
            );
            if self.speech_transcript.is_some() {
                speech = speech.child(
                    div()
                        .id("speech-insert-transcript")
                        .tab_group()
                        .tab_index(0)
                        .tab_stop(true)
                        .focus_visible(|style| style.border_color(theme.accent))
                        .h(px(34.0))
                        .px(px(12.0))
                        .rounded(px(7.0))
                        .border_1()
                        .border_color(theme.border_strong)
                        .flex()
                        .items_center()
                        .text_size(sp(12.5))
                        .text_color(theme.text)
                        .child("Insert into composer")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.insert_speech_transcript(window, cx)
                        }))
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                            if !event.keystroke.modifiers.modified()
                                && matches!(event.keystroke.key.as_str(), "enter" | "space")
                            {
                                this.insert_speech_transcript(window, cx);
                                cx.stop_propagation();
                            }
                        })),
                );
            }
        }
        content = content.child(speech);
        let gateway_kind = Kind::GatewayPairingToken;
        let gateway_status = match self
            .credential_statuses
            .get(&gateway_kind)
            .copied()
            .unwrap_or(CredentialStatus::Missing)
        {
            CredentialStatus::Missing => "Not paired",
            CredentialStatus::StoredInVault => "Device token stored in OS vault",
            CredentialStatus::ManagedFile => "Device token managed by a private service file",
            CredentialStatus::MemoryOnly => "Paired for this app session only",
        };
        let mut gateway = div()
            .w_full().p(px(18.0)).rounded(px(12.0)).bg(theme.raised)
            .flex().flex_col().gap(px(8.0))
            .child(div().text_size(sp(14.0)).font_weight(FontWeight::MEDIUM).text_color(theme.text).child("Personal gateway"))
            .child(div().text_size(sp(12.5)).text_color(theme.text_secondary)
                .child("Open /owner on your gateway, create a one-use invite, then pair this desktop. Remote gateways require HTTPS. The device token uses your OS vault when available."))
            .child(TextField::new("gateway-url", self.gateway_url_input.clone()).w_full())
            .child(TextField::new("gateway-invite", self.gateway_invite_input.clone()).w_full())
            .child(div().flex().flex_wrap().gap(px(8.0))
                .child(self.gateway_button(GatewayAction::Pair, "Pair desktop", cx))
                .child(self.credential_button(gateway_kind, CredentialAction::Test, "Test pairing", cx))
                .child(self.gateway_button(GatewayAction::Accounts, "List accounts", cx))
                .child(self.credential_button(gateway_kind, CredentialAction::Delete, "Remove pairing", cx)))
            .child(self.gateway_button(
                GatewayAction::ToggleProposal,
                if self.gateway_effect_expanded { "Hide action form" } else { "Propose test action" },
                cx,
            ))
            .child(div().text_size(sp(12.0)).text_color(theme.text_secondary).child(gateway_status));
        if let Some(feedback) = self.credential_feedback.get(&gateway_kind) {
            gateway = gateway.child(
                div()
                    .text_size(sp(12.0))
                    .text_color(theme.text_secondary)
                    .child(feedback.clone()),
            );
        }
        for account in &self.gateway_accounts {
            gateway = gateway.child(div().text_size(sp(12.0)).text_color(theme.text).child(
                format!("{} · {} · {}", account.toolkit, account.id, account.status),
            ));
        }
        if self.gateway_effect_expanded {
            let mut form = div()
                .w_full()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(div().text_size(sp(12.0)).text_color(theme.text_secondary)
                    .child("Submit an exact test action to your gateway. The owner must review and execute it separately in /owner. Use a disposable account and a harmless target."));
            for (name, label) in [
                ("toolkit", "Toolkit"),
                ("tool", "Tool slug"),
                ("version", "Pinned tool version"),
                ("account", "Connected account ID"),
                ("target", "Exact target"),
                ("body", "JSON arguments object"),
                ("scopes", "Requested scopes, comma separated"),
            ] {
                form = form.child(
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_size(sp(12.0))
                                .text_color(theme.text)
                                .child(label),
                        )
                        .child(
                            TextField::new(
                                SharedString::from(format!("gateway-effect-{name}")),
                                self.gateway_effect_inputs[name].clone(),
                            )
                            .w_full(),
                        ),
                );
            }
            form = form.child(self.gateway_button(
                GatewayAction::Propose,
                "Submit for owner review",
                cx,
            ));
            gateway = gateway.child(form);
        }
        if let Some(proposal) = &self.gateway_last_proposal {
            gateway = gateway
                .child(
                    div()
                        .text_size(sp(12.0))
                        .text_color(theme.text)
                        .child(format!("Proposal {} · {}", proposal.id, proposal.status)),
                )
                .child(self.gateway_button(GatewayAction::Inspect, "Check proposal status", cx));
        }
        let sandbox_enabled = self.state.nebius_sandbox_enabled();
        let preparing = self.cloud_ui.personal_sandbox_progress.clone();
        let sandbox_status: (SharedString, Hsla) = if let Some(step) = &preparing {
            (format!("{step}…").into(), theme.text_secondary)
        } else if let Some(error) = &self.cloud_ui.personal_sandbox_error {
            (error.clone().into(), theme.danger)
        } else if let Some(target) = &self.cloud_ui.personal_sandbox {
            (
                format!("Ready · {}", target.split('@').next().unwrap_or(target)).into(),
                theme.success,
            )
        } else if self.onboarding_ui.saved_nebius_project.is_some() {
            ("Not prepared in your project yet.".into(), theme.text_secondary)
        } else {
            ("Save your Token Factory key and AI project above to prepare it.".into(), theme.text_secondary)
        };
        let can_prepare = preparing.is_none() && self.onboarding_ui.saved_nebius_project.is_some();
        let prepare_label = if self.cloud_ui.personal_sandbox.is_some() { "Rebuild" } else { "Prepare sandbox" };
        content = content.child(div().p(px(18.0)).rounded(px(12.0)).bg(theme.raised)
            .flex().flex_col().gap(px(14.0))
            .child(div().flex().items_center().gap(px(16.0))
                .child(div().flex_1().min_w_0().flex().flex_col().gap(px(6.0))
                    .child(div().text_size(sp(14.0)).font_weight(FontWeight::MEDIUM).text_color(theme.text).child("Token Factory sandbox"))
                    .child(div().text_size(sp(12.5)).text_color(theme.text_secondary).child("Run sandbox agents and approved commands in your own Token Factory project. Ion builds a private sandbox image there when you save your AI project. Your local project is not uploaded automatically.")))
                .child(div().text_size(sp(12.0)).text_color(theme.text_secondary).child(if sandbox_enabled { "On" } else { "Off" }))
                .child(toggle_switch("tokenfactory-sandbox-toggle", sandbox_enabled, false, theme, cx,
                    move |this, _, cx| { this.state.set_nebius_sandbox_enabled(!sandbox_enabled); this.save(); cx.notify(); })))
            .child(div().flex().items_center().gap(px(12.0))
                .child(div().flex_1().min_w_0().text_size(sp(12.5)).text_color(sandbox_status.1).child(sandbox_status.0))
                .child(div().id("tokenfactory-sandbox-prepare")
                    .h(px(34.0)).px(px(12.0)).rounded(px(7.0)).border_1().border_color(theme.border_strong)
                    .flex().flex_none().items_center().gap(px(7.0))
                    .text_size(sp(12.5)).text_color(theme.text)
                    .opacity(if can_prepare { 1.0 } else { 0.5 })
                    .when(can_prepare, |b| b.cursor_pointer().hover(|b| b.bg(theme.overlay))
                        .on_click(cx.listener(|this, _, _, cx| this.prepare_personal_sandbox(cx))))
                    .child(icon("icons/rotate-cw.svg", 14.0, theme.text_secondary))
                    .child(prepare_label))));
        content = content.child(gateway);
        content.into_any_element()
    }

    pub(super) fn render_settings(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);

        div()
            .key_context("Waku")
            .track_focus(&self.settings_focus)
            .on_action(|_: &CloseWindow, window, _| crate::platform::hide_window(window))
            .on_action(cx.listener(Self::new_session_action))
            .on_action(cx.listener(Self::new_project_action))
            .on_action(cx.listener(Self::open_settings_action))
            .on_action(cx.listener(Self::toggle_sidebar_action))
            .on_action(cx.listener(Self::toggle_right_panel_action))
            .on_action(cx.listener(Self::toggle_command_palette_action))
            .on_action(cx.listener(Self::toggle_fps_counter_action))
            .on_action(cx.listener(Self::navigate_back_action))
            .on_action(cx.listener(Self::navigate_forward_action))
            .on_action(cx.listener(Self::focus_composer_action))
            .on_action(cx.listener(Self::cancel_turn_action))
            .capture_any_mouse_down(cx.listener(Self::navigation_mouse_down))
            .size_full()
            .flex()
            .bg(theme.canvas)
            .text_color(theme.text)
            .font_family(".SystemUIFont")
            .child(self.render_settings_sidebar(window, cx))
            .child(self.render_settings_content(window, cx))
            .into_any_element()
    }

    fn render_settings_sidebar(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        let current_page = self.settings_page.unwrap_or(SettingsPage::General);
        let query = self.settings_search_query(cx);
        let mut navigation = div().flex().flex_col().gap(px(3.0));

        for (page, label, icon_path) in visible_settings_pages(&query) {
            let selected = current_page == page;
            navigation = navigation.child(
                div()
                    .id(SharedString::from(format!(
                        "settings-tab-{}",
                        label.to_lowercase()
                    )))
                    .tab_index(0)
                    .focus_visible(|style| style.border_1().border_color(theme.accent))
                    .h(px(36.0))
                    .px(px(11.0))
                    .rounded(px(8.0))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .cursor_default()
                    .text_size(sp(13.0))
                    .text_color(if selected {
                        theme.text
                    } else {
                        theme.text_secondary
                    })
                    .when(selected, |element| element.bg(theme.accent_soft))
                    .hover(|element| element.bg(theme.sidebar_item_background))
                    .active(|element| element.bg(theme.sidebar_item_background))
                    .child(icon(
                        icon_path,
                        15.0,
                        if selected {
                            theme.accent
                        } else {
                            theme.text_tertiary
                        },
                    ))
                    .child(label)
                    .on_activation(cx, move |this, _, cx| {
                        this.open_settings_page(page, cx);
                    }),
            );
        }

        div()
            .key_context(SETTINGS_SIDEBAR_CONTEXT)
            .on_action(cx.listener(|this, _: &SelectNextEntry, _, cx| {
                this.cycle_settings_page("down", cx);
            }))
            .on_action(cx.listener(|this, _: &SelectPreviousEntry, _, cx| {
                this.cycle_settings_page("up", cx);
            }))
            .w(px(DEFAULT_SIDEBAR_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(theme.sidebar)
            .child(self.render_settings_sidebar_titlebar(window, cx))
            .child(
                div().px(px(12.0)).child(
                    div()
                        .id("settings-back")
                        .tab_index(0)
                        .focus_visible(|style| style.border_1().border_color(theme.accent))
                        .h(px(34.0))
                        .px(px(9.0))
                        .rounded(px(8.0))
                        .flex()
                        .items_center()
                        .gap(px(9.0))
                        .cursor_default()
                        .text_size(sp(13.0))
                        .text_color(theme.text_secondary)
                        .hover(|element| element.bg(theme.overlay))
                        .active(|element| element.bg(theme.overlay_strong))
                        .child(icon("icons/arrow-left.svg", 15.0, theme.text_tertiary))
                        .child(tr!("settings.back"))
                        .on_activation(cx, |this, window, cx| {
                            this.settings_page = None;
                            let focus_handle = this.composer_focus(cx);
                            window.focus(&focus_handle, cx);
                            cx.notify();
                        }),
                ),
            )
            .child(
                div().px(px(12.0)).pt(px(8.0)).child(
                    TextField::new("settings-search-field", self.settings_search.clone())
                        .icon("icons/search.svg", 13.0),
                ),
            )
            .child(div().h(px(18.0)))
            .child(div().px(px(12.0)).child(navigation))
    }

    /// The search field's content, normalized the way the page filter expects.
    fn settings_search_query(&self, cx: &App) -> String {
        self.settings_search
            .read(cx)
            .content()
            .trim()
            .to_lowercase()
    }

    /// Step the selected page through the rows the search leaves visible,
    /// wrapping at both ends. The field keeps focus so typing keeps narrowing
    /// the list; the landing page renders immediately, so there is no separate
    /// confirm step. A selection filtered out by the query re-enters the list
    /// from whichever end matches the key.
    fn cycle_settings_page(&mut self, key: &str, cx: &mut Context<Self>) {
        let query = self.settings_search_query(cx);
        let pages = visible_settings_pages(&query)
            .map(|(page, ..)| page)
            .collect::<Vec<_>>();
        let current_page = self.settings_page.unwrap_or(SettingsPage::General);
        let current = pages.iter().position(|page| *page == current_page);
        let Some(next) = next_picker_highlight(current, pages.len(), key) else {
            return;
        };
        self.open_settings_page(pages[next], cx);
    }

    fn render_settings_sidebar_titlebar(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let left_window_controls = self.render_client_window_controls(
            super::window_chrome::WindowControlSide::Left,
            window,
            cx,
        );
        // Only as tall as whatever actually sits in it: macOS's native
        // traffic lights, or the client-side buttons a Linux desktop puts on
        // this side. Windows keeps all three on the far side, and a desktop
        // like GNOME keeps none here, so there is nothing to clear and the
        // strip is only somewhere to drag the window by — the content
        // column's own titlebar carries the rest of that job.
        let height = if cfg!(target_os = "macos") || left_window_controls.is_some() {
            48.0
        } else {
            12.0
        };

        div()
            .id("settings-sidebar-titlebar")
            .h(px(height))
            .flex_none()
            .flex()
            .items_center()
            .children(left_window_controls)
            .child(
                self.window_drag_region(
                    div()
                        .id("settings-sidebar-traffic-light-drag-region")
                        .w(px(TRAFFIC_LIGHT_CLEARANCE))
                        .h_full()
                        .flex_none(),
                    cx,
                ),
            )
            .child(
                self.render_settings_drag_region("settings-sidebar-titlebar-drag-region", cx)
                    .h(px(height))
                    .flex_1(),
            )
    }

    fn render_settings_content(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        let page = self.settings_page.unwrap_or(SettingsPage::General);
        let right_window_controls = self.render_client_window_controls(
            super::window_chrome::WindowControlSide::Right,
            window,
            cx,
        );
        // The Skills page is a mail-style split that owns the whole content
        // column — no page title, no titlebar strip, no width cap, no card.
        // Window dragging stays with the sidebar's own titlebar region.
        if page == SettingsPage::Skills {
            return div()
                .flex_1()
                .h_full()
                .min_w_0()
                .flex()
                .flex_col()
                .border_l_1()
                .border_color(theme.sidebar_border)
                .bg(theme.surface)
                .child({
                    div()
                        .h(px(48.0))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_end()
                        .child(
                            self.render_settings_drag_region("settings-skills-titlebar", cx)
                                .flex_1(),
                        )
                        .children(self.render_setup_indicator(cx))
                        .children(right_window_controls)
                })
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .child(self.render_skills_settings(cx)),
                );
        }
        // The Monthly and Projects list views own their own scrolling, so
        // their pages fill the viewport instead of riding the shared scroll
        // container.
        let fills_viewport = page == SettingsPage::Usage && self.usage_settings_fills_viewport();
        // The titlebar strip is transparent; once content slides under it, a
        // hairline marks the boundary so the clip edge reads as a header
        // rather than a glitch.
        let content_scrolled = !fills_viewport && self.settings_scroll.offset().y < px(-1.0);

        let inner = div()
            .w_full()
            .max_w(px(match page {
                SettingsPage::Usage => SETTINGS_USAGE_MAX_WIDTH,
                _ => SETTINGS_CONTENT_MAX_WIDTH,
            }))
            .mx_auto()
            .when(fills_viewport, |element| {
                element.h_full().min_h_0().flex().flex_col()
            })
            .child(
                div()
                    .pt(px(2.0))
                    .flex_none()
                    .text_size(sp(22.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text)
                    .child(match page {
                        SettingsPage::General => tr!("settings.general"),
                        SettingsPage::Providers => {
                            if waku_protocol::distribution::DistributionProfile::current()
                                == waku_protocol::distribution::DistributionProfile::NebiusHackathon
                            {
                                "Models & Speech".to_owned()
                            } else {
                                tr!("settings.providers")
                            }
                        }
                        SettingsPage::Skills => tr!("settings.skills"),
                        SettingsPage::Usage => tr!("settings.usage"),
                        SettingsPage::Daemon => tr!("settings.daemon"),
                        SettingsPage::ComputerUse => tr!("settings.computer_use"),
                        SettingsPage::Appearance => tr!("settings.appearance"),
                    }),
            )
            .child(match page {
                SettingsPage::General => self.render_general_settings(cx),
                SettingsPage::Providers => div()
                    .flex()
                    .flex_col()
                    .gap(px(18.0))
                    .child(self.render_providers_settings(cx))
                    .child(self.render_models_speech_settings(cx))
                    .into_any_element(),
                SettingsPage::Skills => self.render_skills_settings(cx),
                SettingsPage::Usage => self.render_usage_settings(cx),
                SettingsPage::Daemon => self.render_daemon_settings(cx),
                SettingsPage::ComputerUse => self.render_computer_use_settings(cx),
                SettingsPage::Appearance => self.render_appearance_settings(cx),
            });

        div()
            .flex_1()
            .h_full()
            .min_w_0()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(theme.sidebar_border)
            .bg(theme.surface)
            .child(
                div()
                    .h(px(48.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_end()
                    .child(
                        self.render_settings_drag_region("settings-content-titlebar", cx)
                            .flex_1(),
                    )
                    .children(self.render_setup_indicator(cx))
                    .children(right_window_controls)
                    .when(content_scrolled, |element| {
                        element.border_b_1().border_color(theme.border)
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(
                        div()
                            .id("settings-content-scroll")
                            .size_full()
                            .when(!fills_viewport, |element| {
                                element
                                    .overflow_y_scroll()
                                    .track_scroll(&self.settings_scroll)
                                    .pb(px(48.0))
                            })
                            .when(fills_viewport, |element| {
                                element.min_h_0().flex().flex_col()
                            })
                            .px(px(32.0))
                            .child(inner),
                    )
                    .when(!fills_viewport, |element| {
                        element.child(scrollbar::vertical(
                            &self.settings_scroll,
                            &self.settings_scrollbar,
                        ))
                    }),
            )
    }

    fn render_general_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let updater_available = cx
            .try_global::<crate::updater::UpdaterState>()
            .is_some_and(|updater| updater.0.is_some());
        let analytics_enabled = self.state.analytics_enabled;
        let analytics_toggle = toggle_switch(
            "anonymous-analytics-toggle",
            analytics_enabled,
            false,
            theme,
            cx,
            move |this, _, cx| this.set_analytics_enabled(!analytics_enabled, cx),
        );
        let render_math = self.state.render_math;
        let math_toggle = toggle_switch(
            "render-math-toggle",
            render_math,
            false,
            theme,
            cx,
            move |this, _, cx| this.set_render_math(!render_math, cx),
        );
        div()
            .child(
                settings_group(&theme).child(
                    settings_row(
                        &theme,
                        true,
                        tr!("settings.connection_setup"),
                        tr!("settings.connection_setup_description"),
                    )
                    .child(self.render_setup_launcher(cx)),
                ),
            )
            .child(
                settings_group(&theme)
                    .child(
                        settings_row(
                            &theme,
                            true,
                            tr!("settings.local_by_default"),
                            tr!("settings.local_by_default_description"),
                        )
                        .child(status_pill(
                            "icons/laptop.svg",
                            tr!("settings.on_this_computer"),
                            theme.accent,
                            theme.accent_soft,
                        )),
                    )
                    .child(
                        settings_row(
                            &theme,
                            false,
                            tr!("settings.share_anonymous_usage_data"),
                            tr!("settings.share_anonymous_usage_data_description"),
                        )
                        .child(analytics_toggle),
                    )
                    .child(
                        settings_row(
                            &theme,
                            false,
                            tr!("settings.render_math"),
                            tr!("settings.render_math_description"),
                        )
                        .child(math_toggle),
                    )
                    .when(updater_available, |group| {
                        let enabled = self.automatic_updates_enabled;
                        let toggle = toggle_switch(
                            "automatic-updates-toggle",
                            enabled,
                            false,
                            theme,
                            cx,
                            move |this, _, cx| this.set_automatic_updates_enabled(!enabled, cx),
                        );
                        group.child(
                            settings_row(
                                &theme,
                                false,
                                tr!("settings.automatic_updates"),
                                tr!("settings.automatic_updates_description"),
                            )
                            .child(toggle),
                        )
                    }),
            )
            .into_any_element()
    }

    fn set_analytics_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.state.analytics_enabled = enabled;
        self.analytics.set_enabled(enabled);
        self.save();
        cx.notify();
    }

    fn set_automatic_updates_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.automatic_updates_enabled = enabled;
        if let Some(updater) = cx
            .try_global::<crate::updater::UpdaterState>()
            .and_then(|updater| updater.0.as_ref())
        {
            updater.set_automatically_checks_for_updates(enabled);
        }
        cx.notify();
    }

    fn render_daemon_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        if self.daemon.is_remote() {
            return div()
                .mt(px(15.0))
                .w_full()
                .px(px(20.0))
                .py(px(16.0))
                .rounded(px(14.0))
                .border_1()
                .border_color(theme.border)
                .shadow(elevation(&theme, Elevation::Rest))
                .bg(theme.raised)
                .child(
                    div()
                        .text_size(sp(13.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child(tr!("daemon.external_title")),
                )
                .child(
                    div()
                        .mt(px(5.0))
                        .text_size(sp(12.5))
                        .line_height(sp(18.0))
                        .text_color(theme.text_secondary)
                        .child(tr!("daemon.external_description")),
                )
                .into_any_element();
        }

        let enabled = self.state.daemon_exposure.enabled;
        let pending = self.daemon_reconfigure_pending;
        let fields_dirty = self.daemon_exposure_fields_dirty(cx);
        let port = self.state.daemon_exposure.port;
        let websocket_url = format!("ws://{}:{port}", self.daemon_hostname);
        let token = self.state.daemon_exposure.token.clone();

        let exposure_toggle = toggle_switch(
            "daemon-exposure-toggle",
            enabled,
            pending,
            theme,
            cx,
            move |this, _, cx| this.set_daemon_exposure_enabled(!enabled, cx),
        );

        let apply_disabled = pending || !fields_dirty;
        let apply_button = div()
            .id("apply-daemon-settings")
            .tab_index(0)
            .h(px(29.0))
            .px(px(11.0))
            .rounded(px(7.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .items_center()
            .justify_center()
            .cursor_default()
            .text_size(sp(12.5))
            .text_color(theme.text_secondary)
            .opacity(if apply_disabled { 0.55 } else { 1.0 })
            .focus_visible(|style| style.border_color(theme.accent))
            .when(!apply_disabled, |element| {
                element
                    .hover(|element| element.bg(theme.overlay))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.apply_daemon_exposure_fields(cx);
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if !event.keystroke.modifiers.modified()
                            && matches!(event.keystroke.key.as_str(), "enter" | "space")
                        {
                            this.apply_daemon_exposure_fields(cx);
                            cx.stop_propagation();
                        }
                    }))
            })
            .child(if pending {
                tr!("daemon.restarting")
            } else {
                tr!("daemon.apply")
            });

        let copy_url_feedback_id = "daemon-url";
        let url_copied = self.control_was_copied(copy_url_feedback_id);
        let copy_url = websocket_url.clone();
        let copy_url_button = div()
            .id("copy-daemon-url")
            .tab_index(0)
            .h(px(27.0))
            .px(px(9.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .items_center()
            .gap(px(5.0))
            .cursor_default()
            .text_size(sp(12.5))
            .text_color(theme.text_secondary)
            .focus_visible(|style| style.border_color(theme.accent))
            .hover(|element| element.bg(theme.overlay))
            .child(icon(
                if url_copied {
                    "icons/check.svg"
                } else {
                    "icons/copy.svg"
                },
                11.0,
                theme.text_tertiary,
            ))
            .child(if url_copied {
                tr!("common.copied")
            } else {
                tr!("common.copy")
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(copy_url.clone()));
                this.show_control_copied(copy_url_feedback_id, cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                if !event.keystroke.modifiers.modified()
                    && matches!(event.keystroke.key.as_str(), "enter" | "space")
                {
                    cx.write_to_clipboard(ClipboardItem::new_string(websocket_url.clone()));
                    this.show_control_copied(copy_url_feedback_id, cx);
                    cx.stop_propagation();
                }
            }));

        let copy_token_feedback_id = "daemon-token";
        let token_copied = self.control_was_copied(copy_token_feedback_id);
        let click_token = token.clone();
        let key_token = token.clone();
        let token_revealed = self.daemon_token_revealed;
        let reveal_token_button = div()
            .id("reveal-daemon-token")
            .tab_index(0)
            .size(px(27.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .items_center()
            .justify_center()
            .cursor_default()
            .text_color(theme.text_secondary)
            .focus_visible(|style| style.border_color(theme.accent))
            .hover(|element| element.bg(theme.overlay))
            .active(|element| element.bg(theme.overlay_strong))
            .child(icon(
                if token_revealed {
                    "icons/eye-off.svg"
                } else {
                    "icons/eye.svg"
                },
                12.0,
                theme.text_tertiary,
            ))
            .tooltip(Tooltip::text(if token_revealed {
                tr!("daemon.hide_token")
            } else {
                tr!("daemon.reveal_token")
            }))
            .on_click(cx.listener(|this, _, _, cx| {
                this.daemon_token_revealed = !this.daemon_token_revealed;
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if !event.keystroke.modifiers.modified()
                    && matches!(event.keystroke.key.as_str(), "enter" | "space")
                {
                    this.daemon_token_revealed = !this.daemon_token_revealed;
                    cx.stop_propagation();
                    cx.notify();
                }
            }));
        let copy_token_button = div()
            .id("copy-daemon-token")
            .tab_index(0)
            .h(px(27.0))
            .px(px(9.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .items_center()
            .gap(px(5.0))
            .cursor_default()
            .text_size(sp(12.5))
            .text_color(theme.text_secondary)
            .focus_visible(|style| style.border_color(theme.accent))
            .hover(|element| element.bg(theme.overlay))
            .child(icon(
                if token_copied {
                    "icons/check.svg"
                } else {
                    "icons/copy.svg"
                },
                11.0,
                theme.text_tertiary,
            ))
            .child(if token_copied {
                tr!("common.copied")
            } else {
                tr!("common.copy")
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(click_token.clone()));
                this.show_control_copied(copy_token_feedback_id, cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                if !event.keystroke.modifiers.modified()
                    && matches!(event.keystroke.key.as_str(), "enter" | "space")
                {
                    cx.write_to_clipboard(ClipboardItem::new_string(key_token.clone()));
                    this.show_control_copied(copy_token_feedback_id, cx);
                    cx.stop_propagation();
                }
            }));

        let regenerate_button = div()
            .id("regenerate-daemon-token")
            .tab_index(0)
            .h(px(27.0))
            .px(px(9.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .items_center()
            .cursor_default()
            .text_size(sp(12.5))
            .text_color(theme.text_secondary)
            .opacity(if pending { 0.55 } else { 1.0 })
            .focus_visible(|style| style.border_color(theme.accent))
            .when(!pending, |element| {
                element
                    .hover(|element| element.bg(theme.overlay))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.regenerate_daemon_token(cx);
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if !event.keystroke.modifiers.modified()
                            && matches!(event.keystroke.key.as_str(), "enter" | "space")
                        {
                            this.regenerate_daemon_token(cx);
                            cx.stop_propagation();
                        }
                    }))
            })
            .child(tr!("daemon.regenerate_token"));

        div()
            .mt(px(15.0))
            .w_full()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .min_h(px(66.0))
                    .px(px(20.0))
                    .py(px(13.0))
                    .rounded(px(14.0))
                    .border_1()
                    .border_color(theme.border)
                    .shadow(elevation(&theme, Elevation::Rest))
                    .bg(theme.raised)
                    .flex()
                    .items_center()
                    .gap(px(24.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(7.0))
                                    .child(
                                        div()
                                            .text_size(sp(13.5))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme.text)
                                            .child(tr!("daemon.expose_title")),
                                    )
                                    .child(
                                        div()
                                            .px(px(6.0))
                                            .py(px(2.0))
                                            .rounded_full()
                                            .text_size(sp(12.5))
                                            .text_color(if enabled {
                                                theme.success
                                            } else {
                                                theme.text_tertiary
                                            })
                                            .bg(theme.overlay)
                                            .child(if pending {
                                                tr!("daemon.status_restarting")
                                            } else if enabled {
                                                tr!("daemon.status_exposed")
                                            } else {
                                                tr!("daemon.status_local")
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .mt(px(5.0))
                                    .min_w_0()
                                    .whitespace_normal()
                                    .text_size(sp(12.5))
                                    .line_height(sp(18.0))
                                    .text_color(theme.text_secondary)
                                    .child(tr!("daemon.expose_description")),
                            ),
                    )
                    .child(exposure_toggle),
            )
            .when(enabled, |column| {
                column.child(
                    div()
                        .px(px(20.0))
                        .py(px(15.0))
                        .rounded(px(14.0))
                        .border_1()
                        .border_color(theme.border)
                        .shadow(elevation(&theme, Elevation::Rest))
                        .bg(theme.raised)
                        .child(
                            div()
                                .text_size(sp(13.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text)
                                .child(tr!("daemon.connection_title")),
                        )
                        .child(
                            div()
                                .mt(px(4.0))
                                .min_w_0()
                                .whitespace_normal()
                                .text_size(sp(12.5))
                                .line_height(sp(16.0))
                                .text_color(theme.text_secondary)
                                .child(tr!("daemon.connection_description")),
                        )
                        .child(
                            div()
                                .mt(px(14.0))
                                .flex()
                                .items_start()
                                .gap(px(24.0))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .text_size(sp(12.5))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(theme.text)
                                                .child(tr!("daemon.port")),
                                        )
                                        .child(
                                            div()
                                                .mt(px(3.0))
                                                .whitespace_normal()
                                                .text_size(sp(12.5))
                                                .line_height(sp(14.0))
                                                .text_color(theme.text_tertiary)
                                                .child(tr!("daemon.port_description")),
                                        ),
                                )
                                .child(
                                    div().flex_1().min_w_0().flex().justify_end().child(
                                        TextField::new(
                                            "daemon-port-field",
                                            self.daemon_port_input.clone(),
                                        )
                                        .w(px(150.0)),
                                    ),
                                ),
                        )
                        .child(
                            div()
                                .mt(px(14.0))
                                .flex()
                                .items_start()
                                .gap(px(24.0))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .text_size(sp(12.5))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(theme.text)
                                                .child(tr!("daemon.allowed_origins")),
                                        )
                                        .child(
                                            div()
                                                .mt(px(3.0))
                                                .whitespace_normal()
                                                .text_size(sp(12.5))
                                                .line_height(sp(14.0))
                                                .text_color(theme.text_tertiary)
                                                .child(tr!("daemon.allowed_origins_description")),
                                        ),
                                )
                                .child(
                                    div().flex_1().min_w_0().flex().justify_end().child(
                                        TextField::new(
                                            "daemon-origins-field",
                                            self.daemon_origins_input.clone(),
                                        )
                                        .w_full()
                                        .max_w(px(360.0)),
                                    ),
                                ),
                        )
                        .child(div().mt(px(13.0)).flex().justify_end().child(apply_button)),
                )
            })
            .when(enabled, |column| {
                column.child(
                    div()
                        .px(px(20.0))
                        .py(px(15.0))
                        .rounded(px(14.0))
                        .border_1()
                        .border_color(theme.border)
                        .shadow(elevation(&theme, Elevation::Rest))
                        .bg(theme.raised)
                        .child(
                            div()
                                .text_size(sp(13.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text)
                                .child(tr!("daemon.credentials_title")),
                        )
                        .child(
                            div()
                                .mt(px(4.0))
                                .min_w_0()
                                .whitespace_normal()
                                .text_size(sp(12.5))
                                .line_height(sp(16.0))
                                .text_color(theme.text_secondary)
                                .child(tr!("daemon.credentials_description")),
                        )
                        .child(
                            div()
                                .mt(px(13.0))
                                .py(px(8.0))
                                .flex()
                                .items_center()
                                .gap(px(10.0))
                                .child(
                                    div()
                                        .w(px(108.0))
                                        .flex_none()
                                        .whitespace_nowrap()
                                        .text_size(sp(12.5))
                                        .text_color(theme.text_tertiary)
                                        .child(tr!("daemon.websocket_url")),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .font_family(".SystemUIFontMonospaced")
                                        .text_size(sp(12.5))
                                        .text_color(theme.text)
                                        .child(SharedString::from(format!(
                                            "ws://{}:{port}",
                                            self.daemon_hostname
                                        ))),
                                )
                                .child(copy_url_button),
                        )
                        .child(
                            div()
                                .py(px(8.0))
                                .border_t_1()
                                .border_color(theme.border)
                                .flex()
                                .items_center()
                                .gap(px(10.0))
                                .child(
                                    div()
                                        .w(px(108.0))
                                        .flex_none()
                                        .whitespace_nowrap()
                                        .text_size(sp(12.5))
                                        .text_color(theme.text_tertiary)
                                        .child(tr!("daemon.token")),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .font_family(".SystemUIFontMonospaced")
                                        .text_size(sp(12.5))
                                        .text_color(theme.text)
                                        .child(SharedString::from(if token_revealed {
                                            token.clone()
                                        } else {
                                            "••••••••••••••••••••••••••••••••".to_owned()
                                        })),
                                )
                                .child(reveal_token_button)
                                .child(copy_token_button)
                                .child(regenerate_button),
                        )
                        .child(
                            div()
                                .mt(px(7.0))
                                .px(px(10.0))
                                .py(px(8.0))
                                .rounded(px(8.0))
                                .bg(theme.inset)
                                .w_full()
                                .min_w_0()
                                .flex()
                                .gap(px(8.0))
                                .child(icon("icons/alert.svg", 13.0, theme.warning))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .whitespace_normal()
                                        .text_size(sp(12.5))
                                        .line_height(sp(15.0))
                                        .text_color(theme.text_secondary)
                                        .child(tr!("daemon.security_warning")),
                                ),
                        ),
                )
            })
            .into_any_element()
    }

    fn daemon_exposure_from_fields(
        &self,
        cx: &App,
    ) -> Result<waku_client::DaemonExposureSettings, String> {
        let port = self
            .daemon_port_input
            .read(cx)
            .content()
            .trim()
            .parse::<u16>()
            .map_err(|_| tr!("daemon.invalid_port"))?;
        if port == 0 {
            return Err(tr!("daemon.invalid_port"));
        }
        let origins = self.daemon_origins_input.read(cx).content().to_owned();
        let mut settings = self.state.daemon_exposure.clone();
        settings.port = port;
        settings
            .with_allowed_origins_text(&origins)
            .and_then(waku_client::DaemonExposureSettings::validate)
            .map_err(|error| error.to_string())
    }

    fn daemon_exposure_fields_dirty(&self, cx: &App) -> bool {
        self.daemon_exposure_from_fields(cx)
            .map(|settings| {
                settings.port != self.state.daemon_exposure.port
                    || settings.allowed_origins != self.state.daemon_exposure.allowed_origins
            })
            .unwrap_or(true)
    }

    fn set_daemon_exposure_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if !enabled {
            self.daemon_token_revealed = false;
        }
        let settings = if enabled {
            match self.daemon_exposure_from_fields(cx) {
                Ok(mut settings) => {
                    settings.enabled = true;
                    settings
                }
                Err(error) => {
                    self.show_toast(tr!("daemon.invalid_settings", error = error));
                    return;
                }
            }
        } else {
            let mut settings = self.state.daemon_exposure.clone();
            settings.enabled = false;
            settings
        };
        self.apply_daemon_exposure(settings, cx);
    }

    pub(super) fn apply_daemon_exposure_fields(&mut self, cx: &mut Context<Self>) {
        let settings = match self.daemon_exposure_from_fields(cx) {
            Ok(settings) => settings,
            Err(error) => {
                self.show_toast(tr!("daemon.invalid_settings", error = error));
                return;
            }
        };
        self.apply_daemon_exposure(settings, cx);
    }

    fn regenerate_daemon_token(&mut self, cx: &mut Context<Self>) {
        let mut settings = match self.daemon_exposure_from_fields(cx) {
            Ok(settings) => settings,
            Err(error) => {
                self.show_toast(tr!("daemon.invalid_settings", error = error));
                return;
            }
        };
        settings.token = waku_client::DaemonExposureSettings::new_token();
        self.daemon_token_revealed = false;
        self.apply_daemon_exposure(settings, cx);
    }

    fn apply_daemon_exposure(
        &mut self,
        settings: waku_client::DaemonExposureSettings,
        cx: &mut Context<Self>,
    ) {
        if self.daemon_reconfigure_pending || settings == self.state.daemon_exposure {
            return;
        }
        if self.daemon.is_remote() {
            self.show_toast(tr!("daemon.external_description"));
            return;
        }
        if self
            .state
            .sessions
            .iter()
            .any(|session| !matches!(session.status, SessionStatus::Idle | SessionStatus::Failed))
        {
            self.show_toast(tr!("daemon.stop_active_tasks"));
            return;
        }

        let needs_restart = self.state.daemon_exposure.enabled || settings.enabled;
        if !needs_restart {
            self.state.daemon_exposure = settings;
            self.save();
            cx.notify();
            return;
        }

        self.daemon_reconfigure_pending = true;
        let daemon = self.daemon.clone();
        let applied = settings.clone();
        let restart = cx
            .background_executor()
            .spawn(async move { daemon.reconfigure(settings) });
        cx.spawn(async move |this, cx| {
            let result = restart.await;
            let _ = this.update(cx, |this, cx| {
                this.daemon_reconfigure_pending = false;
                match result {
                    Ok(()) => {
                        this.state.daemon_exposure = applied.clone();
                        this.runtimes.clear();
                        this.daemon_port_input.update(cx, |input, cx| {
                            input.set_content(applied.port.to_string(), cx)
                        });
                        this.daemon_origins_input.update(cx, |input, cx| {
                            input.set_content(applied.allowed_origins_text(), cx)
                        });
                        this.save();
                        this.show_success_toast(tr!("daemon.settings_applied"));
                    }
                    Err(error) => {
                        this.show_toast(tr!("daemon.restart_failed", error = error.to_string()))
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn render_appearance_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let selected_theme = self.state.theme;
        let selected_language = self.state.language;
        let weak = cx.entity().downgrade();
        let theme_handle = self.menu_handle("theme-selector", cx);
        let theme_selector = dropdown_menu(
            MenuChip::new("theme-selector")
                .label(selected_theme.label())
                .outlined()
                .selected(theme_handle.is_open())
                .w(px(116.0))
                .justify_between(),
            "theme-selector-menu",
            &theme_handle,
            MenuAlign::BelowRight,
            move |_| {
                ThemePreference::ALL
                    .into_iter()
                    .map(|preference| {
                        let weak = weak.clone();
                        MenuItem::new(preference.label(), move |window, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.set_theme_preference(preference, window, cx);
                            });
                        })
                        .selected(preference == selected_theme)
                    })
                    .collect()
            },
        );

        let selected_ui_font_size = self.state.ui_font_size;
        let weak = cx.entity().downgrade();
        let ui_font_size_handle = self.menu_handle("ui-font-size-selector", cx);
        let ui_font_size_selector = dropdown_menu(
            MenuChip::new("ui-font-size-selector")
                .label(font_size_label(selected_ui_font_size))
                .outlined()
                .selected(ui_font_size_handle.is_open())
                .w(px(116.0))
                .justify_between(),
            "ui-font-size-selector-menu",
            &ui_font_size_handle,
            MenuAlign::BelowRight,
            move |_| {
                FONT_SIZES
                    .into_iter()
                    .map(|size| {
                        let weak = weak.clone();
                        MenuItem::new(font_size_label(size), move |window, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.set_ui_font_size(size, window, cx);
                            });
                        })
                        .selected(size == selected_ui_font_size)
                    })
                    .collect()
            },
        );

        let selected_code_font_size = self.state.code_font_size;
        let weak = cx.entity().downgrade();
        let code_font_size_handle = self.menu_handle("code-font-size-selector", cx);
        let code_font_size_selector = dropdown_menu(
            MenuChip::new("code-font-size-selector")
                .label(font_size_label(selected_code_font_size))
                .outlined()
                .selected(code_font_size_handle.is_open())
                .w(px(116.0))
                .justify_between(),
            "code-font-size-selector-menu",
            &code_font_size_handle,
            MenuAlign::BelowRight,
            move |_| {
                FONT_SIZES
                    .into_iter()
                    .map(|size| {
                        let weak = weak.clone();
                        MenuItem::new(font_size_label(size), move |_, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.set_code_font_size(size, cx);
                            });
                        })
                        .selected(size == selected_code_font_size)
                    })
                    .collect()
            },
        );

        let weak = cx.entity().downgrade();
        let language_handle = self.menu_handle("language-selector", cx);
        let language_selector = dropdown_menu(
            MenuChip::new("language-selector")
                .label(selected_language.label())
                .outlined()
                .selected(language_handle.is_open())
                .w(px(116.0))
                .justify_between(),
            "language-selector-menu",
            &language_handle,
            MenuAlign::BelowRight,
            move |_| {
                crate::i18n::AppLanguage::ALL
                    .into_iter()
                    .map(|language| {
                        let weak = weak.clone();
                        MenuItem::new(language.label(), move |window, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.set_language(language, window, cx);
                            });
                        })
                        .selected(language == selected_language)
                    })
                    .collect()
            },
        );

        div()
            .mt(px(15.0))
            .w_full()
            .flex()
            .flex_col()
            .rounded(px(14.0))
            .border_1()
            .border_color(theme.border)
            .shadow(elevation(&theme, Elevation::Rest))
            .overflow_hidden()
            .bg(theme.raised)
            .child(
                div()
                    .w_full()
                    .min_h(px(60.0))
                    .px(px(20.0))
                    .py(px(12.0))
                    .flex()
                    .items_center()
                    .gap(px(24.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(sp(13.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(tr!("settings.theme")),
                            )
                            .child(
                                div()
                                    .mt(px(5.0))
                                    .text_size(sp(12.5))
                                    .line_height(sp(18.0))
                                    .text_color(theme.text_secondary)
                                    .child(tr!("settings.theme_description")),
                            ),
                    )
                    .child(theme_selector),
            )
            .child(div().mx(px(20.0)).h(px(1.0)).bg(theme.border))
            .child(
                div()
                    .w_full()
                    .min_h(px(60.0))
                    .px(px(20.0))
                    .py(px(12.0))
                    .flex()
                    .items_center()
                    .gap(px(24.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(sp(13.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(tr!("language.title")),
                            )
                            .child(
                                div()
                                    .mt(px(5.0))
                                    .text_size(sp(12.5))
                                    .line_height(sp(18.0))
                                    .text_color(theme.text_secondary)
                                    .child(tr!("language.description")),
                            ),
                    )
                    .child(language_selector),
            )
            .child(div().mx(px(20.0)).h(px(1.0)).bg(theme.border))
            .child(
                div()
                    .w_full()
                    .min_h(px(60.0))
                    .px(px(20.0))
                    .py(px(12.0))
                    .flex()
                    .items_center()
                    .gap(px(24.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(sp(13.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(tr!("settings.ui_font_size")),
                            )
                            .child(
                                div()
                                    .mt(px(5.0))
                                    .text_size(sp(12.5))
                                    .line_height(sp(18.0))
                                    .text_color(theme.text_secondary)
                                    .child(tr!("settings.ui_font_size_description")),
                            ),
                    )
                    .child(ui_font_size_selector),
            )
            .child(div().mx(px(20.0)).h(px(1.0)).bg(theme.border))
            .child(
                div()
                    .w_full()
                    .min_h(px(60.0))
                    .px(px(20.0))
                    .py(px(12.0))
                    .flex()
                    .items_center()
                    .gap(px(24.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(sp(13.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(tr!("settings.code_font_size")),
                            )
                            .child(
                                div()
                                    .mt(px(5.0))
                                    .text_size(sp(12.5))
                                    .line_height(sp(18.0))
                                    .text_color(theme.text_secondary)
                                    .child(tr!("settings.code_font_size_description")),
                            ),
                    )
                    .child(code_font_size_selector),
            )
            .into_any_element()
    }

    fn set_render_math(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.state.render_math == enabled {
            return;
        }
        self.state.render_math = enabled;
        self.remeasure_font_sized_surfaces();
        self.save();
        cx.notify();
    }

    fn set_ui_font_size(&mut self, size: f32, window: &mut Window, cx: &mut Context<Self>) {
        let size = waku_client::persistence::sanitized_ui_font_size(size);
        if self.state.ui_font_size == size {
            return;
        }
        self.state.ui_font_size = size;
        // Chrome is authored in `sp` rems; the rem size is the setting.
        window.set_rem_size(px(size));
        self.remeasure_font_sized_surfaces();
        self.save();
        window.refresh();
        cx.notify();
    }

    fn set_code_font_size(&mut self, size: f32, cx: &mut Context<Self>) {
        let size = waku_client::persistence::sanitized_code_font_size(size);
        if self.state.code_font_size == size {
            return;
        }
        self.state.code_font_size = size;
        self.remeasure_font_sized_surfaces();
        self.save();
        cx.notify();
    }

    /// Drop every cached row height that a font size participates in. The
    /// virtualized lists remember measured heights, so a stale entry would
    /// misplace scroll anchors until the row happened to remeasure. The
    /// sidebar list keeps its uniform row height and needs no reset.
    fn remeasure_font_sized_surfaces(&self) {
        self.reset_transcript_rows(self.transcript_row_count());
        let line_count = self
            .right_panel_diff_snapshot
            .as_ref()
            .map_or(0, |snapshot| snapshot.lines.len());
        self.right_panel_diff_list_state.reset(line_count);
        self.right_panel_diff_tree_list_state
            .reset(self.right_panel_diff_tree_rows.borrow().len());
        self.skills_list_state
            .reset(self.skills_rows.borrow().len());
    }

    pub(super) fn render_providers_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let checking = self.provider_detection_remaining > 0;
        let checked_label = self
            .provider_detection_checked_at
            .filter(|_| !checking)
            .map(|checked_at| detection_checked_label(checked_at.elapsed()));

        let refresh = div()
            .id("refresh-providers")
            .tab_index(0)
            .focus_visible(|style| style.border_color(theme.accent))
            .h(px(28.0))
            .px(px(11.0))
            .rounded(px(7.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .items_center()
            .gap(px(6.0))
            .cursor_default()
            .text_size(sp(12.5))
            .text_color(theme.text_secondary)
            .opacity(if checking { 0.6 } else { 1.0 })
            .hover(|element| element.bg(theme.overlay))
            .child(icon("icons/rotate-cw.svg", 11.0, theme.text_tertiary))
            .child(if checking {
                tr!("common.checking")
            } else {
                tr!("common.refresh")
            })
            .on_click(cx.listener(|this, _, _, cx| {
                this.refresh_provider_detection(None);
                cx.notify();
            }));

        let mut rows = div().mt(px(4.0)).flex().flex_col();
        let provider_count = ProviderKind::ALL.len();
        for (index, kind) in ProviderKind::ALL.into_iter().enumerate() {
            let probe = self.provider_probe(kind);
            let installed = probe.is_some_and(|probe| probe.installed);
            let binary_path = probe
                .filter(|probe| probe.installed)
                .and_then(|probe| probe.path.as_deref())
                .map(|path| abbreviate_home_path(path, self.home_directory.as_deref()));
            let model_count = probe.map(|probe| probe.models.len()).unwrap_or(0);
            let version = self
                .provider_versions
                .get(&kind)
                .and_then(|version| version.clone());
            let disabled = self.state.disabled_providers.contains(&kind);

            let dot_color = if !installed {
                theme.text_ghost
            } else if disabled {
                theme.warning
            } else {
                theme.success
            };

            let detail: AnyElement = if installed {
                let mut parts = Vec::new();
                if let Some(path) = binary_path {
                    parts.push(path);
                }
                if disabled {
                    parts.push(tr!("providers.disabled_for_new_tasks"));
                } else if model_count > 0 {
                    parts.push(if model_count == 1 {
                        tr!("providers.model_count_one", count = model_count)
                    } else {
                        tr!("providers.model_count_many", count = model_count)
                    });
                }
                div()
                    .truncate()
                    .child(SharedString::from(parts.join("  ·  ")))
                    .into_any_element()
            } else if kind == ProviderKind::Codex {
                div().child("OpenAI Codex CLI not detected. Choose a binary whose --version reports codex-cli.")
                    .into_any_element()
            } else {
                div()
                    .flex()
                    .items_baseline()
                    .child(SharedString::from(tr!(
                        "providers.not_detected_as",
                        command = kind.command()
                    )))
                    .into_any_element()
            };

            let toggle_on = !disabled;
            let toggle = toggle_switch(
                SharedString::from(format!("provider-enabled-{}", kind.id())),
                toggle_on,
                false,
                theme,
                cx,
                move |this, _, cx| this.set_provider_enabled(kind, disabled, cx),
            );

            let expanded = self.expanded_provider_settings == Some(kind);
            let expand_button = icon_button(
                SharedString::from(format!("provider-expand-{}", kind.id())),
                if expanded {
                    "icons/chevron-down.svg"
                } else {
                    "icons/chevron-right.svg"
                },
                theme,
            )
            .tab_index(0)
            .focus_visible(|style| style.border_1().border_color(theme.accent))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.toggle_provider_expanded(kind, window, cx);
            }));

            let header = div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(
                    div()
                        .relative()
                        .w(px(30.0))
                        .h(px(30.0))
                        .flex_none()
                        .rounded(px(7.0))
                        .bg(theme.overlay)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(provider_mark(
                            &theme,
                            kind,
                            16.0,
                            provider_color(&theme, kind).opacity(if installed { 1.0 } else { 0.5 }),
                        ))
                        .child(
                            div()
                                .absolute()
                                .bottom(px(-2.0))
                                .right(px(-2.0))
                                .w(px(10.0))
                                .h(px(10.0))
                                .rounded_full()
                                .border_2()
                                .border_color(theme.raised)
                                .bg(dot_color),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .flex()
                                .items_baseline()
                                .gap(px(7.0))
                                .child(
                                    div()
                                        .text_size(sp(12.5))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(if installed {
                                            theme.text
                                        } else {
                                            theme.text_secondary
                                        })
                                        .child(kind.display_name()),
                                )
                                .when_some(version, |element, version| {
                                    element.child(
                                        div()
                                            .font_family(crate::md::render::MONO_FAMILY)
                                            .text_size(sp(12.5))
                                            .text_color(theme.text_tertiary)
                                            .child(SharedString::from(format!("v{version}"))),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .mt(px(3.0))
                                .text_size(sp(12.5))
                                .text_color(theme.text_tertiary)
                                .child(detail),
                        ),
                )
                .child(expand_button)
                .when(installed, |element| element.child(toggle));

            rows = rows.child(
                div()
                    .py(px(11.0))
                    .flex()
                    .flex_col()
                    .when(index + 1 != provider_count, |element| {
                        element.border_b_1().border_color(theme.border)
                    })
                    .child(header)
                    .when(expanded, |element| {
                        element.child(self.render_provider_expanded_settings(kind, theme, cx))
                    }),
            );
        }

        div()
            .mt(px(15.0))
            .w_full()
            .px(px(20.0))
            .py(px(14.0))
            .rounded(px(14.0))
            .border_1()
            .border_color(theme.border)
            .shadow(elevation(&theme, Elevation::Rest))
            .bg(theme.raised)
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(20.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(sp(13.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(tr!("providers.coding_agents")),
                            )
                            .child(
                                div()
                                    .mt(px(5.0))
                                    .text_size(sp(12.5))
                                    .line_height(sp(18.0))
                                    .text_color(theme.text_secondary)
                                    .child(tr!("providers.description")),
                            ),
                    )
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .flex_col()
                            .items_end()
                            .gap(px(6.0))
                            .child(refresh)
                            .when_some(checked_label, |element, label| {
                                element.child(
                                    div()
                                        .text_size(sp(12.5))
                                        .text_color(theme.text_ghost)
                                        .child(SharedString::from(label)),
                                )
                            }),
                    ),
            )
            .child(rows)
            .into_any_element()
    }

    /// The expanded row's settings body: the binary override for this
    /// provider, with the detection result as its caption.
    fn render_provider_expanded_settings(
        &self,
        kind: ProviderKind,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let override_value = self.state.provider_binary_overrides.get(&kind).cloned();
        let full_path = self
            .provider_probe(kind)
            .filter(|probe| probe.installed)
            .and_then(|probe| probe.path.as_ref())
            .map(|path| path.display().to_string());

        let caption = match (&override_value, full_path) {
            (Some(_), Some(path)) => tr!("providers.using_override", path = path),
            (Some(_), None) => tr!("providers.invalid_override"),
            (None, Some(path)) => tr!("providers.detected_at", path = path),
            (None, None) => tr!("providers.searches_path", command = kind.command()),
        };

        let reset = div()
            .id(SharedString::from(format!(
                "provider-path-reset-{}",
                kind.id()
            )))
            .tab_index(0)
            .focus_visible(|style| style.border_color(theme.accent))
            .h(px(29.0))
            .px(px(10.0))
            .rounded(px(7.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .flex_none()
            .items_center()
            .cursor_default()
            .text_size(sp(12.5))
            .text_color(theme.text_secondary)
            .hover(|element| element.bg(theme.overlay))
            .child(tr!("common.reset"))
            .on_click(cx.listener(|this, _, _, cx| {
                this.provider_path_input
                    .update(cx, |input, cx| input.clear(cx));
                this.apply_provider_path_override(cx);
            }));

        div()
            .mt(px(10.0))
            .pl(px(42.0))
            .flex()
            .flex_col()
            .gap(px(5.0))
            .child(
                div()
                    .text_size(sp(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child(tr!("providers.binary_path")),
            )
            .child(
                div()
                    .text_size(sp(12.5))
                    .line_height(sp(15.0))
                    .text_color(theme.text_tertiary)
                    .child(SharedString::from(tr!(
                        "providers.binary_path_description",
                        provider = kind.short_name()
                    ))),
            )
            .child(
                div()
                    .mt(px(3.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        TextField::new(
                            SharedString::from(format!("provider-path-field-{}", kind.id())),
                            self.provider_path_input.clone(),
                        )
                        .flex_1()
                        .max_w(px(430.0)),
                    )
                    .when(override_value.is_some(), |element| element.child(reset)),
            )
            .child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_ghost)
                    .child(SharedString::from(caption)),
            )
    }

    fn toggle_provider_expanded(
        &mut self,
        provider: ProviderKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Commit any pending edit for the previously expanded provider before
        // the input is handed to another row.
        self.apply_provider_path_override(cx);
        if self.expanded_provider_settings == Some(provider) {
            self.expanded_provider_settings = None;
        } else {
            self.expanded_provider_settings = Some(provider);
            let override_value = self
                .state
                .provider_binary_overrides
                .get(&provider)
                .cloned()
                .unwrap_or_default();
            self.provider_path_input
                .update(cx, |input, cx| input.set_content(override_value, cx));
            let focus = self.provider_path_input.read(cx).focus();
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    /// Commit the binary override edit for the expanded provider: empty means
    /// detect from PATH. Re-detects that provider and refreshes every catalog
    /// keyed by the executable path.
    pub(super) fn apply_provider_path_override(&mut self, cx: &mut Context<Self>) {
        let Some(provider) = self.expanded_provider_settings else {
            return;
        };
        let text = self
            .provider_path_input
            .read(cx)
            .content()
            .trim()
            .to_owned();
        let current = self
            .state
            .provider_binary_overrides
            .get(&provider)
            .cloned()
            .unwrap_or_default();
        if text == current {
            return;
        }
        if text.is_empty() {
            self.state.provider_binary_overrides.remove(&provider);
        } else {
            self.state.provider_binary_overrides.insert(provider, text);
        }
        self.save();
        self.refresh_provider_detection(Some(provider));
        self.refresh_composer_sources(cx);
        cx.notify();
    }

    /// Providers switched off here stop offering models to new sessions;
    /// sessions already locked to them keep working.
    fn set_provider_enabled(
        &mut self,
        provider: ProviderKind,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        if enabled {
            self.state
                .disabled_providers
                .retain(|kind| *kind != provider);
        } else if !self.state.disabled_providers.contains(&provider) {
            self.state.disabled_providers.push(provider);
        }
        if !enabled
            && let Some(fallback) = ProviderKind::ALL
                .into_iter()
                .find(|kind| self.provider_enabled(*kind))
        {
            // New work must land somewhere usable: move the new-session
            // default and any unstarted drafts off the switched-off provider.
            // The remembered model belongs to the old provider, so it resets
            // with it.
            if self.state.last_provider == provider {
                self.state.last_provider = fallback;
                self.state.last_model = None;
                self.state.last_reasoning_effort = None;
                self.state.last_service_tier = None;
                self.state.last_context_window = None;
            }
            let draft_ids = self
                .state
                .sessions
                .iter()
                .filter(|session| session.provider == provider && !session.has_started())
                .map(|session| session.id)
                .collect::<Vec<_>>();
            for id in draft_ids {
                if let Some(session) = self.state.session_mut(id) {
                    session.provider = fallback;
                    session.model = None;
                    session.reasoning_effort = None;
                    session.service_tier = None;
                    session.context_window = None;
                }
            }
        }
        self.save();
        cx.notify();
    }

    fn render_computer_use_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let enabled = self.state.computer_use_enabled;
        let permissions = self.computer_permissions.clone();
        let pending = self.computer_permission_request_pending;
        let helper_name = crate::computer_use::helper_display_name();
        let mut allowed_apps = div().flex().flex_col().gap(px(1.0));
        if self.state.computer_use_allowed_apps.is_empty() {
            allowed_apps = allowed_apps.child(
                div()
                    .py(px(12.0))
                    .text_size(sp(12.5))
                    .text_color(theme.text_tertiary)
                    .child(tr!("computer_use.no_always_allowed_apps")),
            );
        } else {
            for (index, grant) in self.state.computer_use_allowed_apps.iter().enumerate() {
                let key = grant.key();
                let is_last = index + 1 == self.state.computer_use_allowed_apps.len();
                let app_icon = self.computer_use_app_icon(&grant.bundle_id, cx);
                allowed_apps = allowed_apps.child(
                    div()
                        .py(px(9.0))
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .when(!is_last, |element| {
                            element.border_b_1().border_color(theme.border)
                        })
                        .child(
                            div()
                                .w(px(32.0))
                                .h(px(32.0))
                                .flex_none()
                                .rounded(px(7.0))
                                .when_some(app_icon, |element, app_icon| {
                                    element.child(img(app_icon).size_full().rounded(px(7.0)))
                                }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .text_size(sp(12.5))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.text)
                                        .child(SharedString::from(grant.app_name.clone())),
                                )
                                .child(
                                    div()
                                        .mt(px(2.0))
                                        .text_size(sp(12.5))
                                        .text_color(theme.text_tertiary)
                                        .truncate()
                                        .child(SharedString::from(grant.bundle_id.clone())),
                                ),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("revoke-computer-app-{key}")))
                                .h(px(25.0))
                                .px(px(9.0))
                                .rounded(px(6.0))
                                .border_1()
                                .border_color(theme.border_strong)
                                .flex()
                                .items_center()
                                .cursor_default()
                                .text_size(sp(12.5))
                                .text_color(theme.text_secondary)
                                .hover(|element| element.bg(theme.overlay).text_color(theme.danger))
                                .child(tr!("common.revoke"))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.revoke_computer_app(&key, cx);
                                })),
                        ),
                );
            }
        }

        div()
            .mt(px(15.0))
            .w_full()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .px(px(20.0))
                    .py(px(14.0))
                    .rounded(px(14.0))
                    .border_1()
                    .border_color(theme.border)
                    .shadow(elevation(&theme, Elevation::Rest))
                    .bg(theme.raised)
                    .flex()
                    .items_center()
                    .gap(px(20.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(sp(13.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(tr!("computer_use.allow_apps")),
                            )
                            .child(
                                div()
                                    .mt(px(5.0))
                                    .text_size(sp(12.5))
                                    .line_height(sp(18.0))
                                    .text_color(theme.text_secondary)
                                    .child(tr!("computer_use.availability")),
                            ),
                    )
                    .child(toggle_switch(
                        "computer-use-enabled",
                        enabled,
                        false,
                        theme,
                        cx,
                        move |this, _, cx| this.set_computer_use_enabled(!enabled, cx),
                    )),
            )
            .when(cfg!(target_os = "macos"), |element| {
                element.child(
                    div()
                        .px(px(20.0))
                        .py(px(14.0))
                        .rounded(px(14.0))
                        .border_1()
                        .border_color(theme.border)
                        .shadow(elevation(&theme, Elevation::Rest))
                        .bg(theme.raised)
                        .child(
                            div()
                                .text_size(sp(13.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text)
                                .child(tr!("computer_use.macos_access")),
                        )
                        .child(
                            div()
                                .mt(px(4.0))
                                .text_size(sp(12.5))
                                .text_color(theme.text_secondary)
                                .child(SharedString::from(tr!(
                                    "computer_use.helper_access",
                                    helper = helper_name
                                ))),
                        )
                        .child(permission_status_row(
                            tr!("computer_use.screen_recording"),
                            tr!("computer_use.screen_recording_description"),
                            permissions.screen_recording,
                            "screen-recording-settings",
                            theme,
                            cx,
                        ))
                        .child(permission_status_row(
                            tr!("computer_use.accessibility"),
                            tr!("computer_use.accessibility_description"),
                            permissions.accessibility,
                            "accessibility-settings",
                            theme,
                            cx,
                        ))
                        .child(
                            div().mt(px(11.0)).flex().items_center().gap(px(8.0)).child(
                                div()
                                    .id("recheck-computer-permissions")
                                    .h(px(28.0))
                                    .px(px(11.0))
                                    .rounded(px(7.0))
                                    .border_1()
                                    .border_color(theme.border_strong)
                                    .text_color(theme.text_secondary)
                                    .flex()
                                    .items_center()
                                    .cursor_default()
                                    .text_size(sp(12.5))
                                    .opacity(if pending { 0.6 } else { 1.0 })
                                    .child(if pending {
                                        tr!("common.checking")
                                    } else {
                                        tr!("common.recheck")
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request_computer_permissions(false, cx);
                                    })),
                            ),
                        ),
                )
            })
            .when(!cfg!(target_os = "macos"), |element| {
                element.child(
                    div()
                        .px(px(20.0))
                        .py(px(14.0))
                        .rounded(px(14.0))
                        .border_1()
                        .border_color(theme.border)
                        .shadow(elevation(&theme, Elevation::Rest))
                        .bg(theme.raised)
                        .child(
                            div()
                                .text_size(sp(13.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text)
                                .child(tr!("computer_use.desktop_access")),
                        )
                        .child(
                            div()
                                .mt(px(5.0))
                                .text_size(sp(12.5))
                                .line_height(sp(18.0))
                                .text_color(theme.text_secondary)
                                .child(if cfg!(target_os = "windows") {
                                    tr!("computer_use.windows_access_description")
                                } else {
                                    tr!("computer_use.linux_access_description")
                                }),
                        ),
                )
            })
            .child(
                div()
                    .px(px(20.0))
                    .py(px(14.0))
                    .rounded(px(14.0))
                    .border_1()
                    .border_color(theme.border)
                    .shadow(elevation(&theme, Elevation::Rest))
                    .bg(theme.raised)
                    .child(
                        div()
                            .text_size(sp(13.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(tr!("computer_use.always_allowed_apps")),
                    )
                    .child(
                        div()
                            .mt(px(4.0))
                            .text_size(sp(12.5))
                            .text_color(theme.text_secondary)
                            .child(tr!("computer_use.always_allowed_apps_description")),
                    )
                    .child(allowed_apps),
            )
            .into_any_element()
    }

    fn set_computer_use_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.state.computer_use_enabled = enabled;
        self.save();
        if enabled {
            self.request_computer_permissions(true, cx);
        }
        cx.notify();
    }

    pub(super) fn request_computer_permissions(&mut self, prompt: bool, cx: &mut Context<Self>) {
        if !cfg!(target_os = "macos")
            || !crate::computer_use::is_available()
            || self.computer_permission_request_pending
        {
            return;
        }
        self.computer_permission_request_pending = true;
        let tx = self.computer_permission_tx.clone();
        let event_wake = self.event_wake_tx.clone();
        let daemon = self.daemon.client();
        std::thread::Builder::new()
            .name("waku-computer-permission-request".into())
            .spawn(move || {
                let result = match daemon.request(
                    Uuid::nil(),
                    Uuid::nil(),
                    waku_client::Command::ProbeComputerPermissions { prompt },
                ) {
                    Ok(waku_client::ResponsePayload::ComputerPermissions { permissions }) => {
                        Ok(permissions)
                    }
                    Ok(_) => Err("the daemon returned an invalid permission response".into()),
                    Err(error) => Err(error.to_string()),
                };
                if tx.send(result).is_ok() {
                    signal_event_pump(&event_wake);
                }
            })
            .ok();
        cx.notify();
    }

    fn revoke_computer_app(&mut self, key: &str, cx: &mut Context<Self>) {
        self.state
            .computer_use_allowed_apps
            .retain(|grant| grant.key() != key);
        self.save();
        cx.notify();
    }

    fn computer_use_app_icon(
        &self,
        bundle_id: &str,
        cx: &mut Context<Self>,
    ) -> Option<std::sync::Arc<gpui::Image>> {
        if let Some(icon) = self.computer_use_app_icons.borrow().get(bundle_id) {
            return icon.clone();
        }

        let bundle_id = bundle_id.to_owned();
        if self
            .computer_use_app_icon_loads
            .borrow_mut()
            .insert(bundle_id.clone())
        {
            cx.spawn(async move |this, cx| {
                let load_bundle_id = bundle_id.clone();
                let icon =
                    cx.background_executor()
                        .spawn(async move {
                            crate::platform::load_app_icon_for_bundle_id(&load_bundle_id)
                        })
                        .await;
                let _ = this.update(cx, |this, cx| {
                    this.computer_use_app_icon_loads
                        .borrow_mut()
                        .remove(&bundle_id);
                    this.computer_use_app_icons
                        .borrow_mut()
                        .insert(bundle_id, icon);
                    cx.notify();
                });
            })
            .detach();
        }
        None
    }

    fn render_settings_drag_region(
        &self,
        id: &'static str,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let region = div().id(id);
        // Windows drags from the hit test rather than a mouse-move handler.
        #[cfg(target_os = "windows")]
        let region = region.window_control_area(gpui::WindowControlArea::Drag);

        region
            .h(px(48.0))
            .flex_none()
            .on_click(|event, window, _| {
                if event.click_count() == 2 {
                    crate::platform::titlebar_double_click(window);
                }
            })
            .on_mouse_down_out(cx.listener(|this, _, _, _| {
                this.header_drag_armed = false;
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.header_drag_armed = true;
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.header_drag_armed = false;
                }),
            )
            .on_mouse_move(cx.listener(|this, _, window, _| {
                if this.header_drag_armed {
                    this.header_drag_armed = false;
                    crate::platform::start_window_move(window);
                }
            }))
    }

    pub(super) fn set_theme_preference(
        &mut self,
        preference: ThemePreference,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.theme == preference {
            return;
        }
        self.state.theme = preference;
        crate::theme::apply_theme_preference(preference, window, cx);
        self.save();
        cx.notify();
    }

    fn set_language(
        &mut self,
        language: crate::i18n::AppLanguage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.language == language {
            return;
        }

        self.state.language = language;
        crate::i18n::set_language(language);

        self.composer.update(cx, |input, cx| {
            input.set_placeholder(tr!("input.composer_placeholder"), cx)
        });
        self.model_search.update(cx, |input, cx| {
            input.set_placeholder(tr!("input.search_models"), cx)
        });
        self.branch_search.update(cx, |input, cx| {
            input.set_placeholder(tr!("input.search_branches"), cx)
        });
        self.branch_create_input.update(cx, |input, cx| {
            input.set_placeholder(tr!("input.new_branch_name"), cx)
        });
        self.settings_search.update(cx, |input, cx| {
            input.set_placeholder(tr!("settings.search"), cx)
        });
        self.skills_search.update(cx, |input, cx| {
            input.set_placeholder(tr!("skills.search"), cx)
        });
        self.provider_path_input.update(cx, |input, cx| {
            input.set_placeholder(tr!("input.detected_automatically"), cx)
        });
        self.usage_project_filter.update(cx, |input, cx| {
            input.set_placeholder(tr!("input.filter_projects"), cx)
        });
        self.refresh_command_palette_localized_text(cx);
        self.refresh_file_search_localized_text(cx);
        self.refresh_transcript_search_localized_text(cx);
        for browser in self.right_panel_browsers.values() {
            browser.update(cx, |browser, cx| browser.refresh_localized_text(cx));
        }
        for terminal in self.right_panel_terminals.values() {
            terminal.update(cx, |terminal, cx| terminal.refresh_localized_text(cx));
        }
        for probe in &mut self.probes {
            probe.models = crate::model_catalog::fallback_models(probe.provider);
        }
        self.refresh_provider_detection(None);
        self.invalidate_composer_sources(cx);

        let updater_available = cx
            .try_global::<crate::updater::UpdaterState>()
            .and_then(|updater| updater.0.as_ref())
            .is_some();
        crate::set_app_menus(cx, updater_available);
        self.save();
        window.refresh();
        cx.notify();
    }
}

/// Sizes offered by the font-size dropdowns. A hand-edited `app.json` may
/// hold values outside this list; they render as-is and simply select
/// nothing here.
const FONT_SIZES: [f32; 8] = [11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 18.0, 20.0];

/// One settings group: related rows share a single raised card and are split
/// by hairlines, so a page reads as a few calm blocks rather than a stack of
/// separate floating panels.
pub(super) fn settings_group(theme: &Theme) -> Div {
    div()
        .mt(px(16.0))
        .w_full()
        .rounded(px(14.0))
        .border_1()
        .border_color(theme.border)
        .bg(theme.raised)
        .shadow(elevation(theme, Elevation::Rest))
        .flex()
        .flex_col()
}

/// A settings row: title and description on the left; the caller appends the
/// control on the right.
pub(super) fn settings_row(
    theme: &Theme,
    first: bool,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
) -> Div {
    div()
        .w_full()
        .min_h(px(64.0))
        .px(px(20.0))
        .py(px(14.0))
        .flex()
        .items_center()
        .gap(px(24.0))
        .when(!first, |row| row.border_t_1().border_color(theme.border))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(sp(13.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child(title.into()),
                )
                .child(
                    div()
                        .mt(px(4.0))
                        .text_size(sp(12.5))
                        .line_height(sp(18.0))
                        .text_color(theme.text_secondary)
                        .child(description.into()),
                ),
        )
}

fn font_size_label(size: f32) -> String {
    if size.fract() == 0.0 {
        format!("{size:.0} px")
    } else {
        format!("{size} px")
    }
}

/// "Checked …" caption for the Providers page. Recomputed whenever the page
/// redraws; precision beyond the minute is noise here.
fn detection_checked_label(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    if seconds < 90 {
        tr!("providers.checked_just_now")
    } else if seconds < 3600 {
        tr!("providers.checked_minutes_ago", count = seconds / 60)
    } else {
        tr!("providers.checked_hours_ago", count = seconds / 3600)
    }
}

/// Keep the full binary path, abbreviating only the user's home directory.
fn abbreviate_home_path(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(relative) if relative.as_os_str().is_empty() => "~".to_owned(),
        Some(relative) => format!("~/{}", relative.display()),
        None => path.display().to_string(),
    }
}

fn permission_status_row(
    name: String,
    description: String,
    granted: bool,
    id: &'static str,
    theme: Theme,
    cx: &mut Context<Waku>,
) -> Div {
    let status = if granted {
        div()
            .id(id)
            .h(px(25.0))
            .px(px(4.0))
            .rounded(px(6.0))
            .flex()
            .items_center()
            .gap(px(5.0))
            .cursor_default()
            .text_size(sp(12.5))
            .text_color(theme.success)
            .child(icon("icons/check.svg", 12.0, theme.success))
            .child(tr!("computer_use.access_granted"))
    } else {
        div()
            .id(id)
            .h(px(25.0))
            .px(px(9.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .items_center()
            .cursor_default()
            .text_size(sp(12.5))
            .text_color(theme.text_secondary)
            .hover(|element| element.bg(theme.overlay).text_color(theme.text))
            .child(tr!("computer_use.grant_access"))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.request_computer_permissions(true, cx);
            }))
    };

    div()
        .mt(px(10.0))
        .pt(px(10.0))
        .border_t_1()
        .border_color(theme.border)
        .flex()
        .items_center()
        .gap(px(10.0))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(sp(12.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child(name),
                )
                .child(
                    div()
                        .mt(px(2.0))
                        .text_size(sp(12.5))
                        .text_color(theme.text_tertiary)
                        .child(description),
                ),
        )
        .child(status)
}

#[cfg(test)]
mod tests {
    use super::abbreviate_home_path;
    use std::path::Path;

    #[test]
    fn provider_paths_abbreviate_only_the_home_prefix() {
        let home = Path::new("/Users/example");

        assert_eq!(
            abbreviate_home_path(Path::new("/Users/example/.local/bin/amp"), Some(home)),
            "~/.local/bin/amp"
        );
        assert_eq!(
            abbreviate_home_path(Path::new("/opt/homebrew/bin/codex"), Some(home)),
            "/opt/homebrew/bin/codex"
        );
    }
}
