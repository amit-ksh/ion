//! Starter templates for empty Teams, Workflows and Routines pages. Each one
//! is a complete definition, opened in the editor for review. Selecting one
//! never runs work: workflow and routine templates only fill the editor, and
//! team templates save their agents and team so the editor can show a real
//! hierarchy, then wait there for the user.
use super::*;
use anyhow::Context as _;

struct AgentSeed {
    name: &'static str,
    expertise: &'static str,
    instructions: &'static str,
    output: &'static str,
}

struct StageSeed {
    title: &'static str,
    instructions: &'static str,
    /// Wait for a human decision when this stage completes.
    gate: bool,
}

enum Schedule {
    /// Daily at hour:minute.
    Daily(u8, u8),
    /// Monday to Friday at hour:minute.
    Weekdays(u8, u8),
    /// One weekday (Monday = 0) at hour:minute.
    Weekly(u8, u8, u8),
    /// Every N minutes.
    Every(u32),
}

enum Blueprint {
    Team {
        lead: AgentSeed,
        members: &'static [AgentSeed],
    },
    Workflow(&'static [StageSeed]),
    Routine(Schedule),
}

struct Starter {
    title: &'static str,
    description: &'static str,
    icon: &'static str,
    instructions: &'static str,
    blueprint: Blueprint,
}

impl Starter {
    /// The stub line: what the template is made of, in a few words.
    fn summary(&self) -> String {
        match &self.blueprint {
            Blueprint::Team { members, .. } => format!("Lead + {} agents", members.len()),
            Blueprint::Workflow(stages) => {
                let gates = stages.iter().filter(|stage| stage.gate).count();
                match gates {
                    0 => format!("{} stages", stages.len()),
                    1 => format!("{} stages · 1 review gate", stages.len()),
                    n => format!("{} stages · {n} review gates", stages.len()),
                }
            }
            Blueprint::Routine(schedule) => match schedule {
                Schedule::Daily(h, m) => format!("Daily · {h:02}:{m:02}"),
                Schedule::Weekdays(h, m) => format!("Weekdays · {h:02}:{m:02}"),
                Schedule::Weekly(day, h, m) => {
                    const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
                    format!("{} · {h:02}:{m:02}", DAYS[usize::from(*day) % 7])
                }
                Schedule::Every(minutes) if minutes % 60 == 0 => {
                    format!("Every {} hours", minutes / 60)
                }
                Schedule::Every(minutes) => format!("Every {minutes} minutes"),
            },
        }
    }

    /// The names a person scans to judge fit: roles for a team, stages for
    /// a workflow, the cadence for a routine.
    fn parts(&self) -> Vec<&'static str> {
        match &self.blueprint {
            Blueprint::Team { lead, members } => std::iter::once(lead.name)
                .chain(members.iter().map(|member| member.name))
                .collect(),
            Blueprint::Workflow(stages) => stages.iter().map(|stage| stage.title).collect(),
            Blueprint::Routine(_) => Vec::new(),
        }
    }
}

const ENGINEER_OUTPUT: &str =
    "A short summary with evidence: what changed, which files, which checks ran and their results.";
const REVIEW_OUTPUT: &str =
    "Findings ranked by severity, each with the file, the evidence and a concrete fix.";
const RESEARCH_OUTPUT: &str =
    "A concise report with sources, confidence for each claim and the open questions.";

