//! The personal Token Factory sandbox image. Ion builds it inside the user's own
//! AI project: a public Python base, plus the Linux worker shipped with this
//! install and the reviewed command-worker files, installed by one offline
//! setup run.
//! The image holds no credential. Each disposable model run gets the saved key
//! as a separately mounted file instead.
use crate::{CredentialService, inference::Inference};
use anyhow::{Context as _, Result, bail, ensure};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::{
    io::Read as _,
    path::PathBuf,
    sync::OnceLock,
    thread,
    time::{Duration, Instant},
};

const API: &str = "https://api.tokenfactory.nebius.com/sandboxes/v1";
/// Debian bookworm with Python 3.12 and libseccomp. Token Factory's importer
/// cannot pull by digest, so this is the most specific upstream tag; once
/// imported, the project keeps its own copy under `BASE_TAG`.
const BASE_IMAGE: &str = "docker://docker.io/library/python:3.12.14-slim-bookworm";
const BASE_TAG: &str = "ion-base:python-3.12.14-slim-bookworm";
/// Static Linux x86_64 build of `ion-harness-worker`. The installer ships it
/// beside the app executables (in `Contents/Resources` on macOS), so it is
/// exactly as trusted as the app itself. `scripts/sandbox-worker.ts` builds it.
const WORKER_FILE: &str = "ion-harness-worker-linux-x86_64";
const WORKER_MAX_BYTES: u64 = 96 * 1024 * 1024;
/// Where a disposable model run finds the saved key. Owned by the worker user.
pub const WORKER_KEY_PATH: &str = "/home/ion/.config/ion/nebius-key";
/// Matches the owner's per-call completion bound for Nebius team work.
pub const NEBIUS_OUTPUT_TOKENS: u64 = 4096;
const SETUP: &[u8] = include_bytes!("../../../packages/sandbox-image/setup.sh");
const JOB_FILES: [(&str, &[u8]); 5] = [
    (
        "ion_job_worker.py",
        include_bytes!("../../../packages/job-worker/ion_job_worker.py"),
    ),
    (
        "ion_job_guard.py",
        include_bytes!("../../../packages/job-worker/ion_job_guard.py"),
    ),
    (
        "ion_job_runtime.py",
        include_bytes!("../../../packages/job-worker/ion_job_runtime.py"),
    ),
    (
        "ion_job_landlock.py",
        include_bytes!("../../../packages/job-worker/ion_job_landlock.py"),
    ),
    (
        "job-host.json",
        include_bytes!("../../../packages/job-worker/job-host.json"),
    ),
];

fn hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn worker_path() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("Could not locate the Ion executable")?;
    let dir = exe.parent().context("Could not locate the Ion install")?;
    [dir.join(WORKER_FILE), dir.join("../Resources").join(WORKER_FILE)]
        .into_iter()
        .find(|path| path.is_file())
        .context("This Ion install has no Linux sandbox worker. Reinstall Ion to use the sandbox.")
}

fn worker() -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(worker_path()?)?
        .take(WORKER_MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .context("Could not read the Linux sandbox worker")?;
    ensure!(
        !bytes.is_empty() && bytes.len() as u64 <= WORKER_MAX_BYTES,
        "The Linux sandbox worker in this install is damaged. Reinstall Ion."
    );
    Ok(bytes)
}

/// Hashed once per process; an update replaces the file and restarts Ion.
fn worker_digest() -> Result<&'static str> {
    static DIGEST: OnceLock<String> = OnceLock::new();
    if let Some(digest) = DIGEST.get() {
        return Ok(digest);
    }
    let digest = hex(&worker()?);
    Ok(DIGEST.get_or_init(|| digest))
}

/// The tag for this install's image. It changes whenever anything installed
/// into the image changes, so an app update never reuses an older image.
pub fn image_tag() -> Result<String> {
    Ok(tag_for(worker_digest()?))
}

fn tag_for(worker_digest: &str) -> String {
    let mut recipe = Sha256::new();
    for part in [BASE_IMAGE.as_bytes(), worker_digest.as_bytes(), SETUP] {
        recipe.update(part);
        recipe.update([0]);
    }
    for (name, bytes) in JOB_FILES {
        recipe.update(name.as_bytes());
        recipe.update(bytes);
        recipe.update([0]);
    }
    format!("ion-sandbox:{}", &format!("{:x}", recipe.finalize())[..16])
}

