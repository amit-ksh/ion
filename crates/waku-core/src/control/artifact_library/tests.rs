use super::*;
struct Library {
    store: ControlStore,
    directory: PathBuf,
    project: Uuid,
    task: Uuid,
}
impl Library {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("ion-phone-artifact-tests-{}", Uuid::new_v4()));
        Self {
            store: ControlStore::new(directory.join("control.sqlite")),
            directory,
            project: Uuid::new_v4(),
            task: Uuid::new_v4(),
        }
    }
    fn add(&self, name: &str, format: &str) -> Artifact {
        let snapshot = self
            .store
            .publish_generated_document(
                self.store.load().unwrap().revision,
                name.into(),
                format.into(),
                self.project,
                self.task,
                Uuid::new_v4().to_string(),
                b"published bytes".to_vec(),
            )
            .unwrap();
        snapshot
            .records
            .iter()
            .filter_map(|r| match r {
                ControlRecord::Artifact(a) if a.name == name => Some(a.clone()),
                _ => None,
            })
            .last()
            .unwrap()
    }
    fn query(&self) -> ArtifactLibraryQuery {
        ArtifactLibraryQuery {
            project_id: None,
            task_id: None,
            format: None,
            after: None,
            limit: 2,
        }
    }
}
impl Drop for Library {
    fn drop(&mut self) {
        assert!(
            self.directory.parent() == Some(std::env::temp_dir().as_path())
                && self
                    .directory
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("ion-phone-artifact-tests-")
        );
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
#[test]
fn pages_are_bounded_stable_and_scoped_to_all_filters() {
    let fixture = Library::new();
    for n in 0..5 {
        fixture.add(&format!("doc{n}.md"), "md");
    }
    let mut query = fixture.query();
    let first = fixture.store.list_artifacts(&query).unwrap();
    assert_eq!(first.total, 5);
    assert_eq!(first.items.len(), 2);
    query.after = first.next_cursor.clone();
    let second = fixture.store.list_artifacts(&query).unwrap();
    assert_eq!(second.items.len(), 2);
    assert!(
        second
            .items
            .iter()
            .all(|row| !first.items.iter().any(|a| a.id == row.id))
    );
    for change in 0..4 {
        let mut wrong = query.clone();
        match change {
            0 => wrong.project_id = Some(fixture.project),
            1 => wrong.task_id = Some(fixture.task),
            2 => wrong.format = Some("md".into()),
            _ => wrong.limit = 1,
        };
        assert!(fixture.store.list_artifacts(&wrong).is_err());
    }
}
#[test]
fn unrelated_writes_and_new_artifacts_do_not_replace_the_original_page() {
    let fixture = Library::new();
    for n in 0..4 {
        fixture.add(&format!("doc{n}.md"), "md");
    }
    let mut query = fixture.query();
    let first = fixture.store.list_artifacts(&query).unwrap();
    query.after = first.next_cursor;
    fixture.add("later.md", "md");
    fixture
        .store
        .add_agent_action(WorkspaceAction {
            id: Uuid::new_v4(),
            project_id: fixture.project,
            task_id: fixture.task,
            title: "Later".into(),
            detail: "Unrelated".into(),
            action_kind: WorkspaceActionKind::ProposedTask,
            status: WorkspaceActionStatus::Pending,
            created_at: 100,
        })
        .unwrap();
    let second = fixture.store.list_artifacts(&query).unwrap();
    assert_eq!(second.total, 4);
    assert!(second.items.iter().all(|row| row.name != "later.md"));
}
#[test]
fn changed_or_missing_anchor_refuses_continuation() {
    let fixture = Library::new();
    for n in 0..3 {
        fixture.add(&format!("doc{n}.md"), "md");
    }
    let mut query = fixture.query();
    let first = fixture.store.list_artifacts(&query).unwrap();
    query.after = first.next_cursor;
    let anchor = first.items.last().unwrap();
    let db = fixture.store.connect().unwrap();
    let mut artifact = stored(&db, anchor.id).unwrap();
    artifact.name = "different.md".into();
    db.execute(
        "UPDATE ion_control_records SET body=? WHERE id=?",
        params![
            serde_json::to_string(&ControlRecord::Artifact(artifact)).unwrap(),
            anchor.id.to_string()
        ],
    )
    .unwrap();
    assert!(fixture.store.list_artifacts(&query).is_err());
    db.execute(
        "DELETE FROM ion_control_records WHERE id=?",
        [anchor.id.to_string()],
    )
    .unwrap();
    assert!(fixture.store.list_artifacts(&query).is_err());
}
#[test]
fn project_task_format_and_order_use_actual_document_metadata() {
    let fixture = Library::new();
    let first = fixture.add("first.md", "md");
    fixture.add("second.json", "json");
    let mut query = fixture.query();
    query.project_id = Some(fixture.project);
    query.task_id = Some(fixture.task);
    query.format = Some("md".into());
    let page = fixture.store.list_artifacts(&query).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].id, first.id);
    query.project_id = Some(Uuid::new_v4());
    assert_eq!(fixture.store.list_artifacts(&query).unwrap().total, 0);
}
#[test]
fn old_default_only_imports_and_missing_evidence_are_not_ai_publications() {
    let fixture = Library::new();
    fixture
        .store
        .import_document(
            0,
            "imported.md".into(),
            "md".into(),
            Some(fixture.project),
            Some(fixture.task),
            b"old import".to_vec(),
        )
        .unwrap();
    assert_eq!(
        fixture
            .store
            .list_artifacts(&fixture.query())
            .unwrap()
            .total,
        0
    );
    let artifact = fixture.add("explicit.md", "md");
    let mut legacy = artifact.clone();
    legacy.evidence = None;
    let db = fixture.store.connect().unwrap();
    db.execute(
        "UPDATE ion_control_records SET body=? WHERE id=?",
        params![
            serde_json::to_string(&ControlRecord::Artifact(legacy)).unwrap(),
            artifact.id.to_string()
        ],
    )
    .unwrap();
    assert_eq!(
        fixture
            .store
            .list_artifacts(&fixture.query())
            .unwrap()
            .total,
        0
    );
    assert!(
        fixture
            .store
            .read_artifact_document(&ArtifactDocumentQuery {
                expected: item(&artifact).unwrap()
            })
            .is_err()
    );
}
#[test]
fn full_read_pins_metadata_and_hash_and_rejects_changed_bytes() {
    let fixture = Library::new();
    let artifact = fixture.add("exact.md", "md");
    let expected = item(&artifact).unwrap();
    let read = fixture
        .store
        .read_artifact_document(&ArtifactDocumentQuery {
            expected: expected.clone(),
        })
        .unwrap();
    assert_eq!(read.bytes, b"published bytes");
    for change in 0..5 {
        let mut wrong = expected.clone();
        match change {
            0 => wrong.project_id = Uuid::new_v4(),
            1 => wrong.task_id = Uuid::new_v4(),
            2 => wrong.sha256 = "0".repeat(64),
            3 => wrong.format = "json".into(),
            _ => wrong.name = "wrong.md".into(),
        };
        assert!(
            fixture
                .store
                .read_artifact_document(&ArtifactDocumentQuery { expected: wrong })
                .is_err()
        );
    }
    std::fs::write(
        fixture.store.document_dir().join(artifact.id.to_string()),
        b"substitution",
    )
    .unwrap();
    assert!(
        fixture
            .store
            .read_artifact_document(&ArtifactDocumentQuery { expected })
            .is_err()
    );
}
#[test]
fn metadata_names_bounds_and_cursor_input_fail_closed() {
    let fixture = Library::new();
    for query in [
        ArtifactLibraryQuery {
            limit: 0,
            ..fixture.query()
        },
        ArtifactLibraryQuery {
            limit: 33,
            ..fixture.query()
        },
        ArtifactLibraryQuery {
            format: Some("exe".into()),
            ..fixture.query()
        },
        ArtifactLibraryQuery {
            after: Some("bad-cursor".into()),
            ..fixture.query()
        },
        ArtifactLibraryQuery {
            after: Some("0".repeat(4098)),
            ..fixture.query()
        },
    ] {
        assert!(fixture.store.list_artifacts(&query).is_err());
    }
    let mut artifact = fixture.add("normal.md", "md");
    artifact.name = "../outside.md".into();
    assert!(item(&artifact).is_err());
    artifact.name = "long".repeat(41);
    assert!(item(&artifact).is_err());
}
#[test]
fn library_backfill_indexes_old_explicit_provenance_without_reclassifying_imports() {
    let fixture = Library::new();
    fixture.add("explicit.md", "md");
    fixture
        .store
        .import_document(
            fixture.store.load().unwrap().revision,
            "old.md".into(),
            "md".into(),
            Some(fixture.project),
            Some(fixture.task),
            b"old".to_vec(),
        )
        .unwrap();
    let db = fixture.store.connect().unwrap();
    db.execute_batch("DROP TRIGGER ion_artifact_library_insert; DROP TRIGGER ion_artifact_library_update; DROP TRIGGER ion_artifact_library_delete; DROP TABLE ion_artifact_library; DROP TABLE ion_artifact_library_install;").unwrap();
    drop(db);
    let page = fixture.store.list_artifacts(&fixture.query()).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].name, "explicit.md");
    assert_eq!(
        fixture
            .store
            .list_artifacts(&fixture.query())
            .unwrap()
            .total,
        1
    );
}

