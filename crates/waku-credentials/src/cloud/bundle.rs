//! Bounded source snapshots, prepared on a worker and reviewed before upload.
use super::*;
use std::path::{Component, Path};
#[derive(Clone)]
pub struct Bundle {
    pub files: Vec<Value>,
    pub bytes: u64,
    pub entry: String,
    pub paths: Vec<String>,
    pub export_directory: Option<String>,
}
fn allowed(path: &str) -> bool {
    !path.contains('\\')
        && path.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.starts_with('.')
                && !matches!(
                    part.to_ascii_lowercase().as_str(),
                    "node_modules"
                        | "target"
                        | "vendor"
                        | "dist"
                        | "build"
                        | "coverage"
                        | "credentials"
                        | "secrets"
                        | "id_rsa"
                        | "__pycache__"
                )
        })
        && ![
            "pem", "key", "p12", "pfx", "keystore", "sqlite", "db", "env",
        ]
        .iter()
        .any(|ext| path.to_ascii_lowercase().ends_with(&format!(".{ext}")))
}
fn file(path: &str, bytes: Vec<u8>) -> Result<Value> {
    ensure!(
        allowed(path) && path.len() <= 240,
        "This file is excluded from cloud source exports"
    );
    ensure!(bytes.len() <= 8_000_000, "A source file exceeds 8 MB");
    let prefix = String::from_utf8_lossy(&bytes[..bytes.len().min(8192)]);
    ensure!(
        !prefix.contains("PRIVATE KEY-----"),
        "A private key was found in the source export"
    );
    use base64::engine::general_purpose::STANDARD;
    use sha2::{Digest, Sha256};
    Ok(
        json!({"path":path,"bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(&bytes)),"content":STANDARD.encode(&bytes)}),
    )
}
pub fn html(bytes: Vec<u8>, expected_hash: &str) -> Result<Bundle> {
    use sha2::{Digest, Sha256};
    ensure!(
        format!("{:x}", Sha256::digest(&bytes)) == expected_hash,
        "The artifact changed; refresh it before publishing"
    );
    let size = bytes.len() as u64;
    let file = file("index.html", bytes)?;
    Ok(Bundle {
        files: vec![file],
        bytes: size,
        entry: "index.html".into(),
        paths: vec!["index.html".into()],
        export_directory: None,
    })
}
/// Select an existing static export, without running any build commands.
pub fn project(root: &Path) -> Result<Bundle> {
    let directory = ["dist", "out", "build", "."]
        .into_iter()
        .find(|directory| root.join(directory).join("index.html").is_file())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Build or export the app locally, then review its static output folder."
            )
        })?;
    static_project(root, directory)
}

/// Snapshot only static assets inside the explicitly selected project folder.
pub fn static_project(root: &Path, directory: &str) -> Result<Bundle> {
    let root = root
        .canonicalize()
        .context("Project directory unavailable")?;
    let relative = Path::new(directory);
    ensure!(
        directory == "."
            || (!directory.is_empty()
                && relative
                    .components()
                    .all(|c| matches!(c, Component::Normal(_)))),
        "Choose a relative static export folder inside this project"
    );
    let mut export = root.clone();
    if directory != "." {
        for component in relative.components() {
            export.push(component.as_os_str());
            ensure!(
                !std::fs::symlink_metadata(&export)?.file_type().is_symlink(),
                "Static exports cannot contain links"
            );
        }
    }
    let export = export
        .canonicalize()
        .context("Static export folder unavailable")?;
    ensure!(
        export.starts_with(&root) && export.is_dir(),
        "Static export must stay inside this project"
    );
    let mut pending = vec![export.clone()];
    let mut files = Vec::new();
    let mut paths = Vec::new();
    let mut bytes = 0u64;
    let mut visited = 0usize;
    while let Some(folder) = pending.pop() {
        for child in std::fs::read_dir(folder)? {
            let child = child?;
            visited += 1;
            ensure!(visited <= 10_000, "Static export contains too many entries");
            let path = child.path();
            let name = path
                .strip_prefix(&export)?
                .to_str()
                .context("Static paths must be UTF-8")?
                .replace('\\', "/");
            if !allowed(&name) {
                continue;
            }
            let metadata = std::fs::symlink_metadata(&path)?;
            ensure!(
                !metadata.file_type().is_symlink(),
                "Static exports cannot contain links"
            );
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                ensure!(
                    metadata.file_attributes() & 0x400 == 0,
                    "Static exports cannot contain reparse points"
                );
            }
            ensure!(
                path.canonicalize()?.starts_with(&export),
                "Static files must stay inside the export folder"
            );
            if metadata.is_dir() {
                pending.push(path);
                continue;
            }
            if !metadata.is_file() {
                continue;
            }
            let extension = path
                .extension()
                .and_then(|v| v.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if ![
                "html", "htm", "css", "js", "mjs", "json", "svg", "png", "jpg", "jpeg", "webp",
                "gif", "ico", "woff", "woff2", "ttf", "txt", "pdf", "wasm", "webm", "mp4", "avif",
                "mp3", "wav",
            ]
            .contains(&extension.as_str())
            {
                continue;
            }
            ensure!(metadata.len() <= 8_000_000, "A static file exceeds 8 MB");
            // Bound reads even if a file grows between metadata and read.
            use std::io::Read;
            let mut content = Vec::new();
            std::fs::File::open(&path)?
                .take(8_000_001)
                .read_to_end(&mut content)?;
            ensure!(content.len() <= 8_000_000, "A static file exceeds 8 MB");
            bytes = bytes
                .checked_add(content.len() as u64)
                .context("Static export size overflow")?;
            ensure!(
                bytes <= 20_000_000 && files.len() < 500,
                "Publish at most 500 files and 20 MB"
            );
            files.push(file(&name, content)?);
            paths.push(name);
        }
    }
    ensure!(
        paths.iter().any(|path| path == "index.html"),
        "Static export needs index.html. Build or export the app locally first."
    );
    let mut order: Vec<_> = paths.into_iter().zip(files).collect();
    order.sort_by(|a, b| a.0.cmp(&b.0));
    let (paths, files) = order.into_iter().unzip();
    Ok(Bundle {
        files,
        bytes,
        entry: "index.html".into(),
        paths,
        export_directory: Some(directory.to_owned()),
    })
}

