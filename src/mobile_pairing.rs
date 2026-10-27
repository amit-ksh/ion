//! Native, offline producer for the versioned QR contract consumed by Ion mobile.
//! Run address discovery, encoding and rasterization on a background executor.

use std::net::{IpAddr, Ipv4Addr, UdpSocket};

use image::{Rgba, RgbaImage};
use qrcode::{Color, QrCode};

pub(crate) struct PairingQr {
    pub address: String,
    pub image: RgbaImage,
}

/// Prefer the private address selected by the OS route over a hostname which
/// Android might not resolve. No datagram is sent. The displayed hostname is
/// still available as a fallback for offline or unusual network configurations.
pub(crate) fn local_pairing_qr(hostname: &str, port: u16, token: &str) -> Result<PairingQr, ()> {
    let host = private_route_address()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| hostname.to_owned());
    let address = format!("ws://{host}:{port}");
    let image = encode_pairing_qr(hostname, &address, token)?;
    Ok(PairingQr { address, image })
}

fn private_route_address() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    // A numeric documentation address asks only for the OS's outbound route;
    // it avoids DNS, an external service and any credential-bearing request.
    socket.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if private_address(ip) => Some(ip),
        _ => None,
    }
}

fn private_address(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    ip.is_private() || ip.is_link_local() || (a == 100 && (64..=127).contains(&b))
}

fn pairing_payload(name: &str, address: &str, token: &str) -> Result<Vec<u8>, ()> {
    if name.encode_utf16().count() > 80
        || token.trim().is_empty()
        || token.len() > 4096
        || name.chars().any(char::is_control)
        || token.chars().any(char::is_control)
    {
        return Err(());
    }
    let payload = serde_json::to_vec(&serde_json::json!({
        "type": "ion-pairing", "version": 1,
        "name": name, "address": address, "token": token,
    }))
    .map_err(|_| ())?;
    if payload.len() > 8192 {
        return Err(());
    }
    Ok(payload)
}

pub(crate) fn encode_pairing_qr(name: &str, address: &str, token: &str) -> Result<RgbaImage, ()> {
    let code = QrCode::new(pairing_payload(name, address, token)?).map_err(|_| ())?;
    // Four quiet modules on each side, with integer-sized black/white modules.
    // Neutral pixels also have the BGRA byte order GPUI expects.
    const QUIET: usize = 4;
    const SCALE: usize = 8;
    let size = (code.width() + QUIET * 2) * SCALE;
    let image = RgbaImage::from_fn(size as u32, size as u32, |x, y| {
        let (x, y) = (x as usize / SCALE, y as usize / SCALE);
        let dark = x >= QUIET
            && y >= QUIET
            && x < code.width() + QUIET
            && y < code.width() + QUIET
            && code[(x - QUIET, y - QUIET)] == Color::Dark;
        if dark {
            Rgba([0, 0, 0, 255])
        } else {
            Rgba([255; 4])
        }
    });
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_matches_mobile_contract_and_preserves_escaped_credentials() {
        let bytes =
            pairing_payload("My desktop", "ws://192.168.1.5:34123", "fixture-\"&token").unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"type":"ion-pairing","version":1,"name":"My desktop","address":"ws://192.168.1.5:34123","token":"fixture-\"&token"})
        );
        assert!(pairing_payload("Desktop", "ws://192.168.1.5:34123", "").is_err());
        assert!(pairing_payload("Desktop", "ws://192.168.1.5:34123", "secret\n").is_err());
        assert!(pairing_payload(&"🙂".repeat(41), "ws://192.168.1.5:34123", "fixture").is_err());
    }

    #[test]
    fn route_candidates_are_private_lan_link_local_or_tailnet_addresses() {
        for ip in [
            "10.0.0.2",
            "172.16.0.2",
            "192.168.1.5",
            "169.254.1.2",
            "100.64.0.2",
        ] {
            assert!(private_address(ip.parse().unwrap()));
        }
        for ip in [
            "127.0.0.1",
            "0.0.0.0",
            "8.8.8.8",
            "100.63.0.2",
            "172.32.0.2",
        ] {
            assert!(!private_address(ip.parse().unwrap()));
        }
    }
}
