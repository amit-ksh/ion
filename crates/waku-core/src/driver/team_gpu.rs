//! Nebius Serverless AI Jobs lifecycle transport, separate from ConTree.
//! This module is not exposed as an agent tool yet. The coordinator must add
//! monetary reservations, durable GPU waits and output retrieval before launch
//! can be enabled. A job state is not a command/test receipt or cleanup proof.
use crate::teams::jobs::JobRequest;
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{io::Read, time::Duration};
const BASE: &str = "https://api.nebius.cloud";

pub(super) struct Launch {
    pub request: JobRequest,
    pub project: String,
    pub image: String,
    pub platform: String,
    pub preset: String,
    pub subnet: String,
    pub disk_bytes: u64,
    pub shm_bytes: u64,
    pub timeout_seconds: u32,
}
fn identifier(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 160
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_')),
        "Invalid GPU resource identity"
    );
    Ok(())
}
impl Launch {
    fn name(&self) -> String {
        format!("ion-gpu-{}", self.request.request_id)
    }
    pub(super) fn body(&self) -> Result<Value> {
        for id in [&self.project, &self.platform, &self.preset, &self.subnet] {
            identifier(id)?;
        }
        let (repository, digest) = self
            .image
            .rsplit_once("@sha256:")
            .context("Pin the GPU image by SHA256 digest")?;
        ensure!(
            !repository.is_empty()
                && repository.len() <= 512
                && repository
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"/._-:".contains(&b))
                && digest.len() == 64
                && digest.bytes().all(|b| b.is_ascii_hexdigit()),
            "Invalid pinned GPU image"
        );
        ensure!(
            (1..=86400).contains(&self.timeout_seconds)
                && self.request.version == 1
                && !self.request.request_id.is_nil()
                && self.request.timeout_seconds > 0
                && self.request.timeout_seconds <= 3600
                && self.request.timeout_seconds <= self.timeout_seconds
                && (1024 * 1024 * 1024..=1024 * 1024 * 1024 * 1024).contains(&self.disk_bytes)
                && self.shm_bytes <= self.disk_bytes,
            "GPU resource/time limits are invalid"
        );
        ensure!(
            (1..=64).contains(&self.request.argv.len())
                && self.request.argv[0].starts_with('/')
                && self
                    .request
                    .argv
                    .iter()
                    .all(|a| a.len() <= 8192 && !a.contains('\0'))
                && self.request.argv.iter().map(String::len).sum::<usize>() <= 32768,
            "GPU command arguments exceed their bound"
        );
        ensure!(
            self.request.inputs.len() <= 32 && self.request.output_paths.len() <= 32,
            "Too many GPU transfer paths"
        );
        let mut seen = std::collections::HashSet::new();
        let mut transferred = 0;
        for input in &self.request.inputs {
            super::team_jobs::path(&input.path)?;
            ensure!(
                seen.insert(input.path.to_ascii_lowercase())
                    && input.content_base64.len() <= 88 * 1024,
                "Invalid GPU input path or size"
            );
            let bytes = STANDARD.decode(&input.content_base64)?;
            ensure!(bytes.len() <= 64 * 1024, "GPU input file exceeds its bound");
            transferred += bytes.len();
        }
        ensure!(transferred <= 256 * 1024, "GPU transfer exceeds its bound");
        seen.clear();
        for output in &self.request.output_paths {
            super::team_jobs::path(output)?;
            ensure!(
                seen.insert(output.to_ascii_lowercase()),
                "Duplicate GPU output path"
            );
        }
        let payload = serde_json::to_vec(&self.request)?;
        // Native FileInjection is for small configuration, not a dataset upload.
        ensure!(
            payload.len() <= 64 * 1024,
            "GPU injected configuration exceeds 64 KiB; use an approved dataset volume integration"
        );
        Ok(
            json!({"metadata":{"parentId":self.project,"name":self.name(),
            "labels":{"ion-request":self.request.request_id.to_string()}},
            "spec":{"image":self.image,"containerCommand":"/usr/local/bin/ion-job-worker",
            "args":"--request-file /ion/job-request.json","workingDir":"/ion",
            "environmentVariables":[{"name":"ION_JOB_ISOLATED","value":"1"},{"name":"ION_JOB_CHILD_UID","value":"1000"},{"name":"ION_JOB_CHILD_GID","value":"1000"}],
            "injectedFiles":[{"containerPath":"/ion/job-request.json","content":STANDARD.encode(payload)}],
            "platform":self.platform,"preset":self.preset,"subnetId":self.subnet,
            "disk":{"type":"NETWORK_SSD","sizeBytes":self.disk_bytes.to_string()},
            "shmSizeBytes":self.shm_bytes.to_string(),"timeout":format!("{}s",self.timeout_seconds),
            "publicIp":false,"preemptible":false,"restartAttempts":"0","pricingModel":{"onDemand":{}}}}),
        )
    }
}
pub(super) struct Acknowledged {
    pub operation: String,
    pub job: String,
}
pub(super) struct Observation {
    pub state: String,
    pub terminal: bool,
}
pub(super) struct Client {
    http: reqwest::blocking::Client,
    token: String,
    project: String,
    #[cfg(test)]
    base: String,
}
impl Client {
    pub(super) fn configured() -> Result<Self> {
        let token = std::env::var("ION_NEBIUS_GPU_IAM_TOKEN")
            .context("Configure owner GPU IAM credentials privately")?;
        let project = std::env::var("ION_NEBIUS_GPU_PROJECT")
            .context("Configure the GPU project on this owner")?;
        identifier(&project)?;
        ensure!(!token.is_empty(), "GPU IAM credentials missing");
        Ok(Self {
            http: reqwest::blocking::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(3))
                .timeout(Duration::from_secs(8))
                .build()?,
            token,
            project,
            #[cfg(test)]
            base: BASE.into(),
        })
    }
    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::blocking::RequestBuilder {
        #[cfg(test)]
        let base = &self.base;
        #[cfg(not(test))]
        let base = BASE;
        self.http
            .request(method, format!("{base}{path}"))
            .bearer_auth(&self.token)
    }
    fn response(response: reqwest::blocking::Response) -> Result<Value> {
        ensure!(
            response.status().is_success(),
            "GPU lifecycle request is unresolved (HTTP {})",
            response.status()
        );
        let mut bytes = Vec::new();
        response.take(256 * 1024 + 1).read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= 256 * 1024,
            "GPU lifecycle response exceeded its bound"
        );
        Ok(serde_json::from_slice(&bytes)?)
    }
    /// Call exactly once, after the future coordinator has persisted its request.
    /// Any failure can have an uncertain outcome: never retry this POST or fall back.
    pub(super) fn submit(&self, launch: &Launch) -> Result<Acknowledged> {
        ensure!(
            launch.project == self.project,
            "GPU project differs from its owner context"
        );
        let body = launch.body()?;
        let response=self.request(reqwest::Method::POST,"/ai/v1/jobs").json(&body).send()
            .context("GPU create acknowledgement lost; inspect the original named job, never resubmit or fall back")?;
        let receipt = Self::response(response)?;
        let operation = receipt["id"]
            .as_str()
            .context("GPU creation operation missing")?
            .to_string();
        let job = receipt["resourceId"]
            .as_str()
            .context("GPU job identity missing")?
            .to_string();
        identifier(&operation)?;
        identifier(&job)?;
        ensure!(
            operation != job,
            "GPU creation operation and job identities were conflated"
        );
        Ok(Acknowledged { operation, job })
    }
    pub(super) fn creation_finished(&self, ack: &Acknowledged) -> Result<bool> {
        identifier(&ack.operation)?;
        identifier(&ack.job)?;
        let value = Self::response(
            self.request(
                reqwest::Method::GET,
                &format!("/ai/v1/jobs/operations/{}", ack.operation),
            )
            .send()?,
        )?;
        ensure!(
            value["id"] == ack.operation && value["resourceId"] == ack.job,
            "GPU creation identity changed"
        );
        if value.get("status").is_none_or(Value::is_null) {
            return Ok(false);
        }
        ensure!(
            value["status"].is_object()
                && value["status"].get("code").is_none_or(|code| *code == 0),
            "GPU creation failed; retain its original resource identity for reconciliation"
        );
        Ok(true)
    }
    pub(super) fn observe(&self, launch: &Launch, ack: &Acknowledged) -> Result<Observation> {
        identifier(&ack.job)?;
        ensure!(launch.project == self.project, "GPU owner project changed");
        // SECRET is required to bind injected input bytes, not to forward credentials.
        let value = Self::response(
            self.request(reqwest::Method::GET, &format!("/ai/v1/jobs/{}", ack.job))
                .query(&[("view", "SECRET")])
                .send()?,
        )?;
        let expected = launch.body()?;
        ensure!(
            value["metadata"]["id"] == ack.job
                && value["metadata"]["parentId"] == self.project
                && value["metadata"]["name"] == launch.name()
                && value["metadata"]["labels"]["ion-request"]
                    == launch.request.request_id.to_string(),
            "GPU job provenance changed"
        );
        for (key, expected) in expected["spec"].as_object().unwrap() {
            let actual = &value["spec"][key];
            // Protobuf default false/zero fields may be omitted by the service.
            let default_missing = actual.is_null()
                && (*expected == false
                    || (key == "restartAttempts" && *expected == "0")
                    || (key == "shmSizeBytes" && *expected == "0"));
            ensure!(
                actual == expected || default_missing,
                "GPU pinned specification changed: {key}"
            );
        }
        for key in [
            "registryCredentials",
            "sshAuthorizedKeys",
            "ports",
            "volumes",
        ] {
            let extra = &value["spec"][key];
            ensure!(
                extra.is_null()
                    || extra.as_array().is_some_and(Vec::is_empty)
                    || extra.as_object().is_some_and(serde_json::Map::is_empty),
                "Unapproved GPU capability appeared"
            );
        }
        let state = value["status"]["state"]
            .as_str()
            .context("GPU job has no state")?;
        ensure!(
            matches!(
                state,
                "PROVISIONING"
                    | "STARTING"
                    | "IMAGE_PULLING"
                    | "RUNNING"
                    | "CANCELLING"
                    | "DELETING"
                    | "COMPLETED"
                    | "FAILED"
                    | "CANCELLED"
                    | "ERROR"
            ),
            "Unknown GPU job state"
        );
        Ok(Observation {
            state: state.into(),
            terminal: matches!(state, "COMPLETED" | "FAILED" | "CANCELLED" | "ERROR"),
        })
    }
    /// Cancellation acknowledgement is not terminal job or compute cleanup proof.
    pub(super) fn cancel(&self, ack: &Acknowledged) -> Result<String> {
        identifier(&ack.job)?;
        let value = Self::response(
            self.request(reqwest::Method::POST, "/ai/v1/jobs/cancel")
                .json(&json!({"id":ack.job}))
                .send()?,
        )?;
        ensure!(
            value["resourceId"] == ack.job,
            "GPU cancellation resource changed"
        );
        let operation = value["id"]
            .as_str()
            .context("GPU cancellation operation missing")?;
        identifier(operation)?;
        Ok(operation.into())
    }
}

