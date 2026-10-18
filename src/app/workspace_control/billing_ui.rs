//! Cached owner billing views. Native reads/parsing stay on workers; no provider call from render.
use super::*;
use waku_client::cloud_billing::*;
#[derive(Clone)]
enum BillingRow {
    Scope(Rc<CloudBillingScopeStatus>),
    Report(Rc<CloudBillingReport>),
    Charge(Rc<CloudBillingCharge>),
    ReadyExport(Uuid, u64, Rc<CloudBillingReadyExport>),
}
pub(super) struct BillingUi {
    pub open: bool,
    busy: bool,
    generation: u64,
    error: Option<String>,
    message: Option<String>,
    page: Option<CloudBillingPage>,
    rows: Rc<Vec<BillingRow>>,
    list: ListState,
    focus: FocusHandle,
    selected: usize,
    scope: Option<Uuid>,
    charges: Option<CloudBillingChargePage>,
    history: Option<CloudBillingReportPage>,
    exports: Option<CloudBillingExportPage>,
    history_after: Option<String>,
    history_previous: Vec<Option<String>>,
    after: Option<u64>,
    previous: Vec<Option<u64>>,
    editing: bool,
    id: Uuid,
    revision: Option<u64>,
    name: Entity<TextInput>,
    account: Entity<TextInput>,
    contract: Entity<TextInput>,
    currency: Entity<TextInput>,
    origin: Entity<TextInput>,
    export: Entity<TextInput>,
}
impl BillingUi {
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        let mut input =
            |hint: &str, cx: &mut App| cx.new(|cx| TextInput::new(window, cx).placeholder(hint));
        Self {
            open: false,
            busy: false,
            generation: 0,
            error: None,
            message: None,
            page: None,
            rows: Rc::new(vec![]),
            list: ListState::new(0, ListAlignment::Top, px(120.0)),
            focus: cx.focus_handle(),
            selected: 0,
            scope: None,
            charges: None,
            history: None,
            exports: None,
            history_after: None,
            history_previous: vec![],
            after: None,
            previous: vec![],
            editing: false,
            id: Uuid::nil(),
            revision: None,
            name: input("Billing connection name", cx),
            account: input("Exact billing account ID", cx),
            contract: input("Exact contract ID", cx),
            currency: input("USD", cx),
            origin: input("https://storage.eu-north1.nebius.cloud", cx),
            export: input("Existing single-month export ID", cx),
        }
    }
    pub fn close(&mut self) {
        self.open = false;
        self.generation += 1;
        self.busy = false;
        self.editing = false;
        self.charges = None;
        self.history = None;
        self.exports = None;
        self.history_after = None;
        self.history_previous.clear();
        self.rows = Rc::new(vec![]);
        self.page = None;
        self.scope = None;
        self.after = None;
        self.previous.clear();
    }
    fn rebuild(&mut self) {
        let mut rows = vec![];
        if let Some(page) = &self.exports {
            rows.extend(
                page.items
                    .iter()
                    .cloned()
                    .map(|row| BillingRow::ReadyExport(page.scope_id, page.revision, Rc::new(row))),
            );
        } else if let Some(charges) = &self.charges {
            rows.extend(
                charges
                    .charges
                    .iter()
                    .cloned()
                    .map(|row| BillingRow::Charge(Rc::new(row))),
            );
        } else if let Some(history) = &self.history {
            rows.extend(
                history
                    .reports
                    .iter()
                    .cloned()
                    .map(|r| BillingRow::Report(Rc::new(r))),
            );
        } else if let Some(page) = &self.page {
            for scope in &page.scopes {
                rows.push(BillingRow::Scope(Rc::new(scope.clone())));
                if self.scope == Some(scope.scope.id) {
                    rows.extend(
                        scope
                            .reports
                            .iter()
                            .cloned()
                            .map(|row| BillingRow::Report(Rc::new(row))),
                    );
                }
            }
        }
        self.rows = Rc::new(rows);
        self.selected = 0;
        self.list.reset(self.rows.len());
    }
}
impl Waku {
    pub(super) fn refresh_cloud_billing(&mut self, cx: &mut Context<Self>) {
        self.cloud_billing_request(
            Rpc::LoadCloudBilling {
                after: None,
                limit: 64,
            },
            cx,
        );
    }
    fn cloud_billing_request(&mut self, command: Rpc, cx: &mut Context<Self>) {
        let ui = &mut self.control_ui.billing;
        if !ui.open || ui.busy {
            return;
        }
        let discovery = if let Rpc::DiscoverCloudBillingExports { query } = &command {
            Some(query.clone())
        } else {
            None
        };
        if discovery.is_none() {
            ui.exports = None;
        }
        ui.busy = true;
        ui.error = None;
        ui.message = None;
        ui.generation += 1;
        let generation = ui.generation;
        let expected_discovery = discovery.clone();
        let client = self.daemon.client();
        cx.spawn(async move |this,cx| {
            let result=cx.background_executor().spawn(async move {
                smol::unblock(move ||->anyhow::Result<_> {
                    let reply=client.request(Uuid::nil(),Uuid::nil(),command)?;
                    if let Some(query) = &discovery {
                        let ResponsePayload::CloudBillingExports { page } = &reply else { anyhow::bail!("Unexpected billing discovery response"); };
                        validate_discovery_reply(query, page)?;
                    }
                    let page=if matches!(reply,ResponsePayload::CloudBillingScope{..}|ResponsePayload::CloudBillingReport{..}) {
                        let ResponsePayload::CloudBilling{page}=client.request(Uuid::nil(),Uuid::nil(),Rpc::LoadCloudBilling{after:None,limit:64})? else {anyhow::bail!("Unexpected billing refresh response");};Some(page)
                    }else{None};Ok((reply,page))
                }).await
            }).await;
            let _=this.update(cx,|this,cx| {
                let ui=&mut this.control_ui.billing;if ui.generation!=generation || !ui.open {return;}
                ui.busy=false;
                if let Some(query) = &expected_discovery {
                    if ui.scope != Some(query.scope_id) || ui.page.as_ref().and_then(|page| page.scopes.iter().find(|s| s.scope.id == query.scope_id)).is_none_or(|s| s.revision != query.expected_revision) {
                        ui.exports = None; ui.error = Some("Billing connection changed; refresh before discovery".into()); ui.rebuild(); cx.notify(); return;
                    }
                }
                match result {
                    Ok((reply,page))=>{
                        if let Some(page)=page {ui.page=Some(page);}
                        match reply {
                            ResponsePayload::CloudBilling {page} => {
                                ui.page=Some(page);ui.charges=None;ui.history=None;
                                ui.after=None;ui.previous.clear();ui.history_after=None;ui.history_previous.clear();
                            }
                            ResponsePayload::CloudBillingScope {status} => {
                                if ui.id==status.scope.id {ui.revision=Some(status.revision);}
                                ui.scope=Some(status.scope.id);ui.charges=None;ui.history=None;
                                ui.after=None;ui.previous.clear();ui.history_after=None;ui.history_previous.clear();
                                ui.message=Some("Billing connection saved".into());
                            }
                            ResponsePayload::CloudBillingReport {report} => {
                                ui.scope=Some(report.scope_id);ui.charges=None;ui.history=None;
                                ui.after=None;ui.previous.clear();ui.history_after=None;ui.history_previous.clear();
                                ui.message=Some(if report.current {"Reported charges synced"}else{"Earlier export retained; the newer snapshot remains selected"}.into());
                            }
                            ResponsePayload::CloudBillingReports {page} => {ui.history=Some(page);ui.charges=None;}
                            ResponsePayload::CloudBillingCharges {page} => ui.charges=Some(page),
                            ResponsePayload::CloudBillingExports {page} => {
                                ui.charges=None;ui.history=None;ui.exports=Some(page);
                            }
                            _ => ui.error=Some("Unexpected billing response".into()),
                        }
                        ui.rebuild();
                    }
                    Err(e)=>ui.error=Some(format!("Could not update cloud billing: {e}")),
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
    fn begin_billing_scope(
        &mut self,
        scope: Option<CloudBillingScopeStatus>,
        cx: &mut Context<Self>,
    ) {
        let ui = &mut self.control_ui.billing;
        if ui.busy {
            return;
        }
        ui.exports = None;
        ui.rebuild();
        ui.editing = true;
        ui.id = scope.as_ref().map_or_else(Uuid::new_v4, |s| s.scope.id);
        ui.revision = scope.as_ref().map(|s| s.revision);
        let values = scope
            .as_ref()
            .map(|s| {
                [
                    s.scope.name.clone(),
                    s.scope.account_id.clone(),
                    s.scope.contract_id.clone(),
                    s.scope.currency.clone(),
                    s.scope.archive_origin.clone(),
                ]
            })
            .unwrap_or_else(|| {
                [
                    String::new(),
                    String::new(),
                    String::new(),
                    "USD".into(),
                    String::new(),
                ]
            });
        for (input, value) in [
            ui.name.clone(),
            ui.account.clone(),
            ui.contract.clone(),
            ui.currency.clone(),
            ui.origin.clone(),
        ]
        .into_iter()
        .zip(values)
        {
            input.update(cx, |input, cx| input.set_content(value, cx));
        }
        ui.error = None;
        cx.notify();
    }
    fn save_billing_scope(&mut self, cx: &mut Context<Self>) {
        let ui = &self.control_ui.billing;
        let scope = CloudBillingScope {
            id: ui.id,
            name: ui.name.read(cx).content().trim().into(),
            account_id: ui.account.read(cx).content().trim().into(),
            contract_id: ui.contract.read(cx).content().trim().into(),
            currency: ui.currency.read(cx).content().trim().into(),
            archive_origin: ui.origin.read(cx).content().trim().into(),
        };
        self.cloud_billing_request(
            Rpc::SaveCloudBillingScope {
                scope,
                expected_revision: ui.revision,
            },
            cx,
        );
    }
    fn discover_billing_exports(&mut self, after: Option<Uuid>, cx: &mut Context<Self>) {
        let ui = &self.control_ui.billing;
        if ui.busy {
            return;
        }
        let Some(scope) = ui
            .scope
            .and_then(|id| ui.page.as_ref()?.scopes.iter().find(|s| s.scope.id == id))
        else {
            return;
        };
        self.cloud_billing_request(
            Rpc::DiscoverCloudBillingExports {
                query: CloudBillingExportQuery {
                    scope_id: scope.scope.id,
                    expected_revision: scope.revision,
                    after,
                    limit: 32,
                },
            },
            cx,
        );
    }
    fn sync_billing_report(&mut self, cx: &mut Context<Self>) {
        let ui = &self.control_ui.billing;
        let Some(id) = ui.scope else {
            return;
        };
        let Some(scope) = ui
            .page
            .as_ref()
            .and_then(|p| p.scopes.iter().find(|s| s.scope.id == id))
        else {
            return;
        };
        self.cloud_billing_request(
            Rpc::SyncCloudBillingReport {
                scope_id: id,
                expected_revision: scope.revision,
                export_id: ui.export.read(cx).content().trim().into(),
            },
            cx,
        );
    }
    fn billing_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let ui = &mut self.control_ui.billing;
        let count = ui.rows.len();
        if count == 0 {
            return;
        }
        let index = match event.keystroke.key.as_str() {
            "down" => (ui.selected + 1).min(count - 1),
            "up" => ui.selected.saturating_sub(1),
            "home" => 0,
            "end" => count - 1,
            "pagedown" => (ui.selected + 4).min(count - 1),
            "pageup" => ui.selected.saturating_sub(4),
            "enter" | "space" => {
                let row = ui.rows[ui.selected].clone();
                self.open_billing_row(row, cx);
                cx.stop_propagation();
                return;
            }
            _ => return,
        };
        ui.selected = index;
        ui.list.scroll_to_reveal_item(index);
        cx.stop_propagation();
        cx.notify();
    }
    fn open_billing_row(&mut self, row: BillingRow, cx: &mut Context<Self>) {
        if self.control_ui.billing.busy {
            return;
        }
        match row {
            BillingRow::Scope(scope) => {
                let ui = &mut self.control_ui.billing;
                ui.scope = Some(scope.scope.id);
                ui.exports = None;
                ui.export
                    .update(cx, |input, cx| input.set_content(String::new(), cx));
                ui.charges = None;
                ui.history = None;
                ui.history_after = None;
                ui.history_previous.clear();
                ui.rebuild();
                ui.selected = ui
                    .rows
                    .iter()
                    .position(
                        |row| matches!(row,BillingRow::Scope(s) if s.scope.id==scope.scope.id),
                    )
                    .unwrap_or(0);
                ui.list.scroll_to_reveal_item(ui.selected);
                cx.notify();
            }
            BillingRow::Report(report) => {
                self.control_ui.billing.after = None;
                self.control_ui.billing.previous.clear();
                self.cloud_billing_request(
                    Rpc::LoadCloudBillingCharges {
                        scope_id: report.scope_id,
                        export_id: report.export_id.clone(),
                        after: None,
                        limit: 32,
                    },
                    cx,
                );
            }
            BillingRow::ReadyExport(scope_id, revision, export) => {
                let ui = &mut self.control_ui.billing;
                if ui.scope != Some(scope_id)
                    || ui.exports.as_ref().is_none_or(|page| {
                        page.scope_id != scope_id
                            || page.revision != revision
                            || !page.items.iter().any(|item| item == export.as_ref())
                    })
                {
                    return;
                }
                ui.export.update(cx, |input, cx| {
                    input.set_content(export.export_id.clone(), cx)
                });
                ui.message = Some(format!(
                    "Selected {} report. Use Sync report to import its verified charges.",
                    export.period
                ));
                cx.notify();
            }
            BillingRow::Charge(_) => {}
        }
    }
    fn render_billing_row(
        &self,
        row: BillingRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        let ui = &self.control_ui.billing;
        let (title, detail, meter) = match &row {
            BillingRow::Scope(status) => (
                status.scope.name.clone(),
                format!(
                    "{} · {} · {}",
                    status.scope.account_id, status.scope.contract_id, status.scope.currency
                ),
                format!("{} recent monthly snapshots", status.reports.len()),
            ),
            BillingRow::Report(report) => (
                format!(
                    "{} · {}",
                    report.period,
                    if report.current {
                        "Selected monthly snapshot"
                    } else {
                        "Historical export"
                    }
                ),
                format!(
                    "{} · Created {} · {} rows · {} without resource attribution",
                    report.export_id,
                    report.created_at,
                    report.rows,
                    report.unattributed_resource_rows
                ),
                format!(
                    "{} {} billed · {} effective · {} contracted",
                    report.currency,
                    report.billed_cost,
                    report.effective_cost,
                    report.contracted_cost
                ),
            ),
            BillingRow::ReadyExport(_, _, export) => (
                format!("{} · Ready FOCUS report", export.period),
                format!("{} · Created {}", export.export_id, export.created_at),
                format!("Expires {}", export.expires_at),
            ),
            BillingRow::Charge(charge) => (
                format!(
                    "{} · {}",
                    charge.category,
                    charge.class.as_deref().unwrap_or("Original charge")
                ),
                format!(
                    "{} · {} · {}",
                    charge.container_id,
                    charge
                        .resource_type
                        .as_deref()
                        .unwrap_or("Unspecified resource type"),
                    charge
                        .resource_id
                        .as_deref()
                        .unwrap_or("Resource attribution unavailable")
                ),
                format!(
                    "Billed {} · Effective {} · Contracted {}",
                    charge.billed_cost, charge.effective_cost, charge.contracted_cost
                ),
            ),
        };
        let interactive = !matches!(row, BillingRow::Charge(_));
        let selected = ui.selected == index;
        let mut body = div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .p(px(14.0))
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .text_size(sp(14.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text)
                    .child(title),
            )
            .child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(detail),
            )
            .child(
                div()
                    .text_size(sp(13.0))
                    .text_color(theme.text)
                    .child(meter),
            );
        if interactive {
            body = body.child(self.control_button(
                format!("billing-row-{index}"),
                if matches!(row, BillingRow::Scope(_)) {
                    "Select account"
                } else if matches!(row, BillingRow::ReadyExport(..)) {
                    "Select report"
                } else {
                    "View charges"
                },
                cx,
                move |this, _, cx| this.open_billing_row(row.clone(), cx),
            ));
        }
        body.when(selected, |body| body.bg(theme.overlay))
            .into_any_element()
    }
    pub(super) fn render_cloud_billing(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let ui = &self.control_ui.billing;
        let mut body = div()
            .track_focus(&self.control_ui.focus)
            .tab_group()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .p_0()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.0))
                    .child(
                        self.control_button("billing-usage", "Usage", cx, |this, _, cx| {
                            this.control_ui.billing.close();
                            cx.notify();
                        }),
                    )
                    .child(self.control_button(
                        "billing-new",
                        "New billing connection",
                        cx,
                        |this, _, cx| this.begin_billing_scope(None, cx),
                    ))
                    .child(
                        self.control_button("billing-refresh", "", cx, |this, _, cx| {
                            this.refresh_cloud_billing(cx)
                        })
                        .tooltip(Tooltip::text("Refresh billing reports"))
                        .child(icon(
                            "icons/rotate-cw.svg",
                            16.0,
                            theme.text_secondary,
                        )),
                    ),
            )
            .child(
                div()
                    .text_size(sp(18.0))
                    .text_color(theme.text)
                    .child("Cloud billing"),
            )
            .child(
                div()
                    .text_size(sp(13.0))
                    .text_color(theme.text_secondary)
                    .child("Account charges from selected Nebius reports."),
            );
        if let Some(error) = &ui.error {
            body = body.child(div().text_color(theme.text).child(error.clone()));
        }
        if let Some(message) = &ui.message {
            body = body.child(
                div()
                    .text_color(theme.text_secondary)
                    .child(message.clone()),
            );
        }
        if ui.busy {
            body = body.child(
                div()
                    .text_color(theme.text_secondary)
                    .child("Updating cloud billing…"),
            );
        }
        if let Some(scope) = ui
            .scope
            .and_then(|id| {
                ui.page
                    .as_ref()
                    .and_then(|p| p.scopes.iter().find(|s| s.scope.id == id))
            })
            .cloned()
        {
            body = body
                .child(
                    div()
                        .flex()
                        .gap(px(8.0))
                        .items_center()
                        .child(div().text_color(theme.text).child(scope.scope.name.clone()))
                        .child(self.control_button(
                            "billing-edit",
                            "Edit connection",
                            cx,
                            move |this, _, cx| this.begin_billing_scope(Some(scope.clone()), cx),
                        )),
                )
                .child(control_field(
                    "Selected export ID (or enter one)",
                    ui.export.clone(),
                    "billing-export-input",
                ))
                .child(self.control_button(
                    "billing-discover",
                    "Discover existing exports",
                    cx,
                    |this, _, cx| this.discover_billing_exports(None, cx),
                ))
                .child(
                    self.control_button("billing-sync", "Sync report", cx, |this, _, cx| {
                        this.sync_billing_report(cx)
                    }),
                );
        }
        if let Some(scope_id) = ui.scope {
            body = body.child(self.control_button(
                "billing-history",
                "Imported exports",
                cx,
                move |this, _, cx| {
                    if this.control_ui.billing.busy {
                        return;
                    }
                    this.control_ui.billing.history_after = None;
                    this.control_ui.billing.history_previous.clear();
                    this.cloud_billing_request(
                        Rpc::LoadCloudBillingReports {
                            scope_id,
                            after: None,
                            limit: 16,
                        },
                        cx,
                    );
                },
            ));
        }
        if let Some(page) = &ui.exports {
            let next = page.next_cursor;
            body = body.child(div().text_color(theme.text_secondary).child(format!(
                "{} existing exports checked · {} ready monthly reports on this page",
                page.examined,
                page.items.len()
            )));
            if page.items.is_empty() {
                body = body.child(
                    div()
                        .text_color(theme.text)
                        .child("No ready single-month FOCUS reports on this page."),
                );
            }
            if let Some(next) = next {
                body = body.child(self.control_button(
                    "billing-discovery-next",
                    "Next export page",
                    cx,
                    move |this, _, cx| this.discover_billing_exports(Some(next), cx),
                ));
            }
        }
        if let Some(page) = ui.history.as_ref().filter(|_| ui.charges.is_none()) {
            let scope_id = page.scope_id;
            let next = page.next_cursor.clone();
            let before = ui.history_after.clone();
            body = body
                .child(div().text_color(theme.text_secondary).child(format!(
                    "{} imported exports · snapshots are shown individually",
                    page.total
                )))
                .child(
                    div()
                        .flex()
                        .gap(px(8.0))
                        .child(self.control_button(
                            "billing-current",
                            "Monthly snapshots",
                            cx,
                            |this, _, cx| {
                                let ui = &mut this.control_ui.billing;
                                ui.history = None;
                                ui.history_after = None;
                                ui.history_previous.clear();
                                ui.rebuild();
                                cx.notify();
                            },
                        ))
                        .when(!ui.history_previous.is_empty(), |bar| {
                            bar.child(self.control_button(
                                "billing-history-previous",
                                "Previous",
                                cx,
                                move |this, _, cx| {
                                    if this.control_ui.billing.busy {
                                        return;
                                    }
                                    let after = this
                                        .control_ui
                                        .billing
                                        .history_previous
                                        .pop()
                                        .unwrap_or(None);
                                    this.control_ui.billing.history_after = after.clone();
                                    this.cloud_billing_request(
                                        Rpc::LoadCloudBillingReports {
                                            scope_id,
                                            after,
                                            limit: 16,
                                        },
                                        cx,
                                    );
                                },
                            ))
                        })
                        .when(next.is_some(), |bar| {
                            bar.child(self.control_button(
                                "billing-history-next",
                                "Next",
                                cx,
                                move |this, _, cx| {
                                    if this.control_ui.billing.busy {
                                        return;
                                    }
                                    this.control_ui
                                        .billing
                                        .history_previous
                                        .push(before.clone());
                                    this.control_ui.billing.history_after = next.clone();
                                    this.cloud_billing_request(
                                        Rpc::LoadCloudBillingReports {
                                            scope_id,
                                            after: next.clone(),
                                            limit: 16,
                                        },
                                        cx,
                                    );
                                },
                            ))
                        }),
                );
        }
        if let Some(page) = &ui.charges {
            let report = page.report.clone();
            let before = ui.after;
            let next = page.next_cursor;
            body = body
                .child(div().text_color(theme.text_secondary).child(format!(
                    "{} {} billed · {} · {}",
                    report.currency,
                    report.billed_cost,
                    report.period,
                    if report.current {
                        "Selected monthly snapshot"
                    } else {
                        "Historical export"
                    }
                )))
                .child(
                    div()
                        .flex()
                        .gap(px(8.0))
                        .child(self.control_button(
                            "billing-accounts",
                            "Accounts",
                            cx,
                            |this, _, cx| {
                                let ui = &mut this.control_ui.billing;
                                ui.charges = None;
                                ui.after = None;
                                ui.previous.clear();
                                ui.rebuild();
                                cx.notify();
                            },
                        ))
                        .when(!ui.previous.is_empty(), |bar| {
                            let export = report.clone();
                            bar.child(self.control_button(
                                "billing-previous",
                                "Previous",
                                cx,
                                move |this, _, cx| {
                                    if this.control_ui.billing.busy {
                                        return;
                                    }
                                    let after =
                                        this.control_ui.billing.previous.pop().unwrap_or(None);
                                    this.control_ui.billing.after = after;
                                    this.cloud_billing_request(
                                        Rpc::LoadCloudBillingCharges {
                                            scope_id: export.scope_id,
                                            export_id: export.export_id.clone(),
                                            after,
                                            limit: 32,
                                        },
                                        cx,
                                    );
                                },
                            ))
                        })
                        .when(next.is_some(), |bar| {
                            let export = report.clone();
                            bar.child(self.control_button(
                                "billing-next",
                                "Next",
                                cx,
                                move |this, _, cx| {
                                    if this.control_ui.billing.busy {
                                        return;
                                    }
                                    this.control_ui.billing.previous.push(before);
                                    this.control_ui.billing.after = next;
                                    this.cloud_billing_request(
                                        Rpc::LoadCloudBillingCharges {
                                            scope_id: export.scope_id,
                                            export_id: export.export_id.clone(),
                                            after: next,
                                            limit: 32,
                                        },
                                        cx,
                                    );
                                },
                            ))
                        }),
                );
        }
        let weak = cx.entity().downgrade();
        let rows = ui.rows.clone();
        let mut content = div().flex_1().min_h_0().flex().gap(px(18.0)).child(
            div()
                .id("billing-list")
                .track_focus(&ui.focus)
                .tab_index(0)
                .tab_stop(true)
                .focus_visible(|style| style.border_1().border_color(theme.accent))
                .on_key_down(cx.listener(|this, event, _, cx| this.billing_key(event, cx)))
                .flex_1()
                .min_h_0()
                .child(
                    list(ui.list.clone(), move |index, _, cx| {
                        weak.upgrade()
                            .map(|e| {
                                e.update(cx, |this, cx| {
                                    rows.get(index)
                                        .cloned()
                                        .map(|row| this.render_billing_row(row, index, cx))
                                        .unwrap_or_else(|| div().into_any_element())
                                })
                            })
                            .unwrap_or_else(|| div().into_any_element())
                    })
                    .size_full(),
                ),
        );
        if ui.rows.is_empty() && !ui.busy && ui.exports.is_none() {
            body = body.child(div().text_color(theme.text_secondary).child(
                "Create a billing connection, then sync an existing successful Nebius export.",
            ));
        }
        if ui.editing {
            let mut editor = div()
                .id("billing-editor")
                .w(px(360.0))
                .min_h_0()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap(px(12.0));
            for (label, input, id) in [
                ("Connection name", ui.name.clone(), "billing-name"),
                ("Billing account ID", ui.account.clone(), "billing-account"),
                ("Contract ID", ui.contract.clone(), "billing-contract"),
                ("Billing currency", ui.currency.clone(), "billing-currency"),
                (
                    "Archive storage origin",
                    ui.origin.clone(),
                    "billing-origin",
                ),
            ] {
                editor = editor.child(control_field(label, input, id));
            }
            content = content.child(
                editor.child(
                    div()
                        .flex()
                        .gap(px(8.0))
                        .child(
                            self.control_button("billing-save", "Save", cx, |this, _, cx| {
                                this.save_billing_scope(cx)
                            }),
                        )
                        .child(self.control_button(
                            "billing-cancel",
                            "Close",
                            cx,
                            |this, _, cx| {
                                if !this.control_ui.billing.busy {
                                    this.control_ui.billing.editing = false;
                                    cx.notify();
                                }
                            },
                        )),
                ),
            );
        }
        body.child(content).into_any_element()
    }
}

fn validate_discovery_reply(
    query: &CloudBillingExportQuery,
    page: &CloudBillingExportPage,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        page.scope_id == query.scope_id
            && page.revision == query.expected_revision
            && page.examined <= query.limit
            && page.items.len() <= page.examined as usize
            && !page
                .next_cursor
                .is_some_and(|id| id.is_nil() || Some(id) == query.after),
        "Billing discovery response changed scope or page"
    );
    let mut seen = std::collections::HashSet::new();
    for row in &page.items {
        let month =
            chrono::NaiveDate::parse_from_str(&format!("{}-01", row.period), "%Y-%m-%d").ok();
        let created = chrono::DateTime::parse_from_rfc3339(&row.created_at).ok();
        let expires = chrono::DateTime::parse_from_rfc3339(&row.expires_at).ok();
        anyhow::ensure!(
            month.is_some()
                && created.is_some_and(|at| at.timestamp() >= 0
                    && at.timestamp() as u64 <= page.served_at.saturating_add(300))
                && expires.is_some_and(
                    |at| at.timestamp() > i64::try_from(page.served_at).unwrap_or(i64::MAX)
                ),
            "Billing discovery response contains invalid report times"
        );
        anyhow::ensure!(
            seen.insert(&row.export_id)
                && !row.export_id.is_empty()
                && row.export_id.len() <= 160
                && row
                    .export_id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
                && row.period.len() == 7
                && row.created_at.len() <= 40
                && row.expires_at.len() <= 40,
            "Billing discovery response contains invalid report metadata"
        );
    }
    Ok(())
}