static TEAMS: [Starter; 4] = [
    Starter {
        title: "Feature crew",
        description: "Plan, build, test and review a feature end to end.",
        icon: "icons/bot.svg",
        instructions: "Plan and deliver a feature through scoped assignments. The lead splits the work by expertise, reviews each result and sends work back for rework until it meets the acceptance criteria.",
        blueprint: Blueprint::Team {
            lead: AgentSeed {
                name: "Tech lead",
                expertise: "Planning, delegation and code review",
                instructions: "Break the request into small, verifiable assignments. Delegate each one to the best-suited agent, review every submitted result against the acceptance criteria, and request rework with specific feedback when it falls short.",
                output: "A delivery summary: what was built, who did what, review outcomes and remaining risks.",
            },
            members: &[
                AgentSeed {
                    name: "Implementer",
                    expertise: "Implementation and refactoring",
                    instructions: "Implement the assigned change in the smallest complete way. Follow the project's conventions and keep unrelated code untouched.",
                    output: ENGINEER_OUTPUT,
                },
                AgentSeed {
                    name: "Test engineer",
                    expertise: "Tests and verification",
                    instructions: "Write or update tests that prove the assigned behavior, run the relevant suites and report exact results, including failures.",
                    output: ENGINEER_OUTPUT,
                },
                AgentSeed {
                    name: "Reviewer",
                    expertise: "Code review and risk",
                    instructions: "Review the change for correctness, security and maintainability. Report concrete findings with evidence; do not rewrite the change yourself.",
                    output: REVIEW_OUTPUT,
                },
            ],
        },
    },
    Starter {
        title: "Code review board",
        description: "Architecture, correctness and test reviews in one pass.",
        icon: "icons/file-diff.svg",
        instructions: "Review the project's architecture, correctness and tests. Delegate focused reviews and combine the findings into one ranked report with evidence and proposed next steps.",
        blueprint: Blueprint::Team {
            lead: AgentSeed {
                name: "Review lead",
                expertise: "Review planning and synthesis",
                instructions: "Scope the review, assign each focus area, check every finding for evidence and merge duplicates into one ranked report.",
                output: REVIEW_OUTPUT,
            },
            members: &[
                AgentSeed {
                    name: "Architecture reviewer",
                    expertise: "Module boundaries and design",
                    instructions: "Review module boundaries, dependencies and data flow. Flag coupling, duplication and missing seams, with file references.",
                    output: REVIEW_OUTPUT,
                },
                AgentSeed {
                    name: "Correctness reviewer",
                    expertise: "Bugs and edge cases",
                    instructions: "Hunt for logic errors, unhandled edge cases and unsafe assumptions. Describe each failure scenario with concrete inputs.",
                    output: REVIEW_OUTPUT,
                },
                AgentSeed {
                    name: "Test reviewer",
                    expertise: "Test coverage and flakiness",
                    instructions: "Map risky code to its tests, find untested paths and flaky patterns, and propose the most valuable missing tests.",
                    output: REVIEW_OUTPUT,
                },
            ],
        },
    },
    Starter {
        title: "Research desk",
        description: "Gather primary sources, check them and write it up.",
        icon: "icons/search.svg",
        instructions: "Research a topic using primary sources. Split it into distinct questions, verify the evidence for each answer and produce a concise report with sources and open questions.",
        blueprint: Blueprint::Team {
            lead: AgentSeed {
                name: "Research lead",
                expertise: "Question framing and synthesis",
                instructions: "Split the topic into distinct questions, assign them, reject unsupported claims and assemble the final report.",
                output: RESEARCH_OUTPUT,
            },
            members: &[
                AgentSeed {
                    name: "Source finder",
                    expertise: "Primary sources and documentation",
                    instructions: "Find primary sources for the assigned question: official docs, specifications, papers and source code. Quote the relevant passages with links.",
                    output: RESEARCH_OUTPUT,
                },
                AgentSeed {
                    name: "Fact checker",
                    expertise: "Verification and contradictions",
                    instructions: "Check each claim against at least one independent source. Mark claims as confirmed, contradicted or unverified.",
                    output: RESEARCH_OUTPUT,
                },
                AgentSeed {
                    name: "Report writer",
                    expertise: "Clear technical writing",
                    instructions: "Turn verified findings into a short, well-structured Markdown report for a busy reader. Keep every claim tied to its source.",
                    output: "A Markdown report with a summary, findings with sources and open questions.",
                },
            ],
        },
    },
    Starter {
        title: "Bug squad",
        description: "Reproduce an issue, find the cause and prove the fix.",
        icon: "icons/changes.svg",
        instructions: "Investigate a reported issue. Reproduce it first, diagnose the root cause, review the proposed fix and verify the result before marking the work complete.",
        blueprint: Blueprint::Team {
            lead: AgentSeed {
                name: "Triage lead",
                expertise: "Triage and verification",
                instructions: "Confirm the report is reproducible, assign diagnosis and fix work, and accept a fix only with a passing reproduction as evidence.",
                output: "The root cause, the fix, and the before and after evidence.",
            },
            members: &[
                AgentSeed {
                    name: "Reproducer",
                    expertise: "Minimal reproductions",
                    instructions: "Reproduce the issue as closely to the user's experience as possible and reduce it to the smallest reliable repro.",
                    output: "Exact repro steps, environment and observed versus expected behavior.",
                },
                AgentSeed {
                    name: "Debugger",
                    expertise: "Root cause analysis",
                    instructions: "Trace the repro to its root cause and propose the smallest fix that addresses the cause rather than the symptom.",
                    output: ENGINEER_OUTPUT,
                },
                AgentSeed {
                    name: "Verifier",
                    expertise: "Regression tests",
                    instructions: "Add a regression test that fails before the fix and passes after it, then run the related suites.",
                    output: ENGINEER_OUTPUT,
                },
            ],
        },
    },
];

