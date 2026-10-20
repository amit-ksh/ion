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
        account_scope: None,
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

#[test]
fn additional_text_formats_survive_publication_discovery_and_exact_export() {
    use waku_protocol::artifact_library::{ArtifactDocumentQuery, ArtifactLibraryQuery};
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    for (format, content) in [
        (
            "svg",
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><text>Report</text></svg>",
        ),
        ("xml", "<?xml version=\"1.0\"?><report>complete</report>"),
        ("yaml", "ready: true\nname: Report\n"),
        ("yml", "ready: true\n"),
    ] {
        let request = Uuid::new_v4();
        let artifact = f
            .store
            .publish_document(
                &root.lease,
                request,
                &format!("Report.{format}"),
                format,
                content,
                102,
            )
            .unwrap();
        let page = f
            .definitions
            .list_artifacts(&ArtifactLibraryQuery {
                project_id: Some(run.project_id),
                task_id: Some(root.assignment.task_id),
                format: Some(format.into()),
                after: None,
                limit: 32,
            })
            .unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, artifact.id);
        let document = f
            .definitions
            .read_artifact_document(&ArtifactDocumentQuery {
                expected: page.items[0].clone(),
            })
            .unwrap();
        assert_eq!(document.bytes, content.as_bytes());
        assert_eq!(
            f.store
                .publish_document(
                    &root.lease,
                    request,
                    &format!("Report.{format}"),
                    format,
                    content,
                    103
                )
                .unwrap()
                .id,
            artifact.id
        );
        assert!(
            f.store
                .publish_document(
                    &root.lease,
                    request,
                    &format!("Report.{format}"),
                    format,
                    "changed",
                    103
                )
                .is_err()
        );
    }
}

#[test]
fn large_file_publication_pins_hash_summary_replay_and_private_paths() {
    use sha2::{Digest, Sha256};
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let source =
        "<style>.report{display:grid}</style><p>".to_owned() + &"Report ".repeat(40000) + "</p>";
    std::fs::write(f.dir.join("large.html"), &source).unwrap();
    let hash = format!("{:x}", Sha256::digest(source.as_bytes()));
    let inspected = f
        .store
        .inspect_file_document(&root.lease, &f.dir, "large.html", 102)
        .unwrap();
    assert_eq!(inspected["sha256"], hash);
    assert!(inspected.get("content").is_none());
    let request = Uuid::new_v4();
    let artifact = f
        .store
        .publish_file_document(
            &root.lease,
            request,
            &f.dir,
            "large.html",
            "large.html",
            "html",
            &hash,
            "A complete large report",
            102,
        )
        .unwrap();
    assert_eq!(
        f.definitions.read_document(artifact.id).unwrap(),
        source.as_bytes()
    );
    let reply = f
        .store
        .document_model_reply(artifact.id, source.as_bytes())
        .unwrap();
    assert_eq!(reply["summary"], "A complete large report");
    assert_eq!(reply["content_interpreted"], false);
    assert!(reply.get("content").is_none());
    std::fs::remove_file(f.dir.join("large.html")).unwrap();
    assert_eq!(
        f.store
            .publish_file_document(
                &root.lease,
                request,
                &f.dir,
                "large.html",
                "large.html",
                "html",
                &hash,
                "A complete large report",
                103
            )
            .unwrap()
            .id,
        artifact.id
    );
    assert!(
        f.store
            .publish_file_document(
                &root.lease,
                request,
                &f.dir,
                "large.html",
                "large.html",
                "html",
                &hash,
                "Changed summary",
                103
            )
            .is_err()
    );
    assert!(
        f.store
            .inspect_file_document(&root.lease, &f.dir, "../large.html", 103)
            .is_err()
    );
    std::fs::write(f.dir.join(".env"), "PRIVATE").unwrap();
    assert!(
        f.store
            .inspect_file_document(&root.lease, &f.dir, ".env", 103)
            .is_err()
    );
    let db = f.store.connect().unwrap();
    db.execute(
        "DELETE FROM ion_team_file_documents WHERE artifact_id=?",
        [artifact.id.to_string()],
    )
    .unwrap();
    assert!(f.definitions.read_document(artifact.id).is_err());
}
#[test]
fn file_publication_rejects_changed_hash_oversize_and_wrong_envelopes() {
    use sha2::{Digest, Sha256};
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    std::fs::write(f.dir.join("report.txt"), "changed").unwrap();
    assert!(
        f.store
            .publish_file_document(
                &root.lease,
                Uuid::new_v4(),
                &f.dir,
                "report.txt",
                "report.txt",
                "txt",
                &"0".repeat(64),
                "Report",
                102
            )
            .is_err()
    );
    for format in ["pdf", "png", "docx", "xlsx", "pptx"] {
        let hash = format!("{:x}", Sha256::digest(b"changed"));
        assert!(
            f.store
                .publish_file_document(
                    &root.lease,
                    Uuid::new_v4(),
                    &f.dir,
                    "report.txt",
                    &format!("report.{format}"),
                    format,
                    &hash,
                    "Report",
                    102
                )
                .is_err()
        );
    }
    std::fs::File::create(f.dir.join("oversized.txt"))
        .unwrap()
        .set_len(16 * 1024 * 1024 + 1)
        .unwrap();
    assert!(
        f.store
            .inspect_file_document(&root.lease, &f.dir, "oversized.txt", 102)
            .is_err()
    );
}
#[test]
fn file_publication_binary_originals_export_without_model_decoding() {
    use sha2::{Digest, Sha256};
    use waku_protocol::artifact_library::{ArtifactDocumentQuery, ArtifactLibraryQuery};
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    // File publications validate envelopes; native rendering/parsing is separate.
    for (format, bytes) in [
        ("pdf", pdf_bytes()),
        ("docx", b"PK\x03\x04opaque office archive".to_vec()),
        ("xlsx", b"PK\x03\x04opaque workbook archive".to_vec()),
        ("pptx", b"PK\x03\x04opaque slide archive".to_vec()),
        (
            "png",
            b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDRopaque raster".to_vec(),
        ),
    ] {
        let name = format!("Report.{format}");
        std::fs::write(f.dir.join(&name), &bytes).unwrap();
        let hash = format!("{:x}", Sha256::digest(&bytes));
        let artifact = f
            .store
            .publish_file_document(
                &root.lease,
                Uuid::new_v4(),
                &f.dir,
                &name,
                &name,
                format,
                &hash,
                "Agent summary",
                102,
            )
            .unwrap();
        let page = f
            .definitions
            .list_artifacts(&ArtifactLibraryQuery {
                project_id: Some(run.project_id),
                task_id: None,
                format: Some(format.into()),
                after: None,
                limit: 32,
            })
            .unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(
            f.definitions
                .read_artifact_document(&ArtifactDocumentQuery {
                    expected: page.items[0].clone()
                })
                .unwrap()
                .bytes,
            bytes
        );
        let reply = f.store.document_model_reply(artifact.id, &bytes).unwrap();
        assert_eq!(reply["content_interpreted"], false);
        assert!(reply.get("content").is_none());
    }
}

