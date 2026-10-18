//! Client-side Computer Use presentation data.

pub use waku_protocol::computer_use::*;

#[derive(Clone, Debug)]
pub struct PendingComputerApproval {
    pub request: ComputerToolRequest,
    pub target: ComputerTarget,
    pub sensitive: bool,
}

pub fn helper_display_name() -> String {
    format!("{} Computer Use", waku_protocol::identity::APP_NAME)
}
