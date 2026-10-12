//! Expiring derived pages owned by this coordinator, never by a UI or gateway.
//! All database/file/render work stays on daemon workers. Cached PNG bytes are
//! immutable and every chunk requires fresh accepted-source verification.
use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
use waku_protocol::gateway_portfolio::{history, pdf::WorkflowPdfQuery};
const TTL: Duration = Duration::from_secs(10 * 60);
const CACHE_PAGES: usize = 2;
const CACHE_BYTES: usize = 24 * 1024 * 1024;
#[derive(Default)]
pub(super) struct PageCache {
    pages: Vec<Page>,
}
struct Page {
    id: Uuid,
    query: WorkflowPdfQuery,
    witness: String,
    created: Instant,
    expires_at: u64,
    png: Vec<u8>,
    sha256: String,
    pages: u32,
    width: u32,
    height: u32,
    last_access: Instant,
}
impl PageCache {
    fn expire(&mut self) {
        self.pages.retain(|page| page.created.elapsed() < TTL);
    }
    fn insert(&mut self, page: Page) {
        self.expire();
        while self.pages.len() >= CACHE_PAGES
            || self.pages.iter().map(|p| p.png.len()).sum::<usize>() + page.png.len() > CACHE_BYTES
        {
            self.pages.remove(0);
        }
        self.pages.push(page);
    }
    fn chunk(&mut self, query: &WorkflowPdfQuery, witness: &str, now: u64) -> Result<Value> {
        self.expire();
        let page = self
            .pages
            .iter_mut()
            .find(|p| Some(p.id) == query.transfer_id)
            .context("PDF image transfer expired. Retry its page preview.")?;
        ensure!(
            page.witness == witness
                && page.query.run_id == query.run_id
                && page.query.revision == query.revision
                && page.query.expected == query.expected
                && page.query.page == query.page
                && page.query.width == query.width,
            "PDF source, page or zoom changed. Refresh its evidence."
        );
        ensure!(
            query.limit == 32768 && query.offset % 32768 == 0 && now < page.expires_at,
            "PDF image chunk or expiry changed"
        );
        let offset = query.offset as usize;
        ensure!(
            offset < page.png.len(),
            "PDF image cursor changed. Retry its page preview."
        );
        let end = page.png.len().min(offset + usize::from(query.limit));
        page.last_access = Instant::now();
        let result = json!({"kind":"workflowPdfPage","page":{"runId":query.run_id,"revision":query.revision,
            "document":query.expected,"transferId":page.id,"page":query.page,"pages":page.pages,
            "requestedWidth":query.width,"width":page.width,"height":page.height,"bytes":page.png.len(),
            "sha256":page.sha256,"expiresAt":page.expires_at,"offset":offset,"nextOffset":if end < page.png.len() {Some(end)} else {None},
            "data":STANDARD.encode(&page.png[offset..end])},"pageTruncated":false,"servedAt":now});
        ensure!(
            result.to_string().len() <= 60_000,
            "PDF image chunk exceeds gateway transfer bound"
        );
        Ok(result)
    }
}
impl TeamCoordinator {
    pub(crate) fn pdf_transfer_active(&self) -> bool {
        self.pdf_pages.try_lock().is_some_and(|cache| {
            cache.pages.iter().any(|p| {
                p.created.elapsed() < TTL && p.last_access.elapsed() < Duration::from_secs(15)
            })
        })
    }
    /// Exact existing workflow evidence membership plus full original bytes/hash.
    fn pdf_source(&self, query: &WorkflowPdfQuery) -> Result<(Vec<u8>, String)> {
        query.validate()?;
        let run = self.automation_run(query.run_id)?;
        ensure!(
            run.revision == query.revision,
            "Workflow changed. Refresh its evidence."
        );
        let teams = run
            .stages
            .iter()
            .filter(|s| s.started && !s.skipped)
            .map(|s| self.snapshot(s.team_run_id))
            .collect::<Result<Vec<_>>>()?;
        let snapshot = self.control.load()?;
        let selected = history::accepted_artifacts(&run, &teams, &snapshot)?
            .into_iter()
            .find(|row| row["id"] == query.expected.id.to_string())
            .context("PDF is no longer accepted evidence for this workflow")?;
        ensure!(
            selected == serde_json::to_value(&query.expected)?,
            "Accepted PDF metadata changed. Refresh its evidence."
        );
        let artifact = snapshot
            .records
            .iter()
            .find_map(|r| match r {
                ControlRecord::Artifact(a) if a.id == query.expected.id => Some(a),
                _ => None,
            })
            .context("PDF publication no longer exists")?;
        let bytes = self.control.read_document(query.expected.id)?;
        ensure!(
            bytes.len() == query.expected.bytes as usize
                && artifact
                    .evidence
                    .as_ref()
                    .is_some_and(|e| e.matches(&bytes))
                && format!("{:x}", Sha256::digest(&bytes)) == query.expected.sha256,
            "PDF original content changed. Refresh its evidence."
        );
        // The private seal includes complete publisher provenance and accepted
        // run/team membership, never only the public metadata or inferred labels.
        let witness = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(&run, &teams, artifact))?)
        );
        Ok((bytes, witness))
    }
    pub fn read_workflow_pdf_page(&self, query: &WorkflowPdfQuery) -> Result<Value> {
        let (original, witness) = self.pdf_source(query)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis()
            .try_into()?;
        let result = if query.transfer_id.is_none() {
            // Reserve the bounded cache before rasterization. Concurrent starts
            // cannot create unaccounted pages or race the two-page admission.
            let mut cache = self
                .pdf_pages
                .try_lock()
                .context("Another PDF image read is active. Retry when it finishes.")?;
            cache.expire();
            while cache.pages.len() >= CACHE_PAGES
                || cache.pages.iter().map(|p| p.png.len()).sum::<usize>()
                    + waku_document::MAX_PNG_BYTES
                    > CACHE_BYTES
            {
                cache.pages.remove(0);
            }
            let raster = waku_document::try_render_pdf_png(&original, query.page, query.width)?;
            ensure!(
                self.pdf_source(query)?.1 == witness,
                "Accepted PDF changed during rendering. Refresh its evidence."
            );
            let id = Uuid::new_v4();
            let sha256 = format!("{:x}", Sha256::digest(&raster.png));
            cache.insert(Page {
                id,
                query: query.clone(),
                witness: witness.clone(),
                created: Instant::now(),
                expires_at: now + TTL.as_millis() as u64,
                png: raster.png,
                sha256,
                pages: raster.pages,
                width: raster.width,
                height: raster.height,
                last_access: Instant::now(),
            });
            let mut first = query.clone();
            first.transfer_id = Some(id);
            cache.chunk(&first, &witness, now)?
        } else {
            self.pdf_pages
                .try_lock()
                .context("Another PDF image read is active. Retry when it finishes.")?
                .chunk(query, &witness, now)?
        };
        ensure!(
            self.pdf_source(query)?.1 == witness,
            "Accepted PDF changed during image transfer. Refresh its evidence."
        );
        Ok(result)
    }
}
#[cfg(test)]
#[path = "gateway_pdf_cache_tests.rs"]
mod tests;
