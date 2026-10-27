use super::*;

pub(super) struct ReadyQr {
    image: Arc<gpui::RenderImage>,
    address: SharedString,
}

#[derive(Default)]
pub(super) struct PairingQrState {
    image: Option<ReadyQr>,
    pending: bool,
    generation: u64,
}

impl PairingQrState {
    pub(super) fn clear(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.image = None;
        self.pending = false;
    }
}

impl Waku {
    fn toggle_daemon_pairing_qr(&mut self, cx: &mut Context<Self>) {
        if self.daemon_pairing_qr.image.is_some() || self.daemon_pairing_qr.pending {
            self.daemon_pairing_qr.clear();
            cx.notify();
            return;
        }
        if !self.state.daemon_exposure.enabled
            || self.daemon_reconfigure_pending
            || self.daemon_exposure_fields_dirty(cx)
            || self.daemon.is_remote()
        {
            return;
        }
        self.daemon_pairing_qr.clear();
        self.daemon_pairing_qr.pending = true;
        let generation = self.daemon_pairing_qr.generation;
        let hostname = self.daemon_hostname.clone();
        let port = self.state.daemon_exposure.port;
        let token = self.state.daemon_exposure.token.clone();
        let job = cx.background_executor().spawn(async move {
            let qr = crate::mobile_pairing::local_pairing_qr(&hostname, port, &token)?;
            Ok::<_, ()>(ReadyQr {
                address: qr.address.into(),
                image: Arc::new(gpui::RenderImage::new(vec![image::Frame::new(qr.image)])),
            })
        });
        cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                if this.daemon_pairing_qr.generation != generation {
                    return;
                }
                this.daemon_pairing_qr.pending = false;
                if this.settings_page != Some(SettingsPage::Daemon) {
                    this.daemon_pairing_qr.clear();
                    return;
                }
                match result {
                    Ok(qr) => this.daemon_pairing_qr.image = Some(qr),
                    Err(()) => this.show_toast(tr!("daemon.pairing_qr_failed")),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn render_daemon_pairing_qr(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let unavailable = self.daemon_reconfigure_pending || self.daemon_exposure_fields_dirty(cx);
        let revealed = self.daemon_pairing_qr.image.is_some() || self.daemon_pairing_qr.pending;
        let button = div()
            .id("daemon-pairing-qr-toggle")
            .track_focus(&self.daemon_pairing_qr_focus)
            .tab_index(0)
            .min_h(px(32.0))
            .py(px(6.0))
            .px(px(11.0))
            .rounded(px(7.0))
            .border_1()
            .border_color(theme.border_strong)
            .flex()
            .items_center()
            .justify_center()
            .flex_none()
            .text_size(sp(12.5))
            .text_color(theme.text)
            .cursor_default()
            .opacity(if unavailable { 0.55 } else { 1.0 })
            .focus_visible(|style| style.border_color(theme.accent))
            .child(if revealed {
                tr!("daemon.hide_pairing_qr")
            } else {
                tr!("daemon.show_pairing_qr")
            })
            .when(!unavailable, |button| {
                button
                    .hover(|style| style.bg(theme.overlay))
                    .on_activation(cx, |this, _, cx| this.toggle_daemon_pairing_qr(cx))
            })
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if !event.keystroke.modifiers.modified()
                    && event.keystroke.key == "escape"
                    && (this.daemon_pairing_qr.image.is_some() || this.daemon_pairing_qr.pending)
                {
                    this.daemon_pairing_qr.clear();
                    cx.stop_propagation();
                    cx.notify();
                }
            }));
        div()
            .id("daemon-mobile-pairing")
            .tab_group()
            .mt(px(10.0))
            .pt(px(14.0))
            .border_t_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(sp(13.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(tr!("daemon.pair_mobile")),
                            )
                            .child(
                                div()
                                    .mt(px(4.0))
                                    .whitespace_normal()
                                    .text_size(sp(12.5))
                                    .line_height(sp(17.0))
                                    .text_color(theme.text_secondary)
                                    .child(if unavailable {
                                        tr!("daemon.pairing_qr_apply_first")
                                    } else {
                                        tr!("daemon.pairing_qr_description")
                                    }),
                            ),
                    )
                    .child(button),
            )
            .when(!unavailable, |section| {
                section
                    .when(self.daemon_pairing_qr.pending, |section| {
                        section.child(
                            div()
                                .text_size(sp(12.5))
                                .text_color(theme.text_secondary)
                                .child(tr!("daemon.pairing_qr_preparing")),
                        )
                    })
                    .when_some(self.daemon_pairing_qr.image.as_ref(), |section, qr| {
                        section
                            .child(
                                div()
                                    .w(px(276.0))
                                    .flex_none()
                                    .p(px(10.0))
                                    .rounded(px(12.0))
                                    .bg(rgb(0xffffff))
                                    .child(img(qr.image.clone()).size(px(256.0))),
                            )
                            .child(
                                div()
                                    .font_family(".SystemUIFontMonospaced")
                                    .text_size(sp(12.5))
                                    .text_color(theme.text)
                                    .child(qr.address.clone()),
                            )
                            .child(
                                div()
                                    .whitespace_normal()
                                    .text_size(sp(12.5))
                                    .line_height(sp(17.0))
                                    .text_color(theme.text_secondary)
                                    .child(tr!("daemon.pairing_qr_private")),
                            )
                    })
            })
            .into_any_element()
    }
}