#[test]
fn file_publication_small_summary_and_recovery_cannot_be_removed_or_changed() {
    use sha2::{Digest, Sha256};
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    std::fs::write(f.dir.join("report.txt"), "Reviewed small report").unwrap();
    let bytes = b"Reviewed small report";
    let hash = format!("{:x}", Sha256::digest(bytes));
    let request = Uuid::new_v4();
    let artifact = f
        .store
        .publish_file_document(
            &root.lease,
            request,
            &f.dir,
            "report.txt",
            "report.txt",
            "txt",
            &hash,
            "Agent description",
            102,
        )
        .unwrap();
    let args = serde_json::json!({"path":"report.txt","name":"report.txt","format":"txt","sha256":hash,"summary":"Agent description"});
    let db = f.store.connect().unwrap();
    documents::recover_file(&db, &artifact, &args, bytes).unwrap();
    let mut changed = args.clone();
    changed["summary"] = "Changed".into();
    assert!(documents::recover_file(&db, &artifact, &changed, bytes).is_err());
    let handoff = documents::handoff(&db, &artifact, bytes.to_vec()).unwrap();
    assert!(handoff.content.contains("Agent description"));
    assert!(!handoff.content.contains("Reviewed small report"));
    db.execute(
        "DELETE FROM ion_team_file_documents WHERE artifact_id=?",
        [artifact.id.to_string()],
    )
    .unwrap();
    assert!(f.definitions.read_document(artifact.id).is_err());
    assert!(f.store.document_model_reply(artifact.id, bytes).is_err());
    assert!(
        f.store
            .publish_file_document(
                &root.lease,
                request,
                &f.dir,
                "report.txt",
                "report.txt",
                "txt",
                &hash,
                "Agent description",
                103
            )
            .is_err()
    );
}

