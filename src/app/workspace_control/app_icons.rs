//! App identity marks. Bundled product marks come first (Gmail, Drive and
//! Calendar would otherwise all share Google's company logo). Every other app
//! shows its company logo from CompanyEnrich, fetched once in the background,
//! cached on disk and drawn from the cache, never from a URL in a frame.
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::*;

const LOGO_ENDPOINT: &str = "https://api.companyenrich.com/logo/";
const MAX_LOGO_BYTES: usize = 256 * 1024;
const LOGO_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// Where a fetched logo stands for one toolkit.
#[derive(Clone)]
pub(in crate::app) enum AppLogo {
    Loading,
    Ready(PathBuf),
    Missing,
}

impl Waku {
    /// The app's identity mark at `size`: a bundled mark, its cached company
    /// logo, or a generic glyph while the logo loads or when it has none.
    pub(in crate::app) fn app_mark(
        &self,
        toolkit: &str,
        size: f32,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(mark) = bundled_mark(toolkit, size, theme) {
            return mark;
        }
        let state = self.control_ui.app_logos.borrow().get(toolkit).cloned();
        match state {
            Some(AppLogo::Ready(path)) => img(path)
                .size(px(size))
                .rounded(px(size * 0.22))
                .object_fit(ObjectFit::Cover)
                .into_any_element(),
            Some(AppLogo::Loading | AppLogo::Missing) => generic_mark(toolkit, size, theme),
            None => {
                self.fetch_app_logo(toolkit, cx);
                generic_mark(toolkit, size, theme)
            }
        }
    }

    fn fetch_app_logo(&self, toolkit: &str, cx: &mut Context<Self>) {
        if !valid_toolkit(toolkit) {
            return;
        }
        let toolkit = toolkit.to_owned();
        self.control_ui
            .app_logos
            .borrow_mut()
            .insert(toolkit.clone(), AppLogo::Loading);
        cx.spawn(async move |this, cx| {
            let fetched = {
                let toolkit = toolkit.clone();
                cx.background_executor()
                    .spawn(async move { smol::unblock(move || cached_logo(&toolkit)).await })
                    .await
            };
            let _ = this.update(cx, |this, cx| {
                this.control_ui.app_logos.borrow_mut().insert(
                    toolkit,
                    fetched.map_or(AppLogo::Missing, AppLogo::Ready),
                );
                cx.notify();
            });
        })
        .detach();
    }
}

fn bundled_mark(toolkit: &str, size: f32, theme: &Theme) -> Option<AnyElement> {
    let asset = match toolkit {
        "github" => return Some(icon("app-icons/github.svg", size, theme.text).into_any_element()),
        "elevenlabs" => {
            return Some(icon("app-icons/elevenlabs.svg", size, theme.text).into_any_element());
        }
        "gmail" => "app-icons/gmail.png",
        "googledrive" => "app-icons/googledrive.png",
        "googlecalendar" => "app-icons/googlecalendar.png",
        "outlook" => "app-icons/outlook.svg",
        "supabase" => "app-icons/supabase.svg",
        "gitlab" => "icons/file-types/gitlab.svg",
        _ => return None,
    };
    Some(
        img(asset)
            .size(px(size))
            .object_fit(ObjectFit::Contain)
            .into_any_element(),
    )
}

fn generic_mark(toolkit: &str, size: f32, theme: &Theme) -> AnyElement {
    icon(
        match toolkit {
            "googledocs" | "googlesheets" | "notion" => "icons/file.svg",
            _ => "icons/package.svg",
        },
        size * 0.85,
        theme.text_secondary,
    )
    .into_any_element()
}

