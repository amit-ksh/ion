//! A static document surface, isolated from normal browser profiles and app bridges.
#[derive(Clone)]
pub struct DocumentHtml {
    pub html: std::sync::Arc<str>,
    pub profile_dir: Option<std::path::PathBuf>,
    pub navigation_payload: std::sync::Arc<str>,
    pub resource_url: String,
}
impl DocumentHtml {
    pub fn resource_navigation_allowed(&self, url: &str) -> bool {
        url == self.resource_url
            || url
                .strip_prefix(self.resource_url.as_str())
                .is_some_and(|suffix| suffix.starts_with('#'))
    }
}
pub fn navigation_allowed(url: &str) -> bool {
    url == "about:blank" || url.starts_with("about:blank#")
}
// WebView2 exposes NavigateToString as a data navigation. Permit only the
// verified document, never an arbitrary data link authored inside it.
pub fn data_navigation_allowed(url: &str, payload: &str) -> bool {
    [
        "data:text/html;base64,",
        "data:text/html;charset=utf-8;base64,",
        "data:text/html;charset=UTF-8;base64,",
    ]
    .iter()
    .any(|prefix| url.strip_prefix(prefix).is_some_and(|body| body == payload))
}
pub fn prepare(text: &str) -> anyhow::Result<DocumentHtml> {
    anyhow::ensure!(
        text.len() <= crate::document_render::MAX_DOCUMENT_BYTES,
        "HTML exceeds preview bound"
    );
    // First policy, before any authored token. Later policies can only add restrictions.
    let html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'none'; style-src 'unsafe-inline'; img-src data:; font-src 'none'; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'\"><meta name=\"referrer\" content=\"no-referrer\"><style>html{{color-scheme:light}}body{{margin:24px;background:#fff;color:#17201b;font:16px/1.5 system-ui,sans-serif;overflow-wrap:anywhere}}img,svg{{max-width:100%}}pre{{overflow:auto}}@media(prefers-reduced-motion:reduce){{*,*::before,*::after{{animation:none!important;transition:none!important;scroll-behavior:auto!important}}}}</style></head><body>{text}</body></html>"
    );
    #[cfg(target_os = "windows")]
    let profile_dir = {
        let root = dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("Document profile directory unavailable"))?
            .join(waku_client::identity::DATA_DIRECTORY_NAME)
            .join("DocumentWebView2");
        std::fs::create_dir_all(&root)?;
        Some(root)
    };
    #[cfg(not(target_os = "windows"))]
    let profile_dir = None;
    use base64::Engine;
    // Prepared on the document worker, not from render or a native callback.
    let navigation_payload = base64::engine::general_purpose::STANDARD
        .encode(html.as_bytes())
        .into();
    Ok(DocumentHtml {
        html: html.into(),
        profile_dir,
        navigation_payload,
        resource_url: format!("https://ion-document.invalid/{}", uuid::Uuid::new_v4()),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn document_html_navigation_never_becomes_browser_navigation() {
        for url in [
            "https://example.test",
            "file:///private",
            "javascript:alert(1)",
            "data:text/html,evil",
            "ion://run",
            "about:blank.evil",
            "about:blank?outside",
        ] {
            assert!(!navigation_allowed(url));
        }
        assert!(navigation_allowed("about:blank"));
        assert!(navigation_allowed("about:blank#section"));
    }
    #[test]
    fn document_data_navigation_is_bound_to_verified_html() {
        use base64::Engine;
        let page = prepare("<h1>Verified report</h1>").unwrap();
        let uri = format!("data:text/html;base64,{}", page.navigation_payload);
        assert!(data_navigation_allowed(&uri, &page.navigation_payload));
        assert!(data_navigation_allowed(
            &uri.replacen(";base64", ";charset=utf-8;base64", 1),
            &page.navigation_payload
        ));
        let other = base64::engine::general_purpose::STANDARD.encode("<h1>Other document</h1>");
        assert!(!data_navigation_allowed(
            &format!("data:text/html;base64,{other}"),
            &page.navigation_payload
        ));
        assert!(!data_navigation_allowed(
            &(uri.clone() + "#fragment"),
            &page.navigation_payload
        ));
        assert!(!data_navigation_allowed(
            &uri.replace("text/html", "image/svg+xml"),
            &page.navigation_payload
        ));
        assert!(!data_navigation_allowed(
            "https://example.test",
            &page.navigation_payload
        ));
        assert!(page.resource_navigation_allowed(&page.resource_url));
        assert!(page.resource_navigation_allowed(&(page.resource_url.clone() + "#section")));
        assert!(!page.resource_navigation_allowed(&(page.resource_url.clone() + "/other")));
        assert!(!page.resource_navigation_allowed("https://ion-document.invalid/other"));
    }
    #[test]
    fn document_html_policy_precedes_authored_markup_and_preserves_layout() {
        let original = "<style>.card{display:grid}</style><div class='card'>Readable</div><script>fetch('https://remote.test')</script>";
        let page = prepare(original).unwrap();
        assert!(page.html.find("script-src 'none'").unwrap() < page.html.find(original).unwrap());
        assert!(page.html.contains(original));
        assert!(page.html.contains("frame-src 'none'"));
        assert!(page.html.contains("base-uri 'none'"));
    }
    #[test]
    fn document_html_input_bound_is_utf8_byte_based() {
        let limit = crate::document_render::MAX_DOCUMENT_BYTES;
        // The publication/preview limit grew from 64 KiB to 16 MiB. Check
        // both edges of the current contract rather than its former value.
        assert!(prepare(&"x".repeat(limit)).is_ok());
        assert!(prepare(&"x".repeat(limit + 1)).is_err());
        assert!(prepare(&"語".repeat(limit / "語".len())).is_ok());
        assert!(prepare(&"語".repeat(limit / "語".len() + 1)).is_err());
    }
}
