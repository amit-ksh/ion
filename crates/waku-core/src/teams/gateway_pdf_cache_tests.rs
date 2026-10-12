use super::*;
use waku_protocol::gateway_portfolio::pdf::WorkflowPdfDocument;
fn query() -> WorkflowPdfQuery {
    WorkflowPdfQuery {
        run_id: Uuid::new_v4(),
        revision: 7,
        expected: WorkflowPdfDocument {
            id: Uuid::new_v4(),
            name: "Verified.pdf".into(),
            format: "pdf".into(),
            project_id: Uuid::new_v4(),
            task_id: None,
            stage: 0,
            team_run_id: Uuid::new_v4(),
            assignment_id: Uuid::new_v4(),
            bytes: 100,
            sha256: "a".repeat(64),
            created_at: 100,
        },
        page: 0,
        width: 800,
        transfer_id: None,
        offset: 0,
        limit: 32768,
    }
}
fn page(q: &WorkflowPdfQuery, count: usize) -> Page {
    let png: Vec<u8> = (0..count).map(|i| (i % 251) as u8).collect();
    Page {
        id: Uuid::new_v4(),
        query: q.clone(),
        witness: "private-accepted-source".into(),
        created: Instant::now(),
        expires_at: 1000 + TTL.as_millis() as u64,
        sha256: format!("{:x}", Sha256::digest(&png)),
        png,
        pages: 1,
        width: 800,
        height: 1000,
        last_access: Instant::now(),
    }
}
#[test]
fn bounded_chunks_allow_exact_parallel_offsets_and_replay_without_rerender_or_private_fields() {
    let mut q = query();
    let mut cache = PageCache::default();
    let p = page(&q, 12 * 1024 * 1024);
    let original = p.png.clone();
    q.transfer_id = Some(p.id);
    cache.insert(p);
    // Reverse-order receipt completion is valid; offset/bytes/digest stay immutable.
    for offset in [
        32768,
        7 * 32768,
        3 * 32768,
        0,
        0,
        (original.len() - 32768) as u32,
    ] {
        q.offset = offset;
        let result = cache.chunk(&q, "private-accepted-source", 1000).unwrap();
        let row = &result["page"];
        assert_eq!(
            STANDARD.decode(row["data"].as_str().unwrap()).unwrap(),
            original[offset as usize..offset as usize + 32768]
        );
        assert!(result.to_string().len() < 60000);
        assert!(!result.to_string().contains("private-accepted-source"));
        assert_eq!(cache.pages.len(), 1);
        assert_eq!(
            row["nextOffset"],
            if offset as usize + 32768 < original.len() {
                json!(offset + 32768)
            } else {
                Value::Null
            }
        );
    }
}
#[test]
fn cache_rejects_foreign_transfer_source_metadata_page_and_zoom() {
    let mut q = query();
    let mut cache = PageCache::default();
    let p = page(&q, 65536);
    q.transfer_id = Some(p.id);
    cache.insert(p);
    for change in 0..7 {
        let mut invalid = q.clone();
        match change {
            0 => invalid.transfer_id = Some(Uuid::new_v4()),
            1 => invalid.run_id = Uuid::new_v4(),
            2 => invalid.revision += 1,
            3 => invalid.expected.assignment_id = Uuid::new_v4(),
            4 => invalid.page = 1,
            5 => invalid.width = 1200,
            _ => invalid.offset = 65536,
        };
        assert!(
            cache
                .chunk(&invalid, "private-accepted-source", 1000)
                .is_err()
        );
    }
    assert!(cache.chunk(&q, "changed-private-source", 1000).is_err());
}
#[test]
fn cache_expires_and_evicts_with_strict_page_and_byte_admission() {
    let q = query();
    let mut cache = PageCache::default();
    let mut expired = page(&q, 65536);
    let expired_id = expired.id;
    expired.created = Instant::now() - TTL;
    cache.insert(expired);
    let mut request = q.clone();
    request.transfer_id = Some(expired_id);
    assert!(
        cache
            .chunk(&request, "private-accepted-source", 1000)
            .is_err()
    );
    for _ in 0..8 {
        cache.insert(page(&q, 12 * 1024 * 1024));
        assert!(cache.pages.len() <= 2);
        assert!(cache.pages.iter().map(|p| p.png.len()).sum::<usize>() <= CACHE_BYTES);
    }
    assert_eq!(cache.pages.len(), 2);
}
