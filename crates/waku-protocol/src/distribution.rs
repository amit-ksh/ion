//! Product distribution policy. This does not mutate persisted user preferences.

use crate::model::ProviderKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DistributionProfile {
    NebiusHackathon,
    Full,
}

impl DistributionProfile {
    /// ion ships the complete harness catalog. The limited profile is explicit
    /// packaging policy, never an implicit restriction on the default app.
    pub fn current() -> Self {
        static PROFILE: std::sync::OnceLock<DistributionProfile> = std::sync::OnceLock::new();
        *PROFILE.get_or_init(|| {
            if std::env::var_os("ION_DISTRIBUTION").is_some_and(|value| value == "nebius-only") {
                Self::NebiusHackathon
            } else {
                Self::Full
            }
        })
    }

    pub const fn allows_new_task(self, provider: ProviderKind) -> bool {
        matches!(self, Self::Full) || matches!(provider, ProviderKind::Nebius)
    }

    pub fn visible_providers(self) -> impl Iterator<Item = ProviderKind> {
        ProviderKind::ALL
            .into_iter()
            .filter(move |provider| self.allows_new_task(*provider))
    }
}

pub struct ProviderPolicy;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_ion_distribution_exposes_every_supported_harness() {
        assert_eq!(
            DistributionProfile::current().visible_providers().count(),
            ProviderKind::ALL.len()
        );
    }
}

impl ProviderPolicy {
    pub fn allows_new_task(provider: ProviderKind) -> bool {
        DistributionProfile::current().allows_new_task(provider)
    }

    pub fn require_new_task(provider: ProviderKind) -> anyhow::Result<()> {
        anyhow::ensure!(
            Self::allows_new_task(provider),
            "{} is unavailable in this ion distribution",
            provider.display_name()
        );
        Ok(())
    }
}
