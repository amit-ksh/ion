//! Native read-only report transport. IAM never reaches the archive request; URLs never persist.
use super::*;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{io::Read, time::Duration};
use url::Url;
pub(super) const ARCHIVE_LIMIT: usize = 8 * 1024 * 1024;
const DISCOVERY_METADATA_LIMIT: usize = 512 * 1024;
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
fn owner_token() -> Result<String> {
    let token = std::env::var("ION_NEBIUS_BILLING_IAM_TOKEN")
        .or_else(|_| std::env::var("ION_NEBIUS_GPU_IAM_TOKEN"))
        .map_err(|_| anyhow::anyhow!("Configure the owner's Nebius billing IAM token privately"))?;
    ensure!(
        !token.trim().is_empty() && token.len() <= 16_384 && !token.contains(['\r', '\n']),
        "Invalid owner billing IAM token"
    );
    Ok(token)
}

pub(super) struct ExportDiscovery {
    pub items: Vec<CloudBillingReadyExport>,
    pub examined: u16,
    pub export_ids: Vec<String>,
    pub next_token: String,
}
pub(super) fn page_token(token: &str) -> Result<()> {
    ensure!(
        token.len() <= 4096 && !token.chars().any(char::is_control),
        "Billing continuation exceeds its bound"
    );
    Ok(())
}
fn discovery_request(
    client: &reqwest::blocking::Client,
    scope: &CloudBillingScope,
    token: &str,
    page_size: u16,
    after: &str,
) -> reqwest::blocking::RequestBuilder {
    client
        .get("https://api.nebius.cloud/billing/v1alpha1/one-time-exports")
        .query(&[
            ("parentId", scope.contract_id.clone()),
            ("pageSize", page_size.to_string()),
            ("pageToken", after.into()),
        ])
        .bearer_auth(token.trim())
}
pub(super) fn discovery_page(
    scope: &CloudBillingScope,
    value: &Value,
    limit: u16,
    after: &str,
    now: u64,
) -> Result<ExportDiscovery> {
    let object = value.as_object().context("Invalid billing export list")?;
    ensure!(
        object
            .keys()
            .all(|key| matches!(key.as_str(), "items" | "nextPageToken")),
        "Unrecognized billing export list fields"
    );
    let empty = vec![];
    let rows = match value.get("items") {
        None => &empty,
        Some(value) => value.as_array().context("Invalid billing export list")?,
    };
    ensure!(
        rows.len() <= limit as usize,
        "Billing export list exceeds its requested page limit"
    );
    let next_token = match value.get("nextPageToken") {
        None => "",
        Some(value) => value.as_str().context("Invalid billing continuation")?,
    };
    page_token(next_token)?;
    ensure!(
        next_token.is_empty() || next_token != after,
        "Billing export pagination did not advance"
    );
    let mut items = vec![];
    let mut ids = std::collections::HashSet::new();
    let mut export_ids = vec![];
    for row in rows {
        let id = row["metadata"]["id"]
            .as_str()
            .context("Billing export identity is missing")?;
        identifier(id)?;
        ensure!(
            row["metadata"]["parentId"].as_str() == Some(&scope.contract_id),
            "Billing export list belongs to a different contract"
        );
        ensure!(ids.insert(id), "Billing export list repeated an identity");
        export_ids.push(id.to_owned());
        let state = match row["status"].get("state") {
            None => 0,
            Some(Value::String(s)) => match s.as_str() {
                "ONE_TIME_EXPORT_STATE_UNSPECIFIED" => 0,
                "ONE_TIME_EXPORT_STATE_SCHEDULED" => 1,
                "ONE_TIME_EXPORT_STATE_RUNNING" => 2,
                "ONE_TIME_EXPORT_STATE_SUCCESS" => 3,
                "ONE_TIME_EXPORT_STATE_FAILED" => 4,
                "ONE_TIME_EXPORT_STATE_ARCHIVED" => 5,
                _ => anyhow::bail!("Unrecognized billing export state"),
            },
            Some(v) => v
                .as_u64()
                .filter(|n| *n <= 5)
                .context("Unrecognized billing export state")?,
        };
        if state != 3 {
            continue;
        }
        if row["spec"]["format"] != "EXPORT_FORMAT_FOCUS_1_2_CSV" && row["spec"]["format"] != 1 {
            continue;
        }
        let period = row["spec"]["startPeriod"]
            .as_str()
            .context("Billing export period is missing")?;
        focus::period_bounds(period)?;
        let end = row["spec"]["endPeriod"]
            .as_str()
            .context("Billing export end period is missing")?;
        focus::period_bounds(end)?;
        if end != period {
            continue;
        }
        let created = timestamp(&row["metadata"]["createdAt"])?;
        ensure!(
            created.timestamp() >= 0 && created.timestamp() as u64 <= now.saturating_add(300),
            "Billing export creation time is invalid"
        );
        let expires = timestamp(&row["status"]["expiresAt"])?;
        if expires.timestamp() <= i64::try_from(now)? {
            continue;
        }
        // List does not need a download URL. The existing explicit Sync performs Get for a fresh
        // signed URL, validates its approved origin, then verifies complete account CSV evidence.
        items.push(CloudBillingReadyExport {
            export_id: id.into(),
            period: period.into(),
            created_at: created.to_rfc3339(),
            expires_at: expires.to_rfc3339(),
        });
    }
    Ok(ExportDiscovery {
        items,
        examined: rows.len().try_into()?,
        export_ids,
        next_token: next_token.into(),
    })
}
pub(super) fn discover(
    scope: &CloudBillingScope,
    after: &str,
    limit: u16,
    now: u64,
) -> Result<ExportDiscovery> {
    page_token(after)?;
    let token = owner_token()?;
    let value = read(
        discovery_request(&client()?, scope, &token, limit, after)
            .send()
            .map_err(|_| anyhow::anyhow!("Could not discover the owner's billing exports"))?,
        DISCOVERY_METADATA_LIMIT,
    )?;
    let value: Value = serde_json::from_slice(&value)
        .map_err(|_| anyhow::anyhow!("Invalid billing export list metadata"))?;
    ensure!(value.is_object(), "Invalid billing export list metadata");
    discovery_page(scope, &value, limit, after, now)
}

