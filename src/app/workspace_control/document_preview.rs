//! Immutable, verified document reading. Worker I/O/parsing; bounded cached native rows.
use super::*;
use crate::md::parser::{Block, InlineRun, InlineStyle};
use gpui::Subscription;
struct PdfUi {
    bytes: std::sync::Arc<Vec<u8>>,
    raster: crate::document_render::PdfRaster,
    zoom: u32,
}
struct PreparedDocument {
    reading: Vec<Block>,
    source: Vec<Block>,
    html: Option<crate::document_html::DocumentHtml>,
    pdf: Option<PdfUi>,
}
pub(super) struct DocumentPreview {
    browser: Option<Entity<crate::browser::BrowserView>>,
    browser_observer: Option<Subscription>,
    browser_closed: Option<Subscription>,
    pdf: Option<PdfUi>,
    pdf_scroll: gpui::ScrollHandle,
    pub artifact: Option<Artifact>,
    generation: u64,
    busy: bool,
    error: Option<String>,
    reading: bool,
    rendered: Rc<Vec<MarkdownView>>,
    source: Rc<Vec<MarkdownView>>,
    list: ListState,
    focus: FocusHandle,
    selection: TranscriptSelection,
    selected: usize,
}
impl DocumentPreview {
    pub fn new(cx: &mut App) -> Self {
        Self {
            browser: None,
            browser_observer: None,
            browser_closed: None,
            pdf: None,
            pdf_scroll: gpui::ScrollHandle::new(),
            artifact: None,
            generation: 0,
            busy: false,
            error: None,
            reading: false,
            rendered: Rc::new(vec![]),
            source: Rc::new(vec![]),
            list: ListState::new(0, ListAlignment::Top, px(80.0)),
            focus: cx.focus_handle(),
            selection: TranscriptSelection::default(),
            selected: 0,
        }
    }
    pub fn clear(&mut self, cx: &mut App) {
        if let Some(pdf) = self.pdf.take() {
            cx.drop_image(pdf.raster.image, None);
        }
        if let Some(browser) = self.browser.take() {
            browser.update(cx, |view, cx| view.sync_native_state(false, false, cx));
        }
        self.browser_observer = None;
        self.browser_closed = None;
        self.generation += 1;
        self.artifact = None;
        self.busy = false;
        self.error = None;
        self.rendered = Rc::new(vec![]);
        self.source = Rc::new(vec![]);
        self.selection = TranscriptSelection::default();
        self.selected = 0;
        self.list.reset(0);
        self.pdf_scroll.set_offset(gpui::point(px(0.0), px(0.0)));
    }
    fn rows(&self) -> Rc<Vec<MarkdownView>> {
        if self.reading {
            self.rendered.clone()
        } else {
            self.source.clone()
        }
    }
}
fn plain(text: String) -> Block {
    Block::Paragraph {
        runs: vec![InlineRun {
            text,
            style: InlineStyle::default(),
        }],
    }
}
// Reading does not fetch images or make document links into app commands.
fn inert(block: &mut Block) {
    match block {
        Block::Image { alt, .. } => *block = plain(format!("Image: {alt}")),
        Block::Paragraph { runs } | Block::Heading { runs, .. } => {
            for run in runs {
                run.style.link = None;
            }
        }
        Block::List { items, .. } => {
            for item in items {
                for child in &mut item.blocks {
                    inert(child);
                }
            }
        }
        Block::BlockQuote { children } => {
            for child in children {
                inert(child);
            }
        }
        Block::Table { header, rows, .. } => {
            for cell in header
                .iter_mut()
                .chain(rows.iter_mut().flat_map(|row| row.iter_mut()))
            {
                for run in cell {
                    run.style.link = None;
                }
            }
        }
        _ => {}
    }
}
fn source_blocks(source: &str, language: Option<&str>) -> Vec<Block> {
    let chars: Vec<char> = source.chars().collect();
    chars
        .chunks(1000)
        .map(|part| Block::CodeBlock {
            language: language.map(str::to_owned),
            code: part.iter().collect(),
        })
        .collect()
}
fn prepare(artifact: &Artifact, bytes: Vec<u8>) -> anyhow::Result<PreparedDocument> {
    anyhow::ensure!(
        bytes.len() <= 64 * 1024
            && artifact
                .evidence
                .as_ref()
                .is_some_and(|e| e.matches(&bytes)),
        "Document integrity is unavailable or changed. Refresh its evidence before previewing."
    );
    if artifact.format == "pdf" {
        let bytes = std::sync::Arc::new(bytes);
        let raster = crate::document_render::render_pdf(&bytes, 0, 1200)?;
        return Ok(PreparedDocument {
            reading: vec![],
            source: vec![],
            html: None,
            pdf: Some(PdfUi {
                bytes,
                raster,
                zoom: 1200,
            }),
        });
    }
    let text =
        String::from_utf8(bytes).map_err(|_| anyhow::anyhow!("Document is not valid UTF-8"))?;
    let source = source_blocks(&text, Some(&artifact.format));
    let reading = match artifact.format.as_str() {
        "md" => crate::md::parser::parse(&text)
            .blocks
            .into_iter()
            .flat_map(|top| {
                let raw = &text[top.range];
                if raw.len() > 4096 {
                    source_blocks(raw, None)
                } else {
                    let mut block = top.block;
                    inert(&mut block);
                    vec![block]
                }
            })
            .collect(),
        "json" => match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(value) => source_blocks(&serde_json::to_string_pretty(&value)?, Some("json")),
            Err(_) => vec![], // Invalid authored JSON keeps its verified original source inspectable.
        },
        _ => vec![],
    };
    let html = if artifact.format == "html" {
        Some(crate::document_html::prepare(&text)?)
    } else {
        None
    };
    Ok(PreparedDocument {
        reading,
        source,
        html,
        pdf: None,
    })
}
impl Waku {
    pub(super) fn open_document_preview(
        &mut self,
        artifact: Artifact,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.control_ui.preview.busy
            && self
                .control_ui
                .preview
                .artifact
                .as_ref()
                .is_some_and(|current| current.id == artifact.id)
        {
            return;
        }
        self.control_ui.preview.clear(cx);
        let ui = &mut self.control_ui.preview;
        ui.artifact = Some(artifact.clone());
        ui.busy = true;
        let generation = ui.generation;
        let window_handle = window.window_handle();
        let client = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || -> anyhow::Result<_> {
                        let ResponsePayload::BlobData { bytes } = client.request(
                            Uuid::nil(),
                            Uuid::nil(),
                            Rpc::ReadWorkspaceArtifact { id: artifact.id },
                        )?
                        else {
                            anyhow::bail!("Unexpected document response");
                        };
                        prepare(&artifact, bytes)
                    })
                    .await
                })
                .await;
            let _ = window_handle.update(cx, |_, window, cx| {
                this.update(cx, |this, cx| {
                    let ui = &mut this.control_ui.preview;
                    if ui.generation != generation || ui.artifact.is_none() {
                        return;
                    }
                    ui.busy = false;
                    match result {
                        Ok(PreparedDocument {
                            reading,
                            source,
                            html,
                            pdf,
                        }) => {
                            ui.reading = !reading.is_empty() || html.is_some() || pdf.is_some();
                            ui.pdf = pdf;
                            if let Some(html) = html {
                                let browser = cx.new(|cx| {
                                    crate::browser::BrowserView::new_document(html, window, cx)
                                });
                                ui.browser_observer =
                                    Some(cx.observe(&browser, |_, _, cx| cx.notify()));
                                ui.browser_closed = Some(cx.subscribe(
                                    &browser,
                                    |this, _, _: &crate::browser::DocumentClosed, cx| {
                                        this.control_ui.preview.clear(cx);
                                        cx.notify();
                                    },
                                ));
                                ui.browser = Some(browser);
                            }
                            ui.rendered = Rc::new(
                                reading
                                    .into_iter()
                                    .map(MarkdownView::document_block)
                                    .collect(),
                            );
                            ui.source = Rc::new(
                                source
                                    .into_iter()
                                    .map(MarkdownView::document_block)
                                    .collect(),
                            );
                            ui.list.reset(ui.rows().len());
                        }
                        Err(e) => ui.error = Some(e.to_string()),
                    }
                    cx.notify();
                })
            });
        })
        .detach();
        cx.notify();
    }
    fn document_preview_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        if event.keystroke.modifiers.control || event.keystroke.modifiers.platform {
            return;
        }
        let ui = &mut self.control_ui.preview;
        if event.keystroke.key == "escape" {
            ui.clear(cx);
            cx.stop_propagation();
            cx.notify();
            return;
        }
        if let Some(pdf) = &ui.pdf {
            let (page, pages, zoom) = (pdf.raster.page, pdf.raster.pages, pdf.zoom);
            if matches!(event.keystroke.key.as_str(), "up" | "down") {
                let mut offset = self.control_ui.preview.pdf_scroll.offset();
                offset.y += px(if event.keystroke.key == "up" {
                    120.0
                } else {
                    -120.0
                });
                self.control_ui.preview.pdf_scroll.set_offset(offset);
                cx.stop_propagation();
                cx.notify();
                return;
            }
            let selected = match event.keystroke.key.as_str() {
                "pageup" | "left" => page.saturating_sub(1),
                "pagedown" | "right" => (page + 1).min(pages - 1),
                "home" => 0,
                "end" => pages - 1,
                _ => return,
            };
            if selected != page {
                self.load_pdf_page(selected, zoom, cx);
            }
            cx.stop_propagation();
            return;
        }
        let count = ui.rows().len();
        if count == 0 {
            return;
        }
        ui.selected = match event.keystroke.key.as_str() {
            "down" => (ui.selected + 1).min(count - 1),
            "up" => ui.selected.saturating_sub(1),
            "home" => 0,
            "end" => count - 1,
            "pagedown" => (ui.selected + 4).min(count - 1),
            "pageup" => ui.selected.saturating_sub(4),
            _ => return,
        };
        ui.list.scroll_to_reveal_item(ui.selected);
        cx.stop_propagation();
        cx.notify();
    }
    pub(super) fn render_document_preview(&self, cx: &mut Context<Self>) -> AnyElement {
        let ui = &self.control_ui.preview;
        let artifact = ui.artifact.as_ref().expect("preview selected");
        let theme = Theme::current(cx);
        let record = ControlRecord::Artifact(artifact.clone());
        let refresh = artifact.clone();
        let mut body = div()
            .track_focus(&self.control_ui.focus)
            .tab_group()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .p(px(24.0))
            .on_key_down(cx.listener(|this, event, _, cx| this.document_preview_key(event, cx)))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.0))
                    .child(
                        self.control_button("document-back", "Back", cx, |this, _, cx| {
                            this.control_ui.preview.clear(cx);
                            cx.notify();
                        }),
                    )
                    .child(self.control_button(
                        "document-save",
                        "Save document",
                        cx,
                        move |this, _, cx| this.export_control_artifact(record.clone(), cx),
                    ))
                    .child(
                        self.control_button("document-refresh", "", cx, move |this, window, cx| {
                            this.open_document_preview(refresh.clone(), window, cx)
                        })
                        .tooltip(Tooltip::text("Refresh document"))
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
                    .child(artifact.name.clone()),
            )
            .child(
                div()
                    .text_size(sp(13.0))
                    .text_color(theme.text_secondary)
                    .child(if ui.busy {
                        "Preparing document…"
                    } else if ui.error.is_some() {
                        "Preview unavailable"
                    } else if ui.browser.is_some() || ui.pdf.is_some() {
                        "Verified document · Preview"
                    } else if ui.reading {
                        "Verified document · Reading view"
                    } else {
                        "Verified document · Source"
                    }),
            );
        if let Some(error) = &ui.error {
            return body
                .child(div().text_color(theme.text).child(error.clone()))
                .into_any_element();
        }
        if let Some(pdf) = &ui.pdf {
            return self.render_pdf_document(body, pdf, cx);
        }
        if !ui.rendered.is_empty() || ui.browser.is_some() {
            body = body.child(div().flex().gap(px(8.0)).children(
                [("Reading view", true), ("Source", false)].map(|(label, reading)| {
                    self.control_button(
                        format!("document-mode-{reading}"),
                        label,
                        cx,
                        move |this, _, cx| {
                            let ui = &mut this.control_ui.preview;
                            ui.reading = reading;
                            ui.selection = TranscriptSelection::default();
                            ui.selected = 0;
                            ui.list.reset(ui.rows().len());
                            cx.notify();
                        },
                    )
                    .bg(if ui.reading == reading {
                        theme.overlay_strong
                    } else {
                        theme.surface
                    })
                }),
            ));
        }
        if ui.reading
            && let Some(browser) = &ui.browser
        {
            return body
                .child(div().flex_1().min_h_0().child(browser.clone()))
                .into_any_element();
        }
        let rows = ui.rows();
        let selection = ui.selection.clone();
        let id = artifact.id;
        let font = self.state.ui_font_size;
        let code_font = self.state.code_font_size;
        let input = selection.clone();
        body.child(
            canvas(
                |_, _, _| (),
                move |_, _, window, _| md::render::install_selection_input(window, &input),
            )
            .absolute()
            .w(px(0.0))
            .h(px(0.0)),
        )
        .child(md::render::frame_reset(selection.clone()))
        .child(
            div()
                .id("document-preview-list")
                .track_focus(&ui.focus)
                .tab_index(0)
                .tab_stop(true)
                .focus_visible(|style| style.border_1().border_color(theme.accent))
                .flex_1()
                .min_h_0()
                .child(
                    list(ui.list.clone(), move |index, _, cx| {
                        let Some(view) = rows.get(index) else {
                            return div().into_any_element();
                        };
                        let palette = MarkdownPalette::from_theme(&Theme::current(cx));
                        let ctx = MarkdownCtx::new(
                            format!("document-{id}-{index}"),
                            &palette,
                            MarkdownMetrics::document(font, code_font),
                            selection.clone(),
                        )
                        .with_math_enabled(false);
                        div()
                            .w_full()
                            .px(px(8.0))
                            .py(px(10.0))
                            .children(md::render::markdown(view, &ctx))
                            .into_any_element()
                    })
                    .size_full(),
                ),
        )
        .into_any_element()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn artifact(format: &str, bytes: &[u8]) -> Artifact {
        Artifact {
            id: Uuid::new_v4(),
            name: "test".into(),
            format: format.into(),
            reference: "unused".into(),
            project_id: None,
            task_id: None,
            created_at: 0,
            evidence: Some(ArtifactEvidence::from_bytes(
                bytes,
                ArtifactProvenance::default(),
            )),
        }
    }
    #[test]
    fn document_preview_requires_exact_current_evidence_and_utf8() {
        let a = artifact("md", b"# Heading");
        assert!(prepare(&a, b"# Heading".to_vec()).is_ok());
        assert!(prepare(&a, b"changed".to_vec()).is_err());
        let bad = [255];
        assert!(prepare(&artifact("md", &bad), bad.to_vec()).is_err());
        let mut legacy = a;
        legacy.evidence = None;
        assert!(prepare(&legacy, b"# Heading".to_vec()).is_err());
    }
    #[test]
    fn document_preview_has_no_active_links_or_image_loads() {
        let input =
            b"# Title\n\n[Link](https://remote.test)\n\n![diagram](https://remote.test/pixel)";
        let PreparedDocument {
            reading, source, ..
        } = prepare(&artifact("md", input), input.to_vec()).unwrap();
        assert!(!source.is_empty());
        assert!(
            reading
                .iter()
                .all(|block| !matches!(block, Block::Image { .. }))
        );
        assert!(reading.iter().any(|block| matches!(block,Block::Paragraph{runs} if runs.iter().any(|r|r.text.contains("diagram")))));
        for block in reading {
            if let Block::Paragraph { runs } = block {
                assert!(runs.iter().all(|r| r.style.link.is_none()));
            }
        }
    }
    #[test]
    fn document_preview_caps_long_unicode_rows_without_truncating_source() {
        let text = "語".repeat(21_000);
        let a = artifact("md", text.as_bytes());
        let PreparedDocument { source, .. } = prepare(&a, text.as_bytes().to_vec()).unwrap();
        let joined: String = source
            .iter()
            .map(|b| {
                if let Block::CodeBlock { code, .. } = b {
                    assert!(code.len() <= 4000);
                    code.as_str()
                } else {
                    panic!()
                }
            })
            .collect();
        assert_eq!(joined, text);
        assert!(source.len() > 10);
    }
    #[test]
    fn document_preview_pretty_json_and_inert_html_source() {
        let text = b"{\"name\":\"Ion\"}";
        let PreparedDocument {
            reading, source, ..
        } = prepare(&artifact("json", text), text.to_vec()).unwrap();
        assert!(!reading.is_empty());
        assert!(!source.is_empty());
        let invalid = b"{invalid";
        let PreparedDocument {
            reading, source, ..
        } = prepare(&artifact("json", invalid), invalid.to_vec()).unwrap();
        assert!(reading.is_empty());
        assert!(!source.is_empty());
        let html = b"<script>fetch('https://remote.test')</script>";
        let PreparedDocument {
            reading,
            source,
            html: surface,
            ..
        } = prepare(&artifact("html", html), html.to_vec()).unwrap();
        assert!(surface.is_some());
        assert!(reading.is_empty());
        assert!(matches!(&source[0],Block::CodeBlock{code,..} if code.contains("<script>")));
    }
}

