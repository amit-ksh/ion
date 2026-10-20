//! Verified, cancellable, bounded first-use model download. Blocking workers only.
use anyhow::{Context as _, Result, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub model: String,
    pub filename: String,
    pub bytes: u64,
    pub sha256: String,
    pub url: String,
}
#[derive(Clone, Copy, Debug)]
pub enum Progress {
    Waiting,
    Verifying,
    Downloading { bytes: u64, total: u64 },
    Ready,
}
fn download_host(url: &reqwest::Url) -> bool {
    url.scheme() == "https" && url.username().is_empty() && url.password().is_none()
        && url.port().is_none() && url.fragment().is_none()
        && matches!(url.host_str(), Some("huggingface.co" | "cdn-lfs.huggingface.co" |
            "cdn-lfs-us-1.hf.co" | "cdn-lfs-eu-1.hf.co" | "cas-bridge.xethub.hf.co" | "us.aws.cdn.hf.co" | "api.build-with-ion.tech"))
}
fn pinned_source(url: &reqwest::Url, filename: &str) -> bool {
    if !download_host(url) || url.query().is_some() { return false; }
    if url.host_str() == Some("api.build-with-ion.tech") { return url.path() == format!("/models/{filename}"); }
    if url.host_str() != Some("huggingface.co") { return false; }
    let parts: Vec<_> = url.path().split('/').collect();
    parts.len() == 6 && parts[1] == "amit-ksh" && parts[2] == "ion-stt" && parts[3] == "resolve"
        && parts[4].len() == 40 && parts[4].bytes().all(|b| b.is_ascii_hexdigit())
        && parts[5] == filename
}
pub fn manifest() -> Result<Manifest> {
    let value: Manifest = serde_json::from_str(include_str!(
        "../../../../resources/speech/parakeet-110m.json"
    ))?;
    ensure!(
        value.model == super::local::MODEL
            && value.bytes > 0
            && value.bytes <= 200_000_000
            && value.sha256.len() == 64
            && value.sha256.bytes().all(|b| b.is_ascii_hexdigit())
            && !value.filename.contains(['/', '\\', ':'])
            && value.filename.ends_with(".gguf"),
        "Invalid bundled speech model manifest"
    );
    let url = reqwest::Url::parse(&value.url)?;
    ensure!(
        pinned_source(&url, &value.filename),
        "Invalid bundled model origin"
    );
    Ok(value)
}
pub fn directory() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("ION_SPEECH_MODEL_DIR") {
        return Ok(PathBuf::from(path));
    }
    Ok(dirs::cache_dir()
        .context("Could not locate the model cache")?
        .join("ion/speech"))
}
fn private_file(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    Ok(options.open(path)?)
}
fn verify(
    path: &Path,
    expected: &Manifest,
    canceled: &impl Fn() -> bool,
    deadline: Instant,
) -> Result<bool> {
    let info = match fs::symlink_metadata(path) {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e.into()),
    };
    ensure!(
        info.is_file() && !info.is_symlink(),
        "Speech cache contains an invalid model file"
    );
    if info.len() != expected.bytes {
        return Ok(false);
    }
    let mut input = File::open(path)?.take(expected.bytes + 1);
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        ensure!(!canceled(), "Dictation canceled");
        ensure!(
            Instant::now() < deadline,
            "Model preparation timed out. Retry the download."
        );
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()) == expected.sha256)
}
struct Partial(PathBuf);
impl Drop for Partial {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
struct Download<'a> {
    partial: Partial,
    output: Option<File>,
    destination: &'a Path,
    expected: &'a Manifest,
    bytes: u64,
    hash: Sha256,
    last: Instant,
}
impl<'a> Download<'a> {
    fn new(destination: &'a Path, expected: &'a Manifest) -> Result<Self> {
        let partial = Partial(destination.with_extension("partial"));
        if let Ok(info) = fs::symlink_metadata(&partial.0) {
            ensure!(
                info.is_file() && !info.is_symlink(),
                "Invalid partial model file"
            );
            fs::remove_file(&partial.0)?;
        }
        let output = Some(private_file(&partial.0)?);
        Ok(Self {
            partial,
            output,
            destination,
            expected,
            bytes: 0,
            hash: Sha256::new(),
            last: Instant::now() - Duration::from_secs(1),
        })
    }
    fn write(&mut self, chunk: &[u8], progress: &mut impl FnMut(Progress)) -> Result<()> {
        self.bytes += chunk.len() as u64;
        ensure!(
            self.bytes <= self.expected.bytes,
            "Model download exceeds its pinned size"
        );
        self.output
            .as_mut()
            .unwrap()
            .write_all(chunk)
            .context("Could not save model. Check free disk space and retry.")?;
        self.hash.update(chunk);
        if self.last.elapsed() >= Duration::from_millis(500) {
            progress(Progress::Downloading {
                bytes: self.bytes,
                total: self.expected.bytes,
            });
            self.last = Instant::now();
        }
        Ok(())
    }
    fn finish(
        mut self,
        canceled: &impl Fn() -> bool,
        progress: &mut impl FnMut(Progress),
    ) -> Result<()> {
        ensure!(
            self.bytes == self.expected.bytes
                && format!("{:x}", self.hash.clone().finalize()) == self.expected.sha256,
            "Model verification failed. Retry the download."
        );
        ensure!(!canceled(), "Dictation canceled");
        self.output.take().unwrap().sync_all()?;
        // Replace an invalid cache entry only after the new artifact is verified.
        if self.destination.exists() {
            fs::remove_file(self.destination)?;
        }
        fs::rename(&self.partial.0, self.destination)?;
        progress(Progress::Ready);
        Ok(())
    }
}
// Close the handle before the partial guard removes a failed download on Windows.
impl Drop for Download<'_> {
    fn drop(&mut self) {
        drop(self.output.take());
    }
}
#[cfg(test)]
fn copy_verified(
    mut input: impl Read,
    destination: &Path,
    expected: &Manifest,
    canceled: &impl Fn() -> bool,
    progress: &mut impl FnMut(Progress),
) -> Result<()> {
    let mut download = Download::new(destination, expected)?;
    let mut buffer = [0; 64 * 1024];
    loop {
        ensure!(!canceled(), "Dictation canceled");
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        download.write(&buffer[..count], progress)?;
    }
    download.finish(canceled, progress)
}
async fn cancellable<F: std::future::Future>(
    future: F,
    canceled: &impl Fn() -> bool,
) -> Result<F::Output> {
    let mut future = std::pin::pin!(future);
    loop {
        ensure!(!canceled(), "Dictation canceled");
        if let Ok(value) = tokio::time::timeout(Duration::from_millis(100), &mut future).await {
            return Ok(value);
        }
    }
}
async fn fetch_model(
    path: &Path,
    expected: &Manifest,
    canceled: &impl Fn() -> bool,
    progress: &mut impl FnMut(Progress),
    remaining: Duration,
) -> Result<()> {
    let client = reqwest::Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(10).min(remaining))
        .timeout(remaining)
        // Public model requests contain no access token. Permit only the known
        // Hub/CDN chain, retaining the original byte/hash pin after redirects.
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || !download_host(attempt.url()) {
                attempt.error("Untrusted model download redirect")
            } else { attempt.follow() }
        }))
        .build()?;
    let mut response = cancellable(client.get(&expected.url).send(), canceled)
        .await?
        .context("Could not download Parakeet. Check your connection and retry.")?;
    ensure!(
        response.status() == reqwest::StatusCode::OK,
        "Parakeet download is unavailable (HTTP {}). Retry later or install a release with the published model.",
        response.status()
    );
    if let Some(size) = response.content_length() {
        ensure!(
            size == expected.bytes,
            "Model server returned an unexpected size"
        );
    }
    let mut download = Download::new(path, expected)?;
    while let Some(chunk) = cancellable(response.chunk(), canceled)
        .await?
        .context("Model download interrupted. Retry when connected.")?
    {
        download.write(&chunk, progress)?;
    }
    download.finish(canceled, progress)
}
pub fn prepare(
    canceled: &impl Fn() -> bool,
    progress: &mut impl FnMut(Progress),
) -> Result<PathBuf> {
    let expected = manifest()?;
    let root = directory()?;
    fs::create_dir_all(&root)?;
    let info = fs::symlink_metadata(&root)?;
    ensure!(
        info.is_dir() && !info.is_symlink(),
        "Use a regular private directory for the speech model"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
    }
    let lock_path = root.join("download.lock");
    if let Ok(info) = fs::symlink_metadata(&lock_path) {
        ensure!(
            info.is_file() && !info.is_symlink(),
            "Invalid model download lock"
        );
    }
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)?;
    let deadline = Instant::now() + Duration::from_secs(600);
    let mut waiting = false;
    loop {
        ensure!(!canceled(), "Dictation canceled");
        ensure!(
            Instant::now() < deadline,
            "Another model download is still running. Retry later."
        );
        match lock.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) => {
                if !waiting {
                    progress(Progress::Waiting);
                    waiting = true;
                }
                std::thread::sleep(Duration::from_millis(100))
            }
            Err(error) => return Err(error.into()),
        }
    }
    let path = root.join(&expected.filename);
    progress(Progress::Verifying);
    if verify(&path, &expected, canceled, deadline)? {
        progress(Progress::Ready);
        return Ok(path);
    }
    ensure!(!canceled(), "Dictation canceled");
    progress(Progress::Downloading {
        bytes: 0,
        total: expected.bytes,
    });
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    ensure!(
        !remaining.is_zero(),
        "Model preparation timed out. Retry the download."
    );
    let result = runtime.block_on(fetch_model(&path, &expected, canceled, progress, remaining));
    // A system DNS resolver can outlive its canceled async request. Do not let
    // runtime teardown wait indefinitely for that OS-owned blocking lookup.
    runtime.shutdown_timeout(Duration::from_millis(250));
    result?;
    Ok(path)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn expected(data: &[u8]) -> Manifest {
        Manifest {
            model: super::super::local::MODEL.into(),
            filename: "test.gguf".into(),
            bytes: data.len() as u64,
            sha256: format!("{:x}", Sha256::digest(data)),
            url: "https://build-with-ion.tech/models/test.gguf".into(),
        }
    }
    #[test]
    fn pinned_identity_is_110m_tdt_q8() {
        let m = manifest().unwrap();
        assert_eq!(m.model, "nvidia/parakeet-tdt_ctc-110m");
        assert_eq!(m.bytes, 146_029_600);
        assert_eq!(m.url, format!("https://api.build-with-ion.tech/models/{}", m.filename));
    }
    #[test]
    fn model_sources_require_an_immutable_hub_revision_and_trusted_https_redirects() {
        let file = "ion-parakeet-110m-tdt-q8-v1.gguf";
        let source = format!("https://huggingface.co/amit-ksh/ion-stt/resolve/{}/{file}", "a".repeat(40));
        assert!(pinned_source(&reqwest::Url::parse(&source).unwrap(), file));
        for url in [source.replace(&"a".repeat(40), "main"), source.replace("amit-ksh", "other"), source.clone()+"?token=private", source.replace("https:", "http:")] {
            assert!(!pinned_source(&reqwest::Url::parse(&url).unwrap(), file));
        }
        assert!(download_host(&reqwest::Url::parse("https://cas-bridge.xethub.hf.co/opaque?X-Amz-Signature=public-download").unwrap()));
        assert!(download_host(&reqwest::Url::parse("https://us.aws.cdn.hf.co/xet-bridge-us/model?public-download=signature").unwrap()));
        for url in ["https://huggingface.co.attacker.example/model", "http://huggingface.co/model", "https://user:password@huggingface.co/model", "https://127.0.0.1/model", "https://cdn-lfs-us-1.hf.co:444/model"] {
            assert!(!download_host(&reqwest::Url::parse(url).unwrap()));
        }
    }
    #[test]
    #[ignore = "Downloads the public pinned model; run with an isolated ION_SPEECH_MODEL_DIR"]
    fn live_public_model_download_and_cache_reuse() {
        let root = directory().unwrap();
        assert!(std::env::var_os("ION_SPEECH_MODEL_DIR").is_some(), "Use an isolated acceptance cache");
        let expected = manifest().unwrap();
        assert!(!root.join(&expected.filename).exists(), "First-use acceptance requires an empty model cache");
        let mut downloaded = false;
        let model = prepare(&|| false, &mut |state| {
            if let Progress::Downloading { bytes, .. } = state { downloaded |= bytes > 0; }
        }).unwrap();
        assert!(downloaded);
        assert_eq!(fs::metadata(&model).unwrap().len(), expected.bytes);
        let mut redownloaded = false;
        let reused = prepare(&|| false, &mut |state| {
            if let Progress::Downloading { .. } = state { redownloaded = true; }
        }).unwrap();
        assert_eq!(model, reused);
        assert!(!redownloaded, "A verified cache entry must avoid the network");
        assert!(!model.with_extension("partial").exists());
    }
    #[test]
    fn a_stalled_network_future_is_canceled_promptly() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let start = Instant::now();
        let result = runtime.block_on(cancellable(std::future::pending::<()>(), &|| {
            start.elapsed() >= Duration::from_millis(50)
        }));
        assert!(result.unwrap_err().to_string().contains("canceled"));
        assert!(start.elapsed() < Duration::from_secs(1));
    }
    #[test]
    fn incomplete_corrupt_and_canceled_downloads_never_replace_a_model() {
        let root = std::env::temp_dir().join(format!("ion-model-test-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("test.gguf");
        fs::write(&path, b"old").unwrap();
        let m = expected(b"verified");
        for data in [b"short".as_slice(), b"corrupt!", b"too many bytes"] {
            assert!(copy_verified(data, &path, &m, &|| false, &mut |_| {}).is_err());
            assert_eq!(fs::read(&path).unwrap(), b"old");
            assert!(!path.with_extension("partial").exists());
        }
        assert!(copy_verified(b"verified".as_slice(), &path, &m, &|| true, &mut |_| {}).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"old");
        copy_verified(b"verified".as_slice(), &path, &m, &|| false, &mut |_| {}).unwrap();
        assert!(
            verify(
                &path,
                &m,
                &|| false,
                Instant::now() + Duration::from_secs(1)
            )
            .unwrap()
        );
        assert!(
            verify(&path, &m, &|| false, Instant::now())
                .unwrap_err()
                .to_string()
                .contains("timed out")
        );
        assert!(
            verify(&path, &m, &|| true, Instant::now() + Duration::from_secs(1))
                .unwrap_err()
                .to_string()
                .contains("canceled")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
