use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};

pub(crate) fn pdf_bytes() -> Vec<u8> {
    let mut bytes = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let stream = "BT /F1 12 Tf 30 60 Td (Reviewed report) Tj ET\n";
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_owned(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        format!("<< /Length {} >>\nstream\n{}endstream", stream.len(), stream),
    ];
    let mut offsets = vec![];
    for (i, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend(format!("{} 0 obj\n{}\nendobj\n", i + 1, object).as_bytes());
    }
    let xref = bytes.len();
    bytes.extend(b"xref\n0 6\n0000000000 65535 f \n");
    for offset in offsets {
        bytes.extend(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend(
        format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    bytes
}

#[test]
fn assignment_pdf_exact_replay_and_export_survive_restart_without_repairing_missing_summary() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let bytes = pdf_bytes();
    let encoded = STANDARD.encode(&bytes);
    let request = Uuid::new_v4();
    let artifact = f
        .store
        .publish_pdf(
            &root.lease,
            request,
            "Report.pdf",
            &encoded,
            "Reviewed report",
            102,
        )
        .unwrap();
    assert!(artifact.evidence.as_ref().unwrap().matches(&bytes));
    assert_eq!(
        artifact.evidence.as_ref().unwrap().producer.assignment_id,
        Some(root.assignment.id)
    );
    let revision = f.definitions.load().unwrap().revision;
    let reopened = TeamCoordinator::new(f.definitions.path());
    assert_eq!(
        reopened
            .publish_pdf(
                &root.lease,
                request,
                "Report.pdf",
                &encoded,
                "Reviewed report",
                103
            )
            .unwrap()
            .id,
        artifact.id
    );
    assert_eq!(f.definitions.load().unwrap().revision, revision);
    assert_eq!(f.definitions.read_document(artifact.id).unwrap(), bytes);
    assert_eq!(
        reopened
            .read_assignment_document(&root.lease, artifact.id, 104)
            .unwrap(),
        bytes
    );
    let reply = reopened.document_model_reply(artifact.id, &bytes).unwrap();
    assert_eq!(reply["summary"], "Reviewed report");
    assert_eq!(reply["summary_source"], "agent");
    assert_eq!(reply["content_interpreted"], false);
    assert_eq!(reply["binary"], true);
    assert!(reply.get("content").is_none());
    assert!(!reply.to_string().contains(&encoded));
    assert!(
        reopened
            .publish_pdf(
                &root.lease,
                request,
                "Report.pdf",
                &encoded,
                "Different summary",
                104
            )
            .is_err()
    );
    assert!(
        reopened
            .publish_pdf(
                &root.lease,
                request,
                "Other.pdf",
                &encoded,
                "Reviewed report",
                104
            )
            .is_err()
    );
    let db = f.store.connect().unwrap();
    let (summary, seal): (String, String) = db
        .query_row(
            "SELECT summary,signature FROM ion_team_pdf_summaries WHERE artifact_id=?",
            [artifact.id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    db.execute(
        "DELETE FROM ion_team_pdf_summaries WHERE artifact_id=?",
        [artifact.id.to_string()],
    )
    .unwrap();
    // The original publication timestamp is deliberately reused: existence, not time, fences replay.
    assert!(
        reopened
            .publish_pdf(
                &root.lease,
                request,
                "Report.pdf",
                &encoded,
                "Reviewed report",
                102
            )
            .is_err()
    );
    assert!(f.definitions.read_document(artifact.id).is_err());
    db.execute(
        "INSERT INTO ion_team_pdf_summaries VALUES(?,?,?)",
        params![artifact.id.to_string(), summary, seal],
    )
    .unwrap();
    assert_eq!(f.definitions.read_document(artifact.id).unwrap(), bytes);
    assert_eq!(
        f.store
            .audit_events(run.id, 0, 100)
            .unwrap()
            .iter()
            .filter(|e| e.event == "document")
            .count(),
        1
    );
}

#[test]
fn assignment_pdf_rejects_bad_encoding_bounds_filenames_and_stale_leases_without_publication() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let encoded = STANDARD.encode(pdf_bytes());
    for (name, body, summary) in [
        (
            "Report.txt".to_owned(),
            encoded.clone(),
            "Report".to_owned(),
        ),
        (
            "../Report.pdf".to_owned(),
            encoded.clone(),
            "Report".to_owned(),
        ),
        (
            "Report\n.pdf".to_owned(),
            encoded.clone(),
            "Report".to_owned(),
        ),
        (
            "Report.pdf".to_owned(),
            "invalid%%".to_owned(),
            "Report".to_owned(),
        ),
        (
            "Report.pdf".to_owned(),
            format!("{encoded}\n"),
            "Report".to_owned(),
        ),
        (
            "Report.pdf".to_owned(),
            STANDARD.encode(b"text is not PDF"),
            "Report".to_owned(),
        ),
        (
            "Report.pdf".to_owned(),
            STANDARD.encode(b"%PDF-1.4\nNo EOF"),
            "Report".to_owned(),
        ),
        (
            "Report.pdf".to_owned(),
            STANDARD.encode(vec![b'a'; 24 * 1024 + 1]),
            "Report".to_owned(),
        ),
        ("Report.pdf".to_owned(), encoded.clone(), "x".repeat(4097)),
        ("Report.pdf".to_owned(), encoded.clone(), " \n".to_owned()),
        (
            "Report.pdf".to_owned(),
            encoded.clone(),
            "bad\x01summary".to_owned(),
        ),
    ] {
        assert!(
            f.store
                .publish_pdf(&root.lease, Uuid::new_v4(), &name, &body, &summary, 102)
                .is_err()
        );
    }
    assert!(
        f.store
            .publish_pdf(
                &root.lease,
                Uuid::nil(),
                "Report.pdf",
                &encoded,
                "Report",
                102
            )
            .is_err()
    );
    assert!(
        !f.definitions
            .load()
            .unwrap()
            .records
            .iter()
            .any(|r| matches!(r, ControlRecord::Artifact(_)))
    );
    f.store
        .stop(run.id, f.store.snapshot(run.id).unwrap().revision, 103)
        .unwrap();
    assert!(
        f.store
            .publish_pdf(
                &root.lease,
                Uuid::new_v4(),
                "Report.pdf",
                &encoded,
                "Report",
                104
            )
            .is_err()
    );
}

#[test]
fn assignment_pdf_summary_payload_and_metadata_tamper_block_reads_and_results() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let bytes = pdf_bytes();
    let encoded = STANDARD.encode(&bytes);
    let request = Uuid::new_v4();
    let artifact = f
        .store
        .publish_pdf(
            &root.lease,
            request,
            "Report.pdf",
            &encoded,
            "Actual report",
            102,
        )
        .unwrap();
    let mut out = result();
    out.artifact_ids = vec![artifact.id];
    let db = f.store.connect().unwrap();
    db.execute(
        "UPDATE ion_team_pdf_summaries SET summary='Changed report' WHERE artifact_id=?",
        [artifact.id.to_string()],
    )
    .unwrap();
    assert!(f.definitions.read_document(artifact.id).is_err());
    assert!(f.store.document_model_reply(artifact.id, &bytes).is_err());
    assert!(
        f.store
            .report(&root.lease, AssignmentOutcome::Completed(out.clone()), 103)
            .is_err()
    );
    db.execute(
        "UPDATE ion_team_pdf_summaries SET summary='Actual report' WHERE artifact_id=?",
        [artifact.id.to_string()],
    )
    .unwrap();
    db.execute(
        "UPDATE ion_team_document_receipts SET content=? WHERE artifact_id=?",
        params![
            b"%PDF-1.4\nchanged\n%%EOF".as_slice(),
            artifact.id.to_string()
        ],
    )
    .unwrap();
    assert!(f.definitions.read_document(artifact.id).is_err());
    db.execute(
        "UPDATE ion_team_document_receipts SET content=? WHERE artifact_id=?",
        params![&bytes, artifact.id.to_string()],
    )
    .unwrap();
    let mut changed = artifact.clone();
    changed.name = "Changed.pdf".into();
    db.execute(
        "UPDATE ion_control_records SET body=? WHERE id=?",
        params![
            serde_json::to_string(&ControlRecord::Artifact(changed)).unwrap(),
            artifact.id.to_string()
        ],
    )
    .unwrap();
    assert!(f.definitions.read_document(artifact.id).is_err());
    db.execute(
        "UPDATE ion_control_records SET body=? WHERE id=?",
        params![
            serde_json::to_string(&ControlRecord::Artifact(artifact.clone())).unwrap(),
            artifact.id.to_string()
        ],
    )
    .unwrap();
    assert_eq!(
        f.store
            .report(&root.lease, AssignmentOutcome::Completed(out), 104)
            .unwrap()
            .status,
        TeamRunStatus::Completed
    );
    assert_eq!(f.definitions.read_document(artifact.id).unwrap(), bytes);
}

#[test]
fn assignment_pdf_child_handoffs_keep_document_scope_and_output_contracts() {
    let f = Fixture::new();
    let run = f.run();
    let parent = f.claim(run.id, run.work[0].id);
    let child = f
        .store
        .delegate_with_contract(
            &parent.lease,
            f.delegation(1),
            OutputContract {
                minimum_documents: 1,
                required_formats: vec!["pdf".into()],
                ..Default::default()
            },
            102,
        )
        .unwrap();
    f.materialize();
    let child = f
        .store
        .claim(run.id, child.id, DispatchHost::Local, 103, 100)
        .unwrap();
    let bytes = pdf_bytes();
    let encoded = STANDARD.encode(&bytes);
    let private = f
        .store
        .publish_pdf(
            &parent.lease,
            Uuid::new_v4(),
            "Private.pdf",
            &encoded,
            "Parent document",
            104,
        )
        .unwrap();
    assert!(
        f.store
            .read_assignment_document(&child.lease, private.id, 105)
            .is_err()
    );
    let artifact = f
        .store
        .publish_pdf(
            &child.lease,
            Uuid::new_v4(),
            "Child.pdf",
            &encoded,
            "Child result",
            104,
        )
        .unwrap();
    assert_eq!(
        f.store
            .read_assignment_document(&parent.lease, artifact.id, 105)
            .unwrap(),
        bytes
    );
    let artifact_id = artifact.id;
    let mut out = result();
    out.artifact_ids = vec![artifact.id];
    f.store
        .report(&child.lease, AssignmentOutcome::Completed(out), 106)
        .unwrap();
    assert!(
        f.definitions
            .save_client_record(
                f.definitions.load().unwrap().revision,
                ControlRecord::Artifact(artifact)
            )
            .is_err()
    );
    f.store
        .suspend_for_children(&parent.lease, &[child.assignment.id], "{}", 107)
        .unwrap();
    assert!(
        f.store
            .read_assignment_document(&parent.lease, private.id, 108)
            .is_err()
    );
    // Release the lead slot before claiming another run with the same saved agent.
    let other = f.run();
    let outsider = f.claim(other.id, other.work[0].id);
    assert!(
        f.store
            .read_assignment_document(&outsider.lease, artifact_id, 108)
            .is_err()
    );
}

#[test]
fn assignment_pdf_normal_task_reads_only_its_completed_team_and_returns_labeled_summary() {
    use crate::driver::source_teams::{WorkspaceContext, execute};
    use serde_json::json;
    use std::sync::atomic::AtomicBool;
    let f = Fixture::new();
    let source = Uuid::new_v4();
    let run = f
        .store
        .queue_from_agent_with_access(
            source,
            f.project,
            "source-pdf".into(),
            f.team,
            f.definitions.load().unwrap().revision,
            "Create a PDF".into(),
            AgentAccess::default(),
            100,
        )
        .unwrap();
    f.materialize();
    let root = f.claim(run.id, run.work[0].id);
    let bytes = pdf_bytes();
    let artifact = f
        .store
        .publish_pdf(
            &root.lease,
            Uuid::new_v4(),
            "Report.pdf",
            &STANDARD.encode(&bytes),
            "Reviewed report",
            102,
        )
        .unwrap();
    let context = WorkspaceContext {
        task_id: source,
        project_id: f.project,
        control_path: f.definitions.path(),
        settings_path: f.dir.join("unused-settings.json"),
        mode: waku_protocol::model::RuntimeMode::Ask,
    };
    let args = json!({"run_id":run.id,"artifact_id":artifact.id});
    let canceled = AtomicBool::new(false);
    let read = |context: &WorkspaceContext| {
        execute(
            context,
            "source-read",
            "read_team_document",
            &args,
            &canceled,
            |_, _| panic!("Reading evidence cannot request new grants"),
        )
    };
    assert!(read(&context).is_err());
    let mut output = result();
    output.artifact_ids = vec![artifact.id];
    f.store
        .report(&root.lease, AssignmentOutcome::Completed(output), 103)
        .unwrap();
    let reply: serde_json::Value = serde_json::from_str(&read(&context).unwrap()).unwrap();
    assert_eq!(reply["summary"], "Reviewed report");
    assert_eq!(reply["run_id"], run.id.to_string());
    assert_eq!(reply["content_interpreted"], false);
    assert_eq!(reply["summary_source"], "agent");
    let mut wrong = context.clone();
    wrong.task_id = Uuid::new_v4();
    assert!(read(&wrong).is_err());
    wrong = context.clone();
    wrong.project_id = Uuid::new_v4();
    assert!(read(&wrong).is_err());
    canceled.store(true, std::sync::atomic::Ordering::Release);
    assert!(read(&context).is_err());
    assert_eq!(f.definitions.read_document(artifact.id).unwrap(), bytes);
}

#[test]
fn assignment_pdf_text_document_reads_preserve_escape_budget_and_utf8_boundaries() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    for (i, content) in [
        "\\\"".repeat(6000),
        "\x01\n".repeat(5000),
        "文🚀".repeat(4000),
    ]
    .into_iter()
    .enumerate()
    {
        let artifact = f
            .store
            .publish_document(
                &root.lease,
                Uuid::new_v4(),
                &format!("Report-{i}.md"),
                "md",
                &content,
                102,
            )
            .unwrap();
        let bytes = f
            .store
            .read_assignment_document(&root.lease, artifact.id, 103)
            .unwrap();
        let reply = f.store.document_model_reply(artifact.id, &bytes).unwrap();
        assert_eq!(reply["truncated"], true);
        assert!(reply.to_string().len() < 9000);
        let excerpt = reply["content"].as_str().unwrap();
        assert!(content.starts_with(excerpt));
        assert!(!excerpt.is_empty());
        assert!(reply.get("summary").is_none());
        assert!(reply.get("binary").is_none());
    }
}
