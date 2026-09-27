//! Shared application identity used by the daemon and desktop client.

#[cfg(debug_assertions)]
pub const APP_NAME: &str = "Waku Debug";
#[cfg(not(debug_assertions))]
pub const APP_NAME: &str = "Waku";

#[cfg(debug_assertions)]
pub const APP_ID: &str = "sh.waku.dev";
#[cfg(not(debug_assertions))]
pub const APP_ID: &str = "sh.waku";

#[cfg(debug_assertions)]
pub const DATA_DIRECTORY_NAME: &str = "Waku Debug";
#[cfg(not(debug_assertions))]
pub const DATA_DIRECTORY_NAME: &str = "Waku";
/// Customer-facing identity; APP_NAME and APP_ID remain compatibility identifiers.
pub const PUBLIC_NAME: &str = "ion";