static WORKFLOWS: [Starter; 4] = [
    Starter {
        title: "Release a project",
        description: "Verify, write notes, approve, then package.",
        icon: "icons/git-branch.svg",
        instructions: "Prepare a release of this project: verify the build, draft release notes for approval, then package the release locally without publishing it.",
        blueprint: Blueprint::Workflow(&[
            StageSeed {
                title: "Verify the build",
                instructions: "Build the project and run its test suites. Report the exact commands and results. Stop and report if anything fails.",
                gate: false,
            },
            StageSeed {
                title: "Draft release notes",
                instructions: "Summarize the changes since the last release into user-facing release notes, grouped by features, fixes and breaking changes.",
                gate: true,
            },
            StageSeed {
                title: "Package the release",
                instructions: "Create the release artifacts locally and list the exact commands a person would run to publish them. Do not publish, push tags or contact external services.",
                gate: false,
            },
        ]),
    },
    Starter {
        title: "Review and test",
        description: "Turn a change into a reviewed, tested result.",
        icon: "icons/check.svg",
        instructions: "Review the current change, run the relevant tests and summarize the result for a human decision before integration.",
        blueprint: Blueprint::Workflow(&[
            StageSeed {
                title: "Review the change",
                instructions: "Review the current change for correctness, security and maintainability. List findings with files and evidence.",
                gate: false,
            },
            StageSeed {
                title: "Run the tests",
                instructions: "Run the test suites that cover the changed code and report exact results, including any failures and flaky tests.",
                gate: false,
            },
            StageSeed {
                title: "Summarize for approval",
                instructions: "Write a short summary of the review findings and test results with a clear recommendation to integrate or rework.",
                gate: true,
            },
        ]),
    },
    Starter {
        title: "Ship a static site",
        description: "Build locally, review, then prepare the export.",
        icon: "icons/cloud-upload.svg",
        instructions: "Build and export this project as a static site locally, review the HTML and assets, and prepare the export for publishing as an Ion artifact after approval. Do not add servers, cloud builds or secrets.",
        blueprint: Blueprint::Workflow(&[
            StageSeed {
                title: "Build the export",
                instructions: "Build the project locally into a static export and list the output folder, entry file and total size.",
                gate: false,
            },
            StageSeed {
                title: "Review the output",
                instructions: "Check the exported HTML for broken links, missing assets and console errors. Report each problem with its file.",
                gate: true,
            },
            StageSeed {
                title: "Prepare to publish",
                instructions: "Bundle the reviewed export as a single HTML artifact ready to deploy from Ion's Artifacts page. Do not deploy it.",
                gate: false,
            },
        ]),
    },
    Starter {
        title: "Research and summarize",
        description: "Collect evidence, check it, write the report.",
        icon: "icons/file.svg",
        instructions: "Research the question with primary sources, check the evidence and produce a Markdown report with sources and open questions.",
        blueprint: Blueprint::Workflow(&[
            StageSeed {
                title: "Collect sources",
                instructions: "Find primary sources for the question and quote the relevant passages with links.",
                gate: false,
            },
            StageSeed {
                title: "Check the evidence",
                instructions: "Verify each claim against an independent source and mark it confirmed, contradicted or unverified.",
                gate: false,
            },
            StageSeed {
                title: "Write the report",
                instructions: "Write a concise Markdown report: summary, findings with sources, and open questions.",
                gate: true,
            },
        ]),
    },
];

