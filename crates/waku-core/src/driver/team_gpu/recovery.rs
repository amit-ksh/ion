//! Recover a lost acknowledgement with read-only provider evidence.
//! An operation identity is neither a successful job nor proof of cleanup.
use super::*;
use crate::teams::gpu_jobs::GpuRecoveryWitness;
use uuid::Uuid;
use waku_protocol::teams::{TeamGpuOperationPurpose as Purpose, TeamRecoveryConfirmation};

#[allow(clippy::too_many_arguments)]
pub(crate) fn bind_recovered_operation(
    coordinator: &TeamCoordinator,
    run: Uuid,
    work: Uuid,
    attempt: Uuid,
    revision: u64,
    request: Uuid,
    purpose: Purpose,
    operation: &str,
) -> Result<()> {
    let confirmation = confirmation(request, purpose, operation);
    if coordinator.recovery_confirmation_replayed(run, work, attempt, revision, &confirmation)? {
        return Ok(());
    }
    bind_with(
        &Client::configured()?,
        coordinator,
        run,
        work,
        attempt,
        revision,
        request,
        purpose,
        operation,
    )
}
fn confirmation(request: Uuid, purpose: Purpose, operation: &str) -> TeamRecoveryConfirmation {
    TeamRecoveryConfirmation::BindGpuOperation {
        request_id: request,
        purpose,
        operation_id: operation.into(),
    }
}
#[allow(clippy::too_many_arguments)]
fn bind_with(
    client: &Client,
    coordinator: &TeamCoordinator,
    run: Uuid,
    work: Uuid,
    attempt: Uuid,
    revision: u64,
    request: Uuid,
    purpose: Purpose,
    operation: &str,
) -> Result<()> {
    identifier(operation)?;
    if coordinator.recovery_confirmation_replayed(
        run,
        work,
        attempt,
        revision,
        &confirmation(request, purpose, operation),
    )? {
        return Ok(());
    }
    let witness =
        coordinator.gpu_recovery_witness(run, work, attempt, revision, request, purpose)?;
    ensure!(
        witness.record.project == client.project,
        "GPU recovery owner project changed"
    );
    let (job, compute) = client.verify_recovery(&witness, purpose, operation)?;
    coordinator.bind_recovered_gpu_operation(
        run,
        work,
        attempt,
        revision,
        request,
        purpose,
        operation,
        &job,
        &compute,
        &witness,
        crate::model::unix_time(),
    )
}
impl Client {
    fn verify_recovery(
        &self,
        witness: &GpuRecoveryWitness,
        purpose: Purpose,
        operation: &str,
    ) -> Result<(String, Vec<String>)> {
        let record = &witness.record;
        let value = Self::response(
            self.request(
                reqwest::Method::GET,
                &format!("/ai/v1/jobs/operations/{operation}"),
            )
            .send()?,
        )?;
        let job = value["resourceId"]
            .as_str()
            .context("GPU recovered operation has no job identity")?;
        identifier(job)?;
        ensure!(
            value["id"] == operation && operation != job,
            "GPU recovery operation/resource identity changed"
        );
        // Completion status can be absent (pending) or failed. Neither becomes a success proof.
        if let Some(status) = value.get("status").filter(|v| !v.is_null()) {
            ensure!(
                status.is_object() && status.get("code").is_none_or(|c| c.as_i64().is_some()),
                "GPU operation status invalid"
            );
        }
        ensure!(
            value["requestHeaders"]["x-idempotency-key"]["values"]
                == json!([witness.key.to_string()]),
            "GPU recovery requires the exact original idempotency-key witness"
        );
        let request = &value["request"];
        let name = match purpose {
            Purpose::Creation => "nebius.ai.v1.CreateJobRequest",
            Purpose::Cancellation => "nebius.ai.v1.CancelJobRequest",
            Purpose::Deletion => "nebius.ai.v1.DeleteJobRequest",
        };
        ensure!(
            request["@type"].as_str().and_then(|t| t.rsplit('/').next()) == Some(name),
            "GPU recovery has a different typed request"
        );
        let mut compute = Vec::new();
        match purpose {
            Purpose::Creation => {
                ensure!(
                    creation_intent(request)? == creation_intent(&record.body)?,
                    "GPU original creation intent changed"
                );
                // The API marks injected files immutable. SECRET must expose the actual original
                // bytes; masked history alone is insufficient. Fixed environment values are also checked.
                compute = self
                    .observe_saved(
                        &record.body,
                        &Acknowledged {
                            operation: operation.into(),
                            job: job.into(),
                        },
                    )?
                    .compute_ids;
            }
            Purpose::Cancellation | Purpose::Deletion => {
                ensure!(
                    record.job_id.as_deref() == Some(job)
                        && record.creation_operation.as_deref() != Some(operation)
                        && record.cancellation_operation.as_deref() != Some(operation),
                    "GPU recovery reused another operation/resource"
                );
                ensure!(
                    request.as_object().is_some_and(|r| r.len() == 2) && request["id"] == job,
                    "GPU recovery request differs from the original cancel/delete intent"
                );
            }
        }
        Ok((job.into(), compute))
    }
}
/// Normalize only documented defaults and sensitive leaves. Names, paths, mount
/// configuration and every other field must match the saved request exactly.
fn creation_intent(request: &Value) -> Result<Value> {
    let mut value = request.clone();
    let root = value
        .as_object_mut()
        .context("GPU create request invalid")?;
    root.remove("@type");
    if let Some(dry) = root.remove("dryRun") {
        ensure!(dry == false, "GPU recovery was a dry-run request");
    }
    let spec = root
        .get_mut("spec")
        .and_then(Value::as_object_mut)
        .context("GPU create specification missing")?;
    for (name, default) in [
        ("publicIp", json!(false)),
        ("preemptible", json!(false)),
        ("restartAttempts", json!("0")),
        ("shmSizeBytes", json!("0")),
    ] {
        if spec.get(name).is_none_or(Value::is_null) {
            spec.insert(name.into(), default);
        }
    }
    for (name, leaf) in [
        ("environmentVariables", "value"),
        ("injectedFiles", "content"),
    ] {
        let entries = spec
            .get_mut(name)
            .and_then(Value::as_array_mut)
            .context("GPU original input declaration missing")?;
        ensure!(
            entries.len() <= 32,
            "GPU input declarations exceed their bound"
        );
        for entry in entries {
            entry
                .as_object_mut()
                .context("GPU input declaration invalid")?
                .remove(leaf);
        }
    }
    Ok(value)
}

#[cfg(test)]
mod tests;
