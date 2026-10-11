//! Match private pending calls to durable argument-bound coordinator effects.
use super::*;
pub(super) fn tool(
    db: &Connection,
    run: &TeamRunSnapshot,
    work: &TeamAssignment,
    attempt: Uuid,
    request: Uuid,
    name: &str,
    args: &Value,
) -> Result<String> {
    match name {
        "run_remote_command" | "run_remote_project_command" => {
            if let Some(receipt) =
                super::super::jobs::recover_receipt(db, run, work, attempt, request, args)?
            {
                return Ok(receipt);
            }
        }
        "write_file" | "create_directory" | "delete_file" => {
            let row:Option<(String,String,Option<String>)>=db.query_row(
                "SELECT signature,state,receipt FROM ion_team_workspace_edits WHERE run_id=? AND work_id=? AND attempt_id=? AND request_id=?",
                params![run.id.to_string(),work.id.to_string(),attempt.to_string(),request.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
            if let Some((signature, state, receipt)) = row {
                let s: Value = serde_json::from_str(&signature)?;
                let operation = s["operation"].as_str().unwrap_or("writeFile");
                let expected = match name {
                    "write_file" => {
                        ensure!(operation == "writeFile", "File receipt operation differs");
                        json!({"path":s["path"],"old_content":s["old"],"new_content":s["new"]})
                    }
                    "create_directory" => {
                        ensure!(
                            operation == "createDirectory",
                            "Directory receipt operation differs"
                        );
                        json!({"path":s["path"]})
                    }
                    _ => {
                        ensure!(
                            operation == "deleteFile",
                            "Delete receipt operation differs"
                        );
                        json!({"path":s["path"],"old_content":s["old"]})
                    }
                };
                ensure!(
                    *args == expected,
                    "Pending file call differs from its durable reservation"
                );
                ensure!(
                    matches!(state.as_str(), "applied" | "restored"),
                    "File effect is still uncertain"
                );
                let receipt: Value =
                    serde_json::from_str(&receipt.context("File receipt missing")?)?;
                return Ok(json!({"recovered_receipt":receipt,"outcome":state,"replayed":false,
                    "note":if state=="restored" {"Original content restored by user; the requested edit was not reapplied"}else{"Durable effect acknowledged; no edit was repeated"}}).to_string());
            }
        }
        "delegate" => {
            let row:Option<(String,String)>=db.query_row("SELECT request,body FROM ion_team_work WHERE run_id=? AND parent_id=? AND request_id=?",
                params![run.id.to_string(),work.id.to_string(),request.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            if let Some((signature, body)) = row {
                let s: Value = serde_json::from_str(&signature)?;
                let contract = args
                    .get("output_contract")
                    .filter(|v| !v.is_null())
                    .cloned()
                    .unwrap_or(serde_json::to_value(OutputContract {
                        required_commands: vec![],
                        minimum_documents: 1,
                        ..Default::default()
                    })?);
                ensure!(
                    s["agent"] == args["agent_id"]
                        && s["objective"] == args["objective"]
                        && s["output"] == args["expected_output"]
                        && s["dependencies"]
                            == args.get("dependencies").cloned().unwrap_or(json!([]))
                        && s["contract"] == contract,
                    "Pending delegation differs from its durable receipt"
                );
                let child: TeamAssignment = serde_json::from_str(&body)?;
                return Ok(json!({"assignment_id":child.id,"task_id":child.task_id,"status":child.status,"replayed":false}).to_string());
            }
        }
        "save_document" | "save_pdf" | "publish_change" => {
            let row:Option<(String,String,String,Vec<u8>)>=db.query_row("SELECT artifact_id,name,format,content FROM ion_team_document_receipts WHERE work_id=? AND request_id=?",
                params![work.id.to_string(),request.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
            if let Some((id, file, format, bytes)) = row {
                if name == "save_document" {
                    ensure!(
                        args["name"] == file
                            && args["format"] == format
                            && args["content"]
                                .as_str()
                                .is_some_and(|s| s.as_bytes() == bytes),
                        "Pending document differs from its durable receipt"
                    );
                } else if name == "publish_change" {
                    let change: Value = serde_json::from_slice(&bytes)?;
                    ensure!(
                        change["path"] == args["path"]
                            && file == format!("Change-{}.json", request.simple())
                            && format == "json",
                        "Pending change document differs from its receipt"
                    );
                }
                let body: String = db
                    .query_row(
                        "SELECT body FROM ion_control_records WHERE id=? AND kind='artifact'",
                        [&id],
                        |r| r.get(0),
                    )
                    .context("Recovered document was removed")?;
                let ControlRecord::Artifact(artifact) = serde_json::from_str(&body)? else {
                    anyhow::bail!("Document identity changed")
                };
                ensure!(
                    artifact.task_id == Some(work.task_id)
                        && artifact.project_id == Some(run.project_id),
                    "Recovered document provenance changed"
                );
                ensure!(
                    artifact.name == file
                        && artifact.format == format
                        && crate::control::verified_document(db, &artifact)? == bytes,
                    "Recovered document evidence changed"
                );
                if name == "save_pdf" {
                    documents::recover_pdf(db, &artifact, args, &bytes)?;
                }
                return Ok(
                    json!({"artifact_id":id,"name":file,"format":format,"replayed":false})
                        .to_string(),
                );
            }
        }
        "review_child" => {
            let row:Option<(String,String)>=db.query_row("SELECT signature,body FROM ion_team_reviews WHERE run_id=? AND parent_id=? AND request_id=?",
                params![run.id.to_string(),work.id.to_string(),request.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            if let Some((signature, body)) = row {
                let expected = json!({"assignment":args["assignment_id"],"verdict":args["verdict"],"feedback":args["feedback"],
                    "artifacts":args.get("artifact_ids").cloned().unwrap_or(json!([])),"checks":args.get("checks").cloned().unwrap_or(json!([]))});
                ensure!(
                    serde_json::from_str::<Value>(&signature)? == expected,
                    "Pending review differs from its durable receipt"
                );
                let r: AssignmentReview = serde_json::from_str(&body)?;
                return Ok(json!({"review_id":r.id,"verdict":r.verdict,"replacement_assignment_id":r.replacement_assignment_id,
                    "replacement_task_id":r.replacement_assignment_id.and_then(|id|run.work.iter().find(|w|w.id==id).map(|w|w.task_id)),"replayed":false}).to_string());
            }
        }
        "report_blocker" if args["reason"].as_str() == work.reason.as_deref() => {
            return Ok(
                json!({"status":"blocked","reason":work.reason,"replayed":false}).to_string(),
            );
        }
        "request_decision" | "request_app_action" => {
            let exists: bool = db.query_row(
                "SELECT EXISTS(SELECT 1 FROM ion_team_decisions WHERE work_id=? AND request_id=?)",
                params![work.id.to_string(), request.to_string()],
                |r| r.get(0),
            )?;
            ensure!(
                !exists,
                "A committed decision must resume through its original wait, not recovery"
            );
        }
        "report_result" => ensure!(
            work.result.is_none(),
            "A committed result cannot be reported twice"
        ),
        "await_children"
        | "report_blocker"
        | "read_file"
        | "list_files"
        | "list_inputs"
        | "read_input"
        | "read_remote_output"
        | "read_document"
        | "describe_app_action" => {}
        _ => anyhow::bail!("Unknown native tool cannot be safely recovered: {name}"),
    }
    Ok(json!({"interrupted":true,"replayed":false,"durable_effect":false,"note":"This call was interrupted without a committed effect. Recovery did not run it; reassess the saved task and current state before issuing a new request."}).to_string())
}
pub(super) fn apps(
    tx: &Transaction<'_>,
    run: &TeamRunSnapshot,
    work: &TeamAssignment,
    attempt: Uuid,
    now: u64,
) -> Result<Vec<Value>> {
    let rows:Vec<(String,String)>=tx.prepare("SELECT decision_id,receipt FROM ion_team_app_effects WHERE run_id=? AND work_id=? AND attempt_id=? AND state='settled' ORDER BY rowid LIMIT 65")?
        .query_map(params![run.id.to_string(),work.id.to_string(),attempt.to_string()],|r|Ok((r.get(0)?,r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    ensure!(
        rows.len() <= 64,
        "Recovered app evidence exceeds its count limit"
    );
    rows.into_iter().map(|(id,receipt)|{
        let id=Uuid::parse_str(&id)?;
        let decision=decisions::get(tx,id)?;
        ensure!(decision.assignment_id==work.id && decision.attempt_id==attempt && decision.status==TeamDecisionStatus::Approved,"Recovered app approval changed");
        let body:Value=serde_json::from_str(&receipt)?;
        ensure!(body["successful"].is_boolean(),"App receipt has no explicit outcome");
        let artifact=documents::publish_in_transaction(tx,run,work,id,&format!("App-action-{}.json",id.simple()),"json",&receipt,now)?;
        let mut excerpt=receipt; if excerpt.len()>8192 {let mut end=8192;while !excerpt.is_char_boundary(end){end-=1;}excerpt.truncate(end);}
        Ok(json!({"app_action_receipt":id,"artifact_id":artifact.id,"successful":body["successful"],"confirmed_by":body.get("confirmed_by").cloned().unwrap_or(json!("provider")),"excerpt":excerpt,"replayed":false}))
    }).collect()
}

/// Summarize process/accounting witnesses as data. Cached provider actions are
/// never replayed; pending owner actions use their existing exact durable receipts.
pub(super) fn remote(db: &Connection, attempt: Uuid) -> Result<Value> {
    let rows:Vec<(String,Option<String>,String,Option<u64>,Option<u64>)>=db.prepare("SELECT r.call_id,r.operation,r.state,b.input_tokens,b.output_tokens FROM ion_team_remote_calls r JOIN ion_team_call_budgets b ON b.call_id=r.call_id WHERE r.attempt_id=? ORDER BY r.started_at,r.call_id LIMIT 33")?
        .query_map([attempt.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?.collect::<rusqlite::Result<_>>()?;
    ensure!(
        rows.len() <= 32 && super::super::remote::attempt_stopped(db, attempt)?,
        "Remote process stop is unconfirmed"
    );
    Ok(
        json!({"remote_process_receipts":rows.into_iter().map(|(request,operation,state,input,output)|
        json!({"request_id":request,"operation_id":operation,"state":state,"input_tokens":input,"output_tokens":output})).collect::<Vec<_>>(),
        "note":"Previously submitted sandbox work has stopped. Cached model actions were not replayed. Existing owner effects were reconciled separately. Unknown usage stays reserved. Continue with the same pinned harness, model, host, grants and remaining budget."}),
    )
}