#[test]
fn changed_publisher_provenance_invalidates_the_cursor_even_with_same_public_metadata() {
    let fixture = Library::new();
    for n in 0..3 {
        fixture.add(&format!("doc{n}.md"), "md");
    }
    let mut query = fixture.query();
    let first = fixture.store.list_artifacts(&query).unwrap();
    query.after = first.next_cursor;
    let anchor = first.items.last().unwrap();
    let db = fixture.store.connect().unwrap();
    let mut artifact = stored(&db, anchor.id).unwrap();
    artifact.evidence.as_mut().unwrap().producer.activity_id = Some("different-tool-call".into());
    assert_eq!(item(&artifact).unwrap(), *anchor);
    db.execute(
        "UPDATE ion_control_records SET body=? WHERE id=?",
        params![
            serde_json::to_string(&ControlRecord::Artifact(artifact)).unwrap(),
            anchor.id.to_string()
        ],
    )
    .unwrap();
    assert!(fixture.store.list_artifacts(&query).is_err());
}

fn pdf_fixture() -> Vec<u8> {
    let bodies = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 300] /Resources << >> /Contents 4 0 R >>",
        "<< /Length 0 >>\nstream\n\nendstream",
    ];
    let mut bytes = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![0];
    for (i, body) in bodies.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend(format!("{} 0 obj\n{body}\nendobj\n", i + 1).bytes());
    }
    let start = bytes.len();
    bytes.extend(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()).bytes());
    for offset in offsets.iter().skip(1) {
        bytes.extend(format!("{offset:010} 00000 n \n").bytes());
    }
    bytes.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n",
            offsets.len()
        )
        .bytes(),
    );
    bytes
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
#[test]
fn pdf_pages_verify_selected_original_and_bound_native_output() {
    use sha2::{Digest, Sha256};
    let fixture = Library::new();
    let snapshot = fixture
        .store
        .publish_generated_document(
            fixture.store.load().unwrap().revision,
            "page.pdf".into(),
            "pdf".into(),
            fixture.project,
            fixture.task,
            Uuid::new_v4().to_string(),
            pdf_fixture(),
        )
        .unwrap();
    let artifact = snapshot
        .records
        .iter()
        .find_map(|r| match r {
            ControlRecord::Artifact(a) if a.name == "page.pdf" => Some(a),
            _ => None,
        })
        .unwrap();
    let query = ArtifactPdfQuery {
        expected: item(artifact).unwrap(),
        page: 0,
        width: 800,
    };
    let page = fixture.store.render_artifact_pdf(&query).unwrap();
    assert_eq!(page.document, query.expected);
    assert_eq!(
        (
            page.page,
            page.pages,
            page.requested_width,
            page.width,
            page.height
        ),
        (0, 1, 800, 800, 1200)
    );
    assert!(page.png.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]));
    assert_eq!(page.sha256, format!("{:x}", Sha256::digest(&page.png)));
    for change in 0..7 {
        let mut invalid = query.clone();
        match change {
            0 => invalid.page = 32,
            1 => invalid.width = 10000,
            2 => invalid.expected.bytes = 65537,
            3 => invalid.expected.format = "html".into(),
            4 => invalid.expected.project_id = Uuid::new_v4(),
            5 => invalid.expected.sha256 = "0".repeat(64),
            _ => invalid.page = 1,
        }
        assert!(fixture.store.render_artifact_pdf(&invalid).is_err());
    }
    std::fs::write(
        fixture.store.document_dir().join(artifact.id.to_string()),
        b"%PDF-changed",
    )
    .unwrap();
    assert!(fixture.store.render_artifact_pdf(&query).is_err());
}
