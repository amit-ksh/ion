//! Fixed-origin S3 reads of supervisor receipts. No model/storage secrets enter the child.
use super::*;
use crate::teams::{gpu_jobs::GpuResultVolume, jobs::JobResponse};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use uuid::Uuid;
const MAX_RECEIPT: u64 = 512 * 1024;

impl Client {
    pub(super) fn verify_result_bucket(&self, volume: &GpuResultVolume) -> Result<()> {
        volume.validate()?;
        let bucket = Self::response(
            self.request(
                reqwest::Method::GET,
                &format!("/storage/v1/buckets/{}", volume.bucket_id),
            )
            .send()?,
        )?;
        ensure!(
            bucket["metadata"]["id"] == volume.bucket_id
                && bucket["metadata"]["parentId"] == self.project
                && bucket["metadata"]["name"] == volume.bucket_name
                && bucket["status"]["state"] == "ACTIVE"
                && bucket["status"]["suspensionState"] == "NOT_SUSPENDED"
                && bucket["status"]["bucketType"] == "REGULAR"
                && bucket["status"]["region"] == volume.region
                && (bucket["status"]["anonymousAccessEnabled"].is_null()
                    || bucket["status"]["anonymousAccessEnabled"] == false)
                && (bucket["status"]["domainName"] == volume.host()
                    || bucket["status"]["domainName"]
                        == format!("{}.{}", volume.bucket_name, volume.host())),
            "GPU result bucket identity, readiness or privacy changed"
        );
        Ok(())
    }
}
/// Intentionally has no Debug implementation: these are owner credentials.
struct ResultReader {
    http: reqwest::blocking::Client,
    access_key: String,
    secret_key: String,
    session_token: Option<String>,
    #[cfg(test)]
    base: Option<String>,
}
fn mac(key: &[u8], bytes: &[u8]) -> Vec<u8> {
    let mut h = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts all key sizes");
    h.update(bytes);
    h.finalize().into_bytes().to_vec()
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
impl ResultReader {
    fn configured() -> Result<Self> {
        let reader = Self {
            http: reqwest::blocking::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(3))
                .timeout(Duration::from_secs(8))
                .build()?,
            access_key: std::env::var("ION_NEBIUS_GPU_S3_ACCESS_KEY")
                .context("Configure owner GPU result read credentials privately")?,
            secret_key: std::env::var("ION_NEBIUS_GPU_S3_SECRET_KEY")
                .context("Configure owner GPU result read credentials privately")?,
            session_token: std::env::var("ION_NEBIUS_GPU_S3_SESSION_TOKEN").ok(),
            #[cfg(test)]
            base: None,
        };
        reader.validate_credentials()?;
        Ok(reader)
    }
    fn validate_credentials(&self) -> Result<()> {
        ensure!(
            !self.access_key.is_empty()
                && self.access_key.len() <= 256
                && self
                    .access_key
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
                && !self.secret_key.is_empty()
                && self.secret_key.len() <= 1024
                && self.session_token.as_ref().is_none_or(|t| !t.is_empty()
                    && t.len() <= 8192
                    && t.bytes().all(|b| b.is_ascii_graphic())),
            "GPU result credentials are invalid"
        );
        Ok(())
    }
    /// S3 SigV4 GET only: no query, redirects, user URL, path normalization or unsigned payload.
    fn headers(
        &self,
        host: &str,
        uri: &str,
        region: &str,
        at: chrono::DateTime<chrono::Utc>,
    ) -> Result<reqwest::header::HeaderMap> {
        self.validate_credentials()?;
        let date = at.format("%Y%m%d").to_string();
        let timestamp = at.format("%Y%m%dT%H%M%SZ").to_string();
        let payload = hex(&Sha256::digest([]));
        let mut canonical =
            format!("host:{host}\nx-amz-content-sha256:{payload}\nx-amz-date:{timestamp}\n");
        let mut signed = "host;x-amz-content-sha256;x-amz-date".to_string();
        if let Some(token) = &self.session_token {
            canonical.push_str(&format!("x-amz-security-token:{token}\n"));
            signed.push_str(";x-amz-security-token");
        }
        let request = format!("GET\n{uri}\n\n{canonical}\n{signed}\n{payload}");
        let scope = format!("{date}/{region}/s3/aws4_request");
        let to_sign = format!(
            "AWS4-HMAC-SHA256\n{timestamp}\n{scope}\n{}",
            hex(&Sha256::digest(request.as_bytes()))
        );
        let key = mac(
            format!("AWS4{}", self.secret_key).as_bytes(),
            date.as_bytes(),
        );
        let key = mac(&key, region.as_bytes());
        let key = mac(&key, b"s3");
        let key = mac(&key, b"aws4_request");
        let auth = format!(
            "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed}, Signature={}",
            self.access_key,
            hex(&mac(&key, to_sign.as_bytes()))
        );
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("host", host.parse()?);
        headers.insert("x-amz-content-sha256", payload.parse()?);
        headers.insert("x-amz-date", timestamp.parse()?);
        let mut auth = reqwest::header::HeaderValue::from_str(&auth)?;
        auth.set_sensitive(true);
        headers.insert("authorization", auth);
        if let Some(token) = &self.session_token {
            let mut token = reqwest::header::HeaderValue::from_str(token)?;
            token.set_sensitive(true);
            headers.insert("x-amz-security-token", token);
        }
        Ok(headers)
    }
    fn read(&self, volume: &GpuResultVolume, request: Uuid) -> Result<Option<JobResponse>> {
        volume.validate()?;
        ensure!(!request.is_nil(), "GPU result request missing");
        let host = volume.host();
        let uri = format!("/{}/{}", volume.bucket_name, volume.object_key(request));
        let headers = self.headers(&host, &uri, &volume.region, chrono::Utc::now())?;
        let base = format!("https://{host}");
        #[cfg(test)]
        let base = self.base.as_deref().unwrap_or(&base);
        let response = self.http.get(format!("{base}{uri}")).headers(headers).send()
            .map_err(|_| anyhow::anyhow!("GPU result read unresolved; observe the original receipt without rerunning the job"))?;
        // A missing object means no receipt yet, never proof of resource absence.
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        ensure!(
            response.status() == reqwest::StatusCode::OK,
            "GPU result read unresolved (HTTP {})",
            response.status()
        );
        ensure!(
            response
                .content_length()
                .is_none_or(|len| len <= MAX_RECEIPT),
            "GPU result receipt exceeds its bound"
        );
        let mut bytes = Vec::new();
        response.take(MAX_RECEIPT + 1).read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() as u64 <= MAX_RECEIPT,
            "GPU result receipt exceeds its bound"
        );
        Ok(Some(serde_json::from_slice(&bytes).map_err(|_| {
            anyhow::anyhow!("GPU result receipt has an invalid contract")
        })?))
    }
}
fn collect_record(
    reader: &ResultReader,
    client: &Client,
    coordinator: &TeamCoordinator,
    record: &GpuJobRecord,
) -> Result<bool> {
    if record.result_receipt.is_some() && record.result_artifact.is_some() {
        return Ok(true);
    }
    let Some(volume) = &record.result_volume else {
        return Ok(false);
    };
    if record.state != "terminal" {
        return Ok(false);
    }
    ensure!(
        record.project == client.project,
        "GPU result owner project changed"
    );
    let job = record
        .job_id
        .as_deref()
        .context("GPU result lacks original job identity")?;
    client.verify_result_bucket(volume)?;
    let Some(receipt) = reader.read(volume, record.request_id)? else {
        return Ok(false);
    };
    coordinator.record_gpu_result(record.request_id, job, &receipt, crate::model::unix_time())?;
    Ok(true)
}
/// Owner/background only; missing S3 credentials must never prevent resource Stop/cleanup.
pub(crate) fn collect_attempt(coordinator: &TeamCoordinator, attempt: Uuid) -> Result<bool> {
    let records = coordinator.gpu_jobs(attempt)?;
    let pending: Vec<_> = records
        .iter()
        .filter(|r| r.result_volume.is_some() && r.result_receipt.is_none())
        .collect();
    if pending.is_empty() {
        return Ok(true);
    }
    let reader = ResultReader::configured()?;
    let client = Client::configured()?;
    let mut done = true;
    let mut error = None;
    for record in pending {
        match collect_record(&reader, &client, coordinator, record) {
            Ok(ready) => done &= ready,
            Err(e) => {
                error.get_or_insert(e);
            }
        }
    }
    if let Some(error) = error {
        return Err(error);
    }
    Ok(done)
}
/// Scoped binary pages; outputs never write to the user's source checkout.
pub(crate) fn read_output(
    coordinator: &TeamCoordinator,
    lease: &AssignmentLease,
    request: Uuid,
    path: &str,
    offset: usize,
) -> Result<String> {
    crate::teams::jobs::path(path)?;
    let (receipt, artifact) = coordinator.gpu_result(lease, request, crate::model::unix_time())?;
    let output = receipt
        .outputs
        .iter()
        .find(|o| o.path == path)
        .context("GPU output was not returned by the worker")?;
    let bytes = STANDARD.decode(&output.content_base64)?;
    ensure!(offset <= bytes.len(), "GPU output offset exceeds the file");
    let end = (offset + 4096).min(bytes.len());
    Ok(json!({"request_id":request,"artifact_id":artifact,"path":path,"bytes":bytes.len(),"sha256":output.sha256,
        "offset":offset,"content_base64":STANDARD.encode(&bytes[offset..end]),"next_offset":(end < bytes.len()).then_some(end)}).to_string())
}
#[cfg(test)]
mod tests;
