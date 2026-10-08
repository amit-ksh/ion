//! Typed routing uses original accepted documents; no prompt text grants transitions.
use super::*;
pub(super) fn resolve(
    tx: &Transaction<'_>,
    team: &TeamRunSnapshot,
    result: &AssignmentResult,
    workflow: &Workflow,
    index: usize,
) -> Result<AutomationRoute> {
    let transition = &workflow.stages[index].transition;
    let mut receipt = AutomationRoute {
        target: transition.targets(index, workflow.stages.len())[0],
        artifact_id: None,
        sha256: None,
        matched: None,
    };
    if let WorkflowTransition::DocumentCondition {
        document,
        pointer,
        equals,
        on_match,
        on_mismatch,
    } = transition
    {
        let mut found = None;
        for id in &result.artifact_ids {
            let body: String = tx.query_row(
                "SELECT body FROM ion_control_records WHERE id=? AND kind='artifact'",
                [id.to_string()],
                |r| r.get(0),
            )?;
            let ControlRecord::Artifact(artifact) = serde_json::from_str(&body)? else {
                anyhow::bail!("Condition document changed")
            };
            if artifact.name != *document {
                continue;
            }
            ensure!(
                found.is_none(),
                "Condition document name is ambiguous; submit exactly one matching document"
            );
            ensure!(
                artifact.format == "json" && artifact.project_id == Some(team.project_id),
                "Condition requires a JSON document from this project"
            );
            let bytes = crate::control::verified_document(tx, &artifact)?;
            ensure!(bytes.len() <= 65536, "Condition document exceeds 64 KiB");
            let evidence = artifact
                .evidence
                .as_ref()
                .context("Condition document has no original-byte evidence")?;
            let value: serde_json::Value =
                serde_json::from_slice(&bytes).context("Condition document is not valid JSON")?;
            let value = value
                .pointer(pointer)
                .context("Condition JSON pointer is missing; review the stage document")?;
            ensure!(
                !value.is_object() && !value.is_array(),
                "Condition JSON pointer must resolve to a scalar"
            );
            let matched = value == equals;
            receipt.target = if matched { *on_match } else { *on_mismatch };
            receipt.artifact_id = Some(*id);
            receipt.sha256 = Some(evidence.sha256.clone());
            receipt.matched = Some(matched);
            found = Some(*id);
        }
        ensure!(
            found.is_some(),
            "Required condition JSON document is missing from the accepted result"
        );
    }
    Ok(receipt)
}
