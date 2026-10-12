//! Immutable, verified document reading. Worker I/O/parsing; bounded cached native rows.
use super::*;
use crate::md::parser::{Block, InlineRun, InlineStyle};
pub(super) struct DocumentPreview {
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
    pub fn clear(&mut self) {
        self.generation += 1;
        self.artifact = None;
        self.busy = false;
        self.error = None;
        self.rendered = Rc::new(vec![]);
        self.source = Rc::new(vec![]);
        self.selection = TranscriptSelection::default();
        self.selected = 0;
        self.list.reset(0);
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
fn prepare(artifact: &Artifact, bytes: Vec<u8>) -> anyhow::Result<(Vec<Block>, Vec<Block>)> {
    anyhow::ensure!(
        bytes.len() <= 64 * 1024
            && artifact
                .evidence
                .as_ref()
                .is_some_and(|e| e.matches(&bytes)),
        "Document integrity is unavailable or changed. Refresh its evidence before previewing."
    );
    anyhow::ensure!(
        artifact.format != "pdf",
        "PDF page previews are not available yet. Use Save document to open the verified PDF."
    );
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
    Ok((reading, source))
}
impl Waku {
    pub(super) fn open_document_preview(&mut self, artifact: Artifact, cx: &mut Context<Self>) {
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
        self.control_ui.preview.clear();
        let ui = &mut self.control_ui.preview;
        ui.artifact = Some(artifact.clone());
        ui.busy = true;
        let generation = ui.generation;
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
            let _ = this.update(cx, |this, cx| {
                let ui = &mut this.control_ui.preview;
                if ui.generation != generation || ui.artifact.is_none() {
                    return;
                }
                ui.busy = false;
                match result {
                    Ok((reading, source)) => {
                        ui.reading = !reading.is_empty();
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
            ui.clear();
            cx.stop_propagation();
            cx.notify();
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
                            this.control_ui.preview.clear();
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
                        self.control_button("document-refresh", "", cx, move |this, _, cx| {
                            this.open_document_preview(refresh.clone(), cx)
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
                        "Reading and verifying document…"
                    } else if ui.error.is_some() {
                        "Preview unavailable"
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
        if !ui.rendered.is_empty() {
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
        let (reading, source) = prepare(&artifact("md", input), input.to_vec()).unwrap();
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
        let (_, source) = prepare(&a, text.as_bytes().to_vec()).unwrap();
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
        let (reading, source) = prepare(&artifact("json", text), text.to_vec()).unwrap();
        assert!(!reading.is_empty());
        assert!(!source.is_empty());
        let invalid = b"{invalid";
        let (reading, source) = prepare(&artifact("json", invalid), invalid.to_vec()).unwrap();
        assert!(reading.is_empty());
        assert!(!source.is_empty());
        let html = b"<script>fetch('https://remote.test')</script>";
        let (reading, source) = prepare(&artifact("html", html), html.to_vec()).unwrap();
        assert!(reading.is_empty());
        assert!(matches!(&source[0],Block::CodeBlock{code,..} if code.contains("<script>")));
    }
}
