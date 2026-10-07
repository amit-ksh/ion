//! Submitted output and parent review are separate. A rejected result and its
//! replacement task commit together; a review never repeats the old attempt.
use super::*;

pub struct ReviewRequest {
    pub request_id: Uuid,
    pub assignment_id: Uuid,
    pub verdict: ReviewVerdict,
    pub feedback: String,
    pub artifact_ids: Vec<Uuid>,
    pub checks: Vec<String>,
}

pub(super) fn submitted_status(work: &TeamAssignment) -> AssignmentStatus {
    if work.requires_review && work.parent_id.is_some() {
        AssignmentStatus::InReview
    } else {
        AssignmentStatus::Completed
    }
}

pub(super) fn validate_contract(contract: &OutputContract) -> Result<()> {
    checks::validate_contract(contract)?;
    ensure!(
        contract.minimum_documents <= 64
            && contract.required_formats.len() <= 16
            && contract.required_checks.len() <= 16,
        "Output contract exceeds its document/check limits"
    );
    let mut formats = HashSet::new();
    for format in &contract.required_formats {
        ensure!(
            matches!(
                format.as_str(),
                "html" | "md" | "txt" | "csv" | "json" | "pdf" | "docx" | "xlsx" | "png" | "svg"
            ),
            "Unsupported required document format"
        );
        ensure!(formats.insert(format), "Duplicate required document format");
    }
    let mut checks = HashSet::new();
    for check in &contract.required_checks {
        text(check, 256, "Required check")?;
        ensure!(checks.insert(check), "Duplicate required check");
    }
    Ok(())
}

pub(super) fn validate_output(
    db: &Connection,
    run: &TeamRunSnapshot,
    work: &TeamAssignment,
    result: &AssignmentResult,
) -> Result<()> {
    let contract = &work.output_contract;
    validate_contract(contract)?;
    checks::verify(db, run, work, result)?;
    ensure!(
        result.artifact_ids.len() >= usize::from(contract.minimum_documents),
        "Required output documents are missing"
    );
    ensure!(
        contract
            .required_checks
            .iter()
            .all(|check| result.checks.contains(check)),
        "Required declared checks are missing"
    );
    let mut formats = HashSet::new();
    for id in &result.artifact_ids {
        let body: String = db.query_row(
            "SELECT body FROM ion_control_records WHERE id=? AND kind='artifact'",
            [id.to_string()],
            |r| r.get(0),
        )?;
        let ControlRecord::Artifact(artifact) = serde_json::from_str(&body)? else {
            anyhow::bail!("Output artifact changed type")
        };
        ensure!(
            !run.work
                .iter()
                .any(|w| w.status == AssignmentStatus::Superseded
                    && artifact.task_id == Some(w.task_id)),
            "A superseded result document cannot satisfy current output; publish the corrected result"
        );
        formats.insert(artifact.format);
    }
    ensure!(
        contract
            .required_formats
            .iter()
            .all(|format| formats.contains(format)),
        "Required output document formats are missing"
    );
    Ok(())
}

pub(super) fn same_delegation(old: &str, new: &str) -> Result<bool> {
    let normalize = |body: &str| -> Result<serde_json::Value> {
        let mut value: serde_json::Value = serde_json::from_str(body)?;
        if let Some(object) = value.as_object_mut() {
            object
                .entry("contract")
                .or_insert(serde_json::to_value(OutputContract::default())?);
        }
        Ok(value)
    };
    Ok(normalize(old)? == normalize(new)?)
}

