//! Owner command allocation diagnostics. All reads and formatting run on workers.
use super::*;
use waku_client::command_resources::CommandResourceUsage;
pub(super) struct ResourceUi {
    pub open: bool,
    project: Option<Uuid>,
    busy: bool,
    generation: u64,
    error: Option<String>,
    rows: Rc<Vec<(String, String, String, String)>>,
    note: Option<String>,
    focus: FocusHandle,
    scroll: gpui::ScrollHandle,
}
impl ResourceUi {
    pub fn new(cx: &mut App) -> Self {
        Self {
            open: false,
            project: None,
            busy: false,
            generation: 0,
            error: None,
            rows: Rc::new(vec![]),
            note: None,
            focus: cx.focus_handle(),
            scroll: gpui::ScrollHandle::new(),
        }
    }
    fn clear(&mut self) {
        self.generation += 1;
        self.busy = false;
        self.rows = Rc::new(vec![]);
        self.note = None;
        self.error = None;
    }
    pub(super) fn close(&mut self) {
        self.open = false;
        self.clear();
    }
}
fn rows(usage: &CommandResourceUsage) -> Vec<(String, String, String, String)> {
    let fields = [
        (
            "Concurrent commands",
            usage.used.jobs,
            usage.limit.as_ref().map(|v| v.jobs),
        ),
        (
            "CPU millis",
            usage.used.cpu_millis,
            usage.limit.as_ref().map(|v| v.cpu_millis),
        ),
        (
            "Memory bytes",
            usage.used.memory_bytes,
            usage.limit.as_ref().map(|v| v.memory_bytes),
        ),
        (
            "Processes / threads",
            usage.used.processes,
            usage.limit.as_ref().map(|v| v.processes),
        ),
        (
            "Storage bytes",
            usage.used.storage_bytes,
            usage.limit.as_ref().map(|v| v.storage_bytes),
        ),
    ];
    fields
        .into_iter()
        .map(|(name, used, limit)| {
            (
                name.into(),
                used.to_string(),
                limit
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "No active approval".into()),
                if usage.unknown_jobs > 0 {
                    "Unknown".into()
                } else {
                    limit
                        .map(|v| v.saturating_sub(used).to_string())
                        .unwrap_or_else(|| "Unspecified".into())
                },
            )
        })
        .collect()
}
impl Waku {
    pub(super) fn open_command_resources(&mut self, cx: &mut Context<Self>) {
        self.control_ui.resources.open = true;
        self.control_ui.resources.clear();
        self.refresh_command_resources(cx);
    }
    fn refresh_command_resources(&mut self, cx: &mut Context<Self>) {
        let ui = &mut self.control_ui.resources;
        if !ui.open || ui.busy || self.control_ui.page != Some(Page::Usage) {
            return;
        }
        ui.clear();
        ui.busy = true;
        let generation = ui.generation;
        let project = ui.project;
        let client = self.daemon.client();
        cx.spawn(async move |this,cx| {
            let result=cx.background_executor().spawn(async move {smol::unblock(move || -> anyhow::Result<_> {
                let ResponsePayload::CommandResourceUsage{usage}=client.request(Uuid::nil(),Uuid::nil(),Rpc::LoadCommandResourceUsage{project_id:project})? else {anyhow::bail!("Unexpected command resource response");};
                anyhow::ensure!(usage.project_id==project,"Command resource scope changed");
                let prepared=rows(&usage);
                let note=format!("{} command(s) retain compute · {} byte(s) of retained project archives · {} command(s) have unknown historical quantities",
                    usage.held_compute_jobs,usage.retained_archive_bytes,usage.unknown_jobs);
                Ok((prepared,note))
            }).await}).await;
            let _=this.update(cx,|this,cx| {
                let ui=&mut this.control_ui.resources;
                if !ui.open || ui.generation!=generation || ui.project!=project || this.control_ui.page!=Some(Page::Usage){return;}
                ui.busy=false;match result {Ok((rows,note))=>{ui.rows=Rc::new(rows);ui.note=Some(note);},Err(e)=>ui.error=Some(format!("Could not load command resources: {e}. Refresh to retry."))};cx.notify();
            });
        }).detach();
        cx.notify();
    }
    fn command_resources_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let modifiers = &event.keystroke.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform {
            return;
        }
        if event.keystroke.key == "escape" {
            self.control_ui.resources.close();
            cx.stop_propagation();
            cx.notify();
            return;
        }
        let scroll = &self.control_ui.resources.scroll;
        let offset = scroll.offset();
        if matches!(event.keystroke.key.as_str(), "left" | "right") {
            let next = (offset.x
                + px(if event.keystroke.key == "left" {
                    80.0
                } else {
                    -80.0
                }))
            .clamp(-scroll.max_offset().x, Pixels::ZERO);
            scroll.set_offset(point(next, offset.y));
            cx.stop_propagation();
            cx.notify();
            return;
        }
        let max = scroll.max_offset().y;
        let page = scroll.bounds().size.height * 0.9;
        let next = match event.keystroke.key.as_str() {
            "up" => offset.y + px(40.0),
            "down" => offset.y - px(40.0),
            "pageup" => offset.y + page,
            "pagedown" => offset.y - page,
            "home" => Pixels::ZERO,
            "end" => -max,
            _ => return,
        }
        .clamp(-max, Pixels::ZERO);
        scroll.set_offset(point(offset.x, next));
        cx.stop_propagation();
        cx.notify();
    }
    pub(super) fn render_command_resources(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let ui = &self.control_ui.resources;
        let mut toolbar = div()
            .flex()
            .flex_wrap()
            .gap(px(8.0))
            .child(
                self.control_button("resources-usage", "Usage", cx, |this, _, cx| {
                    this.control_ui.resources.close();
                    cx.notify();
                }),
            )
            .child(
                self.control_button("resources-refresh", "", cx, |this, _, cx| {
                    this.refresh_command_resources(cx)
                })
                .tooltip(Tooltip::text("Refresh command resource reservations"))
                .child(icon("icons/rotate-cw.svg", 16.0, theme.text_secondary)),
            );
        let weak = cx.entity().downgrade();
        let menu = self.menu_handle("resources-project", cx);
        let project = ui
            .project
            .and_then(|id| self.state.projects.iter().find(|p| p.id == id))
            .map(|p| p.name.clone())
            .unwrap_or_else(|| {
                if ui.project.is_some() {
                    "Selected project".into()
                } else {
                    "All projects".into()
                }
            });
        toolbar = toolbar.child(dropdown_menu(
            self.control_button("resources-project", project, cx, |_, _, _| {})
                .child(icon("icons/chevron-down.svg", 12.0, theme.text_secondary)),
            "resources-project",
            &menu,
            MenuAlign::BelowLeft,
            move |cx| {
                let Some(entity) = weak.upgrade() else {
                    return vec![];
                };
                let this = entity.read(cx);
                let mut options = vec![("All projects".to_string(), None)];
                options.extend(
                    this.state
                        .projects
                        .iter()
                        .map(|p| (p.name.clone(), Some(p.id))),
                );
                options
                    .into_iter()
                    .map(|(name, project)| {
                        let weak = weak.clone();
                        MenuItem::new(name, move |_, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.control_ui.resources.project = project;
                                this.control_ui.resources.clear();
                                this.refresh_command_resources(cx);
                            });
                        })
                        .selected(this.control_ui.resources.project == project)
                        .into()
                    })
                    .collect()
            },
        ));
        let mut body = div()
            .id("command-resource-scroll")
            .track_focus(&ui.focus)
            .tab_index(0)
            .tab_stop(true)
            .track_scroll(&ui.scroll)
            .focus_visible(|style| style.border_1().border_color(theme.accent))
            .flex_1()
            .min_h_0()
            .overflow_scroll()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .p(px(20.0));
        body=body.child(div().text_size(sp(18.0)).font_weight(FontWeight::SEMIBOLD).child("Command resources"))
            .child(div().text_size(sp(13.0)).text_color(theme.text_secondary).child("Approval limits apply to command allocations across runs. Compute stays held until a verified original result; storage remains held after command completion."));
        if ui.busy {
            body = body.child(div().child("Loading command resource reservations…"));
        }
        if let Some(error) = &ui.error {
            body = body.child(div().text_color(theme.text_secondary).child(error.clone()));
        }
        if let Some(note) = &ui.note {
            body = body.child(
                div()
                    .text_size(sp(13.0))
                    .text_color(theme.text_secondary)
                    .child(note.clone()),
            );
        }
        let header = div()
            .flex()
            .gap(px(12.0))
            .text_size(sp(12.0))
            .text_color(theme.text_secondary)
            .child(div().flex_1().child("Resource"))
            .child(div().w(px(140.0)).child("Held"))
            .child(div().w(px(160.0)).child("Limit"))
            .child(div().w(px(140.0)).child("Available"));
        let mut table = div().flex().flex_col().min_w(px(660.0)).child(header);
        for (name, held, limit, available) in ui.rows.iter() {
            table = table.child(
                div()
                    .flex()
                    .gap(px(12.0))
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .text_size(sp(13.0))
                    .child(div().flex_1().child(name.clone()))
                    .child(div().w(px(140.0)).child(held.clone()))
                    .child(div().w(px(160.0)).child(limit.clone()))
                    .child(div().w(px(140.0)).child(available.clone())),
            );
        }
        body = body.child(table);
        div()
            .flex()
            .flex_col()
            .size_full()
            .text_color(theme.text)
            .on_key_down(cx.listener(|this, event, _, cx| this.command_resources_key(event, cx)))
            .child(
                div()
                    .p(px(16.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(toolbar),
            )
            .child(body)
            .into_any_element()
    }
}