#[test]
fn html_bundle_publication_keeps_originals_and_replays_without_source_files() {
    use sha2::{Digest, Sha256};
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let source = "<link rel=stylesheet href='theme.css'><img src='drawing.svg'><p>Report</p>";
    let css = ".report{display:grid;background:url(plot.png)}";
    let raster = b"raster bytes";
    let svg = "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 10'><image href='plot.png'/><use href='icons.svg#badge'/><text>Chart</text></svg>";
    let sprite = "<svg><symbol id='badge'><path d='M0 0L1 1'/></symbol></svg>";
    for (path, bytes) in [
        ("index.html", source.as_bytes()),
        ("theme.css", css.as_bytes()),
        ("plot.png", raster.as_slice()),
        ("drawing.svg", svg.as_bytes()),
        ("icons.svg", sprite.as_bytes()),
    ] {
        std::fs::write(f.dir.join(path), bytes).unwrap();
    }
    let sha = |b: &[u8]| format!("{:x}", Sha256::digest(b));
    let args = serde_json::json!({"path":"index.html","sha256":sha(source.as_bytes()),"name":"Report.html","summary":"Report with its local styles and plot","resources":[{"path":"icons.svg","sha256":sha(sprite.as_bytes())},{"path":"drawing.svg","sha256":sha(svg.as_bytes())},{"path":"theme.css","sha256":sha(css.as_bytes())},{"path":"plot.png","sha256":sha(raster)}]});
    let parsed: HtmlBundleArgs = serde_json::from_value(args.clone()).unwrap();
    let request = Uuid::new_v4();
    let artifact = f
        .store
        .publish_html_bundle(&root.lease, request, &f.dir, &parsed, 102)
        .unwrap();
    let bytes = f.definitions.read_document(artifact.id).unwrap();
    let portable = std::str::from_utf8(&bytes).unwrap();
    assert!(portable.contains("display:grid"));
    assert!(portable.contains("data:image/svg+xml;base64,"));
    assert!(portable.contains("data:image/png;base64,"));
    assert_eq!(
        std::fs::read_to_string(f.dir.join("drawing.svg")).unwrap(),
        svg
    );
    assert_eq!(
        std::fs::read_to_string(f.dir.join("icons.svg")).unwrap(),
        sprite
    );
    assert!(artifact.evidence.as_ref().unwrap().matches(&bytes));
    assert_eq!(
        std::fs::read_to_string(f.dir.join("index.html")).unwrap(),
        source
    );
    assert_eq!(
        std::fs::read_to_string(f.dir.join("theme.css")).unwrap(),
        css
    );
    let reply = f.store.document_model_reply(artifact.id, &bytes).unwrap();
    assert_eq!(reply["summary"], parsed.summary);
    assert!(reply.get("content").is_none());
    let db = f.store.connect().unwrap();
    documents::recover_html_bundle(&db, &artifact, &args, &bytes).unwrap();
    let mut changed = args.clone();
    changed["resources"][0]["sha256"] = "0".repeat(64).into();
    assert!(documents::recover_html_bundle(&db, &artifact, &changed, &bytes).is_err());
    for name in [
        "index.html",
        "theme.css",
        "plot.png",
        "drawing.svg",
        "icons.svg",
    ] {
        std::fs::remove_file(f.dir.join(name)).unwrap();
    }
    assert_eq!(
        f.store
            .publish_html_bundle(&root.lease, request, &f.dir, &parsed, 103)
            .unwrap()
            .id,
        artifact.id
    );
    let mut changed = parsed.clone();
    changed.summary = "Different".into();
    assert!(
        f.store
            .publish_html_bundle(&root.lease, request, &f.dir, &changed, 103)
            .is_err()
    );
    db.execute(
        "DELETE FROM ion_team_html_bundles WHERE artifact_id=?",
        [artifact.id.to_string()],
    )
    .unwrap();
    assert!(f.definitions.read_document(artifact.id).is_err());
    assert!(
        f.store
            .publish_html_bundle(&root.lease, request, &f.dir, &parsed, 103)
            .is_err()
    );
}
#[test]
fn html_bundle_refuses_changed_unlisted_private_duplicate_and_unused_resources() {
    use sha2::{Digest, Sha256};
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let source = "<img src=plot.png>";
    std::fs::write(f.dir.join("index.html"), source).unwrap();
    std::fs::write(f.dir.join("plot.png"), b"raster").unwrap();
    let sha = |b: &[u8]| format!("{:x}", Sha256::digest(b));
    let mut args = serde_json::json!({"path":"index.html","sha256":sha(source.as_bytes()),"name":"Report.html","summary":"Report","resources":[]});
    assert!(
        f.store
            .publish_html_bundle(
                &root.lease,
                Uuid::new_v4(),
                &f.dir,
                &serde_json::from_value(args.clone()).unwrap(),
                102
            )
            .is_err()
    );
    args["resources"] = serde_json::json!([{"path":"plot.png","sha256":"0".repeat(64)}]);
    assert!(
        f.store
            .publish_html_bundle(
                &root.lease,
                Uuid::new_v4(),
                &f.dir,
                &serde_json::from_value(args.clone()).unwrap(),
                102
            )
            .is_err()
    );
    args["resources"][0]["sha256"] = sha(b"raster").into();
    let entry = args["resources"][0].clone();
    args["resources"].as_array_mut().unwrap().push(entry);
    assert!(
        f.store
            .publish_html_bundle(
                &root.lease,
                Uuid::new_v4(),
                &f.dir,
                &serde_json::from_value(args.clone()).unwrap(),
                102
            )
            .is_err()
    );
    args["resources"] = serde_json::json!([{"path":".env","sha256":sha(b"private")}]);
    std::fs::write(f.dir.join(".env"), b"private").unwrap();
    assert!(
        f.store
            .publish_html_bundle(
                &root.lease,
                Uuid::new_v4(),
                &f.dir,
                &serde_json::from_value(args).unwrap(),
                102
            )
            .is_err()
    );
    std::fs::write(f.dir.join("unused.css"), "p{color:green}").unwrap();
    let unused = serde_json::json!({"path":"index.html","sha256":sha(source.as_bytes()),"name":"Report.html","summary":"Report","resources":[{"path":"plot.png","sha256":sha(b"raster")},{"path":"unused.css","sha256":sha(b"p{color:green}")}]});
    assert!(
        f.store
            .publish_html_bundle(
                &root.lease,
                Uuid::new_v4(),
                &f.dir,
                &serde_json::from_value(unused).unwrap(),
                102
            )
            .is_err()
    );
    assert!(
        f.definitions
            .load()
            .unwrap()
            .records
            .iter()
            .all(|r| !matches!(r, ControlRecord::Artifact(_)))
    );
}

