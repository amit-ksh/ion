use super::*;
use crate::attachments::{AttachmentStore, AttachmentUpload, AttachmentUploadEntry};
use base64::Engine as _;
use std::fs;
use waku_protocol::model::MessageAttachment;

fn attachment(f: &Fixture, name: &str, bytes: &[u8]) -> (AttachmentStore, MessageAttachment) {
    let store = AttachmentStore::new(f.dir.join("attachments"));
    let stored = store
        .import(
            name,
            AttachmentUpload::File {
                data_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
            },
        )
        .unwrap();
    let input = MessageAttachment {
        path: stored.path,
        mention: stored.name.clone(),
        name: stored.name,
        is_dir: false,
        is_image: false,
        blob_reference: Some(stored.reference),
    };
    (store, input)
}
#[test]
fn team_inputs_pin_bytes_replay_and_original_task_presentation_without_generating_artifacts() {
    let f = Fixture::new();
    let (store, input) = attachment(&f, "brief.md", b"Original user context\n");
    let mut request = f.request();
    request.source_prompt = Some("@{Product team} Implement and validate".into());
    request.source_attachments = vec![input.clone()];
    request.input_data = inputs::snapshot(&store, &request.source_attachments).unwrap();
    let run = f.store.queue_task(request.clone(), 100).unwrap();
    assert_eq!(run.inputs[0].name, "brief.md");
    let materialized = f.store.pending_materializations(8).unwrap();
    let session = super::super::materialize::task_session(&materialized[0]);
    assert_eq!(session.messages[0].attachments, vec![input.clone()]);
    assert_eq!(
        session.messages[0].content,
        "@{Product team} Implement and validate"
    );
    fs::write(&input.path, b"Changed upload").unwrap();
    assert_eq!(
        f.store.queued_input_data(run.id, &[input.clone()]).unwrap()[0].content,
        "Original user context\n"
    );
    assert_eq!(f.store.queue_task(request.clone(), 101).unwrap().id, run.id);
    request.input_data[0].content = "Changed request".into();
    assert!(f.store.queue_task(request, 101).is_err());
    let mut changed = input.clone();
    changed.name = "other.md".into();
    assert!(f.store.queued_input_data(run.id, &[changed]).is_err());
    f.materialize();
    let claim = f.claim(run.id, run.work[0].id);
    let reopened = TeamCoordinator::new(f.definitions.path());
    assert_eq!(
        reopened
            .read_assignment_input(&claim.lease, &run.inputs[0].key, 1, 200, 102)
            .unwrap()["content"],
        "Original user context\n"
    );
    assert!(
        !f.definitions
            .load()
            .unwrap()
            .records
            .iter()
            .any(|r| matches!(r, ControlRecord::Artifact(_)))
    );
    let db = f.store.connect().unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM ion_team_input_files", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        1
    );
}
#[test]
fn team_inputs_follow_intersected_child_grants_run_scope_and_revoked_leases() {
    let f = Fixture::new();
    let (store, input) = attachment(&f, "brief.txt", b"User evidence");
    let mut request = f.request();
    request.source_attachments = vec![input];
    request.input_data = inputs::snapshot(&store, &request.source_attachments).unwrap();
    let run = f.store.prepare(request, 100).unwrap();
    let root = f.claim(run.id, run.work[0].id);
    let child = f.store.delegate(&root.lease, f.delegation(1), 102).unwrap();
    let mut denied = f.delegation(1);
    denied.access.workspace_read = false;
    let blocked = f.store.delegate(&root.lease, denied, 102).unwrap();
    f.materialize();
    let c = f.claim(run.id, child.id);
    assert_eq!(
        f.store.list_assignment_inputs(&c.lease, 102).unwrap(),
        run.inputs
    );
    assert_eq!(
        f.store
            .read_assignment_input(&c.lease, &run.inputs[0].key, 1, 200, 102)
            .unwrap()["content"],
        "User evidence\n"
    );
    f.store
        .report(
            &c.lease,
            AssignmentOutcome::Blocked("Stopped for test".into()),
            103,
        )
        .unwrap();
    assert!(f.store.list_assignment_inputs(&c.lease, 104).is_err());
    let denied = f.claim(run.id, blocked.id);
    assert!(f.store.list_assignment_inputs(&denied.lease, 104).is_err());
    assert!(
        f.store
            .read_assignment_input(&denied.lease, &run.inputs[0].key, 1, 200, 104)
            .is_err()
    );
    assert!(
        f.store
            .read_assignment_input(&root.lease, "other-run/input.txt", 1, 200, 104)
            .is_err()
    );
    assert!(
        f.store
            .read_assignment_input(&root.lease, &run.inputs[0].key, 0, 200, 104)
            .is_err()
    );
    let before = f.store.snapshot(run.id).unwrap();
    f.store.stop(run.id, before.revision, 104).unwrap();
    assert!(
        f.store
            .read_assignment_input(&root.lease, &run.inputs[0].key, 1, 200, 105)
            .is_err()
    );
}
#[test]
fn team_inputs_bound_escaped_replies_and_keep_input_bytes_out_of_projections() {
    let f = Fixture::new();
    let bytes = "é\t\u{1}".repeat(16000);
    let (store, input) = attachment(&f, "brief.txt", bytes.as_bytes());
    let mut request = f.request();
    request.source_attachments = vec![input];
    request.input_data = inputs::snapshot(&store, &request.source_attachments).unwrap();
    let run = f.store.prepare(request, 100).unwrap();
    let root = f.claim(run.id, run.work[0].id);
    let reply = f
        .store
        .read_assignment_input(&root.lease, &run.inputs[0].key, 1, 200, 102)
        .unwrap();
    assert!(reply["truncated"].as_bool().unwrap());
    assert!(reply.to_string().len() < 12 * 1024);
    assert_eq!(reply["source"], "user_input");
    assert_eq!(reply["instructions"], false);
    assert!(!serde_json::to_string(&run).unwrap().contains(&bytes));
}
#[test]
fn team_inputs_accept_sorted_folder_files_and_reject_unsafe_refs_binary_private_and_large_inputs() {
    let f = Fixture::new();
    let store = AttachmentStore::new(f.dir.join("attachments"));
    let entry = |path: &str, text: &[u8]| AttachmentUploadEntry {
        relative_path: path.into(),
        data_base64: base64::engine::general_purpose::STANDARD.encode(text),
    };
    let folder = store
        .import(
            "research",
            AttachmentUpload::Directory {
                entries: vec![entry("z.txt", b"last"), entry("a/brief.md", b"first")],
            },
        )
        .unwrap();
    let input = MessageAttachment {
        path: folder.path,
        mention: folder.name.clone(),
        name: folder.name,
        is_dir: true,
        is_image: false,
        blob_reference: Some(folder.reference),
    };
    let files = inputs::snapshot(&store, &[input.clone()]).unwrap();
    assert_eq!(
        files.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
        vec!["research/a/brief.md", "research/z.txt"]
    );
    let mut changed = input.clone();
    changed.path = f.dir.join("outside.txt");
    assert!(inputs::snapshot(&store, &[changed]).is_err());
    let mut changed = input.clone();
    changed.name = "../outside".into();
    assert!(inputs::snapshot(&store, &[changed]).is_err());
    let mut changed = input.clone();
    changed.blob_reference = Some("waku-attachment:../other".into());
    assert!(inputs::snapshot(&store, &[changed]).is_err());
    let mut changed = input.clone();
    changed.is_dir = false;
    assert!(inputs::snapshot(&store, &[changed]).is_err());
    for (name, bytes) in [
        (".env", b"SECRET".as_slice()),
        ("binary.txt", b"a\0b".as_slice()),
        ("brief.pdf", b"%PDF-1.7".as_slice()),
        ("invalid.txt", [255].as_slice()),
    ] {
        let (store, input) = attachment(&f, name, bytes);
        assert!(inputs::snapshot(&store, &[input]).is_err());
    }
    let (store, input) = attachment(&f, "big.txt", &vec![b'x'; 256 * 1024 + 1]);
    assert!(inputs::snapshot(&store, &[input]).is_err());
    let (store, input) = attachment(&f, "brief.txt", b"evidence");
    assert!(inputs::snapshot(&store, &vec![input; 9]).is_err());
    let mut request = f.request();
    request.source_attachments = vec![];
    request.input_data = files;
    assert!(f.store.prepare(request, 100).is_err());
    assert_eq!(
        f.store
            .connect()
            .unwrap()
            .query_row("SELECT count(*) FROM ion_team_runs", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
}
#[cfg(unix)]
#[test]
fn team_inputs_reject_links_inside_a_managed_folder() {
    let f = Fixture::new();
    let store = AttachmentStore::new(f.dir.join("attachments"));
    let directory = store
        .import(
            "research",
            AttachmentUpload::Directory {
                entries: vec![AttachmentUploadEntry {
                    relative_path: "brief.md".into(),
                    data_base64: base64::engine::general_purpose::STANDARD.encode(b"evidence"),
                }],
            },
        )
        .unwrap();
    fs::write(f.dir.join("outside.txt"), b"Private outside").unwrap();
    std::os::unix::fs::symlink(f.dir.join("outside.txt"), directory.path.join("link.txt")).unwrap();
    let input = MessageAttachment {
        path: directory.path,
        mention: directory.name.clone(),
        name: directory.name,
        is_dir: true,
        is_image: false,
        blob_reference: Some(directory.reference),
    };
    assert!(inputs::snapshot(&store, &[input]).is_err());
}

#[test]
fn team_inputs_enforce_combined_count_size_and_lead_read_bounds_before_creating_a_run() {
    let f = Fixture::new();
    let (store, input) = attachment(&f, "brief.md", b"evidence");
    let files = inputs::snapshot(&store, &[input.clone()]).unwrap();
    let mut request = f.request();
    request.source_attachments = vec![input.clone()];
    request.input_data = files.clone();
    request.access.workspace_read = false;
    request.access.workspace_write = false;
    assert!(f.store.prepare(request, 100).is_err());
    let many = (0..65)
        .map(|i| PreparedInput {
            key: format!("{i}/brief.md"),
            name: "brief.md".into(),
            content: "data".into(),
        })
        .collect::<Vec<_>>();
    assert!(inputs::validate(&many, &[input.clone()]).is_err());
    let large = (0..9)
        .map(|i| PreparedInput {
            key: format!("{i}/brief.md"),
            name: "brief.md".into(),
            content: "x".repeat(256 * 1024),
        })
        .collect::<Vec<_>>();
    assert!(inputs::validate(&large, &[input]).is_err());
    assert_eq!(
        f.store
            .connect()
            .unwrap()
            .query_row("SELECT count(*) FROM ion_team_runs", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
}
