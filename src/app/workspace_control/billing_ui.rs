//! Cached owner billing views. Native reads/parsing stay on workers; no provider call from render.
use super::*;
use waku_client::cloud_billing::*;
#[derive(Clone)]
enum BillingRow {
    Scope(Rc<CloudBillingScopeStatus>),
    Report(Rc<CloudBillingReport>),
    Charge(Rc<CloudBillingCharge>),
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
        if let Some(charges) = &self.charges {
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
        ui.busy = true;
        ui.error = None;
        ui.message = None;
        ui.generation += 1;
        let generation = ui.generation;
        let client = self.daemon.client();
        cx.spawn(async move |this,cx| {
            let result=cx.background_executor().spawn(async move {
                smol::unblock(move ||->anyhow::Result<_> {
                    let reply=client.request(Uuid::nil(),Uuid::nil(),command)?;
                    let page=if matches!(reply,ResponsePayload::CloudBillingScope{..}|ResponsePayload::CloudBillingReport{..}) {
                        let ResponsePayload::CloudBilling{page}=client.request(Uuid::nil(),Uuid::nil(),Rpc::LoadCloudBilling{after:None,limit:64})? else {anyhow::bail!("Unexpected billing refresh response");};Some(page)
                    }else{None};Ok((reply,page))
                }).await
            }).await;
            let _=this.update(cx,|this,cx| {
                let ui=&mut this.control_ui.billing;if ui.generation!=generation || !ui.open {return;}
                ui.busy=false;
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
            .p(px(24.0))
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
                    "Existing single-month export ID",
                    ui.export.clone(),
                    "billing-export-input",
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
        if ui.rows.is_empty() && !ui.busy {
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