#[test]
#[ignore = "Publishes the PPTX source at ION_PPTX_SOURCE_FIXTURE and exports actual protocol bytes to ION_PPTX_NATIVE_FIXTURE"]
fn native_pptx_phone_fixture() {
    use waku_protocol::artifact_library::{ArtifactDocumentQuery, ArtifactLibraryQuery};
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let original = std::fs::read(
        std::env::var_os("ION_PPTX_SOURCE_FIXTURE").expect("Set ION_PPTX_SOURCE_FIXTURE"),
    )
    .unwrap();
    let source = f.dir.join("Review.pptx");
    std::fs::write(&source, &original).unwrap();
    let inspected = f
        .store
        .inspect_file_document(&root.lease, &f.dir, "Review.pptx", 102)
        .unwrap();
    let hash = inspected["sha256"].as_str().unwrap();
    let request = Uuid::new_v4();
    let artifact = f
        .store
        .publish_file_document(
            &root.lease,
            request,
            &f.dir,
            "Review.pptx",
            "Review.pptx",
            "pptx",
            hash,
            "Slide overview, hidden completion slide and table evidence",
            102,
        )
        .unwrap();
    assert!(artifact.evidence.as_ref().unwrap().matches(&original));
    let page = f
        .definitions
        .list_artifacts(&ArtifactLibraryQuery {
            project_id: Some(run.project_id),
            task_id: None,
            format: Some("pptx".into()),
            after: None,
            limit: 32,
        })
        .unwrap();
    assert_eq!(page.items.len(), 1);
    let query = ArtifactDocumentQuery {
        expected: page.items[0].clone(),
    };
    let exported = f.definitions.read_artifact_document(&query).unwrap();
    assert_eq!(exported.bytes, original);
    let model = f
        .store
        .document_model_reply(artifact.id, &original)
        .unwrap();
    assert_eq!(model["content_interpreted"], false);
    assert!(model.get("content").is_none());
    std::fs::remove_file(&source).unwrap();
    let reopened = TeamCoordinator::new(f.definitions.path());
    assert_eq!(
        reopened
            .publish_file_document(
                &root.lease,
                request,
                &f.dir,
                "Review.pptx",
                "Review.pptx",
                "pptx",
                hash,
                "Slide overview, hidden completion slide and table evidence",
                103
            )
            .unwrap()
            .id,
        artifact.id
    );
    assert_eq!(
        f.definitions.read_artifact_document(&query).unwrap().bytes,
        original
    );
    std::fs::write(
        std::env::var_os("ION_PPTX_NATIVE_FIXTURE").expect("Set ION_PPTX_NATIVE_FIXTURE"),
        serde_json::to_vec(&serde_json::json!({"type":"artifactDocument","document":exported}))
            .unwrap(),
    )
    .unwrap();
    let db = f.store.connect().unwrap();
    db.execute(
        "DELETE FROM ion_team_file_documents WHERE artifact_id=?",
        [artifact.id.to_string()],
    )
    .unwrap();
    assert!(f.definitions.read_artifact_document(&query).is_err());
}