impl Waku {
    pub(in crate::app) fn sync_document_webview(&mut self, overlay: bool, cx: &mut Context<Self>) {
        if let Some(browser) = self.control_ui.preview.browser.clone() {
            let visible = self.control_ui.preview.reading
                && self.control_ui.preview.artifact.is_some()
                && self.control_ui.page.is_some()
                && self.settings_page.is_none();
            browser.update(cx, |view, cx| view.sync_native_state(visible, overlay, cx));
        }
    }
    fn load_pdf_page(&mut self, page: u32, zoom: u32, cx: &mut Context<Self>) {
        let ui = &mut self.control_ui.preview;
        if ui.busy {
            return;
        }
        let Some(pdf) = &ui.pdf else {
            return;
        };
        if page >= pdf.raster.pages {
            return;
        }
        let bytes = pdf.bytes.clone();
        ui.busy = true;
        ui.error = None;
        ui.generation += 1;
        let generation = ui.generation;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || crate::document_render::render_pdf(&bytes, page, zoom))
                        .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let ui = &mut this.control_ui.preview;
                if ui.generation != generation || ui.pdf.is_none() {
                    return;
                }
                ui.busy = false;
                match result {
                    Ok(raster) => {
                        let pdf = ui.pdf.as_mut().unwrap();
                        cx.drop_image(pdf.raster.image.clone(), None);
                        pdf.raster = raster;
                        pdf.zoom = zoom;
                        ui.pdf_scroll.set_offset(gpui::point(px(0.0), px(0.0)));
                    }
                    Err(error) => ui.error = Some(format!("Could not render PDF page: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn render_pdf_document(&self, body: Div, pdf: &PdfUi, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let page = pdf.raster.page;
        let pages = pdf.raster.pages;
        let zoom = pdf.zoom;
        let mut bar = div().flex().flex_wrap().gap(px(8.0)).child(
            div()
                .text_color(theme.text)
                .child(format!("Page {} of {pages}", page + 1)),
        );
        if page > 0 {
            bar = bar.child(self.control_button(
                "pdf-previous",
                "Previous page",
                cx,
                move |this, _, cx| this.load_pdf_page(page - 1, zoom, cx),
            ));
        }
        if page + 1 < pages {
            bar = bar.child(self.control_button(
                "pdf-next",
                "Next page",
                cx,
                move |this, _, cx| this.load_pdf_page(page + 1, zoom, cx),
            ));
        }
        for width in [800, 1200, 1600] {
            bar = bar.child(
                self.control_button(
                    format!("pdf-zoom-{width}"),
                    format!("{}%", width * 100 / 1200),
                    cx,
                    move |this, _, cx| this.load_pdf_page(page, width, cx),
                )
                .when(width == zoom, |b| b.bg(theme.overlay)),
            );
        }
        body.child(bar)
            .child(
                div()
                    .id("pdf-page")
                    .track_focus(&self.control_ui.preview.focus)
                    .tab_index(0)
                    .tab_stop(true)
                    .focus_visible(|style| style.border_1().border_color(theme.accent))
                    .flex_1()
                    .min_h_0()
                    .overflow_scroll()
                    .track_scroll(&self.control_ui.preview.pdf_scroll)
                    .child(
                        gpui::img(pdf.raster.image.clone())
                            .w(px(pdf.raster.width as f32))
                            .h(px(pdf.raster.height as f32)),
                    ),
            )
            .into_any_element()
    }
}
