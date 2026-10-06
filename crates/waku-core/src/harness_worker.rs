//! Public entry point for the versioned remote harness worker.
pub fn execute(input: &[u8]) -> anyhow::Result<serde_json::Value> {
    crate::driver::team_worker::execute(input)
}