static ROUTINES: [Starter; 4] = [
    Starter {
        title: "Daily project brief",
        description: "Progress, blockers and decisions waiting on you.",
        icon: "icons/list.svg",
        instructions: "Prepare a brief covering recent project progress, blockers and decisions that need attention. Link to the relevant tasks and evidence.",
        blueprint: Blueprint::Routine(Schedule::Daily(9, 0)),
    },
    Starter {
        title: "Review open work",
        description: "Find stalled tasks and missing inputs.",
        icon: "icons/search.svg",
        instructions: "Review open project work and identify stalled tasks, missing inputs and pending reviews. Recommend next steps without changing task scope or access.",
        blueprint: Blueprint::Routine(Schedule::Weekdays(16, 0)),
    },
    Starter {
        title: "Weekly summary",
        description: "Outcomes of the week and next week's priorities.",
        icon: "icons/file.svg",
        instructions: "Summarize this project's completed work, outstanding issues and suggested priorities for the coming week. Link the relevant tasks and artifacts.",
        blueprint: Blueprint::Routine(Schedule::Weekly(4, 17, 0)),
    },
    Starter {
        title: "Health check",
        description: "Run checks and surface what needs fixing.",
        icon: "icons/gauge.svg",
        instructions: "Run the project's available health checks and review recent failures. Report actionable findings with evidence. Do not deploy, change credentials or enable paid services.",
        blueprint: Blueprint::Routine(Schedule::Every(360)),
    },
];

fn choices(page: Page) -> &'static [Starter] {
    match page {
        Page::Teams => &TEAMS,
        Page::Workflows => &WORKFLOWS,
        Page::Routines => &ROUTINES,
        _ => &[],
    }
}

impl Waku {
    fn apply_starter(&mut self, starter: &'static Starter, window: &mut Window, cx: &mut Context<Self>) {
        self.control_ui.show_templates = false;
        let project = self
            .control_ui
            .filter_project
            .or(self.state.selected_project);
        let kind = match &starter.blueprint {
            Blueprint::Team { lead, members } => {
                self.install_team_starter(starter, lead, members, cx);
                return;
            }
            Blueprint::Workflow(_) => FormKind::Workflow,
            Blueprint::Routine(_) => FormKind::Routine,
        };
        self.begin_control_form(kind, cx);
        self.control_ui.project = project;
        self.control_ui
            .name
            .update(cx, |input, cx| input.set_content(starter.title, cx));
        self.control_ui
            .instructions
            .update(cx, |input, cx| input.set_content(starter.instructions, cx));
        match &starter.blueprint {
            Blueprint::Workflow(stages) => {
                for stage in stages.iter() {
                    self.add_control_stage(
                        stage.title.to_owned(),
                        stage.instructions.to_owned(),
                        stage.gate,
                        window,
                        cx,
                    );
                }
            }
            Blueprint::Routine(schedule) => {
                let calendar = |hour, minute, recurrence| CalendarSchedule {
                    time_zone: "UTC".to_owned(),
                    hour,
                    minute,
                    recurrence,
                };
                match schedule {
                    Schedule::Daily(h, m) => {
                        self.load_routine_calendar(&calendar(*h, *m, CalendarRecurrence::Daily), cx)
                    }
                    Schedule::Weekdays(h, m) => self.load_routine_calendar(
                        &calendar(*h, *m, CalendarRecurrence::Weekly {
                            days: vec![0, 1, 2, 3, 4],
                        }),
                        cx,
                    ),
                    Schedule::Weekly(day, h, m) => self.load_routine_calendar(
                        &calendar(*h, *m, CalendarRecurrence::Weekly { days: vec![*day] }),
                        cx,
                    ),
                    Schedule::Every(minutes) => self
                        .control_ui
                        .interval
                        .update(cx, |input, cx| input.set_content(minutes.to_string(), cx)),
                }
            }
            Blueprint::Team { .. } => {}
        }
        self.control_ui.name.read(cx).focus().focus(window, cx);
        cx.notify();
    }

