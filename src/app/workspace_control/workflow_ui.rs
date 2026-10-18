//! Editable, bounded conditional routes. The owner evaluates original JSON evidence.
use super::*;
#[derive(Clone, Copy, PartialEq)]
enum RouteMode {
    Next,
    GoTo,
    Condition,
}
pub(super) struct TransitionUi {
    mode: RouteMode,
    target: WorkflowTarget,
    mismatch: WorkflowTarget,
    document: Entity<TextInput>,
    pointer: Entity<TextInput>,
    value: Entity<TextInput>,
}
impl TransitionUi {
    pub(super) fn new(transition: WorkflowTransition, window: &mut Window, cx: &mut App) -> Self {
        let (mode, target, mismatch, document, pointer, value) = match transition {
            WorkflowTransition::Next => (
                RouteMode::Next,
                WorkflowTarget::Finish,
                WorkflowTarget::Finish,
                "route.json".into(),
                "/ready".into(),
                "true".into(),
            ),
            WorkflowTransition::GoTo { target } => (
                RouteMode::GoTo,
                target,
                WorkflowTarget::Finish,
                "route.json".into(),
                "/ready".into(),
                "true".into(),
            ),
            WorkflowTransition::DocumentCondition {
                document,
                pointer,
                equals,
                on_match,
                on_mismatch,
            } => (
                RouteMode::Condition,
                on_match,
                on_mismatch,
                document,
                pointer,
                equals.to_string(),
            ),
        };
        let mut input = |placeholder: &'static str, content: String, cx: &mut App| {
            cx.new(|cx| {
                let mut input = TextInput::new(window, cx).placeholder(placeholder);
                input.set_content(content, cx);
                input
            })
        };
        Self {
            mode,
            target,
            mismatch,
            document: input("route.json", document, cx),
            pointer: input("/ready", pointer, cx),
            value: input("true", value, cx),
        }
    }
    fn draft(&self, cx: &App) -> Result<WorkflowTransition, String> {
        match self.mode {
            RouteMode::Next => Ok(WorkflowTransition::Next),
            RouteMode::GoTo => Ok(WorkflowTransition::GoTo {
                target: self.target,
            }),
            RouteMode::Condition => {
                let document = self.document.read(cx).content();
                let pointer = self.pointer.read(cx).content();
                let value = self.value.read(cx).content();
                if document.len() > 128 || pointer.len() > 256 || value.len() > 1024 {
                    return Err(
                        "Keep the condition name, pointer and scalar within their field limits."
                            .into(),
                    );
                }
                let equals: serde_json::Value = serde_json::from_str(value).map_err(
                    |_| "Enter a JSON scalar: true, false, null, a number or a quoted string.",
                )?;
                if equals.is_array()
                    || equals.is_object()
                    || equals.as_str().is_some_and(|s| s.len() > 256)
                {
                    return Err(
                        "Condition values must be JSON scalars of at most 256 bytes.".into(),
                    );
                }
                Ok(WorkflowTransition::DocumentCondition {
                    document: document.to_owned(),
                    pointer: pointer.to_owned(),
                    equals,
                    on_match: self.target,
                    on_mismatch: self.mismatch,
                })
            }
        }
    }
    fn remap(&mut self, mapping: impl Fn(u16) -> u16) {
        for target in [&mut self.target, &mut self.mismatch] {
            if let WorkflowTarget::Stage { index } = target {
                *index = mapping(*index);
            }
        }
    }
}
pub(super) fn target_label(target: WorkflowTarget) -> String {
    match target {
        WorkflowTarget::Stage { index } => format!("Stage {}", index + 1),
        WorkflowTarget::Finish => "Finish workflow".into(),
    }
}
pub(super) fn transition_label(transition: &WorkflowTransition) -> String {
    match transition {
        WorkflowTransition::Next => "Next stage".into(),
        WorkflowTransition::GoTo { target } => target_label(*target),
        WorkflowTransition::DocumentCondition {
            document,
            pointer,
            equals,
            on_match,
            on_mismatch,
        } => format!(
            "{document} {pointer} = {equals} → {}; otherwise {}",
            target_label(*on_match),
            target_label(*on_mismatch)
        ),
    }
}
pub(super) fn retry_label(policy: Option<WorkflowRetryPolicy>) -> String {
    match policy {
        None => "Retries: off".into(),
        Some(p) => format!(
            "Retry before effects: up to {} · {}s delay",
            p.max_attempts.saturating_sub(1),
            p.delay_seconds
        ),
    }
}
impl Waku {
    pub(super) fn workflow_stage_drafts(&self, cx: &App) -> Result<Vec<WorkflowStage>, String> {
        self.control_ui
            .stages
            .iter()
            .map(
                |(title, instructions, gate, team, formats, route, retry, inherit_workspace)| {
                    Ok(WorkflowStage {
                        inherit_workspace: *inherit_workspace,
                        retry: *retry,
                        title: title.read(cx).content().to_owned(),
                        instructions: instructions.read(cx).content().to_owned(),
                        requires_decision: *gate,
                        team_id: *team,
                        required_formats: formats.clone(),
                        transition: route.draft(cx)?,
                    })
                },
            )
            .collect()
    }
    pub(super) fn remove_workflow_stage(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.control_ui.stages.iter().enumerate().any(|(i,s)| i!=index && s.5.mode!=RouteMode::Next && [s.5.target,s.5.mismatch].iter().any(|t|matches!(t,WorkflowTarget::Stage { index: target } if usize::from(*target)==index))) {
            self.control_ui.error=Some("Change incoming routes before removing this stage.".into()); cx.notify(); return
        }
        self.control_ui.stages.remove(index);
        for stage in &mut self.control_ui.stages {
            stage.5.remap(|old| {
                if usize::from(old) > index {
                    old - 1
                } else {
                    old
                }
            });
        }
        self.control_ui.error = None;
        cx.notify();
    }
    pub(super) fn move_workflow_stage(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if to >= self.control_ui.stages.len() {
            return;
        }
        let map = |old: u16| {
            if usize::from(old) == from {
                to as u16
            } else if usize::from(old) == to {
                from as u16
            } else {
                old
            }
        };
        let mut drafts = match self.workflow_stage_drafts(cx) {
            Ok(v) => v,
            Err(e) => {
                self.control_ui.error = Some(e);
                cx.notify();
                return;
            }
        };
        for stage in &mut drafts {
            match &mut stage.transition {
                WorkflowTransition::GoTo { target } => {
                    if let WorkflowTarget::Stage { index } = target {
                        *index = map(*index);
                    }
                }
                WorkflowTransition::DocumentCondition {
                    on_match,
                    on_mismatch,
                    ..
                } => {
                    for target in [on_match, on_mismatch] {
                        if let WorkflowTarget::Stage { index } = target {
                            *index = map(*index);
                        }
                    }
                }
                _ => {}
            }
        }
        drafts.swap(from, to);
        let draft = Workflow {
            id: Uuid::nil(),
            name: String::new(),
            project_id: Uuid::nil(),
            prompt: String::new(),
            limits: None,
            stages: drafts,
        };
        if let Err(error) = draft.validate_graph() {
            self.control_ui.error =
                Some(format!("Cannot reorder: {error}. Change the routes first."));
            cx.notify();
            return;
        }
        for stage in &mut self.control_ui.stages {
            stage.5.remap(map);
        }
        self.control_ui.stages.swap(from, to);
        self.control_ui.error = None;
        cx.notify();
    }
    fn render_workflow_target(
        &self,
        index: usize,
        mismatch: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let route = &self.control_ui.stages[index].5;
        let selected = if mismatch {
            route.mismatch
        } else {
            route.target
        };
        let count = self.control_ui.stages.len();
        let weak = cx.entity().downgrade();
        let id = SharedString::from(format!("workflow-target-{index}-{mismatch}"));
        let menu = self.menu_handle(id.clone(), cx);
        let label = if route.mode == RouteMode::Condition {
            format!(
                "{}: {}",
                if mismatch { "Otherwise" } else { "If equal" },
                target_label(selected)
            )
        } else {
            target_label(selected)
        };
        dropdown_menu(
            self.control_button(id.clone(), label, cx, |_, _, _| {}),
            id,
            &menu,
            MenuAlign::BelowLeft,
            move |_| {
                let targets = (index + 1..count)
                    .map(|index| WorkflowTarget::Stage {
                        index: index as u16,
                    })
                    .chain([WorkflowTarget::Finish]);
                targets
                    .map(|target| {
                        let weak = weak.clone();
                        MenuItem::new(target_label(target), move |_, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                if let Some(stage) = this.control_ui.stages.get_mut(index) {
                                    if mismatch {
                                        stage.5.mismatch = target
                                    } else {
                                        stage.5.target = target
                                    }
                                }
                                cx.notify();
                            });
                        })
                        .selected(target == selected)
                    })
                    .collect()
            },
        )
        .into_any_element()
    }
    pub(super) fn render_workflow_workspace(
        &self,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.control_ui.stages[index].7;
        self.control_button(
            format!("workflow-code-{index}"),
            if selected {
                "Inherit code: on"
            } else {
                "Inherit code: off"
            },
            cx,
            move |this, _, cx| {
                if index == 0 && !this.control_ui.stages.get(index).is_some_and(|s| s.7) {
                    this.control_ui.error = Some("The first stage cannot inherit code.".into());
                } else if let Some(stage) = this.control_ui.stages.get_mut(index) {
                    stage.7 = !stage.7;
                    this.control_ui.error = None;
                }
                cx.notify();
            },
        )
        .into_any_element()
    }
    pub(super) fn render_workflow_retry(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.control_ui.stages[index].6;
        let weak = cx.entity().downgrade();
        let id = SharedString::from(format!("workflow-retry-{index}"));
        let menu = self.menu_handle(id.clone(), cx);
        dropdown_menu(
            self.control_button(id.clone(), retry_label(selected), cx, |_, _, _| {}),
            id,
            &menu,
            MenuAlign::BelowLeft,
            move |_| {
                [
                    None,
                    Some(WorkflowRetryPolicy {
                        max_attempts: 2,
                        delay_seconds: 30,
                    }),
                    Some(WorkflowRetryPolicy {
                        max_attempts: 3,
                        delay_seconds: 30,
                    }),
                ]
                .into_iter()
                .map(|policy| {
                    let weak = weak.clone();
                    MenuItem::new(retry_label(policy), move |_, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            if let Some(stage) = this.control_ui.stages.get_mut(index) {
                                stage.6 = policy;
                            }
                            cx.notify();
                        });
                    })
                    .selected(policy == selected)
                })
                .collect()
            },
        )
        .into_any_element()
    }
    pub(super) fn render_workflow_route(&self, index: usize, cx: &mut Context<Self>) -> Div {
        let theme = Theme::workspace(cx);
        let r = &self.control_ui.stages[index].5;
        let mode = r.mode;
        let count = self.control_ui.stages.len();
        let weak = cx.entity().downgrade();
        let menu_id = SharedString::from(format!("workflow-route-{index}"));
        let menu = self.menu_handle(menu_id.clone(), cx);
        let mut row = div().flex().flex_col().gap(px(8.0)).child(dropdown_menu(
            self.control_button(
                menu_id.clone(),
                match mode {
                    RouteMode::Next => "After completion: next stage",
                    RouteMode::GoTo => "After completion: go to",
                    RouteMode::Condition => "After completion: JSON condition",
                },
                cx,
                |_, _, _| {},
            ),
            menu_id,
            &menu,
            MenuAlign::BelowLeft,
            move |_| {
                [
                    (RouteMode::Next, "Next stage"),
                    (RouteMode::GoTo, "Choose a later stage or finish"),
                    (RouteMode::Condition, "Choose by a JSON document value"),
                ]
                .into_iter()
                .map(|(value, label)| {
                    let weak = weak.clone();
                    MenuItem::new(label, move |_, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            if let Some(stage) = this.control_ui.stages.get_mut(index) {
                                if stage.5.mode != value {
                                    stage.5.target = if index + 1 < count {
                                        WorkflowTarget::Stage {
                                            index: (index + 1) as u16,
                                        }
                                    } else {
                                        WorkflowTarget::Finish
                                    };
                                    stage.5.mismatch = WorkflowTarget::Finish;
                                }
                                stage.5.mode = value;
                            }
                            cx.notify();
                        });
                    })
                    .selected(value == mode)
                })
                .collect()
            },
        ));
        if mode == RouteMode::Condition {
            row=row.child(control_field("JSON document name",r.document.clone(),SharedString::from(format!("workflow-condition-doc-{index}"))))
                .child(control_field("JSON pointer",r.pointer.clone(),SharedString::from(format!("workflow-condition-path-{index}"))))
                .child(control_field("Equals (JSON scalar)",r.value.clone(),SharedString::from(format!("workflow-condition-value-{index}"))))
                .child(div().text_size(sp(12.5)).text_color(theme.text_secondary).child("The stage must publish this JSON document. A missing or invalid value blocks the run for review."));
        }
        if mode != RouteMode::Next {
            row = row.child(self.render_workflow_target(index, false, cx));
        }
        if mode == RouteMode::Condition {
            row = row.child(self.render_workflow_target(index, true, cx));
        }
        row
    }
}