pub(super) fn native(scope: &CloudBillingScope, id: &str, now: u64) -> Result<(Export, Vec<u8>)> {
    let token = owner_token()?;
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
    fn billing_discovery_is_one_fixed_get_with_exact_encoded_contract_and_pagination() {
        let scope = CloudBillingScope {
            id: Uuid::new_v4(),
            name: "Fixture".into(),
            account_id: "account-fixture".into(),
            contract_id: "contract-fixture".into(),
            currency: "USD".into(),
            archive_origin: "https://storage.eu-north1.nebius.cloud".into(),
        };
        let request = discovery_request(
            &client().unwrap(),
            &scope,
            "fixture-private-iam",
            32,
            "opaque+/=&token",
        )
        .build()
        .unwrap();
        assert_eq!(request.method(), reqwest::Method::GET);
        assert!(request.body().is_none());
        assert_eq!(
            request.url().origin().ascii_serialization(),
            "https://api.nebius.cloud"
        );
        assert_eq!(request.url().path(), "/billing/v1alpha1/one-time-exports");
        assert_eq!(
            request.url().query_pairs().collect::<Vec<_>>(),
            vec![
                ("parentId".into(), "contract-fixture".into()),
                ("pageSize".into(), "32".into()),
                ("pageToken".into(), "opaque+/=&token".into())
            ]
        );
        assert!(
            request
                .headers()
                .get("Authorization")
                .unwrap()
                .is_sensitive()
        );
        assert!(!request.url().as_str().contains("fixture-private-iam"));
    }
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
    fn billing_discovery_metadata_body_stops_at_its_read_bound_without_echoing_private_bytes() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let thread = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                socket.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                DISCOVERY_METADATA_LIMIT + 1
            )
            .unwrap();
            socket
                .write_all(&vec![b's'; DISCOVERY_METADATA_LIMIT + 1])
                .unwrap();
        });
        let response = client()
            .unwrap()
            .get(format!("http://{address}/"))
            .send()
            .unwrap();
        let error = read(response, DISCOVERY_METADATA_LIMIT)
            .unwrap_err()
            .to_string();
        assert_eq!(error, "Billing report response exceeds its size limit");
        thread.join().unwrap();
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
