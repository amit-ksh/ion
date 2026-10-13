//! Trusted native calculator receipt. Estimates are provisional, not invoices or locks.
use super::*;
use crate::teams::gpu_jobs::GpuPricePolicy;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
mod decimal;
use decimal::Amount;
const PATH: &str = "/billing/v1/calculator/estimate-batch";
#[derive(Clone, Serialize)]
pub(crate) struct GpuPriceReceipt {
    version: u16,
    request_id: uuid::Uuid,
    body_sha256: String,
    budget_sha256: String,
    estimate_request_sha256: String,
    response_sha256: String,
    checked_at: u64,
    currency: String,
    policy: GpuPricePolicy,
    hourly_cost: String,
    skus: Vec<SkuWitness>,
    horizon_seconds: u64,
    estimated_micros: u64,
    required_micros: u64,
}
#[derive(Clone, Serialize)]
struct SkuWitness {
    id: String,
    quantity: String,
    cost: String,
}
fn hash(value: &impl Serialize) -> Result<String> {
    Ok(Sha256::digest(serde_json::to_vec(value)?)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
pub(crate) fn price_quantity(value: &str) -> Result<()> {
    ensure!(
        Amount::parse(value)? != Amount::ZERO,
        "GPU price quantity must be positive"
    );
    Ok(())
}
fn request(body: &Value, currency: &str) -> Result<Value> {
    let mut native = body.clone();
    native
        .as_object_mut()
        .context("GPU estimate request missing")?
        .insert(
            "@type".into(),
            json!("type.googleapis.com/nebius.ai.v1.CreateJobRequest"),
        );
    let request = json!({"resourceSpecs":[{"spec":native}],"offerTypes":["OFFER_TYPE_CONTRACT_PRICE"],
        "currency":currency.to_ascii_lowercase(),"filterAggregationUnit":{"filterAggregationUnitValues":["FILTER_AGGREGATION_UNIT_HOUR"]}});
    ensure!(
        serde_json::to_vec(&request)?.len() <= 128 * 1024,
        "GPU estimate request exceeds its bound"
    );
    Ok(request)
}
pub(super) fn approved_policy<'a>(
    launch: &Launch,
    budget: &'a GpuBudgetPolicy,
) -> Result<&'a GpuPricePolicy> {
    let resources = budget
        .resources
        .as_ref()
        .context("Approve an exact GPU project/network/resource policy before a new launch")?;
    resources.matches_body(&launch.body()?)?;
    resources.require_child_network()?;
    resources.require_child_runtime()?;
    resources.require_child_group()?;
    resources.require_child_storage()?;
    if let Some(volume) = &budget.result_volume {
        volume.require_retention()?;
    }
    let policy = budget.pricing.as_ref().context(
        "Approve the native job estimator and complete SKU pricing before a new GPU launch",
    )?;
    policy.validate()?;
    ensure!(
        matches!(budget.currency.as_str(), "USD" | "ILS"),
        "GPU calculator currency needs a verified native contract"
    );
    ensure!(
        launch.result_volume.is_none() || policy.extra_reservation_micros > 0,
        "Approve separate GPU result storage/retention spending before estimating launch"
    );
    Ok(policy)
}
fn hourly(value: &Value) -> Result<()> {
    ensure!(
        value["unit"] == "hour" && value["unitType"] == "UNIT_TYPE_TIME_BASED",
        "GPU estimate aggregation unit is unsupported or incomplete"
    );
    Ok(())
}
fn breakdown(value: &Value, policy: &GpuPricePolicy) -> Result<(Amount, String, Vec<SkuWitness>)> {
    let rows = value["skuCosts"]
        .as_array()
        .context("GPU estimate SKU costs missing")?;
    ensure!(
        rows.len() == policy.skus.len(),
        "GPU estimate SKU coverage changed"
    );
    let mut seen = HashSet::new();
    let mut total = Amount::ZERO;
    let mut sanitized = Vec::new();
    for row in rows {
        let id = row["skuId"]
            .as_str()
            .context("GPU estimate SKU identity missing")?;
        ensure!(seen.insert(id), "GPU estimate SKU is duplicated");
        let expected = policy
            .skus
            .iter()
            .find(|s| s.id == id)
            .context("GPU estimate SKU is not approved")?;
        let quantity = row["quantity"]
            .as_str()
            .context("GPU estimate quantity must be an exact decimal string")?;
        ensure!(
            Amount::parse(quantity)? == Amount::parse(&expected.quantity)?,
            "GPU estimate quantity differs from the approved shape"
        );
        let cost = row["cost"]
            .as_str()
            .context("GPU estimate cost must be an exact decimal string")?;
        total = total.add(Amount::parse(cost)?)?;
        sanitized.push(SkuWitness {
            id: id.into(),
            quantity: quantity.into(),
            cost: cost.into(),
        });
    }
    let cost = value["cost"]
        .as_str()
        .context("GPU estimate total cost missing")?;
    ensure!(
        total == Amount::parse(cost)?,
        "GPU estimate SKU sum differs from its total"
    );
    Ok((total, cost.into(), sanitized))
}
fn general(value: &Value) -> Result<&Value> {
    ensure!(
        value["general"].is_object()
            && ["fixedInstance", "autoscale", "range"]
                .iter()
                .all(|k| value.get(*k).is_none_or(Value::is_null)),
        "GPU estimate cost type is unsupported or ambiguous"
    );
    Ok(&value["general"]["total"])
}
fn verify(
    value: &Value,
    body: &Value,
    budget: &GpuBudgetPolicy,
    policy: &GpuPricePolicy,
) -> Result<(String, Vec<SkuWitness>, u64, u64, u64)> {
    ensure!(
        value["currency"].as_str() == Some(budget.currency.to_ascii_lowercase().as_str()),
        "GPU estimate currency changed"
    );
    let resources = value["resourceCosts"]
        .as_array()
        .context("GPU estimate resource costs missing")?;
    ensure!(
        resources.len() == 1,
        "GPU estimate resource coverage is incomplete or ambiguous"
    );
    let resource = &resources[0];
    let metadata = &resource["metadata"];
    ensure!(
        metadata["parentId"] == body["metadata"]["parentId"]
            && metadata["name"] == body["metadata"]["name"]
            && resource
                .get("overrideMetadata")
                .is_none_or(|v| v.is_null() || v == metadata),
        "GPU estimate project/name/override identity changed"
    );
    hourly(&resource["aggregationUnit"])?;
    let (rate, cost, skus) = breakdown(general(resource)?, policy)?;
    ensure!(
        rate != Amount::ZERO && rate.micros()? <= policy.max_hourly_micros,
        "GPU estimate exceeds the approved hourly rate or has no price"
    );
    let totals = value["totalCosts"]
        .as_array()
        .context("GPU estimate aggregate costs missing")?;
    ensure!(
        totals.len() == 1,
        "GPU estimate aggregate units are ambiguous"
    );
    hourly(&totals[0]["aggregationUnit"])?;
    let (total, _, aggregate) = breakdown(general(&totals[0])?, policy)?;
    ensure!(
        rate == total
            && skus.iter().all(|s| aggregate.iter().any(
                |a| a.id == s.id && Amount::parse(&a.cost).ok() == Amount::parse(&s.cost).ok()
            )),
        "GPU estimate aggregate does not match resource SKU costs"
    );
    let timeout = body["spec"]["timeout"]
        .as_str()
        .and_then(|v| v.strip_suffix('s'))
        .and_then(|v| v.parse::<u64>().ok())
        .context("GPU price horizon missing")?;
    let horizon = timeout
        .checked_add(u64::from(policy.startup_seconds))
        .and_then(|n| n.checked_add(u64::from(policy.cleanup_seconds)))
        .context("GPU price horizon overflow")?;
    let estimate = rate.ratio_micros(horizon, 3600)?;
    let required = estimate
        .checked_add(policy.extra_reservation_micros)
        .context("GPU price reservation overflow")?;
    ensure!(
        required <= budget.reservation_micros,
        "GPU estimate plus lifecycle/storage allowance exceeds the approved reservation"
    );
    Ok((cost, skus, horizon, estimate, required))
}
impl Client {
    pub(super) fn verify_price(
        &self,
        launch: &Launch,
        budget: &GpuBudgetPolicy,
    ) -> Result<GpuPriceReceipt> {
        let policy = approved_policy(launch, budget)?;
        ensure!(
            self.project == launch.project,
            "GPU pricing owner project changed"
        );
        let body = launch.body()?;
        let input = request(&body, &budget.currency)?;
        // This POST only queries the calculator. Unsupported native Job Any stays
        // an error; never substitute a Compute shape or issue a creation request.
        let output = Self::response(
            self.request(reqwest::Method::POST, PATH)
                .json(&input)
                .send()?,
        )?;
        let (hourly_cost, skus, horizon_seconds, estimated_micros, required_micros) =
            verify(&output, &body, budget, policy)?;
        Ok(GpuPriceReceipt {
            version: 1,
            request_id: launch.request.request_id,
            body_sha256: hash(&body)?,
            budget_sha256: hash(budget)?,
            estimate_request_sha256: hash(&input)?,
            response_sha256: hash(&output)?,
            checked_at: crate::model::unix_time(),
            currency: budget.currency.clone(),
            policy: policy.clone(),
            hourly_cost,
            skus,
            horizon_seconds,
            estimated_micros,
            required_micros,
        })
    }
}
impl GpuPriceReceipt {
    pub(crate) fn validate(
        &self,
        body: &Value,
        job: &JobRequest,
        budget: &GpuBudgetPolicy,
        now: u64,
    ) -> Result<()> {
        let policy = budget
            .pricing
            .as_ref()
            .context("GPU price approval missing")?;
        policy.validate()?;
        ensure!(
            self.version == 1
                && self.request_id == job.request_id
                && self.body_sha256 == hash(body)?
                && self.budget_sha256 == hash(budget)?
                && self.policy == *policy
                && self.currency == budget.currency
                && self.estimate_request_sha256 == hash(&request(body, &budget.currency)?)?
                && self.checked_at <= now
                && now - self.checked_at <= 30
                && self.required_micros <= budget.reservation_micros,
            "GPU price receipt expired or differs from the original approved launch"
        );
        Ok(())
    }
    pub(crate) fn persisted(&self) -> Result<String> {
        let value = serde_json::to_string(self)?;
        ensure!(
            value.len() <= 32 * 1024,
            "GPU price witness exceeds its bound"
        );
        Ok(value)
    }
}
#[cfg(test)]
pub(super) mod tests;