#[cfg(test)]
mod discovery_tests {
    use super::*;
    fn fixture() -> (CloudBillingExportQuery, CloudBillingExportPage) {
        let query = CloudBillingExportQuery {
            scope_id: Uuid::new_v4(),
            expected_revision: 1,
            after: None,
            limit: 32,
        };
        let page = CloudBillingExportPage {
            scope_id: query.scope_id,
            revision: 1,
            items: vec![CloudBillingReadyExport {
                export_id: "export-fixture".into(),
                period: "2026-09".into(),
                created_at: "2026-10-10T00:00:00Z".into(),
                expires_at: "2026-10-17T00:00:00Z".into(),
            }],
            examined: 1,
            next_cursor: Some(Uuid::new_v4()),
            served_at: 1791763200,
        };
        (query, page)
    }
    #[test]
    fn native_discovery_fences_foreign_scope_revision_counts_and_continuations() {
        let (query, page) = fixture();
        assert!(validate_discovery_reply(&query, &page).is_ok());
        for mutation in 0..6 {
            let mut changed = page.clone();
            match mutation {
                0 => changed.scope_id = Uuid::new_v4(),
                1 => changed.revision += 1,
                2 => changed.examined = 33,
                3 => changed.examined = 0,
                4 => changed.next_cursor = Some(Uuid::nil()),
                _ => changed.items.push(changed.items[0].clone()),
            };
            assert!(validate_discovery_reply(&query, &changed).is_err());
        }
        let mut query = query;
        query.after = page.next_cursor;
        assert!(validate_discovery_reply(&query, &page).is_err());
    }
    #[test]
    fn native_discovery_fences_unsafe_identifiers_and_invalid_or_expired_months() {
        let (query, page) = fixture();
        for mutation in 0..5 {
            let mut changed = page.clone();
            let row = &mut changed.items[0];
            match mutation {
                0 => row.export_id = "https://secret?token=fixture".into(),
                1 => row.period = "2026-99".into(),
                2 => row.created_at = "2026-12-01T00:00:00Z".into(),
                3 => row.expires_at = "2026-01-01T00:00:00Z".into(),
                _ => row.created_at = "not-a-time".into(),
            };
            assert!(validate_discovery_reply(&query, &changed).is_err());
        }
    }
}