#[cfg(test)]
mod tests {
    use super::super::team_test_http::Server;
    use super::*;
    use uuid::Uuid;
    fn launch() -> Launch {
        Launch {
            request: JobRequest {
                version: 1,
                request_id: Uuid::new_v4(),
                argv: vec![
                    "/usr/local/bin/python3".into(),
                    "-c".into(),
                    "print('gpu fixture')".into(),
                ],
                timeout_seconds: 30,
                inputs: vec![],
                output_paths: vec![],
            },
            project: "project-fixture".into(),
            image: format!("registry.example/ion-gpu@sha256:{}", "a".repeat(64)),
            platform: "gpu-l40s-a".into(),
            preset: "1gpu-8vcpu-32gb".into(),
            subnet: "vpcsubnet-fixture".into(),
            disk_bytes: 16 * 1024 * 1024 * 1024,
            shm_bytes: 0,
            timeout_seconds: 60,
        }
    }
    fn bytes(v: Value) -> Vec<u8> {
        serde_json::to_vec(&v).unwrap()
    }
    fn client(s: &Server) -> Client {
        Client {
            http: reqwest::blocking::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(3))
                .build()
                .unwrap(),
            token: "fixture-only".into(),
            project: "project-fixture".into(),
            base: s.url.clone(),
        }
    }
    fn job(l: &Launch, state: &str) -> Value {
        let mut v = l.body().unwrap();
        v["metadata"]["id"] = json!("aijob-fixture");
        v["status"] = json!({"state":state});
        v
    }
    #[test]
    fn team_gpu_real_api_contract_keeps_creation_job_and_cancel_identities_separate() {
        let l = launch();
        let server = Server::new(vec![
            (
                200,
                bytes(json!({"id":"operation-create","resourceId":"aijob-fixture"})),
            ),
            (
                200,
                bytes(json!({"id":"operation-create","resourceId":"aijob-fixture"})),
            ),
            (200, bytes(job(&l, "RUNNING"))),
            (
                200,
                bytes(
                    json!({"id":"operation-cancel","resourceId":"aijob-fixture","status":{"code":0}}),
                ),
            ),
            (200, bytes(job(&l, "CANCELLED"))),
        ]);
        let c = client(&server);
        let ack = c.submit(&l).unwrap();
        assert!(!c.creation_finished(&ack).unwrap());
        let observed = c.observe(&l, &ack).unwrap();
        assert_eq!(observed.state, "RUNNING");
        assert!(!observed.terminal);
        assert_eq!(c.cancel(&ack).unwrap(), "operation-cancel");
        assert!(c.observe(&l, &ack).unwrap().terminal);
        let requests = server.finish();
        assert_eq!(requests.len(), 5);
        assert!(requests[0].line.starts_with("POST /ai/v1/jobs "));
        assert_eq!(
            requests[0].body["spec"]["args"],
            "--request-file /ion/job-request.json"
        );
        assert_eq!(requests[0].body["spec"]["publicIp"], false);
        assert!(requests[2].line.contains("view=SECRET"));
        assert_eq!(requests[3].body, json!({"id":"aijob-fixture"}));
    }
    #[test]
    fn team_gpu_rejects_mutable_images_oversized_configs_and_changed_job_provenance() {
        let mut l = launch();
        let original = l.image.clone();
        l.image = "registry.example/image:latest".into();
        assert!(l.body().is_err());
        l.image = original;
        l.request.inputs.push(crate::teams::jobs::JobFile {
            path: "data.bin".into(),
            content_base64: STANDARD.encode(vec![0; 64 * 1024]),
        });
        assert!(l.body().is_err());
        l.request.inputs.clear();
        let mut wrong = job(&l, "COMPLETED");
        wrong["spec"]["image"] = json!("unapproved:latest");
        let server = Server::new(vec![(200, bytes(wrong))]);
        let c = client(&server);
        assert!(
            c.observe(
                &l,
                &Acknowledged {
                    operation: "operation-create".into(),
                    job: "aijob-fixture".into()
                }
            )
            .is_err()
        );
        assert_eq!(server.finish().len(), 1);
        let server = Server::new(vec![(500, bytes(json!({})))]);
        let c = client(&server);
        assert!(c.submit(&l).is_err());
        assert_eq!(server.finish().len(), 1);
    }
}