fn mathml_publication_fixture() -> serde_json::Value {
    use sha2::{Digest, Sha256};
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let source = r#"<link rel="stylesheet" href="math.css"><h1>Technical report</h1><math display="block" aria-label="Quadratic roots"><semantics><mrow><mi>x</mi><mo>=</mo><mfrac><mrow><mo>&minus;</mo><mi>b</mi><mo>&PlusMinus;</mo><msqrt><msup><mi>b</mi><mn>2</mn></msup><mo>&minus;</mo><mn>4</mn><mi>a</mi><mi>c</mi></msqrt></mrow><mrow><mn>2</mn><mi>a</mi></mrow></mfrac></mrow><annotation encoding="application/x-tex">x = (-b + sqrt(b^2-4ac))/(2a)</annotation></semantics></math><math><munder><mo>&sum;</mo><mi>i</mi></munder><msup><mi>x</mi><mn>2</mn></msup></math>"#;
    let css = "@import 'layout.css';math{font-size:1.2rem;math-style:normal}mfrac{color:green}";
    let imported = "@media(max-width:600px){math{font-size:1rem}}";
    for (name, content) in [
        ("index.html", source),
        ("math.css", css),
        ("layout.css", imported),
    ] {
        std::fs::write(f.dir.join(name), content).unwrap();
    }
    let sha = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    let args = serde_json::json!({"path":"index.html", "sha256":sha(source.as_bytes()), "name":"Equations.html", "summary":"Technical report with equations", "resources":[{"path":"math.css", "sha256":sha(css.as_bytes())}, {"path":"layout.css", "sha256":sha(imported.as_bytes())}]});
    let parsed: HtmlBundleArgs = serde_json::from_value(args.clone()).unwrap();
    let request = Uuid::new_v4();
    let artifact = f
        .store
        .publish_html_bundle(&root.lease, request, &f.dir, &parsed, 102)
        .unwrap();
    let bytes = f.definitions.read_document(artifact.id).unwrap();
    assert!(artifact.evidence.as_ref().unwrap().matches(&bytes));
    let html = std::str::from_utf8(&bytes).unwrap();
    assert!(html.contains("<mfrac>"));
    assert!(html.contains("<msqrt>"));
    assert!(html.contains("∑"));
    assert!(html.contains("math-style:normal"));
    let reply = f.store.document_model_reply(artifact.id, &bytes).unwrap();
    assert_eq!(reply["summary"], parsed.summary);
    assert!(reply.get("content").is_none());
    for (name, content) in [
        ("index.html", source),
        ("math.css", css),
        ("layout.css", imported),
    ] {
        assert_eq!(std::fs::read_to_string(f.dir.join(name)).unwrap(), content);
        std::fs::remove_file(f.dir.join(name)).unwrap();
    }
    assert_eq!(
        f.store
            .publish_html_bundle(&root.lease, request, &f.dir, &parsed, 103)
            .unwrap()
            .id,
        artifact.id
    );
    let replay = f.definitions.read_document(artifact.id).unwrap();
    assert_eq!(replay, bytes);
    serde_json::json!({"html":html, "sha256":sha(&bytes), "source":source, "summary":parsed.summary})
}
#[test]
fn html_bundle_mathml_publication_preserves_equations_and_durable_replay() {
    mathml_publication_fixture();
}
#[test]
#[ignore = "Exports actual publication/replay output to ION_MATHML_NATIVE_FIXTURE"]
fn native_mathml_phone_fixture() {
    std::fs::write(
        std::env::var_os("ION_MATHML_NATIVE_FIXTURE").expect("Set ION_MATHML_NATIVE_FIXTURE"),
        serde_json::to_vec(&mathml_publication_fixture()).unwrap(),
    )
    .unwrap();
}

#[path = "html_source_tests.rs"]
mod html_sources;
