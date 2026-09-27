//! Product distribution policy. This does not mutate persisted user preferences.

use crate::model::ProviderKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DistributionProfile {
    NebiusHackathon,
    Full,
}

impl DistributionProfile {
    /// The full catalog is a development escape hatch, never a Settings option.
    pub fn current() -> Self {
        static PROFILE: std::sync::OnceLock<DistributionProfile> = std::sync::OnceLock::new();
        *PROFILE.get_or_init(|| {
            if cfg!(debug_assertions)
                && std::env::var_os("ION_DEV_FULL_PROVIDERS").is_some_and(|value| value == "1")
            {
                Self::Full
            } else {
                Self::NebiusHackathon
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