/// Model capabilities registered in the image. The worker re-checks each
/// against the live catalog before it spends a token.
fn manifest(key: &str) -> Result<Vec<u8>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let response = Inference::personal(key.to_owned())?
        .request(&client, "models")?
        .query(&[("verbose", "true")])
        .send()
        .context("Could not reach the Token Factory model catalog")?;
    ensure!(
        response.status().is_success(),
        "Token Factory rejected the model catalog request (HTTP {})",
        response.status()
    );
    let mut body = Vec::new();
    response.take(2 * 1024 * 1024 + 1).read_to_end(&mut body)?;
    ensure!(body.len() <= 2 * 1024 * 1024, "Model catalog is too large");
    let catalog: Value = serde_json::from_slice(&body).context("Invalid model catalog")?;
    let mut capabilities: Vec<_> = catalog["data"]
        .as_array()
        .context("Model catalog has no data")?
        .iter()
        .filter_map(|entry| {
            let model = entry["id"].as_str()?;
            let context = entry["context_length"]
                .as_u64()
                .filter(|n| (1..=32_000_000).contains(n))?;
            (crate::is_chat_model(model) && model.len() <= 160).then(|| {
                json!({"provider":"nebius","model":model,"input_cap":context,"output_cap":NEBIUS_OUTPUT_TOKENS})
            })
        })
        .collect();
    capabilities.sort_by(|a, b| a["model"].as_str().cmp(&b["model"].as_str()));
    capabilities.dedup_by(|a, b| a["model"] == b["model"]);
    capabilities.truncate(128);
    ensure!(
        !capabilities.is_empty(),
        "Token Factory lists no chat model with a context limit"
    );
    Ok(serde_json::to_vec(&json!({
        "version": 2,
        "capabilities": capabilities,
        "execution": {"uid": 1000, "gid": 1000, "home": "/home/ion"},
    }))?)
}

struct Project {
    http: reqwest::blocking::Client,
    key: String,
    project: String,
}

impl Project {
    fn new(key: String, project: String) -> Result<Self> {
        Ok(Self {
            http: reqwest::blocking::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(180))
                .build()?,
            key,
            project,
        })
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::blocking::RequestBuilder {
        self.http
            .request(method, format!("{API}{path}"))
            .bearer_auth(&self.key)
            .header("Project", &self.project)
    }

    fn json(response: reqwest::blocking::Response, what: &str) -> Result<Value> {
        let status = response.status();
        let mut body = Vec::new();
        response.take(1024 * 1024 + 1).read_to_end(&mut body)?;
        ensure!(body.len() <= 1024 * 1024, "{what}: response is too large");
        ensure!(status.is_success(), "{what} failed (HTTP {status})");
        serde_json::from_slice(&body).with_context(|| format!("{what}: invalid response"))
    }

    /// Tags resolve with a relative redirect to the image UUID.
    fn lookup(&self, tag: &str) -> Result<Option<String>> {
        let response = self
            .request(reqwest::Method::GET, "/inspect/")
            .query(&[("tag", tag)])
            .send()
            .context("Could not reach Token Factory sandboxes")?;
        match response.status().as_u16() {
            404 => Ok(None),
            302 | 303 | 307 => {
                let location = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .context("Image lookup has no location")?;
                let id = location.trim_end_matches('/').rsplit('/').next().unwrap_or("");
                Ok(Some(uuid(id)?))
            }
            status => bail!("Image lookup failed (HTTP {status})"),
        }
    }

    fn download(&self, image: &str, path: &str) -> Result<Option<Vec<u8>>> {
        let response = self
            .request(reqwest::Method::GET, &format!("/inspect/{image}/download"))
            .query(&[("path", path)])
            .send()?;
        if response.status().as_u16() == 404 {
            return Ok(None);
        }
        ensure!(
            response.status().is_success(),
            "Could not read {path} from the sandbox image (HTTP {})",
            response.status()
        );
        let mut body = Vec::new();
        response.take(256 * 1024 + 1).read_to_end(&mut body)?;
        ensure!(body.len() <= 256 * 1024, "{path} is too large");
        Ok(Some(body))
    }

