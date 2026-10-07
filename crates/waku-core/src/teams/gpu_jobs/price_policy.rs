//! Immutable owner expectations for a native job estimate, not a price lock.
use super::*;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GpuPricePolicy {
    pub version: u16,
    pub method: GpuEstimateMethod,
    pub max_hourly_micros: u64,
    pub startup_seconds: u32,
    pub cleanup_seconds: u32,
    pub extra_reservation_micros: u64,
    pub skus: Vec<GpuPriceSku>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum GpuEstimateMethod {
    NativeJobV1,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GpuPriceSku {
    pub id: String,
    pub quantity: String,
    pub covers: Vec<GpuPriceComponent>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum GpuPriceComponent {
    Gpu,
    Vcpu,
    MemoryGib,
    DiskBytes,
}
impl GpuPricePolicy {
    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(
            self.version == 1
                && self.method == GpuEstimateMethod::NativeJobV1
                && (1..=1_000_000_000_000).contains(&self.max_hourly_micros)
                && self.startup_seconds <= 86400
                && self.cleanup_seconds <= 86400
                && self.extra_reservation_micros <= 1_000_000_000_000
                && (1..=16).contains(&self.skus.len()),
            "GPU estimate approval is invalid"
        );
        let mut ids = std::collections::HashSet::new();
        let mut components = std::collections::HashSet::new();
        for sku in &self.skus {
            resource_id(&sku.id)?;
            crate::driver::team_gpu::price_quantity(&sku.quantity)?;
            ensure!(
                ids.insert(&sku.id) && (1..=4).contains(&sku.covers.len()),
                "GPU estimate SKU approval is duplicated or incomplete"
            );
            for component in &sku.covers {
                ensure!(
                    components.insert(*component),
                    "GPU estimate component approval is duplicated"
                );
            }
        }
        ensure!(
            components.len() == 4,
            "GPU estimate approval must cover GPU, vCPU, RAM and disk"
        );
        Ok(())
    }
}
