//! Private Linux service pairing; never persisted in a workspace or task DB.
use anyhow::{Result, bail};
use std::path::Path;

#[cfg(any(target_os = "linux", test))]
fn decode(bytes: &[u8], now: u64) -> Result<super::GatewayPairing> {
    if bytes.is_empty() || bytes.len() > 16 * 1024 {
        bail!("Gateway pairing file must contain at most 16 KiB of JSON");
    }
    let pairing: super::GatewayPairing = serde_json::from_slice(bytes)
        .map_err(|_| anyhow::anyhow!("Gateway pairing file has invalid JSON fields"))?;
    super::gateway_base_url(&pairing.base_url)?;
    let id = pairing.device_id.as_bytes();
    if id.len() != 36
        || !id.iter().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                *byte == b'-'
            } else {
                byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()
            }
        })
        || pairing.device_id == "00000000-0000-0000-0000-000000000000"
    {
        bail!("Gateway pairing file must contain the registered device ID");
    }
    if !(32..=256).contains(&pairing.token.len())
        || !pairing
            .token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        bail!("Gateway pairing file must contain a 32-256 character URL-safe device token");
    }
    if pairing.expires_at <= now {
        bail!("Gateway pairing file has expired; register this owner again");
    }
    Ok(pairing)
}

#[cfg(target_os = "linux")]
pub(super) fn read(path: &Path) -> Result<super::GatewayPairing> {
    use anyhow::Context as _;
    use std::{
        fs::OpenOptions,
        io::Read as _,
        os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
    };
    // Validate the opened descriptor, not a check-then-read path. NOFOLLOW
    // refuses a replaced symlink; NONBLOCK prevents FIFO/device stalls.
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| anyhow::anyhow!("Gateway pairing file is unavailable"))?;
    let metadata = file
        .metadata()
        .context("Could not inspect gateway pairing file")?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.permissions().mode() & 0o077 != 0
        || metadata.len() == 0
        || metadata.len() > 16 * 1024
    {
        bail!("Gateway pairing file must be a private regular file owned by the service user");
    }
    let mut bytes = Vec::new();
    file.take(16 * 1024 + 1)
        .read_to_end(&mut bytes)
        .context("Could not read gateway pairing file")?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis()
        .try_into()?;
    decode(&bytes, now)
}
#[cfg(not(target_os = "linux"))]
pub(super) fn read(_path: &Path) -> Result<super::GatewayPairing> {
    bail!(
        "ION_GATEWAY_PAIRING_FILE is supported only for Linux services; use the OS credential vault on this platform"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    fn value() -> serde_json::Value {
        serde_json::json!({"baseUrl":"http://127.0.0.1:8787/", "deviceId":"8e2c291f-9f17-4c94-b12e-3d9912ae31d7", "token":"a".repeat(43), "expiresAt":u64::MAX})
    }
    fn accepts(value: &serde_json::Value) -> bool {
        decode(&serde_json::to_vec(value).unwrap(), 100).is_ok()
    }
    #[test]
    fn pairing_file_validates_exact_schema_origin_identity_token_and_expiry() {
        assert!(accepts(&value()));
        for (key, bad) in [
            (
                "baseUrl",
                serde_json::json!("https://secret@gateway.example/"),
            ),
            ("baseUrl", serde_json::json!("http://gateway.example/")),
            ("baseUrl", serde_json::json!("https://gateway.example/path")),
            ("deviceId", serde_json::json!("../other-owner")),
            (
                "deviceId",
                serde_json::json!("00000000-0000-0000-0000-000000000000"),
            ),
            ("token", serde_json::json!("short")),
            ("token", serde_json::json!("x".repeat(257))),
            ("token", serde_json::json!(format!("{}\n", "x".repeat(43)))),
            ("expiresAt", serde_json::json!(100)),
        ] {
            let mut json = value();
            json[key] = bad;
            assert!(!accepts(&json), "accepted invalid {key}");
        }
        let mut json = value();
        json["extra"] = true.into();
        assert!(!accepts(&json));
        assert!(decode(&vec![b' '; 16 * 1024 + 1], 100).is_err());
        assert!(decode(b"", 100).is_err());
    }
    #[test]
    fn file_managed_pairing_cannot_be_silently_replaced_or_deleted() {
        let service = super::super::CredentialService {
            volatile: std::sync::Mutex::new(std::collections::HashMap::new()),
            gateway_pairing_file: std::sync::Mutex::new(Some("managed.json".into())),
        };
        use super::super::CredentialKind as K;
        assert!(service.set(K::GatewayPairingToken, "new".into()).is_err());
        assert!(service.delete(K::GatewayPairingToken).is_err());
        // Refuses before consuming an invite or contacting an HTTP endpoint.
        assert!(
            service
                .pair_gateway("http://127.0.0.1:1/", "invite", "owner")
                .is_err()
        );
        assert!(service.gateway_api().is_err()); // no vault fallback on missing file
    }
    #[test]
    fn pairing_comparison_includes_capability_rotation_without_exposing_tokens() {
        let make = |value| {
            let p = decode(&serde_json::to_vec(&value).unwrap(), 100).unwrap();
            super::super::GatewayApi {
                url: super::super::gateway_base_url(&p.base_url).unwrap(),
                device_id: p.device_id,
                token: p.token,
                managed: None,
                client: super::super::gateway_client().unwrap(),
                owner_sync: None,
            }
        };
        let first = make(value());
        let same = make(value());
        assert!(first.same_pairing(&same));
        let mut rotated = value();
        rotated["token"] = "b".repeat(43).into();
        assert!(!first.same_pairing(&make(rotated)));
        assert!(same.with_timeout(std::time::Duration::ZERO).is_err());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn descriptor_checks_refuse_public_symlink_fifo_hardlink_and_oversized_files() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        let dir = std::env::temp_dir().join(format!(
            "ion-gateway-pairing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let file = dir.join("pairing.json");
        std::fs::write(&file, serde_json::to_vec(&value()).unwrap()).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(read(&file).is_err());
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(read(&file).is_ok());
        let link = dir.join("link");
        symlink(&file, &link).unwrap();
        assert!(read(&link).is_err());
        let hard = dir.join("hard");
        std::fs::hard_link(&file, &hard).unwrap();
        assert!(read(&file).is_err());
        std::fs::remove_file(&hard).unwrap();
        let fifo = dir.join("fifo");
        let c = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        assert!(read(&fifo).is_err());
        let next = dir.join("next");
        let mut json = value();
        json["token"] = "b".repeat(43).into();
        std::fs::write(&next, serde_json::to_vec(&json).unwrap()).unwrap();
        std::fs::set_permissions(&next, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::rename(&next, &file).unwrap();
        assert_eq!(read(&file).unwrap().token, "b".repeat(43));
        std::fs::write(&file, vec![b' '; 16 * 1024 + 1]).unwrap();
        assert!(read(&file).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
