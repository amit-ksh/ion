//! Bounded source snapshots, prepared on a worker and reviewed before upload.
use super::*;
use std::path::{Component, Path};
#[derive(Clone)]
pub struct Bundle {
    pub files: Vec<Value>,
    pub bytes: u64,
    pub entry: String,
    pub paths: Vec<String>,
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
    })
}
pub fn project(root: &Path) -> Result<Bundle> {
    let root = root
        .canonicalize()
        .context("Project directory unavailable")?;
    let output = std::process::Command::new("git")
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ])
        .current_dir(&root)
        .output()
        .context("Could not prepare the Git source export")?;
    ensure!(
        output.status.success(),
        "Project deployment needs a Git source directory"
    );
    let mut files = Vec::new();
    let mut paths = Vec::new();
    let mut bytes = 0u64;
    let mut names = std::collections::BTreeSet::new();
    for name in output.stdout.split(|b| *b == 0).filter(|v| !v.is_empty()) {
        let name = std::str::from_utf8(name).context("Source paths must be UTF-8")?;
        if !allowed(name) || !names.insert(name.to_owned()) {
            continue;
        }
        let relative = Path::new(name);
        ensure!(
            relative
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
            "Invalid project source path"
        );
        let path = root.join(relative);
        let mut ancestor = root.clone();
        for component in relative.components() {
            ancestor.push(component.as_os_str());
            ensure!(
                !std::fs::symlink_metadata(&ancestor)?
                    .file_type()
                    .is_symlink(),
                "Source exports cannot contain links"
            );
        }
        let metadata = std::fs::metadata(&path)?;
        if !metadata.is_file() {
            continue;
        }
        ensure!(metadata.len() <= 8_000_000, "A source file exceeds 8 MB");
        bytes = bytes
            .checked_add(metadata.len())
            .context("Source size overflow")?;
        ensure!(
            bytes <= 20_000_000 && files.len() < 500,
            "Deploy at most 500 files and 20 MB"
        );
        let content = std::fs::read(&path)?;
        files.push(file(name, content)?);
        paths.push(name.to_owned());
    }
    ensure!(!files.is_empty(), "No deployable source files were found");
    let entry = if paths.iter().any(|p| p == "index.html") {
        "index.html"
    } else if paths.iter().any(|p| p == "package.json") {
        "package.json"
    } else if paths.iter().any(|p| p == "main.py") {
        "main.py"
    } else {
        paths[0].as_str()
    }
    .to_owned();
    Ok(Bundle {
        files,
        bytes,
        entry,
        paths,
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
    })
}