pub fn resources(sources: Vec<(String, Vec<u8>, String, u64)>, entry: String) -> Result<Bundle> {
    use sha2::{Digest, Sha256};
    ensure!(
        !sources.is_empty() && sources.len() <= 33,
        "Invalid HTML resource membership"
    );
    let mut paths = Vec::new();
    let mut files = Vec::new();
    let mut size = 0u64;
    for (path, bytes, expected, length) in sources {
        ensure!(
            bytes.len() as u64 == length && format!("{:x}", Sha256::digest(&bytes)) == expected,
            "HTML resource changed"
        );
        size += length;
        ensure!(
            size <= 20_000_000 && !paths.contains(&path),
            "HTML bundle exceeds its limit"
        );
        files.push(file(&path, bytes)?);
        paths.push(path);
    }
    ensure!(paths.contains(&entry), "HTML entry is unavailable");
    Ok(Bundle {
        files,
        bytes: size,
        entry,
        paths,
        export_directory: None,
    })
}

#[cfg(test)]
mod static_export_tests {
    use super::*;
    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "ion-static-export-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn write(&self, path: &str, content: &[u8]) {
            let path = self.0.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn exports_built_assets_without_git_or_source_upload() {
        let root = Fixture::new();
        root.write("dist/index.html", b"<h1>Static</h1>");
        root.write("dist/assets/app.js", b"console.log('static')");
        root.write("dist/.env", b"SECRET=private");
        root.write("dist/server.py", b"server implementation");
        root.write("dist/node_modules/dependency.js", b"not a static asset");
        root.write("src/private.ts", b"private source");
        let bundle = project(&root.0).unwrap();
        assert_eq!(bundle.export_directory.as_deref(), Some("dist"));
        assert_eq!(bundle.paths, vec!["assets/app.js", "index.html"]);
        assert_eq!(bundle.entry, "index.html");
    }
    #[test]
    fn explicit_export_stays_inside_project_and_requires_html() {
        let root = Fixture::new();
        root.write("package.json", b"{}");
        assert!(project(&root.0).is_err());
        root.write("site/index.html", b"<h1>Static</h1>");
        assert!(static_project(&root.0, "site").is_ok());
        for directory in ["", "..", "site/../site", "/", "C:\\", "\\\\server\\share"] {
            assert!(static_project(&root.0, directory).is_err(), "{directory}");
        }
        assert!(static_project(&root.0, ".").is_err());
        root.write("index.html", b"<h1>Plain HTML</h1>");
        assert!(static_project(&root.0, ".").is_ok());
    }
    #[test]
    fn rejects_private_keys_and_oversized_assets() {
        let root = Fixture::new();
        root.write("out/index.html", b"<h1>Static</h1>");
        root.write("out/leak.txt", b"-----BEGIN PRIVATE KEY-----");
        assert!(project(&root.0).is_err());
        std::fs::remove_file(root.0.join("out/leak.txt")).unwrap();
        root.write("out/large.js", &vec![0; 8_000_001]);
        assert!(project(&root.0).is_err());
    }
    #[test]
    #[ignore = "publishes and removes a disposable static site using the actual native account"]
    fn live_native_static_publication() {
        assert_eq!(
            std::env::var("ION_STATIC_PUBLISH_ACCEPTANCE").as_deref(),
            Ok("1")
        );
        let report_path =
            std::env::var_os("ION_STATIC_PUBLISH_REPORT").expect("Sanitized evidence path");
        assert!(
            !Path::new(&report_path).exists(),
            "Inspect existing publication evidence before retrying"
        );
        let workspace = super::super::verified_status()
            .unwrap()
            .workspace_id
            .unwrap();
        let controls = super::super::services::controls(&workspace).unwrap();
        assert!(!controls.cloud, "This check must run with Nebius Cloud OFF");
        assert_eq!(
            super::super::services::bootstrap().unwrap()["features"]["appDeployment"],
            true
        );
        let call = |method: &str, path: &str, body: Option<Value>| -> Result<Value> {
            ensure!(
                super::super::status()?.workspace_id.as_deref() == Some(workspace.as_str()),
                "The native account changed"
            );
            super::super::services::hosting_request(&workspace, method, path, body)
        };
        let operation = format!(
            "static-acceptance-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let html = b"<!doctype html><title>Ion static acceptance</title><h1>Static app</h1>";
        let js = b"document.querySelector('h1').textContent='Ion static app'";
        let body = json!({"sourceId":operation,"title":"Temporary Ion static acceptance","kind":"project","visibility":"private","idempotencyKey":operation,
            "settings":{"runtime":"static","entry":"index.html","build":[],"start":[],"outputDirectory":"dist","port":3000,"healthPath":"/","spa":true},
            "files":[file("index.html",html.to_vec()).unwrap(),file("assets/app.js",js.to_vec()).unwrap()]});
        std::fs::write(
            &report_path,
            serde_json::to_vec_pretty(
                &json!({"state":"publish-intent","sourceId":operation,"paidComputeCalls":0}),
            )
            .unwrap(),
        )
        .unwrap();
        let mut site: Option<Value> = None;
        let result = (|| -> Result<()> {
            let created = call("POST", "/v1/hosting/releases", Some(body.clone()))?;
            site = Some(created.clone());
            let id = created["id"].as_str().context("Site ID missing")?;
            let url = created["url"].as_str().context("Site URL missing")?;
            ensure!(
                created["releases"][0]["status"] == "ready",
                "Static release was not ready"
            );
            let client = reqwest::blocking::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(45))
                .build()?;
            ensure!(
                client.get(url).send()?.status().as_u16() == 403,
                "Private content accessible anonymously"
            );
            ensure!(
                call("POST", "/v1/hosting/releases", Some(body.clone()))?["active"]
                    == created["active"],
                "Idempotency changed the active release"
            );
            call(
                "POST",
                &format!("/v1/hosting/sites/{id}/visibility"),
                Some(json!({"visibility":"public"})),
            )?;
            ensure!(
                client.get(url).send()?.text()?.as_bytes() == html,
                "HTML content changed"
            );
            ensure!(
                client
                    .get(format!("{url}assets/app.js"))
                    .send()?
                    .text()?
                    .as_bytes()
                    == js,
                "Asset content changed"
            );
            ensure!(
                client
                    .get(format!("{url}settings/profile"))
                    .send()?
                    .text()?
                    .as_bytes()
                    == html,
                "SPA fallback failed"
            );
            let mut dynamic = body.clone();
            dynamic["settings"]["runtime"] = json!("node");
            dynamic["settings"]["start"] = json!(["node", "index.html"]);
            let failure = call("POST", "/v1/hosting/releases", Some(dynamic))
                .expect_err("Server publishing must be rejected");
            ensure!(
                failure.to_string().contains("403"),
                "Unexpected server rejection"
            );
            call(
                "POST",
                &format!("/v1/hosting/sites/{id}/stop"),
                Some(json!({})),
            )?;
            ensure!(
                client.get(url).send()?.status().as_u16() == 403,
                "Unpublished content still accessible"
            );
            call(
                "POST",
                &format!("/v1/hosting/sites/{id}/start"),
                Some(json!({})),
            )?;
            ensure!(
                client.get(url).send()?.status().is_success(),
                "Static republish failed"
            );
            Ok(())
        })();
        let cleaned_up = site
            .as_ref()
            .and_then(|site| site["id"].as_str())
            .is_some_and(|id| call("DELETE", &format!("/v1/hosting/sites/{id}"), None).is_ok());
        let after = super::super::services::controls(&workspace).unwrap();
        let unchanged = after.cloud == controls.cloud
            && after.token_factory == controls.token_factory
            && after.app_actions == controls.app_actions;
        std::fs::write(&report_path, serde_json::to_vec_pretty(&json!({"state":"completed","passed":result.is_ok() && cleaned_up && unchanged,
            "siteId":site.as_ref().map(|site| &site["id"]),"cleanedUp":cleaned_up,"controlsUnchanged":unchanged,"paidComputeCalls":0,
            "failure":result.as_ref().err().map(|e|e.to_string())})).unwrap()).unwrap();
        assert!(
            cleaned_up && unchanged,
            "Inspect the sanitized evidence and reconcile cleanup"
        );
        result.unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn rejects_linked_export_folders_and_assets() {
        let root = Fixture::new();
        root.write("site/index.html", b"<h1>Static</h1>");
        std::os::unix::fs::symlink(root.0.join("site"), root.0.join("dist")).unwrap();
        assert!(project(&root.0).is_err());
        std::os::unix::fs::symlink(
            root.0.join("site/index.html"),
            root.0.join("site/linked.html"),
        )
        .unwrap();
        assert!(static_project(&root.0, "site").is_err());
    }
}
