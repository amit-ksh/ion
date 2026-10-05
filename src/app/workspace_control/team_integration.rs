//! Cached, virtualized source review. RPC and filesystem work stay off frames.
use super::*;
use crate::model::{ActivityFileChange, ActivityFileChangeStatus};
use std::sync::Arc;
use waku_client::teams::{TeamIntegrationStatus, TeamSourceChange};

pub(super) struct SourceChangeRow {
    change: TeamSourceChange,
    pub(super) lines: Vec<Arc<crate::review_diff::Line>>,
}
fn diff_body(change: &TeamSourceChange) -> String {
    let old: Vec<_> = change
        .before
        .as_deref()
        .unwrap_or("")
        .split_inclusive('\n')
        .collect();
    let new: Vec<_> = change
        .after
        .as_deref()
        .unwrap_or("")
        .split_inclusive('\n')
        .collect();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let start = prefix.saturating_sub(3);
    let end_old = (old.len() - suffix + 3).min(old.len());
    let end_new = (new.len() - suffix + 3).min(new.len());
    let mut body = format!(
        "@@ -{},{} +{},{} @@\n",
        if old.is_empty() { 0 } else { start + 1 },
        end_old - start,
        if new.is_empty() { 0 } else { start + 1 },
        end_new - start
    );
    let mut append = |marker: char, lines: &[&str]| {
        for line in lines {
            body.push(marker);
            body.push_str(line);
            if !line.ends_with('\n') {
                body.push_str("\n\\ No newline at end of file\n");
            }
        }
    };
    append(' ', &old[start..prefix]);
    append('-', &old[prefix..old.len() - suffix]);
    append('+', &new[prefix..new.len() - suffix]);
    append(' ', &old[old.len() - suffix..end_old]);
    body
}
fn preview_note() -> Arc<crate::review_diff::Line> {
    Arc::new(crate::review_diff::Line {
        file_index: 0,
        old_line: None,
        new_line: None,
        kind: crate::review_diff::LineKind::Meta,
        content: "Preview shortened. Save review to inspect the full before/after content.".into(),
        tokens: vec![],
    })
}
pub(super) fn presentation(changes: &[TeamSourceChange]) -> Vec<Arc<SourceChangeRow>> {
    let mut remaining = 50_000usize;
    changes
        .iter()
        .map(|change| {
            let lines = if change.directory {
                vec![]
            } else if remaining == 0 {
                vec![preview_note()]
            } else {
                let status = if change.before.is_none() {
                    ActivityFileChangeStatus::Added
                } else if change.after.is_none() {
                    ActivityFileChangeStatus::Deleted
                } else {
                    ActivityFileChangeStatus::Modified
                };
                let snapshot = crate::review_diff::from_file_changes(&[ActivityFileChange {
                    path: change.path.clone(),
                    additions: None,
                    deletions: None,
                    status: Some(status),
                    diff: Some(diff_body(change)),
                }]);
                let mut lines: Vec<_> = snapshot
                    .lines
                    .into_iter()
                    .filter(|line| !matches!(line.kind, crate::review_diff::LineKind::FileHeader))
                    .map(Arc::new)
                    .collect();
                let shortened = lines.len() > remaining;
                lines.truncate(remaining);
                remaining = remaining.saturating_sub(lines.len());
                if shortened {
                    lines.push(preview_note());
                }
                lines
            };
            Arc::new(SourceChangeRow {
                change: change.clone(),
                lines,
            })
        })
        .collect()
}
impl Waku {
    pub(super) fn clear_team_integration(&mut self) {
        self.control_ui.integration_generation += 1;
        self.control_ui.integration = None;
        self.control_ui.integration_rows = Rc::new(vec![]);
        self.control_ui.integration_busy = false;
        self.control_ui.integration_error = None;
        self.control_ui.integration_expanded.clear();
        self.control_ui.activity_revision += 1;
    }
    pub(super) fn request_team_integration(&mut self, command: Rpc, cx: &mut Context<Self>) {
        if self.control_ui.integration_busy {
            return;
        }
        let run = self.control_ui.selected_run;
        self.control_ui.integration_generation += 1;
        let generation = self.control_ui.integration_generation;
        self.control_ui.integration_busy = true;
        self.control_ui.integration_error = None;
        let client = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || -> anyhow::Result<_> {
                        let ResponsePayload::TeamIntegration { review } =
                            client.request(Uuid::nil(), Uuid::nil(), command)?
                        else {
                            anyhow::bail!("Unexpected source review response")
                        };
                        let rows = presentation(&review.changes);
                        Ok((review, rows))
                    })
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.control_ui.integration_generation != generation
                    || this.control_ui.selected_run != run
                {
                    return;
                }
                this.control_ui.integration_busy = false;
                match result {
                    Ok((review, rows)) if Some(review.run_id) == run => {
                        this.control_ui.integration = Some(Arc::new(review));
                        this.control_ui.integration_rows = Rc::new(rows);
                        this.control_ui.activity_revision += 1;
                    }
                    Ok(_) => {
                        this.control_ui.integration_error =
                            Some("Unexpected source review response. Reopen the review.".into())
                    }
                    Err(error) => this.control_ui.integration_error = Some(error.to_string()),
                }
                this.refresh_team_activity(true, cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn save_team_integration_review(&mut self, cx: &mut Context<Self>) {
        let Some(review) = self.control_ui.integration.clone() else {
            return;
        };
        let receiver = cx.prompt_for_new_path(
            &PathBuf::from("."),
            Some(&format!("Team-changes-{}.json", review.run_id)),
        );
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = receiver.await else {
                return;
            };
            let result = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || -> anyhow::Result<()> {
                        std::fs::write(path, serde_json::to_vec_pretty(&*review)?)?;
                        Ok(())
                    })
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(()) => this.show_toast("Review saved"),
                    Err(error) => this.control_ui.integration_error = Some(error.to_string()),
                };
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn render_team_integration_header(&self, cx: &mut Context<Self>) -> Div {
        let review = self.control_ui.integration.as_ref().unwrap();
        let theme = Theme::current(cx);
        let id = review.id;
        let revision = review.revision;
        let status = match review.status {
            TeamIntegrationStatus::Ready => "Ready for your review",
            TeamIntegrationStatus::Applying => "Applying source changes",
            TeamIntegrationStatus::Uncertain => "Source needs reconciliation",
            TeamIntegrationStatus::Applied => "Applied to source; changes remain uncommitted",
            TeamIntegrationStatus::Reverted => "Application canceled; original source restored",
        };
        let mut actions = div()
            .flex()
            .flex_wrap()
            .gap(px(8.0))
            .child(self.control_button(
                "team-review-close",
                "Back to assignments",
                cx,
                |this, _, cx| {
                    this.clear_team_integration();
                    cx.notify();
                },
            ));
        actions = actions.child(self.control_button(
            "team-review-save",
            "Save review",
            cx,
            |this, _, cx| this.save_team_integration_review(cx),
        ));
        if review.status == TeamIntegrationStatus::Ready
            && review.conflicts.is_empty()
            && !self.control_ui.integration_busy
        {
            actions = actions.child(self.control_button(
                "team-review-apply",
                "Apply reviewed changes",
                cx,
                move |this, _, cx| {
                    this.request_team_integration(
                        Rpc::ApplyTeamIntegration {
                            id,
                            expected_revision: revision,
                        },
                        cx,
                    )
                },
            ));
        }
        if review.status == TeamIntegrationStatus::Uncertain && !self.control_ui.integration_busy {
            actions = actions.child(self.control_button(
                "team-review-reconcile",
                "Confirm repaired source",
                cx,
                move |this, _, cx| {
                    this.request_team_integration(
                        Rpc::ReconcileTeamIntegration {
                            id,
                            expected_revision: revision,
                            keep_changes: true,
                        },
                        cx,
                    )
                },
            ));
        }
        if review.status == TeamIntegrationStatus::Uncertain && !self.control_ui.integration_busy {
            actions = actions.child(self.control_button(
                "team-review-revert",
                "Confirm restored source",
                cx,
                move |this, _, cx| {
                    this.request_team_integration(
                        Rpc::ReconcileTeamIntegration {
                            id,
                            expected_revision: revision,
                            keep_changes: false,
                        },
                        cx,
                    )
                },
            ));
        }
        if let Some(run) = &self.control_ui.team_run {
            let run_id = run.id;
            let expected_revision = run.revision;
            actions = actions.child(self.control_button(
                "team-review-refresh",
                "Refresh review",
                cx,
                move |this, _, cx| {
                    this.request_team_integration(
                        Rpc::PrepareTeamIntegration {
                            run_id,
                            expected_revision,
                        },
                        cx,
                    )
                },
            ));
        }
        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .p(px(12.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(theme.border)
            .child(
                div()
                    .text_size(sp(14.0))
                    .font_weight(FontWeight::MEDIUM)
                    .child(format!(
                        "{} · {} changes · {} applied",
                        status,
                        review.changes.len(),
                        review.applied_changes
                    )),
            )
            .child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_secondary)
                    .child(format!("Source: {}", review.source_path.display())),
            )
            .child(actions);
        if self.control_ui.integration_busy {
            body = body.child(div().text_size(sp(13.0)).child("Checking source…"));
        }
        if !review.conflicts.is_empty() {
            body = body.child(
                div()
                    .text_size(sp(13.0))
                    .text_color(theme.danger)
                    .child(format!(
                        "{} source conflicts; resolve them before applying",
                        review.conflicts.len()
                    )),
            );
        }
        if review.status == TeamIntegrationStatus::Uncertain {
            body=body.child(div().text_size(sp(13.0)).child("Inspect the source and any private .ion-edit backup files. Repair to the reviewed result or restore the original source, and preserve unexpected backup content before confirming. Application will not repeat automatically."));
        }
        body
    }
    pub(super) fn render_team_source_change(
        &self,
        index: usize,
        row: Arc<SourceChangeRow>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        let change = &row.change;
        let expanded = self.control_ui.integration_expanded.contains(&index);
        let operation = if change.directory {
            "Create directory"
        } else if change.after.is_none() {
            "Delete file"
        } else if change.before.is_none() {
            "Create file"
        } else {
            "Update file"
        };
        let mut body = div()
            .id(SharedString::from(format!("source-change-{index}")))
            .py(px(14.0))
            .flex()
            .flex_col()
            .gap(px(10.0))
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(sp(14.0))
                            .font_weight(FontWeight::MEDIUM)
                            .child(change.path.clone()),
                    )
                    .child(
                        div()
                            .text_size(sp(12.5))
                            .text_color(theme.text_secondary)
                            .child(operation),
                    ),
            );
        if !change.directory {
            body = body.child(self.control_button(
                format!("source-change-expand-{index}"),
                if expanded {
                    "Hide changes"
                } else {
                    "Show changes"
                },
                cx,
                move |this, _, cx| {
                    if !this.control_ui.integration_expanded.remove(&index) {
                        this.control_ui.integration_expanded.insert(index);
                    }
                    this.control_ui.activity_revision += 1;
                    cx.notify();
                },
            ));
        }
        body.into_any_element()
    }
    pub(super) fn render_team_source_diff_line(
        &self,
        index: usize,
        line: Arc<crate::review_diff::Line>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        if matches!(
            line.kind,
            crate::review_diff::LineKind::Context
                | crate::review_diff::LineKind::Addition
                | crate::review_diff::LineKind::Deletion
        ) {
            return super::super::right_panel::render_diff_code_row(
                &line,
                index,
                "teams-source-diff",
                &self.right_panel_diff_selection,
                super::super::right_panel::DiffRowStyle::review(self.state.code_font_size),
                &theme,
            );
        }
        div()
            .py(px(4.0))
            .font_family(crate::md::render::MONO_FAMILY)
            .text_size(sp(12.5))
            .text_color(theme.text_secondary)
            .child(line.content.clone())
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn change(before: &str, after: &str) -> TeamSourceChange {
        TeamSourceChange {
            path: "src/example.rs".into(),
            directory: false,
            before: Some(before.into()),
            after: Some(after.into()),
            edit_ids: vec![],
        }
    }
    #[test]
    fn source_review_diff_preserves_line_numbers_and_missing_newline() {
        let change = change("one\ntwo\nlast", "one\ntwo\nnext\n");
        let body = diff_body(&change);
        assert!(body.contains("\\ No newline at end of file"));
        let snapshot = crate::review_diff::from_file_changes(&[ActivityFileChange {
            path: change.path.clone(),
            additions: None,
            deletions: None,
            status: Some(ActivityFileChangeStatus::Modified),
            diff: Some(body),
        }]);
        assert!(
            snapshot
                .lines
                .iter()
                .any(|l| l.kind == crate::review_diff::LineKind::Deletion
                    && l.content == "last"
                    && l.old_line == Some(3))
        );
        assert!(
            snapshot
                .lines
                .iter()
                .any(|l| l.kind == crate::review_diff::LineKind::Addition
                    && l.content == "next"
                    && l.new_line == Some(3))
        );
    }
    #[test]
    fn source_review_preview_caps_cached_lines_and_retains_exact_manifest() {
        let mut change = change(&"\n".repeat(16 * 1024), &"x\n".repeat(8 * 1024));
        let changes = (0..4)
            .map(|index| {
                change.path = format!("src/file-{index}.txt");
                change.clone()
            })
            .collect();
        let review = waku_client::teams::TeamIntegration {
            id: Uuid::new_v4(),
            run_id: Uuid::new_v4(),
            revision: 1,
            source_path: PathBuf::from("source"),
            base_commit: "base".into(),
            branch: "team".into(),
            status: TeamIntegrationStatus::Ready,
            changes,
            conflicts: vec![],
            applied_changes: 0,
            created_at: 1,
        };
        let rows = presentation(&review.changes);
        assert!(rows.iter().map(|r| r.lines.len()).sum::<usize>() <= 50_000 + rows.len());
        assert!(rows.iter().any(|r| {
            r.lines
                .iter()
                .any(|l| l.content.contains("Preview shortened"))
        }));
        assert_eq!(
            rows.last().unwrap().change.after,
            review.changes.last().unwrap().after
        );
        assert_eq!(
            review.changes.last().unwrap().after.as_ref().unwrap().len(),
            16 * 1024
        );
    }
}
