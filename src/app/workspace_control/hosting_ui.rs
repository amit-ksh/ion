//! Account hosting controls. Snapshot preparation and HTTP run on workers.
use super::*;
use crate::credentials::cloud::{
    bundle::{self, Bundle},
    services,
};
use std::sync::Arc;
#[derive(Clone)]
enum Source {
    Artifact(Artifact),
}
pub(super) struct HostingUi {
    pub open: bool,
    busy: bool,
    generation: u64,
    source: Option<Source>,
    workspace: Option<String>,
    bundle: Option<Arc<Bundle>>,
    sites: Vec<serde_json::Value>,
    error: Option<String>,
    runtime: &'static str,
    public: bool,
    spa: bool,
    operation: Uuid,
    entry: Entity<TextInput>,
    output: Entity<TextInput>,
    paths: ListState,
    focus: FocusHandle,
}
impl HostingUi {
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        Self {
            open: false,
            busy: false,
            generation: 0,
            source: None,
            workspace: None,
            bundle: None,
            sites: vec![],
            error: None,
            runtime: "static",
            public: false,
            spa: false,
            operation: Uuid::new_v4(),
            entry: cx.new(|cx| TextInput::new(window, cx).placeholder("index.html")),
            output: cx.new(|cx| TextInput::new(window, cx).placeholder("dist, out, build or .")),
            paths: ListState::new(0, ListAlignment::Top, px(22.0)),
            focus: cx.focus_handle(),
        }
    }
    fn source_id(&self) -> Option<String> {
        self.source.as_ref().map(|s| match s {
            Source::Artifact(a) => format!("artifact-{}", a.id),
        })
    }
}
fn artifact_bundle(
    client: waku_client::DaemonClient,
    artifact: Artifact,
) -> anyhow::Result<Bundle> {
    let evidence = artifact
        .evidence
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Only verified AI artifacts can be published"))?;
    let ResponsePayload::BlobData { bytes } = client.request(
        Uuid::nil(),
        Uuid::nil(),
        Rpc::ReadWorkspaceArtifact { id: artifact.id },
    )?
    else {
        anyhow::bail!("Could not read the artifact");
    };
    if !bytes.starts_with(waku_client::html_resources::PREFIX.as_bytes()) {
        return bundle::html(bytes, &evidence.sha256);
    }
    use waku_client::artifact_library::*;
    use waku_client::html_resources::*;
    let expected = ArtifactLibraryItem {
        id: artifact.id,
        name: artifact.name.clone(),
        format: artifact.format.clone(),
        project_id: artifact
            .project_id
            .ok_or_else(|| anyhow::anyhow!("Artifact project is unavailable"))?,
        task_id: artifact
            .task_id
            .ok_or_else(|| anyhow::anyhow!("Artifact task is unavailable"))?,
        created_at: artifact.created_at,
        bytes: evidence.bytes,
        sha256: evidence.sha256.clone(),
        origin: evidence.producer.origin,
    };
    let ResponsePayload::ArtifactHtmlManifest { result } = client.request(
        Uuid::nil(),
        Uuid::nil(),
        Rpc::ReadArtifactHtmlManifest {
            query: ArtifactDocumentQuery {
                expected: expected.clone(),
            },
        },
    )?
    else {
        anyhow::bail!("HTML resources are unavailable");
    };
    let manifest = result
        .manifest
        .ok_or_else(|| anyhow::anyhow!("HTML resource manifest is unavailable"))?;
    let (commitment, total) = commitment(&manifest.entry_path, &manifest.sources)?;
    anyhow::ensure!(
        commitment == manifest.sha256
            && total == manifest.total_bytes
            && manifest.artifact_sha256 == evidence.sha256,
        "HTML resources changed"
    );
    let mut sources = Vec::new();
    for descriptor in &manifest.sources {
        let mut data = Vec::new();
        let mut offset = 0u32;
        loop {
            let query = HtmlSourceQuery {
                manifest_sha256: manifest.sha256.clone(),
                source_index: descriptor.index,
                source_sha256: descriptor.sha256.clone(),
                offset,
                limit: CHUNK_BYTES,
            };
            let ResponsePayload::ArtifactHtmlSource { result } = client.request(
                Uuid::nil(),
                Uuid::nil(),
                Rpc::ReadArtifactHtmlSource {
                    query: ArtifactHtmlSourceQuery {
                        expected: expected.clone(),
                        source: query,
                    },
                },
            )?
            else {
                anyhow::bail!("HTML source read failed");
            };
            anyhow::ensure!(
                result.chunk.offset == offset
                    && result.chunk.source_sha256 == descriptor.sha256
                    && result.chunk.manifest_sha256 == manifest.sha256
                    && result.chunk.artifact_sha256 == evidence.sha256,
                "HTML source changed"
            );
            data.extend(&result.chunk.data);
            anyhow::ensure!(
                data.len() <= descriptor.bytes as usize,
                "HTML source exceeded its size"
            );
            if let Some(next) = result.chunk.next_offset {
                anyhow::ensure!(
                    next > offset && next as usize == data.len(),
                    "Invalid HTML source cursor"
                );
                offset = next;
            } else {
                break;
            }
        }
        sources.push((
            descriptor.path.clone(),
            data,
            descriptor.sha256.clone(),
            descriptor.bytes as u64,
        ));
    }
    bundle::resources(sources, manifest.entry_path)
}
impl Waku {
    pub(super) fn open_artifact_hosting(&mut self, artifact: Artifact, cx: &mut Context<Self>) {
        self.prepare_hosting(Some(Source::Artifact(artifact)), cx);
    }
    pub(in crate::app) fn open_hosting_library(&mut self, cx: &mut Context<Self>) {
        self.control_ui.page = Some(Page::Artifacts);
        self.prepare_hosting(None, cx);
    }
    fn prepare_hosting(&mut self, source: Option<Source>, cx: &mut Context<Self>) {
        if self.control_ui.hosting.busy {
            return;
        }
        self.prepare_hosting_export(source, cx);
    }
    fn prepare_hosting_export(&mut self, source: Option<Source>, cx: &mut Context<Self>) {
        if self.control_ui.hosting.busy {
            return;
        }
        let ui = &mut self.control_ui.hosting;
        ui.open = true;
        ui.busy = true;
        ui.generation += 1;
        ui.source = source.clone();
        ui.workspace = self.cloud_ui.status.workspace_id.clone();
        let expected_workspace = ui.workspace.clone();
        ui.bundle = None;
        ui.sites.clear();
        ui.error = None;
        ui.operation = Uuid::new_v4();
        ui.public = false;
        ui.spa = false;
        ui.runtime = if matches!(&source, Some(Source::Artifact(_))) {
            "document"
        } else {
            "static"
        };
        ui.output
            .update(cx, |input, cx| input.set_content(String::new(), cx));
        let generation = ui.generation;
        let client = self.daemon.client();
        cx.spawn(async move |view, cx| {
            let (bundle, sites) = cx
                .background_executor()
                .spawn(async move {
                    let bundle = match source {
                        Some(Source::Artifact(a)) => artifact_bundle(client, a).map(Some),
                        None => Ok(None),
                    };
                    let sites = expected_workspace
                        .as_deref()
                        .ok_or_else(|| anyhow::anyhow!("Sign in to Ion before publishing"))
                        .and_then(|workspace| {
                            services::hosting_request(workspace, "GET", "/v1/hosting/sites", None)
                        });
                    (bundle, sites)
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                let ui = &mut this.control_ui.hosting;
                if ui.generation != generation {
                    return;
                }
                ui.busy = false;
                match bundle {
                    Ok(Some(bundle)) => {
                        ui.entry
                            .update(cx, |i, cx| i.set_content(bundle.entry.clone(), cx));
                        if let Some(directory) = &bundle.export_directory {
                            ui.output
                                .update(cx, |i, cx| i.set_content(directory.clone(), cx));
                        }
                        ui.paths.reset(bundle.paths.len());
                        ui.bundle = Some(Arc::new(bundle));
                    }
                    Ok(None) => {}
                    Err(error) => ui.error = Some(error.to_string()),
                }
                match sites {
                    Ok(value) => ui.sites = value["items"].as_array().cloned().unwrap_or_default(),
                    Err(error) => ui.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn publish_hosting(&mut self, cx: &mut Context<Self>) {
        let ui = &self.control_ui.hosting;
        if ui.busy {
            return;
        }
        let Some(bundle) = ui.bundle.clone() else {
            return;
        };
        let Some(source) = ui.source.clone() else {
            return;
        };
        let source_id = ui.source_id().unwrap();
        let (title, kind) = match source {
            Source::Artifact(a) => (a.name, "artifact"),
        };
        if bundle
            .export_directory
            .as_deref()
            .is_some_and(|directory| directory != ui.output.read(cx).content().trim())
        {
            self.control_ui.hosting.error =
                Some("Review the selected static export folder before publishing.".into());
            cx.notify();
            return;
        }
        let settings = serde_json::json!({"runtime":ui.runtime,"entry":ui.entry.read(cx).content(),"build":[],"start":[],"outputDirectory":"dist","port":3000,"healthPath":"/","spa":ui.spa});
        let visibility = if ui.public { "public" } else { "private" };
        let operation = ui.operation.to_string();
        self.run_hosting_request("POST","/v1/hosting/releases".into(),Some((bundle,serde_json::json!({"sourceId":source_id,"title":title,"kind":kind,"visibility":visibility,"idempotencyKey":operation,"settings":settings}))),None,cx);
    }

    fn hosting_action(
        &mut self,
        id: String,
        action: &'static str,
        body: serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        if self.control_ui.hosting.busy {
            return;
        }
        self.run_hosting_request(
            "POST",
            format!("/v1/hosting/sites/{id}/{action}"),
            None,
            Some(body),
            cx,
        );
    }
    fn remove_hosting_version(&mut self, id: String, version: String, cx: &mut Context<Self>) {
        if self.control_ui.hosting.busy {
            return;
        }
        self.run_hosting_request(
            "DELETE",
            format!("/v1/hosting/sites/{id}/versions/{version}"),
            None,
            None,
            cx,
        );
    }
    fn run_hosting_request(
        &mut self,
        method: &'static str,
        path: String,
        bundle: Option<(Arc<Bundle>, serde_json::Value)>,
        body: Option<serde_json::Value>,
        cx: &mut Context<Self>,
    ) {
        let ui = &mut self.control_ui.hosting;
        let Some(workspace) = ui.workspace.clone() else {
            ui.error = Some("Reopen publishing in the intended Ion account.".into());
            cx.notify();
            return;
        };
        ui.busy = true;
        ui.error = None;
        ui.generation += 1;
        let generation = ui.generation;
        let open_view = path.ends_with("/view");
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let body = if let Some((bundle, mut value)) = bundle {
                        value["files"] = serde_json::Value::Array(bundle.files.clone());
                        Some(value)
                    } else {
                        body
                    };
                    let response = services::hosting_request(&workspace, method, &path, body)?;
                    let sites =
                        services::hosting_request(&workspace, "GET", "/v1/hosting/sites", None)?;
                    anyhow::Ok((response, sites))
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                let ui = &mut this.control_ui.hosting;
                if ui.generation != generation {
                    return;
                }
                ui.busy = false;
                match result {
                    Ok((response, sites)) => {
                        ui.sites = sites["items"].as_array().cloned().unwrap_or_default();
                        if let Some(url) = response["url"].as_str().filter(|_| open_view) {
                            cx.open_url(url);
                        }
                    }
                    Err(error) => ui.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    /// What is being deployed, how it will be served, and the one Deploy action.
    fn hosting_deploy_card(&self, artifact: &Artifact, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::workspace(cx);
        let ui = &self.control_ui.hosting;
        let facts = match &ui.bundle {
            Some(bundle) => format!(
                "{} · {} {}",
                super::super::skills_page::format_bytes(bundle.bytes),
                bundle.paths.len(),
                if bundle.paths.len() == 1 {
                    "file"
                } else {
                    "files"
                }
            ),
            None => artifact
                .evidence
                .as_ref()
                .map(|e| super::super::skills_page::format_bytes(e.bytes))
                .unwrap_or_default(),
        };
        let header = div()
            .px(px(16.0))
            .py(px(14.0))
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(avatar_tile(40.0, theme.inset, theme.border).child(
                if artifact.format.eq_ignore_ascii_case("html") {
                    crate::ui::file_icon("icons/file-types/html.svg", 20.0).into_any_element()
                } else {
                    icon("icons/file.svg", 18.0, theme.text_secondary).into_any_element()
                },
            ))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .truncate()
                            .text_size(sp(15.0))
                            .line_height(sp(20.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(artifact.name.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .text_size(sp(12.5))
                            .text_color(theme.text_secondary)
                            .child(
                                div()
                                    .h(px(20.0))
                                    .px(px(7.0))
                                    .rounded(px(6.0))
                                    .bg(theme.accent_soft)
                                    .flex()
                                    .items_center()
                                    .font_family(crate::md::render::MONO_FAMILY)
                                    .text_size(sp(10.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.accent)
                                    .child(artifact.format.to_uppercase()),
                            )
                            .child(facts),
                    ),
            )
            .when(ui.bundle.is_some(), |header| {
                header.child(status_pill(
                    "icons/shield-check.svg",
                    "Verified snapshot",
                    theme.accent,
                    theme.accent_soft,
                ))
            });
        let body = if let Some(bundle) = &ui.bundle {
            let mut modes = segmented_track(&theme);
            for runtime in ["document", "static"] {
                modes = modes.child(
                    segment(
                        SharedString::from(format!("hosting-mode-{runtime}")),
                        runtime_copy(runtime).0,
                        ui.runtime == runtime,
                        &theme,
                    )
                    .on_activation(cx, move |this, _, cx| {
                        if !this.control_ui.hosting.busy {
                            this.control_ui.hosting.runtime = runtime;
                            cx.notify();
                        }
                    }),
                );
            }
            let mut form = div()
                .p(px(16.0))
                .border_t_1()
                .border_color(theme.border)
                .flex()
                .flex_col()
                .gap(px(16.0))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .child(div().text_size(sp(12.5)).child("Publish as"))
                        // A row parent keeps the track at its content width.
                        .child(div().flex().child(modes))
                        .child(hosting_hint(runtime_copy(ui.runtime).1, &theme)),
                )
                .child(control_field(
                    "Entry file",
                    ui.entry.clone(),
                    "hosting-entry",
                ));
            if bundle.export_directory.is_some() {
                form = form.child(control_field(
                    "Output folder",
                    ui.output.clone(),
                    "hosting-output",
                ));
            }
            if bundle.paths.len() > 1 {
                let snapshot = bundle.clone();
                let visible = bundle.paths.len().min(6) as f32;
                form = form.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_size(sp(12.5))
                                .child(format!("Files ({})", bundle.paths.len())),
                        )
                        .child(
                            div()
                                .h(px(visible * 24.0 + 14.0))
                                .px(px(12.0))
                                .py(px(6.0))
                                .rounded(px(8.0))
                                .bg(theme.inset)
                                .border_1()
                                .border_color(theme.border)
                                .child(
                                    list(ui.paths.clone(), move |index, _, cx| {
                                        let theme = Theme::workspace(cx);
                                        div()
                                            .h(px(24.0))
                                            .flex()
                                            .items_center()
                                            .gap(px(8.0))
                                            .child(icon(
                                                "icons/file.svg",
                                                12.0,
                                                theme.text_tertiary,
                                            ))
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .truncate()
                                                    .text_size(sp(12.5))
                                                    .text_color(theme.text_secondary)
                                                    .child(snapshot.paths[index].clone()),
                                            )
                                            .into_any_element()
                                    })
                                    .size_full(),
                                ),
                        ),
                );
            }
            let mut toggles = div().flex().flex_col().gap(px(12.0)).child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(toggle_row(
                        "hosting-visibility",
                        "Public link",
                        ui.public,
                        theme,
                        cx,
                        |this, _, cx| {
                            if !this.control_ui.hosting.busy {
                                this.control_ui.hosting.public = !this.control_ui.hosting.public;
                                cx.notify();
                            }
                        },
                    ))
                    // Indented past the 38px switch and its 10px gap.
                    .child(
                        hosting_hint(
                            "Anyone with the link can open it. Off keeps it private to your Ion account.",
                            &theme,
                        )
                        .pl(px(48.0)),
                    ),
            );
            if ui.runtime == "static" {
                toggles = toggles.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(toggle_row(
                            "hosting-spa",
                            "Single-page app routing",
                            ui.spa,
                            theme,
                            cx,
                            |this, _, cx| {
                                if !this.control_ui.hosting.busy {
                                    this.control_ui.hosting.spa = !this.control_ui.hosting.spa;
                                    cx.notify();
                                }
                            },
                        ))
                        .child(
                            hosting_hint(
                                "Unknown paths load the entry file, for client-side routers.",
                                &theme,
                            )
                            .pl(px(48.0)),
                        ),
                );
            }
            Some(form.child(toggles).into_any_element())
        } else if ui.busy {
            Some(
                div()
                    .p(px(16.0))
                    .border_t_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(crate::ui::motion::spin(icon(
                        "icons/loader-circle.svg",
                        14.0,
                        theme.text_secondary,
                    )))
                    .child("Preparing a verified snapshot…")
                    .into_any_element(),
            )
        } else {
            None
        };
        let error = ui.error.as_deref().map(|error| {
            div()
                .px(px(16.0))
                .pb(px(16.0))
                .child(hosting_error(error, &theme))
        });
        let deploy = if ui.busy && ui.bundle.is_some() {
            // The label is the button's first child; reversing the row leads with the spinner.
            self.control_button_tone(
                "hosting-publish",
                "Working…",
                ControlTone::Primary,
                cx,
                |this, _, cx| this.publish_hosting(cx),
            )
            .flex_row_reverse()
            .child(crate::ui::motion::spin(icon(
                "icons/loader-circle.svg",
                14.0,
                theme.on_primary,
            )))
        } else {
            self.control_button_icon(
                "hosting-publish",
                "icons/cloud-upload.svg",
                "Deploy",
                ControlTone::Primary,
                cx,
                |this, _, cx| this.publish_hosting(cx),
            )
        };
        let footer = div()
            .px(px(16.0))
            .py(px(12.0))
            .border_t_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .whitespace_normal()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(
                        "Uploads this exact verified snapshot. Deploying again keeps the same link.",
                    ),
            )
            .child(deploy.when(ui.bundle.is_none(), |button| button.opacity(0.55)));
        hosting_card(&theme)
            .child(header)
            .children(body)
            .children(error)
            .child(footer)
            .into_any_element()
    }
    /// One published site: status, its link, and the actions that change it.
    fn hosting_site_card(&self, site: &serde_json::Value, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::workspace(cx);
        let id = site["id"].as_str().unwrap_or_default().to_owned();
        let view = id.clone();
        let stop = id.clone();
        let visibility = id.clone();
        let url = site["url"].as_str().unwrap_or_default().to_owned();
        let copy_url = url.clone();
        let public = site["visibility"] == "public";
        let stopped = site["stopped"] == true;
        let releases = site["releases"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        // Releases arrive newest first.
        let deploying = releases.first().is_some_and(|r| {
            matches!(
                r["status"].as_str(),
                Some("uploading" | "queued" | "building")
            )
        });
        let state = if stopped {
            status_pill(
                "icons/stop.svg",
                "Unpublished",
                theme.text_secondary,
                theme.overlay,
            )
        } else if deploying {
            status_pill(
                "icons/loader-circle.svg",
                "Deploying",
                theme.accent,
                theme.accent_soft,
            )
        } else if site["active"].is_null() {
            status_pill(
                "icons/hourglass.svg",
                "Not live yet",
                theme.warning,
                theme.overlay,
            )
        } else {
            status_pill(
                "icons/check.svg",
                "Live",
                theme.success,
                theme.success.opacity(0.12),
            )
        };
        let mut facts = Vec::new();
        if let Some(at) = releases
            .iter()
            .filter_map(|r| r["created"].as_u64())
            .max()
            .or_else(|| site["created"].as_u64())
        {
            facts.push(format!("Updated {}", hosting_ago(at)));
        }
        if !releases.is_empty() {
            facts.push(format!(
                "{} {}",
                releases.len(),
                if releases.len() == 1 {
                    "version"
                } else {
                    "versions"
                }
            ));
        }
        let header = div()
            .px(px(16.0))
            .pt(px(14.0))
            .pb(px(12.0))
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(
                avatar_tile(36.0, theme.accent_soft, gpui::transparent_black()).child(icon(
                    "icons/globe.svg",
                    16.0,
                    theme.accent,
                )),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .min_w_0()
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(sp(14.0))
                                    .line_height(sp(20.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(site["title"].as_str().unwrap_or("Deployment").to_owned()),
                            )
                            .child(state)
                            .child(status_pill(
                                if public {
                                    "icons/globe.svg"
                                } else {
                                    "icons/lock.svg"
                                },
                                if public { "Public" } else { "Private" },
                                theme.text_secondary,
                                theme.overlay,
                            )),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(sp(12.0))
                            .text_color(theme.text_tertiary)
                            .child(facts.join(" · ")),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .child(self.control_button_icon(
                        format!("hosting-access-{id}"),
                        if public {
                            "icons/lock.svg"
                        } else {
                            "icons/globe.svg"
                        },
                        if public { "Make private" } else { "Make public" },
                        ControlTone::Ghost,
                        cx,
                        move |this, _, cx| {
                            this.hosting_action(
                                visibility.clone(),
                                "visibility",
                                serde_json::json!({"visibility":if public{"private"}else{"public"}}),
                                cx,
                            )
                        },
                    ))
                    .child(self.control_button_icon(
                        format!("hosting-stop-{id}"),
                        if stopped {
                            "icons/play.svg"
                        } else {
                            "icons/stop.svg"
                        },
                        if stopped { "Republish" } else { "Unpublish" },
                        ControlTone::Ghost,
                        cx,
                        move |this, _, cx| {
                            this.hosting_action(
                                stop.clone(),
                                if stopped { "start" } else { "stop" },
                                serde_json::json!({}),
                                cx,
                            )
                        },
                    )),
            );
        let link = div()
            .px(px(16.0))
            .pb(px(14.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h(px(36.0))
                    .pl(px(12.0))
                    .pr(px(4.0))
                    .rounded(px(8.0))
                    .bg(theme.inset)
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(icon(
                        if public {
                            "icons/globe.svg"
                        } else {
                            "icons/lock.svg"
                        },
                        12.5,
                        theme.text_tertiary,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_family(crate::md::render::MONO_FAMILY)
                            .text_size(sp(12.5))
                            .text_color(theme.text)
                            .child(
                                url.trim_start_matches("https://")
                                    .trim_end_matches('/')
                                    .to_owned(),
                            ),
                    )
                    .child(
                        self.control_button_tone(
                            format!("hosting-copy-{id}"),
                            "",
                            ControlTone::Ghost,
                            cx,
                            move |this, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(copy_url.clone()));
                                this.show_success_toast("Link copied");
                                cx.notify();
                            },
                        )
                        .size(px(28.0))
                        .px_0()
                        // The empty label is still a layout node; no gap keeps the icon centered.
                        .gap(px(0.0))
                        .justify_center()
                        .tooltip(Tooltip::text("Copy link"))
                        .child(icon(
                            "icons/copy.svg",
                            13.0,
                            theme.text_secondary,
                        )),
                    ),
            )
            .child(self.control_button_icon(
                format!("hosting-view-{id}"),
                "icons/external-link.svg",
                "Open",
                ControlTone::Secondary,
                cx,
                move |this, _, cx| {
                    this.hosting_action(view.clone(), "view", serde_json::json!({}), cx)
                },
            ));
        let mut versions = None;
        if !releases.is_empty() {
            let mut rows = div()
                .border_t_1()
                .border_color(theme.border)
                .px(px(16.0))
                .pt(px(10.0))
                .pb(px(6.0))
                .flex()
                .flex_col()
                .child(
                    div()
                        .pb(px(2.0))
                        .text_size(sp(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_tertiary)
                        .child("Versions"),
                );
            for release in releases {
                rows = rows.child(self.hosting_release_row(site, release, cx));
            }
            versions = Some(rows);
        }
        hosting_card(&theme)
            .child(header)
            .child(link)
            .children(versions)
            .into_any_element()
    }
    fn hosting_release_row(
        &self,
        site: &serde_json::Value,
        release: &serde_json::Value,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = Theme::workspace(cx);
        let id = site["id"].as_str().unwrap_or_default().to_owned();
        let rid = release["id"].as_str().unwrap_or_default().to_owned();
        let status = release["status"].as_str().unwrap_or("");
        let (glyph, label, tone) = release_status(status, &theme);
        let current = !site["active"].is_null() && site["active"] == release["id"];
        let removable =
            site["active"] != release["id"] && ["ready", "failed", "uncertain"].contains(&status);
        let restorable = status == "ready" && site["active"] != release["id"];
        let sid = id.clone();
        let target = rid.clone();
        let remove_site = id;
        let remove_version = rid.clone();
        let logs = release["logs"].as_str().unwrap_or("").trim().to_owned();
        let row = div()
            .min_h(px(36.0))
            .flex()
            .items_center()
            .gap(px(10.0))
            .child(icon(glyph, 13.0, tone))
            .child(
                div()
                    .flex_none()
                    .font_family(crate::md::render::MONO_FAMILY)
                    .text_size(sp(11.5))
                    .text_color(theme.text_tertiary)
                    .child(format!("#{}", &rid[..rid.len().min(8)])),
            )
            .child(div().flex_none().text_size(sp(12.5)).child(label))
            .when(current, |row| {
                row.child(
                    div()
                        .flex_none()
                        .h(px(20.0))
                        .px(px(7.0))
                        .rounded_full()
                        .bg(theme.accent_soft)
                        .flex()
                        .items_center()
                        .text_size(sp(11.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.accent)
                        .child("Current"),
                )
            })
            .when_some(release["created"].as_u64(), |row, at| {
                row.child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(sp(12.0))
                        .text_color(theme.text_tertiary)
                        .child(hosting_ago(at)),
                )
            })
            .child(div().flex_1())
            .when(restorable, |row| {
                row.child(
                    self.control_button_icon(
                        format!("hosting-rollback-{rid}"),
                        "icons/rewind.svg",
                        "Roll back",
                        ControlTone::Ghost,
                        cx,
                        move |this, _, cx| {
                            this.hosting_action(
                                sid.clone(),
                                "rollback",
                                serde_json::json!({"releaseId":target}),
                                cx,
                            )
                        },
                    )
                    .h(px(28.0))
                    .px(px(10.0))
                    .text_size(sp(12.5))
                    .tooltip(Tooltip::text("Make this the live version")),
                )
            })
            .when(removable, |row| {
                row.child(
                    self.control_button_tone(
                        format!("hosting-clean-{rid}"),
                        "",
                        ControlTone::Ghost,
                        cx,
                        move |this, _, cx| {
                            this.remove_hosting_version(
                                remove_site.clone(),
                                remove_version.clone(),
                                cx,
                            )
                        },
                    )
                    .size(px(28.0))
                    .px_0()
                    .gap(px(0.0))
                    .justify_center()
                    .tooltip(Tooltip::text("Remove this version"))
                    .child(icon("icons/trash.svg", 13.0, theme.text_secondary)),
                )
            });
        div()
            .flex()
            .flex_col()
            .child(row)
            .when(!logs.is_empty(), |release| {
                // Indented under the label, past the 13px glyph and its gap.
                release.child(
                    div()
                        .pl(px(23.0))
                        .pb(px(8.0))
                        .whitespace_normal()
                        .text_size(sp(12.0))
                        .line_height(sp(17.0))
                        .text_color(theme.text_secondary)
                        .child(logs),
                )
            })
    }
    pub(super) fn render_hosting(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::workspace(cx);
        let ui = &self.control_ui.hosting;
        let deploying_artifact = matches!(&ui.source, Some(Source::Artifact(_)));
        // Same shape as the workspace page header so the view reads as part of the set.
        let header = div()
            .w_full()
            .flex_none()
            .pb(px(16.0))
            .border_b_1()
            .border_color(theme.border)
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(20.0))
            .child(
                div()
                    .flex_1()
                    .min_w(px(140.0))
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .size(px(32.0))
                            .flex_none()
                            .rounded(px(9.0))
                            .bg(theme.accent_soft)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon("icons/cloud-upload.svg", 16.0, theme.accent)),
                    )
                    .child(
                        div()
                            .text_size(sp(22.0))
                            .line_height(sp(28.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(if deploying_artifact {
                                "Deploy artifact"
                            } else {
                                "Deployments"
                            }),
                    ),
            )
            .child(self.control_button_icon(
                "hosting-back",
                "icons/arrow-left.svg",
                "Back",
                ControlTone::Secondary,
                cx,
                |this, _, cx| {
                    let ui = &mut this.control_ui.hosting;
                    ui.open = false;
                    ui.generation += 1;
                    ui.busy = false;
                    cx.notify();
                },
            ));
        let mut content = div()
            .w_full()
            .max_w(px(880.0))
            .mx_auto()
            .pb(px(24.0))
            .flex()
            .flex_col()
            .gap(px(24.0))
            .text_color(theme.text)
            .child(header);
        if let Some(Source::Artifact(artifact)) = &ui.source {
            content = content.child(self.hosting_deploy_card(artifact, cx));
        } else if let Some(error) = &ui.error {
            content = content.child(hosting_error(error, &theme));
        }
        let source = ui.source_id();
        let sites: Vec<&serde_json::Value> = ui
            .sites
            .iter()
            .filter(|s| {
                source
                    .as_ref()
                    .is_none_or(|id| s["sourceId"].as_str() == Some(id))
            })
            .collect();
        let refresh = self
            .control_button_tone(
                "hosting-refresh",
                "",
                ControlTone::Ghost,
                cx,
                |this, _, cx| {
                    if !this.control_ui.hosting.busy {
                        this.run_hosting_request("GET", "/v1/hosting/sites".into(), None, None, cx);
                    }
                },
            )
            .size(px(32.0))
            .px_0()
            .gap(px(0.0))
            .justify_center()
            .tooltip(Tooltip::text(if ui.busy {
                "Refreshing"
            } else {
                "Refresh deployments"
            }))
            .child(if ui.busy {
                crate::ui::motion::spin(icon("icons/rotate-cw.svg", 15.0, theme.text_secondary))
            } else {
                icon("icons/rotate-cw.svg", 15.0, theme.text_secondary).into_any_element()
            });
        let mut section = div().flex().flex_col().gap(px(12.0)).child(
            div()
                .min_h(px(32.0))
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(sp(15.0))
                        .line_height(sp(20.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(if deploying_artifact {
                            "Deployments"
                        } else {
                            "Published sites"
                        }),
                )
                .when(!sites.is_empty(), |head| {
                    head.child(
                        div()
                            .h(px(20.0))
                            .px(px(7.0))
                            .rounded_full()
                            .bg(theme.overlay_strong)
                            .flex()
                            .items_center()
                            .text_size(sp(11.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text_secondary)
                            .child(sites.len().to_string()),
                    )
                })
                .child(div().flex_1())
                .child(refresh),
        );
        if sites.is_empty() && ui.busy {
            section = section.child(
                div()
                    .h(px(72.0))
                    .rounded(px(14.0))
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(8.0))
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(crate::ui::motion::spin(icon(
                        "icons/loader-circle.svg",
                        14.0,
                        theme.text_secondary,
                    )))
                    .child("Checking Ion Cloud…"),
            );
        } else if sites.is_empty() {
            let failed = ui.error.is_some();
            section = section.child(
                div()
                    .py(px(32.0))
                    .px(px(24.0))
                    .rounded(px(14.0))
                    .border_1()
                    .border_dashed()
                    .border_color(theme.border_strong)
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        avatar_tile(40.0, theme.inset, gpui::transparent_black()).child(icon(
                            if failed {
                                "icons/alert.svg"
                            } else {
                                "icons/globe.svg"
                            },
                            18.0,
                            theme.text_tertiary,
                        )),
                    )
                    .child(
                        div()
                            .mt(px(4.0))
                            .text_size(sp(14.0))
                            .font_weight(FontWeight::MEDIUM)
                            .child(if failed {
                                "Deployments unavailable"
                            } else {
                                "No deployments yet"
                            }),
                    )
                    .child(
                        div()
                            .max_w(px(420.0))
                            .text_center()
                            .whitespace_normal()
                            .text_size(sp(12.5))
                            .text_color(theme.text_secondary)
                            .child(if failed {
                                "Check the message above, then refresh."
                            } else if deploying_artifact {
                                "Deploy this artifact to get a link you can share."
                            } else {
                                "Deploy an HTML artifact from its card in Artifacts to see it here."
                            }),
                    ),
            );
        } else {
            for site in sites {
                section = section.child(self.hosting_site_card(site, cx));
            }
        }
        div()
            .id("cloud-hosting")
            .track_focus(&ui.focus)
            .tab_group()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(28.0))
            .py(px(16.0))
            .child(content.child(section))
            .into_any_element()
    }
}

/// Readable name and consequence of each publish runtime id.
fn runtime_copy(runtime: &str) -> (&'static str, &'static str) {
    match runtime {
        "document" => (
            "Document",
            "Scripts are turned off. Best for reports and read-only pages.",
        ),
        _ => (
            "Interactive site",
            "Scripts run on an isolated website origin, for interactive HTML.",
        ),
    }
}

/// A release status as glyph, readable label and tone.
fn release_status(status: &str, theme: &Theme) -> (&'static str, &'static str, Hsla) {
    match status {
        "ready" => ("icons/check.svg", "Ready", theme.success),
        "failed" => ("icons/alert.svg", "Failed", theme.danger),
        "uncertain" => ("icons/alert.svg", "Interrupted", theme.warning),
        "uploading" => ("icons/cloud-upload.svg", "Uploading", theme.accent),
        "queued" => ("icons/hourglass.svg", "Queued", theme.text_secondary),
        "building" => ("icons/loader-circle.svg", "Building", theme.accent),
        _ => ("icons/info.svg", "Unknown", theme.text_tertiary),
    }
}

/// Hosting timestamps are Unix milliseconds.
fn hosting_ago(millis: u64) -> String {
    let seconds = unix_time().saturating_sub(millis / 1000);
    let ago = super::super::sidebar::format_time_ago(seconds);
    if seconds < 60 {
        ago
    } else {
        format!("{ago} ago")
    }
}

/// A failed request, wrapped inside a soft danger panel so it never overflows.
fn hosting_error(error: &str, theme: &Theme) -> Div {
    div()
        .w_full()
        .overflow_hidden()
        .px(px(12.0))
        .py(px(10.0))
        .rounded(px(10.0))
        .bg(theme.danger_soft)
        .flex()
        .items_start()
        .gap(px(8.0))
        .text_size(sp(12.5))
        .line_height(sp(18.0))
        .text_color(theme.danger)
        .child(div().h(sp(18.0)).flex().items_center().child(icon(
            "icons/alert.svg",
            13.0,
            theme.danger,
        )))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .whitespace_normal()
                .child(error.to_owned()),
        )
}

/// Card stock shared by the deploy card and each site card.
fn hosting_card(theme: &Theme) -> Div {
    div()
        .w_full()
        .flex()
        .flex_col()
        .rounded(px(14.0))
        .border_1()
        .border_color(theme.ticket_border)
        .bg(theme.ticket)
        .shadow(elevation(theme, Elevation::Rest))
}

/// A quiet explanatory line under a form control.
fn hosting_hint(text: &'static str, theme: &Theme) -> Div {
    div()
        .whitespace_normal()
        .text_size(sp(12.0))
        .line_height(sp(17.0))
        .text_color(theme.text_secondary)
        .child(text)
}

impl ControlUi {
    pub(in crate::app) fn clear_hosting_account(&mut self, _cx: &mut App) {
        self.hosting.generation += 1;
        self.hosting.busy = false;
        self.hosting.sites.clear();
        self.hosting.workspace = None;
        self.hosting.bundle = None;
        self.hosting.source = None;
        self.hosting.open = false;
    }
}
