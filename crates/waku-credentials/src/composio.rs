//! Only safe account/config summaries leave this boundary. OAuth credentials
//! stay in Composio; the project key stays in the OS credential vault.
use super::*;
use reqwest::blocking::{Client, Response};
use serde_json::{Value, json};
use std::time::Duration;

const BASE: &str = "https://backend.composio.dev/api/v3.1";
#[derive(Clone)]
pub struct ComposioAuthConfig {
    pub id: String,
    pub toolkit: String,
    pub name: String,
    pub description: String,
}
fn client() -> Result<Client> {
    Ok(Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()?)
}
fn payload(response: Response) -> Result<Value> {
    if !response.status().is_success() {
        bail!(
            "Composio request failed (HTTP {}). Check your project key and app permissions.",
            response.status()
        );
    }
    use std::io::Read;
    let mut bytes = Vec::new();
    response.take(1_000_001).read_to_end(&mut bytes)?;
    if bytes.len() > 1_000_000 {
        bail!("Composio response is too large");
    }
    Ok(serde_json::from_slice(&bytes).context("Invalid Composio response")?)
}
impl CredentialService {
    pub fn composio_apps(&self) -> Result<(Vec<ComposioAuthConfig>, Vec<GatewayAccount>)> {
        let key = self.secret(CredentialKind::ComposioScopedProjectKey)?;
        let client = client()?;
        let configs = payload(
            client
                .get(format!("{BASE}/auth_configs"))
                .query(&[("limit", "100")])
                .header("x-api-key", &key)
                .send()?,
        )?;
        let mut apps = Vec::new();
        for raw in configs["items"]
            .as_array()
            .context("Composio returned no app catalog")?
        {
            apps.push(ComposioAuthConfig {
                id: raw["id"].as_str().context("Missing config ID")?.to_owned(),
                toolkit: raw["toolkit"]["slug"]
                    .as_str()
                    .context("Missing toolkit")?
                    .to_owned(),
                name: raw["name"].as_str().unwrap_or("").to_owned(),
                description: String::new(),
            });
        }
        let mut accounts = Vec::new();
        let mut cursor = None::<String>;
        // Bound pagination, and never present a silently truncated account list.
        for page in 0..10 {
            let mut request = client
                .get(format!("{BASE}/connected_accounts"))
                .query(&[("limit", "100")])
                .header("x-api-key", &key);
            if let Some(cursor) = &cursor {
                request = request.query(&[("cursor", cursor)]);
            }
            let data = payload(request.send()?)?;
            for raw in data["items"]
                .as_array()
                .context("Composio returned no account list")?
            {
                accounts.push(GatewayAccount {
                    id: raw["id"].as_str().context("Missing account ID")?.into(),
                    toolkit: raw["toolkit"]["slug"]
                        .as_str()
                        .context("Missing toolkit")?
                        .into(),
                    status: raw["status"]
                        .as_str()
                        .context("Missing account status")?
                        .into(),
                });
            }
            cursor = data["next_cursor"]
                .as_str()
                .filter(|c| !c.is_empty())
                .map(str::to_owned);
            if cursor.is_none() {
                break;
            }
            if page == 9 {
                bail!("Too many Composio accounts. Use a smaller scoped project.");
            }
        }
        if configs["next_cursor"]
            .as_str()
            .is_some_and(|c| !c.is_empty())
        {
            bail!("Too many auth configurations. Use a smaller scoped project.");
        }
        let mut catalog = Vec::new();
        let mut cursor = None::<String>;
        for page in 0..30 {
            let mut request = client
                .get(format!("{BASE}/toolkits"))
                .query(&[("limit", "100"), ("sort_by", "usage")])
                .header("x-api-key", &key);
            if let Some(cursor) = &cursor {
                request = request.query(&[("cursor", cursor)]);
            }
            let data = payload(request.send()?)?;
            for raw in data["items"]
                .as_array()
                .context("Composio returned no app catalog")?
            {
                let slug = raw["slug"].as_str().context("Missing toolkit slug")?;
                let managed = raw["composio_managed_auth_schemes"]
                    .as_array()
                    .is_some_and(|a| !a.is_empty());
                let configured = apps.iter().find(|a| a.toolkit == slug);
                let prefix = if raw["no_auth"].as_bool() == Some(true) {
                    "available"
                } else if managed {
                    "toolkit"
                } else {
                    "configure"
                };
                catalog.push(ComposioAuthConfig {
                    id: configured
                        .map(|a| a.id.clone())
                        .unwrap_or_else(|| format!("{prefix}:{slug}")),
                    toolkit: slug.into(),
                    name: raw["name"].as_str().unwrap_or(slug).into(),
                    description: raw["meta"]["description"].as_str().unwrap_or("").into(),
                });
            }
            cursor = data["next_cursor"]
                .as_str()
                .filter(|c| !c.is_empty())
                .map(str::to_owned);
            if cursor.is_none() {
                break;
            }
            if page == 29 {
                bail!("Composio catalog exceeded 3000 apps. Use a scoped project.");
            }
        }
        for app in apps {
            if !catalog.iter().any(|a| a.toolkit == app.toolkit) {
                catalog.push(app);
            }
        }
        apps = catalog;
        Ok((apps, accounts))
    }
    pub fn composio_connect(&self, config_id: &str, user_id: &str) -> Result<String> {
        if config_id.is_empty()
            || config_id.len() > 160
            || user_id.is_empty()
            || user_id.len() > 160
        {
            bail!("Invalid app connection");
        }
        let key = self.secret(CredentialKind::ComposioScopedProjectKey)?;
        let client = client()?;
        let id = if let Some(slug) = config_id.strip_prefix("toolkit:") {
            if !slug
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
            {
                bail!("Invalid app");
            }
            // Reuse an existing config if a previous connect attempt created it.
            let configs = payload(
                client
                    .get(format!("{BASE}/auth_configs"))
                    .query(&[("toolkit_slug", slug)])
                    .header("x-api-key", &key)
                    .send()?,
            )?;
            if let Some(id) = configs["items"]
                .as_array()
                .and_then(|a| a.first())
                .and_then(|a| a["id"].as_str())
            {
                id.to_owned()
            } else {
                let created=payload(client.post(format!("{BASE}/auth_configs")).header("x-api-key",&key).json(&json!({"toolkit":{"slug":slug},"auth_config":{"type":"use_composio_managed_auth"}})).send()?)?;
                created["auth_config"]["id"]
                    .as_str()
                    .context("Composio returned no auth config")?
                    .to_owned()
            }
        } else {
            config_id.into()
        };
        let data = payload(
            client
                .post(format!("{BASE}/connected_accounts/link"))
                .header("x-api-key", key)
                .json(&json!({"auth_config_id":id,"user_id":user_id}))
                .send()?,
        )?;
        let url = data["redirect_url"]
            .as_str()
            .context("Composio returned no sign-in URL")?;
        validate_connect_url(url)?;
        Ok(url.into())
    }
}
fn validate_connect_url(value: &str) -> Result<()> {
    let url = reqwest::Url::parse(value)?;
    let host = url.host_str().unwrap_or("");
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || !(host == "composio.dev"
            || host.ends_with(".composio.dev")
            || host == "composio.app"
            || host.ends_with(".composio.app"))
    {
        bail!("Composio returned an unexpected sign-in URL");
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hosted_auth_only_accepts_composio_https() {
        assert!(validate_connect_url("https://connect.composio.dev/link/test").is_ok());
        for url in [
            "http://connect.composio.dev/test",
            "https://composio.dev.evil.test/test",
            "https://user@connect.composio.dev/test",
            "https://localhost/test",
        ] {
            assert!(validate_connect_url(url).is_err());
        }
    }
}
