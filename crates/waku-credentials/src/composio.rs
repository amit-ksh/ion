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
/// Safe pinned tool metadata. No account authentication parameters leave the vault boundary.
#[derive(Clone)]
pub struct ComposioTool {
    pub slug: String,
    pub toolkit: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub input_parameters: Value,
}
pub struct ComposioToolPage {
    pub items: Vec<ComposioTool>,
    pub next_cursor: Option<String>,
}
/// Consumed by one approved effect. It cannot be cloned or passed to the model.
pub struct ComposioPreparedAction {
    key: String,
    action: String,
    body: Value,
}
fn identifier(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
pub fn validate_action_identity(account: &str, action: &str, version: &str) -> Result<()> {
    if !identifier(account, 256)
        || account == "instant_account"
        || !identifier(action, 256)
        || !action
            .bytes()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
        || !identifier(version, 128)
        || version == "latest"
    {
        bail!("Choose an exact connected account, action and published version");
    }
    Ok(())
}
fn parse_tool(raw: &Value) -> Result<ComposioTool> {
    let slug = raw["slug"].as_str().context("Missing app action")?;
    let version = raw["version"]
        .as_str()
        .context("Missing app action version")?;
    validate_action_identity("metadata", slug, version)?;
    let toolkit = raw["toolkit"]["slug"]
        .as_str()
        .context("Missing action toolkit")?;
    if !identifier(toolkit, 128) || raw["is_deprecated"] == true {
        bail!("This action toolkit or version is unavailable");
    }
    let schema = &raw["input_parameters"];
    if !schema.is_object() || serde_json::to_vec(schema)?.len() > 24 * 1024 {
        bail!("App action input schema is missing or too large");
    }
    Ok(ComposioTool {
        slug: slug.into(),
        toolkit: toolkit.into(),
        name: raw["name"]
            .as_str()
            .unwrap_or(slug)
            .chars()
            .take(160)
            .collect(),
        description: raw["description"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(2048)
            .collect(),
        version: version.into(),
        input_parameters: schema.clone(),
    })
}
impl CredentialService {
    pub fn composio_tools(
        &self,
        toolkit: &str,
        query: &str,
        cursor: Option<&str>,
    ) -> Result<ComposioToolPage> {
        if !identifier(toolkit, 128) || query.len() > 256 || cursor.is_some_and(|c| c.len() > 2048)
        {
            bail!("Invalid app action search");
        }
        let key = self.secret(CredentialKind::ComposioScopedProjectKey)?;
        let mut request = client()?
            .get(format!("{BASE}/tools"))
            .header("x-api-key", &key)
            .query(&[
                ("toolkit_slug", toolkit),
                ("query", query),
                ("include_deprecated", "false"),
                ("limit", "20"),
            ]);
        if let Some(cursor) = cursor {
            request = request.query(&[("cursor", cursor)]);
        }
        let data = payload(request.send().context("Could not load app actions")?)?;
        let rows = data["items"]
            .as_array()
            .context("Missing app action catalog")?;
        if rows.len() > 20 {
            bail!("App action page exceeds its bound");
        }
        let items = rows.iter().map(parse_tool).collect::<Result<Vec<_>>>()?;
        if items.iter().any(|t| t.toolkit != toolkit) {
            bail!("App action catalog returned another toolkit");
        }
        let next_cursor = data["next_cursor"]
            .as_str()
            .filter(|c| !c.is_empty())
            .map(str::to_owned);
        if next_cursor.as_ref().is_some_and(|c| c.len() > 2048) {
            bail!("App action cursor exceeds its bound");
        }
        Ok(ComposioToolPage { items, next_cursor })
    }
    pub fn composio_tool(
        &self,
        account: &str,
        action: &str,
        version: &str,
    ) -> Result<ComposioTool> {
        validate_action_identity(account, action, version)?;
        let key = self.secret(CredentialKind::ComposioScopedProjectKey)?;
        verified_tool(&client()?, &key, account, action, version)
    }
    pub fn prepare_composio_action(
        &self,
        account: &str,
        action: &str,
        version: &str,
        arguments: &Value,
    ) -> Result<ComposioPreparedAction> {
        validate_action_identity(account, action, version)?;
        if !arguments.is_object() || serde_json::to_vec(arguments)?.len() > 4096 {
            bail!("Approved app arguments must be an object no larger than 4 KiB");
        }
        let key = self.secret(CredentialKind::ComposioScopedProjectKey)?;
        verified_tool(&client()?, &key, account, action, version)?;
        Ok(ComposioPreparedAction {
            key,
            action: action.into(),
            body: json!({"connected_account_id":account,"version":version,"arguments":arguments}),
        })
    }
}
fn verified_tool(
    client: &Client,
    key: &str,
    account: &str,
    action: &str,
    version: &str,
) -> Result<ComposioTool> {
    let account_data = payload(
        client
            .get(format!("{BASE}/connected_accounts/{account}"))
            .header("x-api-key", key)
            .send()
            .context("Could not verify app account")?,
    )?;
    if account_data["id"].as_str() != Some(account)
        || account_data["status"] != "ACTIVE"
        || account_data["is_disabled"] == true
        || account_data["auth_config"]["is_disabled"] == true
    {
        bail!("The exact connected app account is inactive or unavailable");
    }
    let data = payload(
        client
            .get(format!("{BASE}/tools/{action}"))
            .query(&[("version", version)])
            .header("x-api-key", key)
            .send()
            .context("Could not verify app action")?,
    )?;
    let tool = parse_tool(&data)?;
    if tool.slug != action
        || tool.version != version
        || Some(tool.toolkit.as_str()) != account_data["toolkit"]["slug"].as_str()
    {
        bail!("App action version or connected account toolkit changed");
    }
    Ok(tool)
}
impl ComposioPreparedAction {
    /// Exactly one POST. Redirects/retries, proxy calls and authentication overrides are unavailable.
    pub fn execute(self) -> Result<String> {
        let data = payload(
            client()?
                .post(format!("{BASE}/tools/execute/{}", self.action))
                .header("x-api-key", &self.key)
                .json(&self.body)
                .send()
                .context(
                    "App action has no acknowledgement; verify its outcome before further work",
                )?,
        )?;
        safe_receipt(data)
    }
}
fn safe_receipt(data: Value) -> Result<String> {
    let successful = data["successful"]
        .as_bool()
        .context("App action has no explicit acknowledgement")?;
    let mut result = data["data"].clone();
    redact_auth(&mut result);
    let mut truncated = false;
    if serde_json::to_vec(&result)?.len() > 40 * 1024 {
        result = Value::Null;
        truncated = true;
    }
    let log_id = data["log_id"].as_str().filter(|s| identifier(s, 256));
    Ok(json!({"successful":successful,"data":result,"truncated":truncated,"log_id":log_id,
        "error":if successful {None} else {Some("App action returned a failure; inspect its provider log before submitting another action")}}).to_string())
}
fn redact_auth(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                if matches!(
                    key.to_ascii_lowercase().as_str(),
                    "access_token"
                        | "refresh_token"
                        | "oauth_token"
                        | "oauth_token_secret"
                        | "api_key"
                        | "authorization"
                        | "client_secret"
                        | "password"
                ) {
                    *value = json!("[redacted]");
                } else {
                    redact_auth(value);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                redact_auth(value);
            }
        }
        _ => {}
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
    #[test]
    fn team_app_identity_rejects_routes_aliases_and_account_substitution() {
        assert!(validate_action_identity("ca_123", "GITHUB_GET_ISSUE", "20250905_00").is_ok());
        for (account, action, version) in [
            ("instant_account", "GITHUB_GET_ISSUE", "20250905_00"),
            ("ca/../../proxy", "GITHUB_GET_ISSUE", "20250905_00"),
            ("ca_123", "github_get_issue", "20250905_00"),
            ("ca_123", "GITHUB_GET_ISSUE", "latest"),
            ("ca_123", "GITHUB_GET_ISSUE", ""),
        ] {
            assert!(validate_action_identity(account, action, version).is_err());
        }
    }
    #[test]
    fn team_app_receipt_excludes_auth_envelope_and_redacts_nested_tokens() {
        let receipt:Value = serde_json::from_str(&safe_receipt(json!({"successful":true,"data":{"item":{"access_token":"secret","title":"Issue"}},"session_info":{"session_id":"private"},"error":"private raw error","log_id":"log_123"})).unwrap()).unwrap();
        assert_eq!(receipt["data"]["item"]["access_token"], "[redacted]");
        assert_eq!(receipt["data"]["item"]["title"], "Issue");
        assert!(
            !receipt.to_string().contains("secret") && !receipt.to_string().contains("private")
        );
        assert!(safe_receipt(json!({"data":{}})).is_err());
        let receipt: Value = serde_json::from_str(
            &safe_receipt(json!({"successful":true,"data":{"body":"x".repeat(50000)}})).unwrap(),
        )
        .unwrap();
        assert_eq!(receipt["truncated"], true);
        assert!(receipt["data"].is_null());
    }
    #[test]
    fn team_app_metadata_requires_exact_available_schema() {
        let mut tool = json!({"slug":"GITHUB_GET_ISSUE","toolkit":{"slug":"github"},"version":"20250905_00","input_parameters":{"type":"object"}});
        assert_eq!(parse_tool(&tool).unwrap().version, "20250905_00");
        tool["version"] = json!("latest");
        assert!(parse_tool(&tool).is_err());
        tool["version"] = json!("20250905_00");
        tool["is_deprecated"] = json!(true);
        assert!(parse_tool(&tool).is_err());
        tool["is_deprecated"] = json!(false);
        tool["input_parameters"] = json!({"body":"x".repeat(25000)});
        assert!(parse_tool(&tool).is_err());
    }
}
