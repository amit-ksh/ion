use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use waku_protocol::gateway_portfolio::{history, pdf::WorkflowPdfQuery};
fn noisy_pdf() -> Vec<u8> {
    let mut bytes = b"%PDF-1.4\n".to_vec();
    let mut stream = String::new();
    let mut seed = 7u32;
    for _ in 0..350 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let x = seed % 195;
        let y = (seed >> 8) % 95;
        stream.push_str(&format!(
            "{}.{} {}.{} {}.{} rg {} {} 5 5 re f\n",
            (seed >> 3) % 2,
            (seed >> 7) % 10,
            (seed >> 9) % 2,
            (seed >> 13) % 10,
            (seed >> 15) % 2,
            (seed >> 19) % 10,
            x,
            y
        ));
    }
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R >>".to_owned(),
        format!(
            "<< /Length {} >>\nstream\n{}endstream",
            stream.len(),
            stream
        ),
    ];
    let mut offsets = vec![];
    for (i, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend(format!("{} 0 obj\n{}\nendobj\n", i + 1, object).as_bytes());
    }
    let xref = bytes.len();
    bytes.extend(b"xref\n0 5\n0000000000 65535 f \n");
    for offset in offsets {
        bytes.extend(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend(
        format!("trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    assert!(bytes.len() < 24 * 1024);
    bytes
}
fn fixture(f: &Fixture) -> WorkflowPdfQuery {
    let workflow = Workflow {
        id: Uuid::new_v4(),
        name: "Reviewed documents".into(),
        project_id: f.project,
        prompt: "Publish a report".into(),
        limits: None,
        stages: vec![WorkflowStage {
            inherit_workspace: false,
            retry: None,
            title: "Report".into(),
            instructions: "Create the PDF".into(),
            requires_decision: false,
            team_id: Some(f.team),
            required_formats: vec![],
            transition: WorkflowTransition::default(),
        }],
    };
    let revision = f
        .definitions
        .save(
            f.definitions.load().unwrap().revision,
            ControlRecord::Workflow(workflow.clone()),
        )
        .unwrap()
        .revision;
    let run = f
        .store
        .start_automation(Uuid::new_v4(), workflow.id, revision, 100)
        .unwrap();
    f.store
        .tick_automations(&HashSet::from([f.project]), 100)
        .unwrap();
    f.materialize();
    let run = f.store.automation_run(run.id).unwrap();
    let team = f.store.snapshot(run.stages[0].team_run_id).unwrap();
    let claim = f.claim(team.id, team.work[0].id);
    let artifact = f
        .store
        .publish_pdf(
            &claim.lease,
            Uuid::new_v4(),
            "Report.pdf",
            &STANDARD.encode(noisy_pdf()),
            "Reviewed report",
            102,
        )
        .unwrap();
    f.store
        .report(
            &claim.lease,
            AssignmentOutcome::Completed(AssignmentResult {
                summary: "Reviewed report".into(),
                artifact_ids: vec![artifact.id],
                checks: vec![],
                executed_checks: vec![],
            }),
            103,
        )
        .unwrap();
    f.store
        .tick_automations(&HashSet::from([f.project]), 104)
        .unwrap();
    let run = f.store.automation_run(run.id).unwrap();
    let team = f.store.snapshot(team.id).unwrap();
    let expected = serde_json::from_value(
        history::accepted_artifacts(&run, &[team], &f.definitions.load().unwrap())
            .unwrap()
            .remove(0),
    )
    .unwrap();
    WorkflowPdfQuery {
        run_id: run.id,
        revision: run.revision,
        expected,
        page: 0,
        width: 800,
        transfer_id: None,
        offset: 0,
        limit: 32768,
    }
}
#[cfg(any(target_os = "windows", target_os = "macos"))]
#[test]
fn gateway_pdf_renders_actual_accepted_publication_once_and_checks_each_chunk_source() {
    use sha2::{Digest, Sha256};
    let f = Fixture::new();
    let mut q = fixture(&f);
    q.width = 1600;
    let first = f.store.read_workflow_pdf_page(&q).unwrap();
    let row = &first["page"];
    assert_eq!(row["document"], serde_json::to_value(&q.expected).unwrap());
    assert_eq!(row["runId"], q.run_id.to_string());
    assert!(!first.to_string().contains("Produce reviewable work"));
    assert!(first.to_string().len() < 60000);
    let transfer: Uuid = serde_json::from_value(row["transferId"].clone()).unwrap();
    let count = row["bytes"].as_u64().unwrap() as usize;
    assert!(
        count > 32768,
        "Real native raster must exercise continuation chunks"
    );
    let hash = row["sha256"].as_str().unwrap().to_owned();
    let mut bytes = STANDARD.decode(row["data"].as_str().unwrap()).unwrap();
    while bytes.len() < count {
        q.offset = bytes.len() as u32;
        q.transfer_id = Some(transfer);
        let chunk = f.store.read_workflow_pdf_page(&q).unwrap();
        assert_eq!(chunk["page"]["transferId"], transfer.to_string());
        assert_eq!(chunk["page"]["sha256"], hash);
        bytes.extend(
            STANDARD
                .decode(chunk["page"]["data"].as_str().unwrap())
                .unwrap(),
        );
    }
    assert_eq!(hash, format!("{:x}", Sha256::digest(&bytes)));
    assert!(bytes.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]));
    // A fabricated accepted result cannot turn an unrelated source into preview authority.
    q.offset = 0;
    q.transfer_id = None;
    q.expected.assignment_id = Uuid::new_v4();
    assert!(f.store.read_workflow_pdf_page(&q).is_err());
}
#[test]
fn gateway_pdf_rejects_changed_or_unaccepted_evidence_before_native_rendering() {
    let f = Fixture::new();
    let q = fixture(&f);
    for change in 0..8 {
        let mut wrong = q.clone();
        match change {
            0 => wrong.run_id = Uuid::new_v4(),
            1 => wrong.revision += 1,
            2 => wrong.expected.project_id = Uuid::new_v4(),
            3 => wrong.expected.bytes = 65537,
            4 => wrong.expected.sha256 = "b".repeat(64),
            5 => wrong.page = 32,
            6 => wrong.width = 801,
            _ => wrong.expected.team_run_id = Uuid::new_v4(),
        };
        assert!(f.store.read_workflow_pdf_page(&wrong).is_err());
    }
    let db = f.store.connect().unwrap();
    db.execute(
        "DELETE FROM ion_team_pdf_summaries WHERE artifact_id=?",
        [q.expected.id.to_string()],
    )
    .unwrap();
    assert!(f.store.read_workflow_pdf_page(&q).is_err());
}
#[cfg(any(target_os = "windows", target_os = "macos"))]
#[test]
fn cached_gateway_pdf_cannot_survive_accepted_result_membership_or_provenance_change() {
    let f = Fixture::new();
    let q = fixture(&f);
    let first = f.store.read_workflow_pdf_page(&q).unwrap();
    let transfer: Uuid = serde_json::from_value(first["page"]["transferId"].clone()).unwrap();
    // Change the accepted automation snapshot while retaining all original bytes/hash.
    let db = f.store.connect().unwrap();
    db.execute("UPDATE ion_automation_runs SET body=json_set(body,'$.stages[0].skipped',json('true')) WHERE id=?",[q.run_id.to_string()]).unwrap();
    let mut next = q;
    next.transfer_id = Some(transfer);
    next.offset = 32768;
    assert!(f.store.read_workflow_pdf_page(&next).is_err());
}
