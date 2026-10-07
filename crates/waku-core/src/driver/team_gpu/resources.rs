//! Native GPU VM/disk inventory and deletion proofs. Never directly delete a VM/disk.
use super::*;
use crate::teams::gpu_jobs::{GpuCleanupRecord, GpuComputeInventory, GpuDiskInventory};
use std::collections::HashSet;
impl Client {
    fn resource(&self, path: &str) -> Result<Option<Value>> {
        let response = self.request(reqwest::Method::GET, path).send()?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            let mut bytes = Vec::new();
            response.take(16 * 1024 + 1).read_to_end(&mut bytes)?;
            ensure!(
                bytes.len() <= 16 * 1024,
                "GPU absence response exceeded its bound"
            );
            let status: Value = serde_json::from_slice(&bytes)
                .context("GPU absence lacks a structured service error")?;
            ensure!(
                status["code"] == 5
                    || (status["error"]["code"] == 404 && status["error"]["status"] == "NOT_FOUND"),
                "GPU HTTP 404 is not a resource-not-found witness"
            );
            return Ok(None);
        }
        Ok(Some(Self::response(response)?))
    }
    pub(super) fn inventory(
        &self,
        body: &Value,
        compute: &str,
    ) -> Result<Option<GpuComputeInventory>> {
        identifier(compute)?;
        let Some(vm) = self.resource(&format!("/compute/v1/instances/{compute}"))? else {
            return Ok(None);
        };
        ensure!(
            vm["metadata"]["id"] == compute && vm["metadata"]["parentId"] == self.project,
            "GPU compute identity/project changed"
        );
        ensure!(
            vm["spec"]["resources"]["platform"] == body["spec"]["platform"]
                && vm["spec"]["resources"]["preset"] == body["spec"]["preset"],
            "GPU allocated platform/preset changed"
        );
        ensure!(
            vm["spec"]
                .get("filesystems")
                .is_none_or(|v| v.is_null() || v.as_array().is_some_and(Vec::is_empty)),
            "Unapproved GPU filesystem appeared"
        );
        let mut desired = vec![&vm["spec"]["bootDisk"]];
        if let Some(extra) = vm["spec"].get("secondaryDisks").filter(|v| !v.is_null()) {
            let extra = extra
                .as_array()
                .context("GPU disk specifications invalid")?;
            ensure!(extra.len() <= 16, "GPU disk count exceeded");
            desired.extend(extra);
        }
        let mut names = HashSet::new();
        let mut existing = HashSet::new();
        for disk in &desired {
            let managed = &disk["managedDisk"];
            let external = &disk["existingDisk"];
            ensure!(
                managed.is_object() != external.is_object(),
                "GPU disk attachment has ambiguous ownership"
            );
            if managed.is_object() {
                let name = managed["name"]
                    .as_str()
                    .context("GPU managed disk name missing")?;
                ensure!(
                    !name.is_empty() && name.len() <= 160 && names.insert(name),
                    "GPU managed disk name invalid or repeated"
                );
            } else {
                let id = external["id"]
                    .as_str()
                    .context("GPU existing disk identity missing")?;
                identifier(id)?;
                ensure!(existing.insert(id), "Repeated existing GPU disk");
            }
        }
        let attachments = vm["status"]
            .get("diskAttachments")
            .filter(|v| !v.is_null())
            .map(|v| v.as_array().context("GPU disk status invalid"))
            .transpose()?;
        ensure!(
            attachments.is_none_or(|v| v.len() <= 17),
            "GPU attachment status exceeded its bound"
        );
        let mut seen = HashSet::new();
        let mut matched_names = HashSet::new();
        let mut matched_existing = HashSet::new();
        let mut disks = Vec::new();
        let mut complete = true;
        for attachment in attachments.into_iter().flatten() {
            let managed = attachment
                .get("isManaged")
                .map(|v| v.as_bool().context("GPU disk ownership status invalid"))
                .transpose()?
                .unwrap_or(false);
            let id = attachment["id"].as_str().unwrap_or("");
            if managed {
                let name = attachment["name"]
                    .as_str()
                    .context("GPU disk status name missing")?;
                ensure!(
                    names.contains(name) && matched_names.insert(name),
                    "Unapproved/repeated GPU managed disk appeared"
                );
            } else {
                ensure!(
                    existing.contains(id) && matched_existing.insert(id),
                    "Unapproved/repeated GPU existing disk appeared"
                );
            }
            if id.is_empty() {
                complete = false;
                continue;
            }
            identifier(id)?;
            ensure!(seen.insert(id), "Repeated GPU attachment identity");
            let disk = self
                .resource(&format!("/compute/v1/disks/{id}"))?
                .context("GPU disk disappeared before inventory capture")?;
            ensure!(
                disk["metadata"]["id"] == id && disk["metadata"]["parentId"] == self.project,
                "GPU disk identity/project changed"
            );
            disks.push(GpuDiskInventory {
                id: id.into(),
                managed,
            });
        }
        complete &= matched_names.len() == names.len()
            && matched_existing.len() == existing.len()
            && disks.len() == desired.len();
        disks.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(Some(GpuComputeInventory {
            compute_id: compute.into(),
            disks,
            complete,
        }))
    }
    pub(super) fn delete_job(&self, ack: &Acknowledged, key: uuid::Uuid) -> Result<String> {
        identifier(&ack.job)?;
        ensure!(!key.is_nil(), "GPU deletion key missing");
        let value = Self::response(
            self.request(reqwest::Method::DELETE, &format!("/ai/v1/jobs/{}", ack.job))
                .header("X-Idempotency-Key", key.to_string())
                .send()?,
        )?;
        ensure!(
            value["resourceId"] == ack.job,
            "GPU deletion resource changed"
        );
        let operation = value["id"]
            .as_str()
            .context("GPU deletion operation missing")?;
        identifier(operation)?;
        ensure!(
            operation != ack.job && operation != ack.operation,
            "GPU deletion reused creation/resource identity"
        );
        Ok(operation.into())
    }
    pub(super) fn cleanup_proven(
        &self,
        ack: &Acknowledged,
        claim: &GpuCleanupRecord,
    ) -> Result<bool> {
        identifier(&ack.job)?;
        let operation = claim
            .operation
            .as_deref()
            .context("GPU deletion acknowledgement is unknown; never repeat DELETE")?;
        identifier(operation)?;
        let value = Self::response(
            self.request(
                reqwest::Method::GET,
                &format!("/ai/v1/jobs/operations/{operation}"),
            )
            .send()?,
        )?;
        ensure!(
            value["id"] == operation && value["resourceId"] == ack.job,
            "GPU deletion operation identity changed"
        );
        if value.get("status").is_none_or(Value::is_null) {
            return Ok(false);
        }
        ensure!(
            value["status"].is_object() && value["status"].get("code").is_none_or(|v| *v == 0),
            "GPU deletion failed; cleanup remains unresolved"
        );
        let request = &value["request"];
        ensure!(
            request["@type"].as_str().and_then(|v| v.rsplit('/').next())
                == Some("nebius.ai.v1.DeleteJobRequest")
                && request["id"] == ack.job
                && request.as_object().is_some_and(|o| o.len() == 2),
            "GPU completed operation does not prove this job deletion"
        );
        if let Some(header) = value["requestHeaders"].get("x-idempotency-key") {
            ensure!(
                header["values"] == json!([claim.key.to_string()]),
                "GPU deletion key changed"
            );
        }
        if self
            .resource(&format!("/ai/v1/jobs/{}", ack.job))?
            .is_some()
        {
            return Ok(false);
        }
        ensure!(
            !claim.inventory.is_empty()
                && claim.inventory.len() <= 16
                && claim
                    .inventory
                    .iter()
                    .all(|i| i.complete && !i.disks.is_empty()),
            "GPU cleanup has incomplete resource inventory"
        );
        let mut disks = HashSet::new();
        for inventory in &claim.inventory {
            identifier(&inventory.compute_id)?;
            if self
                .resource(&format!("/compute/v1/instances/{}", inventory.compute_id))?
                .is_some()
            {
                return Ok(false);
            }
            for disk in &inventory.disks {
                identifier(&disk.id)?;
                disks.insert(&disk.id);
            }
        }
        for disk in disks {
            if self
                .resource(&format!("/compute/v1/disks/{disk}"))?
                .is_some()
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