impl TeamCoordinator {
    /// Only a live parent lease can review its direct child's immutable result.
    /// Exact tool replay returns the same verdict and replacement task UUID.
    pub fn review_child(
        &self,
        lease: &AssignmentLease,
        request: ReviewRequest,
        now: u64,
    ) -> Result<AssignmentReview> {
        ensure!(!request.request_id.is_nil(), "Review request ID required");
        text(&request.feedback, 4096, "Review feedback")?;
        let signature = serde_json::to_string(&serde_json::json!({
            "assignment":request.assignment_id,"verdict":request.verdict,"feedback":request.feedback,
            "artifacts":request.artifact_ids,"checks":request.checks
        }))?;
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, lease.run_id)?;
        active(&run, now)?;
        let parent = checked_lease(&run, lease, now, false)?;
        if let Some((old,body)) = tx.query_row("SELECT signature,body FROM ion_team_reviews WHERE run_id=? AND parent_id=? AND request_id=?",
            params![run.id.to_string(),parent.id.to_string(),request.request_id.to_string()], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional()? {
            ensure!(old == signature, "Review ID reused with changed arguments");
            return Ok(serde_json::from_str(&body)?);
        }
        let mut child = run
            .work
            .iter()
            .find(|w| w.id == request.assignment_id && w.parent_id == Some(parent.id))
            .context("Review only your direct child assignments")?
            .clone();
        ensure!(
            child.status == AssignmentStatus::InReview && child.result.is_some(),
            "Child result is not awaiting review"
        );
        // The document collection can change after submission. Acceptance
        // validates the immutable result against the current stored evidence.
        if request.verdict == ReviewVerdict::Accepted {
            let result = child.result.as_ref().unwrap();
            validate_result(&tx, &run, &child, result)?;
            validate_output(&tx, &run, &child, result)?;
        }
        // Evidence belongs to the parent's own work or permitted handoffs. It
        // records declared observations; it is not a fabricated test receipt.
        validate_result(
            &tx,
            &run,
            parent,
            &AssignmentResult {
                executed_checks: vec![],
                summary: request.feedback.clone(),
                artifact_ids: request.artifact_ids.clone(),
                checks: request.checks.clone(),
            },
        )?;
        let mut replacement = None;
        if request.verdict == ReviewVerdict::Rework {
            let mut rounds = 0;
            let mut ancestor = child.rework_of;
            let mut seen = HashSet::from([child.id]);
            while let Some(id) = ancestor {
                ensure!(seen.insert(id), "Rework lineage contains a cycle");
                let old = run
                    .work
                    .iter()
                    .find(|w| {
                        w.id == id && w.parent_id == child.parent_id && w.agent_id == child.agent_id
                    })
                    .context("Rework lineage changed owner")?;
                rounds += 1;
                ancestor = old.rework_of;
            }
            ensure!(
                rounds < run.limits.max_rework_rounds,
                "Rework round limit reached; report a blocker with the remaining findings"
            );
            ensure!(
                run.work.len() < usize::from(run.limits.max_work_items),
                "Team assignment limit reached"
            );
            ensure!(
                run.team
                    .members
                    .iter()
                    .any(|m| m.agent_id == child.agent_id && m.parent_id == Some(parent.agent_id)),
                "Rework must retain a pinned direct report"
            );
            let mut corrected = child.clone();
            corrected.id = Uuid::new_v4();
            corrected.task_id = Uuid::new_v4();
            corrected.rework_of = Some(child.id);
            corrected.review = None;
            corrected.status = AssignmentStatus::Queued;
            corrected.reason = None;
            corrected.attempts.clear();
            corrected.result = None;
            corrected
                .dependencies
                .retain(|id| Some(*id) != child.rework_of);
            corrected.dependencies.push(child.id);
            ensure!(
                corrected.dependencies.len() <= 64,
                "Rework input count exceeds its dependency limit"
            );
            insert_work(
                &tx,
                run.id,
                &corrected,
                Some(request.request_id),
                Some(&signature),
                false,
            )?;
            // Consumers must wait for the corrected output, not consume a
            // rejected result. Active attempts cannot have consumed InReview.
            for mut consumer in run
                .work
                .iter()
                .filter(|w| {
                    w.status == AssignmentStatus::Queued && w.dependencies.contains(&child.id)
                })
                .cloned()
            {
                for dependency in &mut consumer.dependencies {
                    if *dependency == child.id {
                        *dependency = corrected.id;
                    }
                }
                tx.execute(
                    "DELETE FROM ion_team_dependencies WHERE work_id=? AND depends_on=?",
                    params![consumer.id.to_string(), child.id.to_string()],
                )?;
                tx.execute(
                    "INSERT INTO ion_team_dependencies VALUES(?,?)",
                    params![consumer.id.to_string(), corrected.id.to_string()],
                )?;
                save_work(&tx, &consumer, false)?;
                audit(
                    &tx,
                    run.id,
                    Some(consumer.id),
                    "rework_dependency",
                    &format!("Await corrected assignment {}", corrected.id),
                    now,
                )?;
            }
            replacement = Some(corrected.id);
        }
        let review = AssignmentReview {
            id: request.request_id,
            verdict: request.verdict,
            feedback: request.feedback,
            artifact_ids: request.artifact_ids,
            checks: request.checks,
            reviewer_task_id: parent.task_id,
            replacement_assignment_id: replacement,
            created_at: now,
        };
        child.status = if review.verdict == ReviewVerdict::Accepted {
            AssignmentStatus::Completed
        } else {
            AssignmentStatus::Superseded
        };
        child.reason = if review.verdict == ReviewVerdict::Rework {
            Some(review.feedback.clone())
        } else {
            None
        };
        child.review = Some(review.clone());
        if let Some(attempt) = child.attempts.last_mut() {
            attempt.status = child.status;
        }
        save_work(&tx, &child, false)?;
        tx.execute(
            "INSERT INTO ion_team_reviews VALUES(?,?,?,?,?,?)",
            params![
                child.id.to_string(),
                run.id.to_string(),
                parent.id.to_string(),
                request.request_id.to_string(),
                signature,
                serde_json::to_string(&review)?
            ],
        )?;
        bump(&tx, run.id)?;
        audit(
            &tx,
            run.id,
            Some(child.id),
            if review.verdict == ReviewVerdict::Accepted {
                "review_accepted"
            } else {
                "rework_requested"
            },
            &format!("Parent review {}; replacement {:?}", review.id, replacement),
            now,
        )?;
        settle_run(&tx, run.id)?;
        tx.commit()?;
        Ok(review)
    }
}