    /// Content-addressed: an identical file already in the project is reused.
    fn file(&self, bytes: &[u8]) -> Result<String> {
        let digest = hex(bytes);
        let response = self
            .request(reqwest::Method::GET, &format!("/files/{digest}"))
            .send()?;
        let value = if response.status().as_u16() == 404 {
            Self::json(
                self.request(reqwest::Method::POST, "/files")
                    .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
                    .body(bytes.to_vec())
                    .send()
                    .context("Could not upload a sandbox file")?,
                "Sandbox file upload",
            )?
        } else {
            Self::json(response, "Sandbox file lookup")?
        };
        ensure!(
            value["sha256"].as_str() == Some(digest.as_str()),
            "Token Factory stored different file contents"
        );
        uuid(value["uuid"].as_str().unwrap_or(""))
    }

    fn wait(&self, operation: &str, limit: Duration) -> Result<Value> {
        let deadline = Instant::now() + limit;
        let mut missing = 0;
        let mut delay = Duration::from_millis(500);
        loop {
            let response = self
                .request(reqwest::Method::GET, &format!("/operations/{operation}"))
                .send()?;
            // A just-created operation can briefly be invisible.
            if response.status().as_u16() == 404 && missing < 10 {
                missing += 1;
            } else {
                let value = Self::json(response, "Sandbox operation status")?;
                if matches!(
                    value["status"].as_str(),
                    Some("SUCCESS" | "FAILED" | "CANCELLED")
                ) {
                    return Ok(value);
                }
            }
            if Instant::now() >= deadline {
                let _ = self
                    .request(reqwest::Method::DELETE, &format!("/operations/{operation}"))
                    .send();
                bail!("Sandbox operation {operation} did not finish in time and was cancelled");
            }
            thread::sleep(delay);
            delay = (delay * 3 / 2).min(Duration::from_secs(5));
        }
    }

    fn base(&self, progress: &dyn Fn(&str)) -> Result<String> {
        if let Some(image) = self.lookup(BASE_TAG)? {
            return Ok(image);
        }
        progress("Importing the Python base image into your project");
        let value = Self::json(
            self.request(reqwest::Method::POST, "/images/import")
                .json(&json!({"registry":{"url":BASE_IMAGE},"tag":BASE_TAG,"timeout":900}))
                .send()
                .context("Could not start the base image import")?,
            "Base image import",
        )?;
        let operation = value["uuid"].as_str().context("Import has no operation")?;
        let done = self.wait(operation, Duration::from_secs(960))?;
        ensure!(
            done["status"] == "SUCCESS",
            "The base image import did not succeed: {}",
            done["error"].as_str().unwrap_or("no detail")
        );
        uuid(
            done["result"]["image"]
                .as_str()
                .or(done["result_image_uuid"].as_str())
                .unwrap_or(""),
        )
    }

    fn build(&self, base: &str, manifest: &[u8], progress: &dyn Fn(&str)) -> Result<String> {
        progress("Uploading the Ion worker");
        let mut files = serde_json::Map::new();
        let mut mount = |name: &str, uuid: String, mode: &str| {
            files.insert(
                format!("/ion-setup/{name}"),
                json!({"uuid":uuid,"uid":0,"gid":0,"mode":mode}),
            );
        };
        mount("ion-harness-worker", self.file(&worker()?)?, "0755");
        mount("setup.sh", self.file(SETUP)?, "0644");
        mount("host.json", self.file(manifest)?, "0644");
        for (name, bytes) in JOB_FILES {
            mount(name, self.file(bytes)?, "0644");
        }
        progress("Building your sandbox image");
        let value = Self::json(
            self.request(reqwest::Method::POST, "/instances")
                .json(&json!({
                    "image": base,
                    "command": "/bin/sh",
                    "args": ["/ion-setup/setup.sh"],
                    "shell": false,
                    "cwd": "/",
                    "uid": 0,
                    "gid": 0,
                    "env": {"PATH": "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"},
                    "preserve_env": false,
                    "hostname": "ion-image-build",
                    "timeout": 300,
                    "truncate_output_at": 64 * 1024,
                    "disposable": false,
                    "networking": {"enabled": false},
                    "files": files,
                }))
                .send()
                .context("Could not start the sandbox image build")?,
            "Sandbox image build",
        )?;
        let operation = value["uuid"].as_str().context("Build has no operation")?;
        let done = self.wait(operation, Duration::from_secs(420))?;
        let state = &done["metadata"]["result"]["state"];
        ensure!(
            done["status"] == "SUCCESS" && state["exit_code"] == 0 && state["timed_out"] != true,
            "The sandbox image build failed: {}",
            done["error"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| output_tail(&done))
        );
        let image = uuid(
            done["result_image_uuid"]
                .as_str()
                .or(done["result"]["image"].as_str())
                .unwrap_or(""),
        )?;
        ensure!(image != base, "The sandbox image build produced no new image");
        Ok(image)
    }
}

