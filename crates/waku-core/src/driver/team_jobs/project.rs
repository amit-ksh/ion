//! Project transfer is staged durably before uploading or launching compute.
use super::*;
use crate::teams::project::{ProjectSnapshot, snapshot};

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProjectArgs {
    pub argv: Vec<String>,
    pub output_paths: Vec<String>,
    pub timeout_seconds: u32,
}
pub(crate) fn body_for(
    coordinator: &TeamCoordinator,
    p: &Policy,
    request: &JobRequest,
) -> Result<Value> {
    let mut value = body(p, request)?;
    if request.project_snapshot.is_some() {
        let id = coordinator.project_file(request.request_id)?;
        value["args"] = json!(["--project-file", "/ion/job-project.ion"]);
        value["files"] = json!({"/ion/job-project.ion":{"uuid":id,"uid":0,"gid":0,"mode":"0400"}});
    }
    Ok(value)
}
fn file_identity(value: &Value, facts: &ProjectSnapshot) -> Result<Uuid> {
    ensure!(
        value["sha256"].as_str() == Some(facts.sha256.as_str())
            && value["size"].as_u64() == Some(facts.bytes),
        "Uploaded project hash or size differs from its snapshot"
    );
    let id = Uuid::parse_str(
        value["uuid"]
            .as_str()
            .context("Uploaded project identity missing")?,
    )?;
    ensure!(!id.is_nil(), "Uploaded project identity is empty");
    Ok(id)
}
fn lookup(client: &Client, facts: &ProjectSnapshot) -> Result<Option<Uuid>> {
    let response = client
        .request(reqwest::Method::GET, &format!("/files/{}", facts.sha256))
        .send()?;
    if response.status().as_u16() == 404 {
        return Ok(None);
    }
    ensure!(
        response.status().as_u16() == 200,
        "Project lookup unavailable in its original namespace"
    );
    Ok(Some(file_identity(
        &read_json(response, 16 * 1024)?,
        facts,
    )?))
}
fn upload(client: &Client, facts: &ProjectSnapshot, data: &[u8]) -> Result<Uuid> {
    crate::teams::project::matches(data, facts)?;
    if let Some(id) = lookup(client, facts)? {
        return Ok(id);
    }
    let response = client
        .request(reqwest::Method::POST, "/files")
        .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
        .timeout(Duration::from_secs(60))
        .body(data.to_vec())
        .send();
    if let Ok(response) = response {
        if response.status().as_u16() == 201 {
            // A malformed/mismatched acknowledgement is not silently substituted.
            return file_identity(&read_json(response, 16 * 1024)?, facts);
        }
        if matches!(response.status().as_u16(), 400 | 401 | 403 | 404) {
            anyhow::bail!("Project upload rejected before command launch");
        }
    }
    // A lost acknowledgement can be resolved by the original content hash. Never
    // repeat POST /files or launch compute unless the exact bytes are confirmed.
    lookup(client, facts)?.context("Project upload unconfirmed; command was not launched")
}
pub(crate) fn run_project(
    coordinator: &TeamCoordinator,
    claimed: &ClaimedAssignment,
    workspace: &Path,
    request_id: Uuid,
    args: ProjectArgs,
    cancelled: &AtomicBool,
) -> Result<String> {
    run_with_host(
        coordinator,
        claimed,
        workspace,
        request_id,
        args,
        cancelled,
        |target| Ok((policy(target)?, Client::configured()?)),
    )
}
#[allow(clippy::too_many_arguments)]
pub(super) fn run_with_host(
    coordinator: &TeamCoordinator,
    claimed: &ClaimedAssignment,
    workspace: &Path,
    request_id: Uuid,
    args: ProjectArgs,
    cancelled: &AtomicBool,
    configure: impl FnOnce(Uuid) -> Result<(Policy, Client)>,
) -> Result<String> {
    let profile = coordinator.authorize_remote_job(&claimed.lease, crate::model::unix_time())?;
    let target = profile
        .execution
        .sandbox_target
        .as_deref()
        .context("Pinned sandbox target missing")?;
    let target = Uuid::parse_str(target.rsplit('@').next().unwrap_or(target))?;
    let (policy, client) = configure(target)?;
    validate_policy(&policy)?;
    ensure!(
        policy.target == target && policy.resources.is_some(),
        "Project command host or child resource approval missing"
    );
    let project_policy = policy
        .project_snapshot
        .as_ref()
        .context("Approve project snapshot transfer in the command image registry")?;
    validate_args(
        &policy,
        &CommandArgs {
            argv: args.argv.clone(),
            input_paths: vec![],
            output_paths: args.output_paths.clone(),
            timeout_seconds: args.timeout_seconds,
        },
    )?;
    let context =
        coordinator.project_source_context(&claimed.lease, workspace, crate::model::unix_time())?;
    let (facts, archive) = snapshot(workspace, project_policy)?;
    let request = JobRequest {
        version: 1,
        request_id,
        argv: args.argv,
        timeout_seconds: args.timeout_seconds,
        inputs: vec![],
        output_paths: args.output_paths,
        project_snapshot: Some(facts.clone()),
    };
    body(&policy, &request)?;
    let host = manifest(&client, &policy)?;
    ensure!(
        host["project_snapshot"] == "ion-project-archive/v1",
        "Registered image does not support project snapshots"
    );
    // Recheck the entire inventory/content before reserving the effect. Changes
    // during snapshot creation cannot become a validation of different sources.
    ensure!(
        snapshot(workspace, project_policy)?.0 == facts,
        "Project changed while creating its snapshot"
    );
    let output = match coordinator.reserve_remote_project(
        &claimed.lease,
        &request,
        policy.image,
        &client.project,
        &serde_json::to_string(&policy)?,
        policy.max_commands,
        crate::model::unix_time(),
        &context,
        &archive,
    )? {
        Some(receipt) => receipt,
        None => {
            let stage = (|| {
                ensure!(
                    !cancelled.load(Ordering::Acquire),
                    "Stopped before project upload"
                );
                coordinator.authorize_remote_job(&claimed.lease, crate::model::unix_time())?;
                let file = upload(&client, &facts, &archive)?;
                ensure!(
                    !cancelled.load(Ordering::Acquire),
                    "Stopped before project command submission"
                );
                coordinator.confirm_project_upload(
                    &claimed.lease,
                    request_id,
                    file,
                    crate::model::unix_time(),
                )
            })();
            if let Err(error) = stage {
                coordinator.reject_job(request_id)?;
                return Err(error);
            }
            execute_with(
                &client,
                coordinator,
                &claimed.lease,
                &policy,
                &request,
                cancelled,
            )?
        }
    };
    let artifact = coordinator.publish_document(
        &claimed.lease,
        request_id,
        &format!("command-{request_id}"),
        "json",
        &output.summary()?.to_string(),
        crate::model::unix_time(),
    )?;
    output.tool_reply(artifact.id)
}