/// Toolkit slugs become file names and URL path segments.
fn valid_toolkit(toolkit: &str) -> bool {
    !toolkit.is_empty()
        && toolkit.len() <= 64
        && toolkit
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// The company domain CompanyEnrich knows an app by. Slugs that are not
/// simply `<slug>.com` are listed; anything else is tried as `.com`, and an
/// unknown domain comes back as a neutral initials tile.
fn logo_domain(toolkit: &str) -> String {
    match toolkit {
        "airtable" => "airtable.com",
        "asana" => "asana.com",
        "bitbucket" => "bitbucket.org",
        "calendly" => "calendly.com",
        "clickup" => "clickup.com",
        "discord" => "discord.com",
        "dropbox" => "dropbox.com",
        "figma" => "figma.com",
        "hubspot" => "hubspot.com",
        "jira" | "confluence" => "atlassian.com",
        "linear" => "linear.app",
        "microsoftteams" | "one_drive" | "onedrive" | "excel" | "sharepoint" => "microsoft.com",
        "monday" => "monday.com",
        "notion" => "notion.so",
        "perplexityai" => "perplexity.ai",
        "posthog" => "posthog.com",
        "sentry" => "sentry.io",
        "shopify" => "shopify.com",
        "slack" | "slackbot" => "slack.com",
        "stripe" => "stripe.com",
        "telegram" => "telegram.org",
        "trello" => "trello.com",
        "twitter" => "x.com",
        "vercel" => "vercel.com",
        "whatsapp" => "whatsapp.com",
        "youtube" => "youtube.com",
        "zendesk" => "zendesk.com",
        "zoom" => "zoom.us",
        other => return format!("{other}.com"),
    }
    .to_owned()
}

/// Debug builds cache beside the debug database in the checkout's `temp/`.
fn logo_cache_dir() -> PathBuf {
    if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("temp")
            .join("app-logos")
    } else {
        dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join(waku_protocol::identity::DATA_DIRECTORY_NAME)
            .join("app-logos")
    }
}

/// A fresh cached logo for `toolkit`, fetching it when absent or stale.
fn cached_logo(toolkit: &str) -> anyhow::Result<PathBuf> {
    let dir = logo_cache_dir();
    for extension in ["png", "jpg"] {
        let path = dir.join(format!("{toolkit}.{extension}"));
        if fresh(&path) {
            return Ok(path);
        }
    }
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?
        .get(format!("{LOGO_ENDPOINT}{}", logo_domain(toolkit)))
        .send()?
        .error_for_status()?;
    let extension = match response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
    {
        Some(kind) if kind.starts_with("image/png") => "png",
        Some(kind) if kind.starts_with("image/jpeg") => "jpg",
        _ => anyhow::bail!("logo response is not a PNG or JPEG"),
    };
    let bytes = response.bytes()?;
    anyhow::ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_LOGO_BYTES,
        "logo response has an implausible size"
    );
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{toolkit}.{extension}"));
    let staging = path.with_extension("part");
    std::fs::write(&staging, &bytes)?;
    std::fs::rename(&staging, &path)?;
    Ok(path)
}

fn fresh(path: &Path) -> bool {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age < LOGO_TTL)
}

/// A readable name for a connected app. Connected-account lists can carry the
/// toolkit slug ("googlecalendar") where the catalog has a product name.
pub(in crate::app) fn display_name(toolkit: &str, name: &str) -> String {
    if !name.eq_ignore_ascii_case(toolkit) {
        return name.to_owned();
    }
    match toolkit {
        "elevenlabs" => "ElevenLabs",
        "github" => "GitHub",
        "gitlab" => "GitLab",
        "gmail" => "Gmail",
        "googlecalendar" => "Google Calendar",
        "googledocs" => "Google Docs",
        "googledrive" => "Google Drive",
        "googlesheets" => "Google Sheets",
        "linear" => "Linear",
        "notion" => "Notion",
        "outlook" => "Outlook",
        "slack" => "Slack",
        "supabase" => "Supabase",
        _ => {
            let mut chars = name.chars();
            return chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect())
                .unwrap_or_default();
        }
    }
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolkit_slugs_are_safe_path_segments() {
        assert!(valid_toolkit("googlecalendar"));
        assert!(valid_toolkit("one_drive"));
        assert!(!valid_toolkit("../secrets"));
        assert!(!valid_toolkit("Slack"));
        assert!(!valid_toolkit(""));
    }

    #[test]
    fn logo_domains_cover_irregular_slugs() {
        assert_eq!(logo_domain("linear"), "linear.app");
        assert_eq!(logo_domain("notion"), "notion.so");
        assert_eq!(logo_domain("asana"), "asana.com");
        assert_eq!(logo_domain("loom"), "loom.com");
    }

    #[test]
    fn display_names_prefer_catalog_names() {
        assert_eq!(display_name("googlecalendar", "googlecalendar"), "Google Calendar");
        assert_eq!(display_name("slack", "Slack Workspace"), "Slack Workspace");
        assert_eq!(display_name("loom", "loom"), "Loom");
    }
}