    /// Saves the template's agents, then the team over them, each against the
    /// revision the previous save returned, and opens the team for review.
    fn install_team_starter(
        &mut self,
        starter: &'static Starter,
        lead: &'static AgentSeed,
        members: &'static [AgentSeed],
        cx: &mut Context<Self>,
    ) {
        if self.control_ui.busy {
            return;
        }
        let profile = |id, seed: &AgentSeed| {
            ControlRecord::Agent(AgentProfile {
                id,
                name: seed.name.to_owned(),
                expertise: seed.expertise.to_owned(),
                instructions: seed.instructions.to_owned(),
                provider: ProviderKind::Nebius,
                model: None,
                expected_output: seed.output.to_owned(),
                execution: AgentExecution::default(),
                access: AgentAccess::default(),
                max_parallel_tasks: 1,
            })
        };
        let lead_id = Uuid::new_v4();
        let mut records = vec![profile(lead_id, lead)];
        let mut team_members = vec![TeamMember {
            agent_id: lead_id,
            parent_id: None,
        }];
        for seed in members {
            let id = Uuid::new_v4();
            records.push(profile(id, seed));
            team_members.push(TeamMember {
                agent_id: id,
                parent_id: Some(lead_id),
            });
        }
        let team = Team {
            id: Uuid::new_v4(),
            name: starter.title.to_owned(),
            objective: starter.instructions.to_owned(),
            limits: Some(waku_client::teams::TeamRunLimits::default()),
            members: team_members,
        };
        records.push(ControlRecord::Team(team.clone()));
        let mut revision = self.control_ui.snapshot.revision;
        self.control_ui.generation += 1;
        let generation = self.control_ui.generation;
        self.control_ui.busy = true;
        self.control_ui.error = None;
        let client = self.daemon.client();
        let agent_count = members.len() + 1;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || -> anyhow::Result<ControlSnapshot> {
                        let mut installed = None;
                        for record in records {
                            match client.request(
                                Uuid::nil(),
                                Uuid::nil(),
                                Rpc::SaveWorkspaceRecord {
                                    expected_revision: revision,
                                    record,
                                },
                            )? {
                                ResponsePayload::WorkspaceControl { snapshot } => {
                                    revision = snapshot.revision;
                                    installed = Some(snapshot);
                                }
                                _ => anyhow::bail!("Unexpected workspace response"),
                            }
                        }
                        installed.context("The template had nothing to save")
                    })
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.control_ui.generation != generation {
                    return;
                }
                this.control_ui.busy = false;
                match result {
                    Ok(snapshot) => {
                        this.install_control_snapshot(snapshot);
                        this.edit_control_record(ControlRecord::Team(team), cx);
                        this.show_success_toast(format!(
                            "Added {} with {agent_count} agents. Review it, then save.",
                            starter.title
                        ));
                    }
                    Err(error) => this.control_ui.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

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
        let (heading, note) = match page {
            Page::Teams => (
                "Start with a team",
                "Each template adds a lead and three specialist agents, then opens the team so you can adjust roles, harnesses and limits.",
            ),
            Page::Workflows => (
                "Start with a workflow",
                "Each template fills in its stages and review gates. Nothing is saved or run until you save it.",
            ),
            _ => (
                "Start with a routine",
                "Each template fills in its schedule and instructions. Routines are saved paused until you enable them.",
            ),
        };
        let mut grid = div()
            .w_full()
            .max_w(px(920.0))
            .mx_auto()
            .flex()
            .flex_col()
            .gap(px(14.0))
            .child(
                div()
                    .mb(px(6.0))
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(sp(18.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child(heading),
                    )
                    .child(
                        div()
                            .max_w(px(620.0))
                            .text_size(sp(13.5))
                            .line_height(sp(20.0))
                            .text_color(theme.text_secondary)
                            .child(note),
                    ),
            );
        let busy = self.control_ui.busy;
        for (row_index, row) in choices(page).chunks(columns).enumerate() {
            let mut line = div().w_full().flex().gap(px(14.0));
            for (column, starter) in row.iter().enumerate() {
                line = line.child(self.render_starter_card(
                    starter,
                    format!("workspace-starter-{row_index}-{column}"),
                    busy,
                    &theme,
                    cx,
                ));
            }
            if row.len() < columns {
                line = line.child(div().flex_1().min_w_0());
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

    /// A template as a ticket: the stub says what it is made of, the body
    /// names it and lists its parts, and the whole card is the action.
    fn render_starter_card(
        &self,
        starter: &'static Starter,
        id: String,
        busy: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = *theme;
        let parts = starter.parts();
        let has_parts = !parts.is_empty();
        let mono = crate::md::render::MONO_FAMILY;
        let mut chips = div().flex().flex_wrap().gap(px(6.0));
        for part in parts {
            chips = chips.child(
                div()
                    .h(px(22.0))
                    .px(px(8.0))
                    .rounded_full()
                    .bg(theme.overlay)
                    .flex()
                    .items_center()
                    .text_size(sp(11.5))
                    .text_color(theme.text_secondary)
                    .child(part),
            );
        }
        div()
            .id(SharedString::from(id))
            .tab_index(0)
            .flex_1()
            .min_w_0()
            .rounded(px(14.0))
            .border_1()
            .border_color(theme.ticket_border)
            .bg(theme.ticket)
            .shadow(elevation(&theme, Elevation::Rest))
            .cursor_default()
            .when(busy, |card| card.opacity(0.6))
            .hover(move |style| {
                style
                    .border_color(theme.border_strong)
                    .shadow(elevation(&theme, Elevation::Lifted))
            })
            .focus_visible(move |style| style.border_color(theme.accent))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(36.0))
                    .px(px(16.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .font_family(mono)
                            .text_size(sp(11.5))
                            .text_color(theme.text_tertiary)
                            .child(starter.summary()),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(sp(11.5))
                            .text_color(theme.text_tertiary)
                            .child("Template"),
                    ),
            )
            .child(super::super::home::perforation(theme.border_strong))
            .child(
                div()
                    .flex_1()
                    .p(px(16.0))
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap(px(14.0))
                            .child(
                                avatar_tile(40.0, theme.accent_soft, gpui::transparent_black())
                                    .child(icon(starter.icon, 18.0, theme.accent)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.0))
                                    .child(
                                        div()
                                            .text_size(sp(14.5))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.text)
                                            .child(starter.title),
                                    )
                                    .child(
                                        div()
                                            .text_size(sp(13.0))
                                            .line_height(sp(19.0))
                                            .text_color(theme.text_secondary)
                                            .child(starter.description),
                                    ),
                            ),
                    )
                    .when(has_parts, |body| body.child(chips))
                    .child(
                        div()
                            .mt_auto()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap(px(6.0))
                            .text_size(sp(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.accent)
                            .child("Use template")
                            .child(icon("icons/arrow-right.svg", 13.0, theme.accent)),
                    ),
            )
            .on_activation(cx, move |this, window, cx| {
                if !this.control_ui.busy {
                    this.apply_starter(starter, window, cx);
                }
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirrors the daemon's workspace validation so a template can never open
    /// an editor that refuses to save.
    #[test]
    fn every_template_is_a_saveable_definition() {
        for page in [Page::Teams, Page::Workflows, Page::Routines] {
            let starters = choices(page);
            assert_eq!(starters.len(), 4, "each page offers four templates");
            for starter in starters {
                assert!(!starter.title.trim().is_empty() && starter.title.len() <= 160);
                assert!(!starter.instructions.trim().is_empty());
                match &starter.blueprint {
                    Blueprint::Team { lead, members } => {
                        assert!(members.len() < 32, "a team holds at most 32 agents");
                        for agent in std::iter::once(lead).chain(members.iter()) {
                            assert!(!agent.name.trim().is_empty() && agent.name.len() <= 160);
                            assert!(!agent.instructions.trim().is_empty());
                            assert!(agent.output.len() <= 16_000);
                        }
                    }
                    Blueprint::Workflow(stages) => {
                        assert!((1..=24).contains(&stages.len()));
                        assert!(stages.iter().all(|stage| !stage.title.trim().is_empty()
                            && !stage.instructions.trim().is_empty()));
                    }
                    Blueprint::Routine(schedule) => match schedule {
                        Schedule::Daily(h, m) | Schedule::Weekdays(h, m) => {
                            assert!(*h < 24 && *m < 60)
                        }
                        Schedule::Weekly(day, h, m) => assert!(*day < 7 && *h < 24 && *m < 60),
                        Schedule::Every(minutes) => assert!((1..=525_600).contains(minutes)),
                    },
                }
            }
        }
    }

    #[test]
    fn template_stubs_say_what_they_contain() {
        assert_eq!(TEAMS[0].summary(), "Lead + 3 agents");
        assert_eq!(WORKFLOWS[0].summary(), "3 stages · 1 review gate");
        assert_eq!(ROUTINES[0].summary(), "Daily · 09:00");
        assert_eq!(ROUTINES[2].summary(), "Fri · 17:00");
        assert_eq!(ROUTINES[3].summary(), "Every 6 hours");
        assert_eq!(TEAMS[0].parts(), ["Tech lead", "Implementer", "Test engineer", "Reviewer"]);
    }
}
