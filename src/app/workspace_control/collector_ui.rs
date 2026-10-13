//! Original GPU collector reservations. Output bytes and credentials stay on the daemon.
use super::*;
use waku_client::artifact_retention::*;
pub(super) struct CollectorUi {
    pub request: Option<Uuid>,
    busy: bool,
    generation: u64,
    error: Option<String>,
    summary: Option<String>,
    rows: Rc<Vec<(String, String, String)>>,
    list: ListState,
    focus: FocusHandle,
    selected: usize,
}
impl CollectorUi {
    pub fn new(cx: &mut App) -> Self {
        Self {
            request: None,
            busy: false,
            generation: 0,
            error: None,
            summary: None,
            rows: Rc::new(vec![]),
            list: ListState::new(0, ListAlignment::Top, px(90.0)),
            focus: cx.focus_handle(),
            selected: 0,
        }
    }
    pub(super) fn close(&mut self) {
        self.request = None;
        self.generation += 1;
        self.busy = false;
        self.error = None;
        self.summary = None;
        self.rows = Rc::new(vec![]);
        self.list.reset(0);
    }
}
fn prepared(usage: GpuCollectorSnapshot) -> (String, Vec<(String, String, String)>) {
    let summary = format!(
        "{} · {} · {}\n{} receipts / {} bytes reserved in this prefix; {} receipts / {} encoded bytes observed.\n{} receipts / {} bytes reserved across the bucket. Further full receipts allowed: {}. Historical receipts without original limits: {}.",
        usage.bucket_name,
        usage.region,
        usage.prefix,
        usage.reserved_receipts,
        usage.reserved_bytes,
        usage.observed_receipts,
        usage.observed_bytes,
        usage.bucket_reserved_receipts,
        usage.bucket_reserved_bytes,
        usage
            .admissible_receipts
            .map(|n| n.to_string())
            .unwrap_or_else(|| "No approved ceiling".into()),
        usage.historical_unbounded_receipts
    );
    let number = |n: Option<u64>| {
        n.map(|n| n.to_string())
            .unwrap_or_else(|| "Unspecified".into())
    };
    let rows = usage
        .scopes
        .into_iter()
        .map(|s| {
            (
                s.prefix,
                format!(
                    "Reserved: {} receipts / {} bytes · observed: {} receipts / {} bytes",
                    s.reserved_receipts, s.reserved_bytes, s.observed_receipts, s.observed_bytes
                ),
                format!(
                    "Limits: {} receipts / {} bytes · remaining: {} receipts / {} bytes",
                    number(s.max_receipts.map(u64::from)),
                    number(s.max_bytes),
                    number(s.remaining_receipts.map(u64::from)),
                    number(s.remaining_bytes)
                ),
            )
        })
        .collect();
    (summary, rows)
}
impl Waku {
    pub(super) fn open_gpu_collector(&mut self, request: Uuid, cx: &mut Context<Self>) {
        self.control_ui.collector.close();
        self.control_ui.collector.request = Some(request);
        self.refresh_gpu_collector(cx);
    }
    fn refresh_gpu_collector(&mut self, cx: &mut Context<Self>) {
        let ui = &mut self.control_ui.collector;
        let Some(request) = ui.request else {
            return;
        };
        if ui.busy || self.control_ui.page != Some(Page::Usage) {
            return;
        }
        ui.generation += 1;
        ui.busy = true;
        ui.error = None;
        ui.summary = None;
        ui.rows = Rc::new(vec![]);
        ui.list.reset(0);
        let generation = ui.generation;
        let client = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || -> anyhow::Result<_> {
                        let ResponsePayload::GpuCollectorUsage { usage } = client.request(
                            Uuid::nil(),
                            Uuid::nil(),
                            Rpc::LoadGpuCollectorUsage {
                                request_id: request,
                            },
                        )?
                        else {
                            anyhow::bail!("Unexpected GPU collector response");
                        };
                        anyhow::ensure!(
                            usage.request_id == request && usage.scopes.len() <= 4096,
                            "GPU collector identity or bounds changed"
                        );
                        Ok(prepared(usage))
                    })
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let ui = &mut this.control_ui.collector;
                if ui.request != Some(request)
                    || ui.generation != generation
                    || this.control_ui.page != Some(Page::Usage)
                {
                    return;
                }
                ui.busy = false;
                match result {
                    Ok((summary, rows)) => {
                        ui.summary = Some(summary);
                        ui.list.reset(rows.len());
                        ui.rows = Rc::new(rows);
                        ui.selected = 0;
                    }
                    Err(e) => {
                        ui.error = Some(format!(
                            "Could not load the original collector: {e}. Refresh to retry."
                        ))
                    }
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn collector_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let modifiers = &event.keystroke.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform {
            return;
        }
        if event.keystroke.key == "escape" {
            self.control_ui.collector.close();
            cx.stop_propagation();
            cx.notify();
            return;
        }
        let ui = &mut self.control_ui.collector;
        let count = ui.rows.len();
        if count == 0 {
            return;
        }
        let index = match event.keystroke.key.as_str() {
            "up" => ui.selected.saturating_sub(1),
            "down" => (ui.selected + 1).min(count - 1),
            "home" => 0,
            "end" => count - 1,
            "pageup" => ui.selected.saturating_sub(5),
            "pagedown" => (ui.selected + 5).min(count - 1),
            _ => return,
        };
        ui.selected = index;
        ui.list.scroll_to_reveal_item(index);
        cx.stop_propagation();
        cx.notify();
    }
    pub(super) fn render_gpu_collector(&self, cx: &mut Context<Self>) -> AnyElement {
        let ui = &self.control_ui.collector;
        let theme = Theme::current(cx);
        let rows = ui.rows.clone();
        let weak = cx.entity().downgrade();
        let mut header=div().flex().flex_col().gap(px(12.0)).p(px(20.0))
            .child(div().flex().gap(px(8.0))
                .child(self.control_button("collector-back","Remote executions",cx,|this,_,cx|{this.control_ui.collector.close();cx.notify();}))
                .child(self.control_button("collector-refresh","",cx,|this,_,cx|this.refresh_gpu_collector(cx)).tooltip(Tooltip::text("Refresh retained collector storage")).child(icon("icons/rotate-cw.svg",16.0,theme.text_secondary))))
            .child(div().text_size(sp(18.0)).font_weight(FontWeight::SEMIBOLD).child("Collector storage"))
            .child(div().text_size(sp(13.0)).text_color(theme.text_secondary).child("Keep until explicitly cleaned. Remote collector deletion is unavailable; completing or cleaning a job keeps these storage reservations."));
        if ui.busy {
            header = header.child(div().child("Loading collector storage…"));
        }
        if let Some(error) = &ui.error {
            header = header.child(div().text_color(theme.danger).child(error.clone()));
        }
        if let Some(summary) = &ui.summary {
            header = header.child(div().text_size(sp(13.0)).child(summary.clone()));
        }
        div()
            .track_focus(&self.control_ui.focus)
            .tab_group()
            .flex()
            .flex_col()
            .size_full()
            .text_color(theme.text)
            .on_key_down(cx.listener(|this, event, _, cx| this.collector_key(event, cx)))
            .child(header)
            .child(
                div()
                    .id("collector-scope-list")
                    .track_focus(&ui.focus)
                    .tab_index(0)
                    .tab_stop(true)
                    .focus_visible(|s| s.border_1().border_color(theme.accent))
                    .flex_1()
                    .min_h_0()
                    .child(
                        list(ui.list.clone(), move |index, _, cx| {
                            let Some((prefix, held, limit)) = rows.get(index) else {
                                return div().into_any_element();
                            };
                            let selected = weak
                                .upgrade()
                                .is_some_and(|e| e.read(cx).control_ui.collector.selected == index);
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.0))
                                .p(px(16.0))
                                .border_b_1()
                                .border_color(theme.border)
                                .when(selected, |s| s.bg(theme.overlay))
                                .child(
                                    div()
                                        .text_size(sp(14.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(prefix.clone()),
                                )
                                .child(div().text_size(sp(12.0)).child(held.clone()))
                                .child(
                                    div()
                                        .text_size(sp(12.0))
                                        .text_color(theme.text_secondary)
                                        .child(limit.clone()),
                                )
                                .into_any_element()
                        })
                        .size_full(),
                    ),
            )
            .into_any_element()
    }
}
