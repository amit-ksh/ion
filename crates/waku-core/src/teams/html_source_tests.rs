use super::*;
use sha2::{Digest, Sha256};

struct SourcesFixture {
    f: Fixture,
    lease: AssignmentLease,
    request: Uuid,
    args: HtmlBundleArgs,
    originals: Vec<(&'static str, &'static str)>,
}
impl SourcesFixture {
    fn new() -> Self {
        let f = Fixture::new();
        let run = f.run();
        let lease = f.claim(run.id, run.work[0].id).lease;
        let originals = vec![
            (
                "index.html",
                r##"<link rel="stylesheet" href="theme.css"><svg><use href="a.svg#badge"/><use href="b.svg#badge"/></svg>"##,
            ),
            (
                "b.svg",
                r##"<svg xmlns="http://www.w3.org/2000/svg"><style>@keyframes pulse{to{opacity:.8}}.shape{--chosen:pulse;animation:1s var(--chosen)}</style><symbol id="badge"><text class="shape">Second</text></symbol></svg>"##,
            ),
            ("theme.css", ".page{display:grid;color:green}"),
            (
                "a.svg",
                r##"<svg xmlns="http://www.w3.org/2000/svg"><style>@keyframes pulse{to{opacity:.2}}.shape{--chosen:pulse;animation-name:var(--chosen)}</style><symbol id="badge"><text class="shape">First</text></symbol></svg>"##,
            ),
        ];
        let sha = |content: &str| format!("{:x}", Sha256::digest(content.as_bytes()));
        let args: HtmlBundleArgs = serde_json::from_value(serde_json::json!({"path":"index.html", "sha256":sha(originals[0].1), "name":"Report.html", "summary":"Report with independent SVG documents", "resources":originals[1..].iter().map(|(path, content)| serde_json::json!({"path":path,"sha256":sha(content)})).collect::<Vec<_>>()})).unwrap();
        for (path, content) in &originals {
            std::fs::write(f.dir.join(path), content).unwrap();
        }
        Self {
            f,
            lease,
            request: Uuid::new_v4(),
            args,
            originals,
        }
    }
    fn publish(&self) -> Artifact {
        self.f
            .store
            .publish_html_bundle(&self.lease, self.request, &self.f.dir, &self.args, 102)
            .unwrap()
    }
    fn remove_originals(&self) {
        for (path, content) in &self.originals {
            assert_eq!(
                std::fs::read_to_string(self.f.dir.join(path)).unwrap(),
                *content
            );
            std::fs::remove_file(self.f.dir.join(path)).unwrap();
        }
    }
}
#[test]
fn html_sources_retain_exact_ordered_documents_after_workspace_removal_and_restart() {
    let f = SourcesFixture::new();
    let artifact = f.publish();
    let portable = f.f.definitions.read_document(artifact.id).unwrap();
    assert!(
        std::str::from_utf8(&portable)
            .unwrap()
            .contains("animation_pulse")
    );
    f.remove_originals();
    let reopened = TeamCoordinator::new(f.f.definitions.path());
    let saved = reopened
        .read_html_bundle_sources(&artifact)
        .unwrap()
        .unwrap();
    assert_eq!(saved.artifact_id, artifact.id);
    assert_eq!(
        saved.artifact_sha256,
        format!("{:x}", Sha256::digest(&portable))
    );
    assert_eq!(saved.entry_path, "index.html");
    assert_eq!(
        saved.bytes_total,
        f.originals
            .iter()
            .map(|(_, content)| content.len() as u64)
            .sum::<u64>()
    );
    assert_eq!(saved.sources.len(), f.originals.len());
    for (item, (path, content)) in saved.sources.iter().zip(&f.originals) {
        assert_eq!(item.path, *path);
        assert_eq!(item.bytes, content.as_bytes());
        assert_eq!(
            item.sha256,
            format!("{:x}", Sha256::digest(content.as_bytes()))
        );
        assert_eq!(
            item.media_type,
            match *path {
                "index.html" => "text/html",
                "theme.css" => "text/css",
                _ => "image/svg+xml",
            }
        );
    }
    assert_eq!(
        reopened
            .publish_html_bundle(&f.lease, f.request, &f.f.dir, &f.args, 103)
            .unwrap()
            .id,
        artifact.id
    );
    assert_eq!(
        reopened
            .read_html_bundle_sources(&artifact)
            .unwrap()
            .unwrap(),
        saved
    );
}
#[test]
fn html_sources_require_the_exact_current_artifact_and_do_not_enter_model_reads() {
    let f = SourcesFixture::new();
    let artifact = f.publish();
    let mut changed = artifact.clone();
    changed.name = "Another.html".into();
    assert!(f.f.store.read_html_bundle_sources(&changed).is_err());
    let bytes = f.f.definitions.read_document(artifact.id).unwrap();
    let reply = f.f.store.document_model_reply(artifact.id, &bytes).unwrap();
    assert_eq!(reply["summary"], f.args.summary);
    let encoded = serde_json::to_string(&reply).unwrap();
    assert!(!encoded.contains("--chosen"));
    assert!(!encoded.contains("@keyframes"));
    assert!(!encoded.contains("<svg"));
}
#[test]
fn html_sources_refuse_corruption_membership_mime_and_size_changes_without_hiding_original_export()
{
    for mutation in [
        "UPDATE ion_team_html_sources SET content=x'6368616e676564' WHERE path='theme.css'",
        "UPDATE ion_team_html_sources SET mime='text/plain' WHERE path='a.svg'",
        "UPDATE ion_team_html_sources SET sha256=printf('%064d',0) WHERE path='a.svg'",
        "DELETE FROM ion_team_html_sources WHERE path='a.svg'",
        "UPDATE ion_team_html_source_archives SET source_count=33",
        "UPDATE ion_team_html_source_archives SET byte_count=9437184",
        "INSERT INTO ion_team_html_sources SELECT artifact_id,'extra.css',mime,sha256,content FROM ion_team_html_sources WHERE path='theme.css'",
        "PRAGMA ignore_check_constraints=ON;UPDATE ion_team_html_sources SET content=zeroblob(2097153) WHERE path='a.svg'",
        "DELETE FROM ion_team_html_sources;DELETE FROM ion_team_html_source_archives",
    ] {
        let f = SourcesFixture::new();
        let artifact = f.publish();
        let original = f.f.definitions.read_document(artifact.id).unwrap();
        let db = f.f.store.connect().unwrap();
        db.execute_batch(mutation).unwrap();
        assert!(
            f.f.store.read_html_bundle_sources(&artifact).is_err(),
            "{mutation}"
        );
        assert_eq!(
            f.f.definitions.read_document(artifact.id).unwrap(),
            original
        );
        // A replay preserves the existing publication; it cannot silently repair
        // lost sources using files that may have changed or been removed.
        f.remove_originals();
        assert_eq!(
            f.f.store
                .publish_html_bundle(&f.lease, f.request, &f.f.dir, &f.args, 103)
                .unwrap()
                .id,
            artifact.id
        );
        assert!(f.f.store.read_html_bundle_sources(&artifact).is_err());
    }
}
#[test]
fn html_sources_legacy_receipts_remain_exportable_without_fabricated_source_history() {
    let f = SourcesFixture::new();
    let artifact = f.publish();
    let original = f.f.definitions.read_document(artifact.id).unwrap();
    let db = f.f.store.connect().unwrap();
    let signature = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(&artifact, &f.args)).unwrap())
    );
    db.execute(
        "UPDATE ion_team_html_bundles SET signature=? WHERE artifact_id=?",
        params![signature, artifact.id.to_string()],
    )
    .unwrap();
    db.execute("DELETE FROM ion_team_html_sources", []).unwrap();
    db.execute("DELETE FROM ion_team_html_source_archives", [])
        .unwrap();
    f.remove_originals();
    assert!(
        f.f.store
            .read_html_bundle_sources(&artifact)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        f.f.definitions.read_document(artifact.id).unwrap(),
        original
    );
    assert_eq!(
        f.f.store
            .publish_html_bundle(&f.lease, f.request, &f.f.dir, &f.args, 103)
            .unwrap()
            .id,
        artifact.id
    );
}
#[test]
fn html_sources_failed_storage_rolls_back_publication_receipts_and_artifact_together() {
    let f = SourcesFixture::new();
    let db = f.f.store.connect().unwrap();
    db.execute_batch("CREATE TRIGGER refuse_html_source BEFORE INSERT ON ion_team_html_sources BEGIN SELECT RAISE(ABORT,'fixture source failure');END;").unwrap();
    assert!(
        f.f.store
            .publish_html_bundle(&f.lease, f.request, &f.f.dir, &f.args, 102)
            .is_err()
    );
    for table in [
        "ion_team_html_bundles",
        "ion_team_html_source_archives",
        "ion_team_html_sources",
        "ion_team_document_receipts",
    ] {
        assert_eq!(
            db.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            0
        );
    }
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM ion_control_records WHERE kind='artifact'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    db.execute_batch("DROP TRIGGER refuse_html_source").unwrap();
    let artifact = f.publish();
    assert!(
        f.f.store
            .read_html_bundle_sources(&artifact)
            .unwrap()
            .is_some()
    );
}

