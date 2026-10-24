//! Starter choices prepare editable definitions; selecting one never runs work.
use super::*;

#[derive(Clone, Copy)]
struct Starter {
    title: &'static str,
    description: &'static str,
    icon: &'static str,
    instructions: &'static str,
}

fn choices(page: Page) -> &'static [Starter] {
    match page {
        Page::Teams => &[
            Starter {
                title: "Build a feature",
                description: "Plan, implement and review with a team.",
                icon: "icons/bot.svg",
                instructions: "Plan and deliver a feature through scoped assignments. Have the lead delegate by expertise and review each result before integration.",
            },
            Starter {
                title: "Review a codebase",
                description: "Bring architecture and quality reviews together.",
                icon: "icons/file-diff.svg",
                instructions: "Review the project's architecture, correctness and tests. Delegate focused reviews and combine findings with evidence and proposed next steps.",
            },
            Starter {
                title: "Research a topic",
                description: "Gather sources and review the findings.",
                icon: "icons/search.svg",
                instructions: "Research a topic using primary sources. Delegate distinct questions, verify the evidence and produce a concise report with sources and open questions.",
            },
            Starter {
                title: "Investigate an issue",
                description: "Reproduce a problem and verify its fix.",
                icon: "icons/changes.svg",
                instructions: "Investigate a reported issue. Reproduce it, delegate focused diagnosis, review the proposed fix and verify the result before marking the work complete.",
            },
        ],
        Page::Workflows => &[
            Starter {
                title: "Release a project",
                description: "Plan checks, review and release gates.",
                icon: "icons/git-branch.svg",
                instructions: "Generate an editable release workflow for this project. Include verification and a human approval gate before publishing or any external release action.",
            },
            Starter {
                title: "Review and test",
                description: "Turn a change into a reviewed result.",
                icon: "icons/check.svg",
                instructions: "Generate an editable workflow to review changes, run relevant tests and summarize results. Assign appropriate teams and require review before integration.",
            },
            Starter {
                title: "Publish a static app",
                description: "Build locally, review and publish an export.",
                icon: "icons/cloud-upload.svg",
                instructions: "Generate an editable static publishing workflow. Build and export locally, review the HTML and assets, then require approval before publishing the artifact. Do not add server runtimes, cloud builds or runtime secrets.",
            },
            Starter {
                title: "Research and summarize",
                description: "Collect evidence and create a document.",
                icon: "icons/file.svg",
                instructions: "Generate an editable research workflow with source collection, evidence review and a final document artifact. Use connected apps only within the configured access and approval rules.",
            },
        ],
        Page::Routines => &[
            Starter {
                title: "Daily project brief",
                description: "Summarize progress and upcoming decisions.",
                icon: "icons/list.svg",
                instructions: "Prepare a brief covering recent project progress, blockers and decisions that need attention. Link to the relevant tasks and evidence.",
            },
            Starter {
                title: "Review open work",
                description: "Find tasks that need attention.",
                icon: "icons/search.svg",
                instructions: "Review open project work and identify stalled tasks, missing inputs and pending reviews. Summarize recommended next steps without changing task scope or access.",
            },
            Starter {
                title: "Weekly project summary",
                description: "Collect outcomes and plan the next steps.",
                icon: "icons/file.svg",
                instructions: "Summarize this project's completed work, outstanding issues and suggested priorities for the coming week. Include links to relevant tasks and artifacts.",
            },
            Starter {
                title: "Check project health",
                description: "Review checks and surface actionable issues.",
                icon: "icons/gauge.svg",
                instructions: "Review the project's available health checks and recent failures. Report actionable findings and supporting evidence. Do not deploy, change credentials or enable paid services.",
            },
        ],
        _ => &[],
    }
}

impl Waku {
    pub(super) fn render_control_starters(
        &self,
        page: Page,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::workspace(cx);
        let available = f32::from(window.viewport_size().width)
            - self.sidebar_rendered_width
            - self.right_panel_rendered_width
            - 56.0;
        let columns = if available >= 760.0 && self.control_ui.form.is_none() {
            2
        } else {
            1
        };
        let mut grid = div()
            .w_full()
            .max_w(px(880.0))
            .mx_auto()
            .flex()
            .flex_col()
            .gap(px(16.0));
        for (row_index, row) in choices(page).chunks(columns).enumerate() {
            let mut line = div().w_full().flex().gap(px(16.0));
            for (column, starter) in row.iter().copied().enumerate() {
                line = line.child(
                    self.control_button(
                        format!("workspace-starter-{row_index}-{column}"),
                        "",
                        cx,
                        move |this, _, cx| {
                            let project = this
                                .control_ui
                                .filter_project
                                .or(this.state.selected_project);
                            let kind = match page {
                                Page::Teams => FormKind::Team,
                                Page::Workflows => FormKind::Workflow,
                                Page::Routines => FormKind::Routine,
                                _ => return,
                            };
                            this.begin_control_form(kind, cx);
                            this.control_ui.project = project;
                            this.control_ui
                                .name
                                .update(cx, |input, cx| input.set_content(starter.title, cx));
                            this.control_ui.instructions.update(cx, |input, cx| {
                                input.set_content(starter.instructions, cx)
                            });
                            cx.notify();
                        },
                    )
                    .flex_1()
                    .min_w_0()
                    .h_auto()
                    .min_h(px(108.0))
                    .p(px(18.0))
                    .rounded(px(16.0))
                    .items_center()
                    .justify_start()
                    .gap(px(16.0))
                    .bg(theme.surface)
                    .border_color(theme.border)
                    .child(icon(starter.icon, 24.0, theme.text_secondary).flex_none())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(sp(14.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(starter.title),
                            )
                            .child(
                                div()
                                    .text_size(sp(13.0))
                                    .text_color(theme.text_secondary)
                                    .child(starter.description),
                            ),
                    ),
                );
            }
            grid = grid.child(line);
        }
        div()
            .id("workspace-starter-options")
            .w_full()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .py(px(24.0))
            .child(grid)
            .into_any_element()
    }
}