fn uuid(value: &str) -> Result<String> {
    let value = value.to_ascii_lowercase();
    ensure!(
        value.len() == 36
            && value.char_indices().all(|(i, c)| match i {
                8 | 13 | 18 | 23 => c == '-',
                _ => c.is_ascii_hexdigit(),
            }),
        "Token Factory returned no valid ID"
    );
    Ok(value)
}

/// The last line the setup script printed, for a failed build.
fn output_tail(done: &Value) -> String {
    let stream = &done["metadata"]["result"]["stderr"];
    let text = match stream["encoding"].as_str() {
        Some("base64") => {
            use base64::Engine as _;
            base64::engine::general_purpose::STANDARD
                .decode(stream["value"].as_str().unwrap_or(""))
                .ok()
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        }
        _ => stream["value"].as_str().map(str::to_owned),
    };
    text.and_then(|t| t.lines().rev().find(|l| !l.trim().is_empty()).map(str::to_owned))
        .map(|line| line.chars().take(200).collect())
        .unwrap_or_else(|| "setup exited unsuccessfully".into())
}

impl CredentialService {
    fn personal_project(&self) -> Result<Project> {
        let (key, project) = self
            .personal_sandbox()?
            .context("Save your Token Factory key and AI project first")?;
        Project::new(key, project)
    }

    /// `ion-sandbox:<recipe>@<image>` when this project already has the image
    /// for this Ion build. One GET; call on a worker.
    pub fn personal_sandbox_target(&self) -> Result<Option<String>> {
        if self.personal_sandbox()?.is_none() {
            return Ok(None);
        }
        let tag = image_tag()?;
        Ok(self
            .personal_project()?
            .lookup(&tag)?
            .map(|image| format!("{tag}@{image}")))
    }

    /// Build (or reuse) the sandbox image in the saved project and return its
    /// target. Idempotent: an image whose models match is reused as is.
    pub fn prepare_personal_sandbox(&self, progress: &dyn Fn(&str)) -> Result<String> {
        let project = self.personal_project()?;
        let tag = image_tag()?;
        progress("Checking your sandbox image");
        let manifest = manifest(&project.key)?;
        if let Some(image) = project.lookup(&tag)? {
            if project.download(&image, "/ion/host.json")?.as_deref() == Some(&manifest[..]) {
                return Ok(format!("{tag}@{image}"));
            }
        }
        let base = project.base(progress)?;
        let image = project.build(&base, &manifest, progress)?;
        progress("Verifying the image");
        ensure!(
            project.download(&image, "/ion/host.json")?.as_deref() == Some(&manifest[..]),
            "The built image does not contain the expected worker manifest"
        );
        Project::json(
            project
                .request(reqwest::Method::PATCH, &format!("/images/{image}/tag"))
                .json(&json!({ "tag": tag }))
                .send()
                .context("Could not tag the sandbox image")?,
            "Sandbox image tag",
        )?;
        Ok(format!("{tag}@{image}"))
    }

    /// Uploads the saved key as a content-addressed project file and returns its
    /// ID, so a disposable model run can mount it. Only the user's own project
    /// can read it; it never enters an image, a request body or an environment.
    pub fn personal_worker_key_file(&self) -> Result<String> {
        let project = self.personal_project()?;
        project.file(project.key.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_tag_is_a_valid_spawn_reference() {
        let tag = tag_for(&hex(b"worker"));
        assert_ne!(tag, tag_for(&hex(b"another worker")));
        assert!(tag.starts_with("ion-sandbox:"));
        assert_eq!(tag.len(), "ion-sandbox:".len() + 16);
        assert!(
            tag.trim_start_matches("ion-sandbox:")
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
        );
        assert!(BASE_TAG.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_:.".contains(&b)));
    }

    #[test]
    fn setup_installs_every_worker_file() {
        let setup = std::str::from_utf8(SETUP).unwrap();
        for (name, _) in JOB_FILES {
            assert!(setup.contains(&format!("$s/{name}")), "{name}");
        }
        for name in ["ion-harness-worker", "host.json"] {
            assert!(setup.contains(&format!("$s/{name}")), "{name}");
        }
    }
}
