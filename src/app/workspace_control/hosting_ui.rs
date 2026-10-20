//! Account hosting controls. Snapshot preparation and HTTP run on workers.
use super::*;
use crate::credentials::cloud::{
    bundle::{self, Bundle},
    services,
};
use std::sync::Arc;
#[derive(Clone)]
enum Source {
    Project(Project),
    Artifact(Artifact),
}
pub(super) struct HostingUi {
    pub open: bool,
    busy: bool,
    generation: u64,
    source: Option<Source>,
    bundle: Option<Arc<Bundle>>,
    sites: Vec<serde_json::Value>,
    error: Option<String>,
    runtime: &'static str,
    public: bool,
    spa: bool,
    operation: Uuid,
    entry: Entity<TextInput>,
    build: Entity<TextInput>,
    start: Entity<TextInput>,
    output: Entity<TextInput>,
    port: Entity<TextInput>,
    health: Entity<TextInput>,
    environment: Entity<TextInput>,
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
            bundle: None,
            sites: vec![],
            error: None,
            runtime: "static",
            public: false,
            spa: false,
            operation: Uuid::new_v4(),
            entry: cx.new(|cx| TextInput::new(window, cx).placeholder("index.html")),
            build: cx
                .new(|cx| TextInput::new(window, cx).placeholder(r#"["npm","run","build"] or []"#)),
            start: cx.new(|cx| TextInput::new(window, cx).placeholder(r#"["node","server.js"]"#)),
            output: cx.new(|cx| TextInput::new(window, cx).placeholder("dist")),
            port: cx.new(|cx| TextInput::new(window, cx).placeholder("3000")),
            health: cx.new(|cx| TextInput::new(window, cx).placeholder("/")),
            environment: cx.new(|cx| {
                TextInput::new(window, cx)
                    .placeholder(r#"{"SERVICE_KEY":"value"}"#)
                    .secure()
            }),
            paths: ListState::new(0, ListAlignment::Top, px(22.0)),
            focus: cx.focus_handle(),
        }
    }
    fn source_id(&self) -> Option<String> {
        self.source.as_ref().map(|s| match s {
            Source::Project(p) => format!("project-{}", p.id),
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
    pub(super) fn open_project_hosting(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let Some(project) = self.state.projects.iter().find(|p| p.id == id).cloned() else {
            return;
        };
        self.prepare_hosting(Some(Source::Project(project)), cx);
    }
    pub(super) fn open_artifact_hosting(&mut self, artifact: Artifact, cx: &mut Context<Self>) {
        self.prepare_hosting(Some(Source::Artifact(artifact)), cx);
    }
    pub(in crate::app) fn open_hosting_library(&mut self, cx: &mut Context<Self>) {
        self.control_ui.page = Some(Page::Projects);
        self.prepare_hosting(None, cx);
    }
    fn prepare_hosting(&mut self, source: Option<Source>, cx: &mut Context<Self>) {
        if self.control_ui.hosting.busy {
            return;
        }
        let ui = &mut self.control_ui.hosting;
        ui.open = true;
        ui.busy = true;
        ui.generation += 1;
        ui.source = source.clone();
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
        for input in [&ui.build, &ui.start, &ui.environment] {
            input.update(cx, |input, cx| input.set_content("{}", cx));
        }
        ui.build.update(cx, |input, cx| input.set_content("[]", cx));
        ui.start.update(cx, |input, cx| input.set_content("[]", cx));
        ui.output
            .update(cx, |input, cx| input.set_content("dist", cx));
        ui.port
            .update(cx, |input, cx| input.set_content("3000", cx));
        ui.health.update(cx, |input, cx| input.set_content("/", cx));
        let generation = ui.generation;
        let client = self.daemon.client();
        cx.spawn(async move |view, cx| {
            let (bundle, sites) = cx
                .background_executor()
                .spawn(async move {
                    let bundle = match source {
                        Some(Source::Project(p)) => bundle::project(&p.path).map(Some),
                        Some(Source::Artifact(a)) => artifact_bundle(client, a).map(Some),
                        None => Ok(None),
                    };
                    let sites = services::request("GET", "/v1/hosting/sites", None);
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
            Source::Project(p) => (p.name, "project"),
            Source::Artifact(a) => (a.name, "artifact"),
        };
        let build = serde_json::from_str::<Vec<String>>(ui.build.read(cx).content());
        let start = serde_json::from_str::<Vec<String>>(ui.start.read(cx).content());
        let environment = serde_json::from_str::<std::collections::HashMap<String, String>>(
            ui.environment.read(cx).content(),
        );
        let (Ok(build), Ok(start), Ok(environment)) = (build, start, environment) else {
            self.control_ui.hosting.error = Some(
                "Commands must be JSON arrays; runtime settings must be a JSON object.".into(),
            );
            cx.notify();
            return;
        };
        let settings = serde_json::json!({"runtime":ui.runtime,"entry":ui.entry.read(cx).content(),"build":build,"start":start,"outputDirectory":ui.output.read(cx).content(),"port":ui.port.read(cx).content().parse::<u16>().unwrap_or(0),"healthPath":ui.health.read(cx).content(),"spa":ui.spa});
        let visibility = if ui.public { "public" } else { "private" };
        let operation = ui.operation.to_string();
        self.control_ui
            .hosting
            .environment
            .update(cx, |input, cx| input.clear(cx));
        self.run_hosting_request("POST","/v1/hosting/releases".into(),Some((bundle,serde_json::json!({"sourceId":source_id,"title":title,"kind":kind,"visibility":visibility,"idempotencyKey":operation,"settings":settings,"environment":environment}))),None,cx);
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
                    let response = services::request(method, &path, body)?;
                    let sites = services::request("GET", "/v1/hosting/sites", None)?;
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
    fn hosting_field(&self, label: &'static str, input: Entity<TextInput>, cx: &App) -> AnyElement {
        let theme = Theme::workspace(cx);
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(label),
            )
            .child(
                div()
                    .w_full()
                    .min_h(px(36.0))
                    .px(px(10.0))
                    .py(px(8.0))
                    .rounded(px(7.0))
                    .bg(theme.raised)
                    .border_1()
                    .border_color(theme.border_strong)
                    .child(input),
            )
            .into_any_element()
    }
    pub(super) fn render_hosting(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::workspace(cx);
        let ui = &self.control_ui.hosting;
        let mut content = div()
            .flex()
            .flex_col()
            .gap(px(18.0))
            .w_full()
            .max_w(px(1080.0))
            .mx_auto()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(sp(20.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Deployments"),
                    )
                    .child(
                        self.control_button("hosting-back", "Back", cx, |this, _, cx| {
                            let ui = &mut this.control_ui.hosting;
                            ui.open = false;
                            ui.generation += 1;
                            ui.busy = false;
                            ui.environment.update(cx, |input, cx| input.clear(cx));
                            cx.notify();
                        }),
                    ),
            );
        if let Some(bundle) = &ui.bundle {
            content = content.child(
                div()
                    .text_size(sp(13.0))
                    .text_color(theme.text_secondary)
                    .child(format!(
                        "Upload this reviewed snapshot: {} files · {} bytes. Private by default.",
                        bundle.paths.len(),
                        bundle.bytes
                    )),
            );
            let mut modes = div().flex().flex_wrap().gap(px(8.0));
            for runtime in ["document", "static", "node", "bun", "python"] {
                if matches!(&ui.source, Some(Source::Artifact(_)))
                    && runtime != "document"
                    && runtime != "static"
                {
                    continue;
                }
                if matches!(&ui.source, Some(Source::Project(_))) && runtime == "document" {
                    continue;
                }
                modes = modes.child(self.control_button(
                    format!("hosting-mode-{runtime}"),
                    if ui.runtime == runtime {
                        format!("{runtime} · selected")
                    } else {
                        runtime.into()
                    },
                    cx,
                    move |this, _, cx| {
                        if !this.control_ui.hosting.busy {
                            this.control_ui.hosting.runtime = runtime;
                            cx.notify();
                        }
                    },
                ));
            }
            content =
                content
                    .child(modes)
                    .child(self.hosting_field("Entry file", ui.entry.clone(), cx));
            if matches!(&ui.source, Some(Source::Artifact(_))) {
                content=content.child(div().text_size(sp(12.5)).text_color(theme.text_secondary).child("Document disables scripts. Static allows interactive HTML on an isolated website origin."));
            }
            if ui.runtime == "static" {
                content = content.child(self.control_button(
                    "hosting-spa",
                    if ui.spa {
                        "SPA routing · on"
                    } else {
                        "SPA routing · off"
                    },
                    cx,
                    |this, _, cx| {
                        if !this.control_ui.hosting.busy {
                            this.control_ui.hosting.spa = !this.control_ui.hosting.spa;
                            cx.notify();
                        }
                    },
                ));
            }
            if ui.runtime != "document" && matches!(&ui.source, Some(Source::Project(_))) {
                content = content
                    .child(self.hosting_field(
                        "Build command · JSON argument array · optional",
                        ui.build.clone(),
                        cx,
                    ))
                    .child(self.hosting_field("Build output directory", ui.output.clone(), cx));
                if ui.runtime != "static" {
                    content = content
                        .child(self.hosting_field(
                            "Start command · JSON argument array",
                            ui.start.clone(),
                            cx,
                        ))
                        .child(self.hosting_field("HTTP port", ui.port.clone(), cx))
                        .child(self.hosting_field("Readiness path", ui.health.clone(), cx))
                        .child(self.hosting_field(
                            "Runtime secrets · JSON object · encrypted in cloud storage",
                            ui.environment.clone(),
                            cx,
                        ));
                }
            }
            content = content.child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.0))
                    .child(self.control_button(
                        "hosting-visibility",
                        if ui.public {
                            "Public · anyone with the link"
                        } else {
                            "Private · your account"
                        },
                        cx,
                        |this, _, cx| {
                            if !this.control_ui.hosting.busy {
                                this.control_ui.hosting.public = !this.control_ui.hosting.public;
                                cx.notify();
                            }
                        },
                    ))
                    .child(self.control_button(
                        "hosting-publish",
                        if ui.busy {
                            "Working…"
                        } else {
                            "Publish reviewed snapshot"
                        },
                        cx,
                        |this, _, cx| this.publish_hosting(cx),
                    )),
            );
            let snapshot = bundle.clone();
            let paths = div().h(px(160.0)).child(
                list(ui.paths.clone(), move |index, _, cx| {
                    div()
                        .h(px(22.0))
                        .text_size(sp(12.0))
                        .text_color(Theme::workspace(cx).text_secondary)
                        .child(snapshot.paths[index].clone())
                        .into_any_element()
                })
                .size_full(),
            );
            content = content.child(paths);
        }
        content = content.child(self.control_button(
            "hosting-refresh",
            if ui.busy {
                "Working…"
            } else {
                "Refresh deployments"
            },
            cx,
            |this, _, cx| {
                if !this.control_ui.hosting.busy {
                    this.run_hosting_request("GET", "/v1/hosting/sites".into(), None, None, cx);
                }
            },
        ));
        if let Some(error) = &ui.error {
            content = content.child(
                div()
                    .text_size(sp(13.0))
                    .text_color(theme.text)
                    .child(error.clone()),
            );
        }
        let source = ui.source_id();
        for site in ui.sites.iter().filter(|s| {
            source
                .as_ref()
                .is_none_or(|id| s["sourceId"].as_str() == Some(id))
        }) {
            let id = site["id"].as_str().unwrap_or_default().to_owned();
            let view = id.clone();
            let stop = id.clone();
            let visibility = id.clone();
            let logs = id.clone();
            let copy_url = site["url"].as_str().unwrap_or_default().to_owned();
            let public = site["visibility"] == "public";
            let stopped = site["stopped"] == true;
            let mut row=div().border_t_1().border_color(theme.border).pt(px(18.0)).flex().flex_col().gap(px(10.0))
                .child(div().text_size(sp(15.0)).font_weight(FontWeight::MEDIUM).child(site["title"].as_str().unwrap_or("Deployment").to_owned()))
                .child(div().text_size(sp(12.5)).text_color(theme.text_secondary).child(format!("{} · {}",if stopped{"Stopped"}else if site["active"].is_null(){"Awaiting first version"}else{"Active"},if public{"Public"}else{"Private"})))
                .child(div().text_size(sp(12.5)).text_color(theme.text_secondary).child(site["url"].as_str().unwrap_or_default().to_owned()))
                .child(div().flex().flex_wrap().gap(px(8.0))
                    .child(self.control_button(format!("hosting-view-{id}"),"Open",cx,move|this,_,cx|this.hosting_action(view.clone(),"view",serde_json::json!({}),cx)))
                    .child(self.control_button(format!("hosting-stop-{id}"),if stopped{"Start"}else{"Stop / Unpublish"},cx,move|this,_,cx|this.hosting_action(stop.clone(),if stopped{"start"}else{"stop"},serde_json::json!({}),cx)))
                    .child(self.control_button(format!("hosting-access-{id}"),if public{"Make private"}else{"Make public"},cx,move|this,_,cx|this.hosting_action(visibility.clone(),"visibility",serde_json::json!({"visibility":if public{"private"}else{"public"}}),cx))));
            row = row.child(self.control_button(
                format!("hosting-logs-{id}"),
                "Runtime logs",
                cx,
                move |this, _, cx| {
                    this.hosting_action(logs.clone(), "logs", serde_json::json!({}), cx)
                },
            ));
            row = row.child(self.control_button(
                format!("hosting-copy-{id}"),
                "Copy link",
                cx,
                move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(copy_url.clone())),
            ));
            if let Some(releases) = site["releases"].as_array() {
                for release in releases {
                    let rid = release["id"].as_str().unwrap_or_default().to_owned();
                    let sid = id.clone();
                    let target = rid.clone();
                    let remove_site = id.clone();
                    let remove_version = rid.clone();
                    row = row.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(5.0))
                            .child(
                                div()
                                    .text_size(sp(12.5))
                                    .text_color(theme.text_secondary)
                                    .child(format!(
                                        "{} · {} · {}",
                                        &rid[..rid.len().min(8)],
                                        release["status"].as_str().unwrap_or("Unknown"),
                                        release["logs"].as_str().unwrap_or("")
                                    )),
                            )
                            .when(
                                site["active"] != release["id"]
                                    && ["ready", "failed", "uncertain"]
                                        .contains(&release["status"].as_str().unwrap_or("")),
                                |row| {
                                    row.child(self.control_button(
                                        format!("hosting-clean-{rid}"),
                                        "Remove inactive version",
                                        cx,
                                        move |this, _, cx| {
                                            this.remove_hosting_version(
                                                remove_site.clone(),
                                                remove_version.clone(),
                                                cx,
                                            )
                                        },
                                    ))
                                },
                            )
                            .when(
                                release["status"] == "ready" && site["active"] != release["id"],
                                |row| {
                                    row.child(self.control_button(
                                        format!("hosting-rollback-{rid}"),
                                        "Roll back to this version",
                                        cx,
                                        move |this, _, cx| {
                                            this.hosting_action(
                                                sid.clone(),
                                                "rollback",
                                                serde_json::json!({"releaseId":target}),
                                                cx,
                                            )
                                        },
                                    ))
                                },
                            ),
                    );
                }
            }
            content = content.child(row);
        }
        div()
            .id("cloud-hosting")
            .track_focus(&ui.focus)
            .tab_group()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(28.0))
            .py(px(20.0))
            .child(content)
            .into_any_element()
    }
}

impl ControlUi {
    pub(in crate::app) fn clear_hosting_account(&mut self, cx: &mut App) {
        self.hosting.generation += 1;
        self.hosting.busy = false;
        self.hosting.sites.clear();
        self.hosting
            .environment
            .update(cx, |input, cx| input.clear(cx));
    }
}
