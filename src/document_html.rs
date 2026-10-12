//! A static document surface, isolated from normal browser profiles and app bridges.
#[derive(Clone)]
pub struct DocumentHtml {
    pub html: String,
    pub profile_dir: Option<std::path::PathBuf>,
}
pub fn navigation_allowed(url: &str) -> bool {
    url == "about:blank" || url.starts_with("about:blank#")
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
    Ok(DocumentHtml { html, profile_dir })
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
        assert!(prepare(&"x".repeat(64 * 1024 + 1)).is_err());
        assert!(prepare(&"語".repeat(64 * 1024)).is_err());
    }
}
