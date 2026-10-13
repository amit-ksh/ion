//! Resumable desktop onboarding preferences. Credentials stay in their existing
//! secure stores; this marker contains only the current screen and dismissal.

use serde::{Deserialize, Deserializer, Serialize};

pub(crate) const SETTINGS_KEY: &str = "ion_onboarding";
const VERSION: u32 = 1;

/// Screens in the desktop setup flow, in presentation order.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    #[default]
    Welcome,
    Models,
    Apps,
    Speech,
    Gateway,
}

impl Step {
    pub const ALL: [Self; 5] = [
        Self::Welcome,
        Self::Models,
        Self::Apps,
        Self::Speech,
        Self::Gateway,
    ];

    pub fn next(self) -> Option<Self> {
        match self {
            Self::Welcome => Some(Self::Models),
            Self::Models => Some(Self::Apps),
            Self::Apps => Some(Self::Speech),
            Self::Speech => Some(Self::Gateway),
            Self::Gateway => None,
        }
    }

    pub fn previous(self) -> Option<Self> {
        match self {
            Self::Welcome => None,
            Self::Models => Some(Self::Welcome),
            Self::Apps => Some(Self::Models),
            Self::Speech => Some(Self::Apps),
            Self::Gateway => Some(Self::Speech),
        }
    }
}

/// Nonsecret progress persisted through the existing daemon settings channel.
/// Unknown fields and versions are rejected so a future or malformed marker
/// cannot be mistaken for a completed setup.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Progress {
    #[serde(deserialize_with = "deserialize_version")]
    version: u32,
    pub step: Step,
    pub dismissed: bool,
}

impl Default for Progress {
    fn default() -> Self {
        Self::new(Step::Welcome)
    }
}

impl Progress {
    pub fn new(step: Step) -> Self {
        Self {
            version: VERSION,
            step,
            dismissed: false,
        }
    }

    pub fn version(self) -> u32 {
        self.version
    }

    pub fn with_dismissed(mut self, dismissed: bool) -> Self {
        self.dismissed = dismissed;
        self
    }
}

fn deserialize_version<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let version = u32::deserialize(deserializer)?;
    if version == VERSION {
        Ok(version)
    } else {
        Err(serde::de::Error::custom("unsupported onboarding version"))
    }
}

/// Existing installations stay in their workspace. A user who explicitly
/// began setup resumes it after a restart, while dismissal always wins.
pub fn should_show(
    progress: Option<&Progress>,
    has_existing_window: bool,
    has_started_tasks: bool,
) -> bool {
    progress.map_or(!has_existing_window && !has_started_tasks, |progress| {
        !progress.dismissed
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screens_have_a_finite_bidirectional_order() {
        for (index, step) in Step::ALL.into_iter().enumerate() {
            assert_eq!(step.next(), Step::ALL.get(index + 1).copied());
            assert_eq!(
                step.previous(),
                index
                    .checked_sub(1)
                    .and_then(|index| Step::ALL.get(index))
                    .copied()
            );
        }
    }

    #[test]
    fn absent_marker_only_opens_for_a_new_installation() {
        assert!(should_show(None, false, false));
        assert!(!should_show(None, true, false));
        assert!(!should_show(None, false, true));
        assert!(!should_show(None, true, true));
    }

    #[test]
    fn explicit_progress_resumes_after_window_and_history_exist() {
        for step in Step::ALL {
            let progress = Progress::new(step);
            for existing_window in [false, true] {
                for started_tasks in [false, true] {
                    assert!(should_show(Some(&progress), existing_window, started_tasks));
                    assert!(!should_show(
                        Some(&progress.with_dismissed(true)),
                        existing_window,
                        started_tasks
                    ));
                }
            }
        }
    }

    #[test]
    fn progress_serializes_only_the_nonsecret_screen_marker() {
        let progress = Progress::new(Step::Speech).with_dismissed(true);
        let encoded = serde_json::to_value(progress).unwrap();
        assert_eq!(
            encoded,
            serde_json::json!({"version": 1, "step": "speech", "dismissed": true})
        );
        assert_eq!(
            serde_json::from_value::<Progress>(encoded).unwrap(),
            progress
        );
        assert_eq!(progress.version(), 1);
    }

    #[test]
    fn malformed_or_future_markers_are_rejected() {
        for encoded in [
            serde_json::json!({"version": 2, "step": "apps", "dismissed": false}),
            serde_json::json!({"version": 0, "step": "apps", "dismissed": false}),
            serde_json::json!({"version": "1", "step": "apps", "dismissed": false}),
            serde_json::json!({"version": 1, "step": "future", "dismissed": false}),
            serde_json::json!({"version": 1, "step": "apps"}),
            serde_json::json!({"version": 1, "dismissed": false}),
            serde_json::json!({"step": "apps", "dismissed": false}),
            serde_json::json!({"version": 1, "step": "apps", "dismissed": false, "api_key": "unexpected"}),
            serde_json::Value::Null,
        ] {
            assert!(serde_json::from_value::<Progress>(encoded).is_err());
        }
    }
}
