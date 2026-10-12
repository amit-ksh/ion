//! Native read-only report transport. IAM never reaches the archive request; URLs never persist.
use super::*;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{io::Read, time::Duration};
use url::Url;
pub(super) const ARCHIVE_LIMIT: usize = 8 * 1024 * 1024;
pub(super) struct Export {
    pub id: String,
    pub period: String,
    pub created: DateTime<Utc>,
    pub url: Url,
}
fn timestamp(value: &Value) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(
        value
            .as_str()
            .context("Billing export timestamp is missing")?,
    )
    .map_err(|_| anyhow::anyhow!("Invalid billing export timestamp"))?
    .with_timezone(&Utc))
}
pub(super) fn origin(value: &str) -> Result<Url> {
    ensure!(
        value.len() <= 240,
        "Billing archive origin exceeds its bound"
    );
    let url = Url::parse(value).map_err(|_| anyhow::anyhow!("Invalid billing archive origin"))?;
    let host = url.host_str().unwrap_or_default();
    let parts: Vec<_> = host.split('.').collect();
    ensure!(
        url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && url.path() == "/"
            && url.port_or_known_default() == Some(443)
            && parts.len() == 4
            && parts[0] == "storage"
            && !parts[1].is_empty()
            && parts[1]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            && parts[2..] == ["nebius", "cloud"],
        "Use the approved HTTPS Nebius storage origin"
    );
    Ok(url)
}
pub(super) fn validate_export(
    scope: &CloudBillingScope,
    id: &str,
    value: &Value,
    now: u64,
) -> Result<Export> {
    ensure!(
        value["metadata"]["id"].as_str() == Some(id)
            && value["metadata"]["parentId"].as_str() == Some(&scope.contract_id),
        "Billing export belongs to a different contract or identity"
    );
    ensure!(
        value["status"]["state"] == "ONE_TIME_EXPORT_STATE_SUCCESS"
            || value["status"]["state"] == 3,
        "Billing export is not ready"
    );
    ensure!(
        value["spec"]["format"] == "EXPORT_FORMAT_FOCUS_1_2_CSV" || value["spec"]["format"] == 1,
        "Billing export is not FOCUS 1.2 CSV"
    );
    let period = value["spec"]["startPeriod"]
        .as_str()
        .context("Billing export period is missing")?;
    ensure!(
        value["spec"]["endPeriod"].as_str() == Some(period),
        "Select an export for exactly one billing month"
    );
    focus::period_bounds(period)?;
    let created = timestamp(&value["metadata"]["createdAt"])?;
    ensure!(
        created.timestamp() >= 0 && created.timestamp() as u64 <= now.saturating_add(300),
        "Billing export creation time is invalid"
    );
    ensure!(
        timestamp(&value["status"]["expiresAt"])?.timestamp() > i64::try_from(now)?,
        "Billing export has expired"
    );
    let url = Url::parse(
        value["status"]["downloadUrl"]
            .as_str()
            .context("Billing export archive URL is missing")?,
    )
    .map_err(|_| anyhow::anyhow!("Invalid billing archive URL"))?;
    let allowed = origin(&scope.archive_origin)?;
    ensure!(
        url.origin() == allowed.origin()
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
            && url.path() != "/",
        "Billing archive origin differs from the approved scope"
    );
    Ok(Export {
        id: id.into(),
        period: period.into(),
        created,
        url,
    })
}
fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|_| anyhow::anyhow!("Could not create the billing HTTPS transport"))?)
}
fn read(response: reqwest::blocking::Response, limit: usize) -> Result<Vec<u8>> {
    ensure!(
        response.status().is_success(),
        "Billing report request failed (HTTP {})",
        response.status()
    );
    let mut bytes = Vec::new();
    response
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| anyhow::anyhow!("Billing report response interrupted"))?;
    ensure!(
        bytes.len() <= limit,
        "Billing report response exceeds its size limit"
    );
    Ok(bytes)
}
fn api_request(
    client: &reqwest::blocking::Client,
    id: &str,
    token: &str,
) -> reqwest::blocking::RequestBuilder {
    client
        .get(format!(
            "https://api.nebius.cloud/billing/v1alpha1/one-time-exports/{id}"
        ))
        .bearer_auth(token.trim())
}
fn archive_request(
    client: &reqwest::blocking::Client,
    url: url::Url,
) -> reqwest::blocking::RequestBuilder {
    client.get(url)
}
pub(super) fn native(scope: &CloudBillingScope, id: &str, now: u64) -> Result<(Export, Vec<u8>)> {
    let token = std::env::var("ION_NEBIUS_BILLING_IAM_TOKEN")
        .or_else(|_| std::env::var("ION_NEBIUS_GPU_IAM_TOKEN"))
        .map_err(|_| anyhow::anyhow!("Configure the owner's Nebius billing IAM token privately"))?;
    ensure!(
        !token.trim().is_empty() && token.len() <= 16_384 && !token.contains(['\r', '\n']),
        "Invalid owner billing IAM token"
    );
    let value = read(
        api_request(&client()?, id, &token)
            .send()
            .map_err(|_| anyhow::anyhow!("Could not read the native billing export"))?,
        65_536,
    )?;
    let value: Value = serde_json::from_slice(&value)
        .map_err(|_| anyhow::anyhow!("Invalid billing export metadata"))?;
    let export = validate_export(scope, id, &value, now)?;
    // Independent client/request with no IAM, cookies, retries or redirect following.
    let archive = read(
        archive_request(&client()?, export.url.clone())
            .send()
            .map_err(|_| anyhow::anyhow!("Could not download the billing archive"))?,
        ARCHIVE_LIMIT,
    )?;
    Ok((export, archive))
}

#[cfg(test)]
mod http_tests {
    use super::*;
    use std::io::{Read, Write};
    #[test]
    fn cloud_billing_native_requests_are_reads_and_archive_never_receives_iam() {
        let api = api_request(&client().unwrap(), "export-test", "fixture-private-iam")
            .build()
            .unwrap();
        assert_eq!(api.method(), reqwest::Method::GET);
        assert!(api.body().is_none());
        let auth = api.headers().get("Authorization").unwrap();
        assert_eq!(auth.to_str().unwrap(), "Bearer fixture-private-iam");
        assert!(auth.is_sensitive());
        let archive=archive_request(&client().unwrap(),Url::parse("https://storage.eu-north1.nebius.cloud/billing-report-exports-external/onetimeexport/fixture?fake=sensitive").unwrap()).build().unwrap();
        assert_eq!(archive.method(), reqwest::Method::GET);
        assert!(archive.headers().get("Authorization").is_none());
        assert!(archive.body().is_none());
    }
    #[test]
    fn cloud_billing_redirect_and_error_bodies_cannot_escape_as_credentials_or_data() {
        let target = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        target.set_nonblocking(true).unwrap();
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = server.local_addr().unwrap();
        let destination = target.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            let (mut socket, _) = server.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = vec![];
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                socket.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            write!(socket,"HTTP/1.1 302 Found\r\nLocation: http://{destination}/\r\nContent-Length: 19\r\nConnection: close\r\n\r\nfixture-private-iam").unwrap();
        });
        let response = client()
            .unwrap()
            .get(format!("http://{address}/"))
            .send()
            .unwrap();
        let error = read(response, 65_536).unwrap_err();
        assert!(!error.to_string().contains("fixture-private-iam"));
        assert!(matches!(target.accept(),Err(e) if e.kind()==std::io::ErrorKind::WouldBlock));
        worker.join().unwrap();
    }
}
