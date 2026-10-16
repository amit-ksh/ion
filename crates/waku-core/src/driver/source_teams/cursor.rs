//! Cursor native binding and hook transport. These components are staged for
//! the ACP/shared-worker integration; source tools are not enabled yet.
mod binding;
use anyhow::{Result, ensure};

pub(super) fn hook_command(port: u16) -> Result<String> {
    ensure!(port != 0, "Invalid Cursor hook transport port");
    Ok(include_str!("cursor/hook.ps1").replace("__ION_HOOK_PORT__", &port.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cursor_hook_command_keeps_jsonc_comment_markers_out_of_private_transport() {
        let command = hook_command(12345).unwrap();
        assert!(!command.contains("//"));
        assert!(!command.contains("/*"));
        assert!(!command.contains("__ION_HOOK_"));
        assert!(command.contains("'127.0.0.1', 12345"));
        assert!(command.contains("$IonHandler.UseProxy = $false"));
        assert!(command.contains("$IonHandler.AllowAutoRedirect = $false"));
        assert!(command.contains("exit 0"));
    }
    #[test]
    fn cursor_hook_command_keeps_transport_credentials_out_of_configuration() {
        assert!(hook_command(0).is_err());
        let command = hook_command(12345).unwrap();
        assert!(command.contains("GetEnvironmentVariable('ION_CURSOR_TEAM_HOOK_TOKEN')"));
        assert!(command.contains("'Bearer', $IonToken"));
        assert!(!command.contains("__ION_HOOK_TOKEN__"));
    }
}