#[test]
fn html_sources_actual_publication_inherits_holds_and_cleanup_revokes_source_reads() {
    use waku_protocol::artifact_retention::ArtifactCleanupRequest;
    let f = SourcesFixture::new();
    let artifact = f.publish();
    let request = ArtifactCleanupRequest {
        request_id: Uuid::new_v4(),
        expected: artifact.clone(),
    };
    assert!(
        f.f.definitions
            .cleanup_artifact(&request, &HashSet::new(), &HashSet::new(), 103)
            .is_err()
    );
    assert!(
        f.f.store
            .read_html_bundle_sources(&artifact)
            .unwrap()
            .is_some()
    );
    let run_id = artifact.evidence.as_ref().unwrap().producer.run_id.unwrap();
    let run = f.f.store.snapshot(run_id).unwrap();
    f.f.store.stop(run_id, run.revision, 103).unwrap();
    f.f.store
        .report(&f.lease, AssignmentOutcome::Canceled, 104)
        .unwrap();
    let receipt =
        f.f.definitions
            .cleanup_artifact(&request, &HashSet::new(), &HashSet::new(), 105)
            .unwrap();
    assert_eq!(
        f.f.definitions
            .cleanup_artifact(&request, &HashSet::new(), &HashSet::new(), 106)
            .unwrap()
            .cleaned_at,
        receipt.cleaned_at
    );
    assert!(f.f.definitions.read_document(artifact.id).is_err());
    assert!(f.f.store.read_html_bundle_sources(&artifact).is_err());
    let db = f.f.store.connect().unwrap();
    assert_eq!(
        db.query_row(
            "SELECT sum(length(content)) FROM ion_team_html_sources WHERE artifact_id=?",
            [artifact.id.to_string()],
            |r| r.get::<_, u64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT source_count FROM ion_team_html_source_archives WHERE artifact_id=?",
            [artifact.id.to_string()],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        4
    );
    for (path, content) in &f.originals {
        assert_eq!(
            std::fs::read_to_string(f.f.dir.join(path)).unwrap(),
            *content
        );
    }
}
#[test]
fn html_sources_receipt_removal_cascades_copies_and_refuses_remaining_payload() {
    let f = SourcesFixture::new();
    let artifact = f.publish();
    let db = f.f.store.connect().unwrap();
    db.execute(
        "DELETE FROM ion_team_html_bundles WHERE artifact_id=?",
        [artifact.id.to_string()],
    )
    .unwrap();
    for table in ["ion_team_html_source_archives", "ion_team_html_sources"] {
        assert_eq!(
            db.query_row(
                &format!("SELECT count(*) FROM {table} WHERE artifact_id=?"),
                [artifact.id.to_string()],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            0
        );
    }
    assert!(f.f.definitions.read_document(artifact.id).is_err());
    assert!(f.f.store.read_html_bundle_sources(&artifact).is_err());
}

fn resource_transfer_fixture() -> serde_json::Value {
    use waku_protocol::{artifact_library::*, html_resources::*};
    let mut f = SourcesFixture::new();
    let entry = "経緯🚀.html";
    std::fs::rename(f.f.dir.join("index.html"), f.f.dir.join(entry)).unwrap();
    f.args.path = entry.into();
    f.originals[0].0 = entry;
    let css = format!(
        "@import 'empty.css';.page{{color:green}}/*{}*/",
        "x".repeat(50_000)
    );
    std::fs::write(f.f.dir.join("theme.css"), &css).unwrap();
    f.args
        .resources
        .iter_mut()
        .find(|r| r.path == "theme.css")
        .unwrap()
        .sha256 = format!("{:x}", Sha256::digest(css.as_bytes()));
    std::fs::write(f.f.dir.join("empty.css"), b"").unwrap();
    let inspected =
        f.f.store
            .inspect_file_document(&f.lease, &f.f.dir, "empty.css", 102)
            .unwrap();
    assert_eq!(inspected["bytes"], 0);
    assert_eq!(inspected["sha256"], format!("{:x}", Sha256::digest(b"")));
    std::fs::write(f.f.dir.join("empty.woff2"), b"").unwrap();
    assert!(
        f.f.store
            .inspect_file_document(&f.lease, &f.f.dir, "empty.woff2", 102)
            .is_err()
    );
    std::fs::remove_file(f.f.dir.join("empty.woff2")).unwrap();
    f.args.resources.push(
        serde_json::from_value(
            serde_json::json!({"path":"empty.css","sha256":format!("{:x}",Sha256::digest(b""))}),
        )
        .unwrap(),
    );
    let artifact = f.publish();
    let expected =
        f.f.definitions
            .list_artifacts(&ArtifactLibraryQuery {
                project_id: None,
                task_id: None,
                format: None,
                after: None,
                limit: 32,
            })
            .unwrap()
            .items
            .into_iter()
            .find(|a| a.id == artifact.id)
            .unwrap();
    let original = f.f.definitions.read_document(artifact.id).unwrap();
    let manifest =
        f.f.store
            .read_artifact_html_manifest(&ArtifactDocumentQuery {
                expected: expected.clone(),
            })
            .unwrap()
            .manifest
            .unwrap();
    let (root, total) = commitment(&manifest.entry_path, &manifest.sources).unwrap();
    assert_eq!(root, manifest.sha256);
    assert_eq!(total, manifest.total_bytes);
    assert!(original.starts_with(anchor(&root).as_bytes()));
    assert_eq!(manifest.entry_path, entry);
    for path in std::iter::once(&f.args.path).chain(f.args.resources.iter().map(|r| &r.path)) {
        std::fs::remove_file(f.f.dir.join(path)).unwrap();
    }
    let mut chunks = Vec::new();
    for source in &manifest.sources {
        let mut offset = 0;
        let mut body = Vec::new();
        while offset < source.bytes {
            let query = HtmlSourceQuery {
                manifest_sha256: root.clone(),
                source_index: source.index,
                source_sha256: source.sha256.clone(),
                offset,
                limit: CHUNK_BYTES,
            };
            let reply =
                f.f.store
                    .read_artifact_html_source(&ArtifactHtmlSourceQuery {
                        expected: expected.clone(),
                        source: query,
                    })
                    .unwrap();
            assert_eq!(reply.document, expected);
            assert!(reply.chunk.data.len() <= usize::from(CHUNK_BYTES));
            body.extend(&reply.chunk.data);
            offset += reply.chunk.data.len() as u32;
            assert_eq!(
                reply.chunk.next_offset,
                (offset < source.bytes).then_some(offset)
            );
            chunks.push(reply.chunk);
        }
        assert_eq!(format!("{:x}", Sha256::digest(&body)), source.sha256);
    }
    serde_json::json!({"expected":expected,"original":STANDARD.encode(&original),"manifest":manifest,"chunks":chunks})
}
#[test]
fn html_resources_direct_transfer_retains_publication_commitment_unicode_and_chunk_cursors() {
    resource_transfer_fixture();
}
#[test]
#[ignore = "Exports actual HTML resource RPC shapes to ION_HTML_RESOURCES_FIXTURE"]
fn native_html_resources_phone_fixture() {
    std::fs::write(
        std::env::var_os("ION_HTML_RESOURCES_FIXTURE").expect("Set ION_HTML_RESOURCES_FIXTURE"),
        serde_json::to_vec(&resource_transfer_fixture()).unwrap(),
    )
    .unwrap();
}
#[test]
fn html_resources_transfer_refuses_selection_receipt_anchor_scope_and_cursor_changes() {
    use waku_protocol::html_resources::*;
    let f = SourcesFixture::new();
    let artifact = f.publish();
    let manifest =
        f.f.store
            .html_resource_manifest(&artifact)
            .unwrap()
            .unwrap();
    let query = HtmlSourceQuery {
        manifest_sha256: manifest.sha256.clone(),
        source_index: 0,
        source_sha256: manifest.sources[0].sha256.clone(),
        offset: 0,
        limit: CHUNK_BYTES,
    };
    assert!(f.f.store.html_resource_chunk(&artifact, &query).is_ok());
    for value in [
        serde_json::json!({"manifestSha256":"0".repeat(64)}),
        serde_json::json!({"sourceSha256":"0".repeat(64)}),
        serde_json::json!({"sourceIndex":32}),
        serde_json::json!({"offset":1}),
        serde_json::json!({"offset":16384}),
        serde_json::json!({"limit":1}),
    ] {
        let mut changed = serde_json::to_value(&query).unwrap();
        for (k, v) in value.as_object().unwrap() {
            changed[k] = v.clone();
        }
        let changed: HtmlSourceQuery = serde_json::from_value(changed).unwrap();
        assert!(f.f.store.html_resource_chunk(&artifact, &changed).is_err());
    }
    let mut changed = artifact.clone();
    changed.name = "Other.html".into();
    assert!(f.f.store.html_resource_manifest(&changed).is_err());
    let db = f.f.store.connect().unwrap();
    db.execute("UPDATE ion_team_document_receipts SET content=zeroblob(length(content)) WHERE artifact_id=?",[artifact.id.to_string()]).unwrap();
    assert!(f.f.store.html_resource_manifest(&artifact).is_err());
    assert!(f.f.store.html_resource_chunk(&artifact, &query).is_err());
}
#[test]
fn html_resources_uncommitted_v1_archives_stay_local_and_cleanup_fences_new_chunks() {
    use waku_protocol::{artifact_retention::ArtifactCleanupRequest, html_resources::*};
    let f = SourcesFixture::new();
    let artifact = f.publish();
    let db = f.f.store.connect().unwrap();
    let v1 = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&("ion-html-source-archive-v1", &artifact, &f.args)).unwrap()
        )
    );
    db.execute(
        "UPDATE ion_team_html_bundles SET signature=? WHERE artifact_id=?",
        params![v1, artifact.id.to_string()],
    )
    .unwrap();
    assert!(
        f.f.store
            .read_html_bundle_sources(&artifact)
            .unwrap()
            .is_some()
    );
    assert!(
        f.f.store
            .html_resource_manifest(&artifact)
            .unwrap()
            .is_none()
    );
    let v2 = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&("ion-html-source-archive-v2", &artifact, &f.args)).unwrap()
        )
    );
    db.execute(
        "UPDATE ion_team_html_bundles SET signature=? WHERE artifact_id=?",
        params![v2, artifact.id.to_string()],
    )
    .unwrap();
    let manifest =
        f.f.store
            .html_resource_manifest(&artifact)
            .unwrap()
            .unwrap();
    let query = HtmlSourceQuery {
        manifest_sha256: manifest.sha256,
        source_index: 0,
        source_sha256: manifest.sources[0].sha256.clone(),
        offset: 0,
        limit: CHUNK_BYTES,
    };
    let run_id = artifact.evidence.as_ref().unwrap().producer.run_id.unwrap();
    let run = f.f.store.snapshot(run_id).unwrap();
    f.f.store.stop(run_id, run.revision, 103).unwrap();
    f.f.store
        .report(&f.lease, AssignmentOutcome::Canceled, 104)
        .unwrap();
    f.f.definitions
        .cleanup_artifact(
            &ArtifactCleanupRequest {
                request_id: Uuid::new_v4(),
                expected: artifact.clone(),
            },
            &HashSet::new(),
            &HashSet::new(),
            105,
        )
        .unwrap();
    assert!(f.f.store.html_resource_manifest(&artifact).is_err());
    assert!(f.f.store.html_resource_chunk(&artifact, &query).is_err());
}
