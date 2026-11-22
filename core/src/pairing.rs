//! `pbridge://pair` QR URI builder and parser with 120-second expiry.
//!
//! ```text
//! pbridge://pair?v=1&pub=<base64url(S_pub)>&adv=<8-char-hex-id>&n=<base64url(16-byte-nonce)>
//! ```
//!
//! The Bridge displays the URI as a QR code; the Client scans it, verifies the
//! Bridge's Noise static key during the XX handshake (`pub`), disambiguates
//! nearby advertisers via the `adv` short ID in BLE Manufacturer Data, and
//! rejects pairing when the one-time nonce `n` is older than 120 seconds.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

use crate::error::{PbError, PbResult};

/// Protocol version carried in the `v` field.
pub const PAIR_URI_VERSION: u8 = 1;
/// Pairing nonce lifetime (blueprint §5.4).
pub const PAIRING_TTL: Duration = Duration::from_secs(120);
/// Length of the base64url-decoded static public key (`pub`).
pub const STATIC_PUBKEY_LEN: usize = 32;
/// Length of the base64url-decoded pairing nonce (`n`).
pub const NONCE_LEN: usize = 16;
/// Length of the hex advertiser short ID (`adv`).
pub const ADV_ID_LEN: usize = 8;

/// A parsed or freshly generated pairing payload.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PairingPayload {
    /// Bridge X25519 static public key.
    pub bridge_pub: Vec<u8>,
    /// 8-char uppercase hex ID advertised in BLE Manufacturer Data.
    pub adv_id: Vec<u8>,
    /// One-time 16-byte nonce.
    pub nonce: Vec<u8>,
    /// Seconds since epoch when the payload was created.
    pub created_at: u64,
}

impl PairingPayload {
    /// Builds a fresh payload from caller-supplied randomness.
    pub fn new(
        bridge_pub: [u8; STATIC_PUBKEY_LEN],
        adv_id: [u8; ADV_ID_LEN],
        nonce: [u8; NONCE_LEN],
        now_unix: u64,
    ) -> Self {
        PairingPayload {
            bridge_pub: bridge_pub.to_vec(),
            adv_id: adv_id.to_vec(),
            nonce: nonce.to_vec(),
            created_at: now_unix,
        }
    }

    /// Serializes to the QR URI format.
    pub fn to_uri(&self) -> String {
        format!(
            "pbridge://pair?v={}&pub={}&adv={}&n={}",
            PAIR_URI_VERSION,
            URL_SAFE_NO_PAD.encode(&self.bridge_pub),
            String::from_utf8(self.adv_id.clone()).unwrap_or_default(),
            URL_SAFE_NO_PAD.encode(&self.nonce),
        )
    }
}

/// Parses a `pbridge://pair` URI into a [`PairingPayload`].
///
/// Only `v=1` is understood; anything else is a forward-compat failure the
/// UI must surface ("update the other app").
pub fn parse_pair_uri(uri: &str) -> PbResult<PairingPayload> {
    let rest = uri
        .strip_prefix("pbridge://pair?")
        .ok_or_else(|| PbError::PairingInvalid {
            reason: "missing pbridge://pair? prefix".into(),
        })?;

    let mut bridge_pub: Option<[u8; 32]> = None;
    let mut adv_id: Option<[u8; 8]> = None;
    let mut nonce: Option<[u8; 16]> = None;

    for kv in rest.split('&') {
        let (key, value) = kv.split_once('=').ok_or_else(|| PbError::PairingInvalid {
            reason: format!("malformed query component {kv:?}"),
        })?;
        match key {
            "v" => {
                if value != "1" {
                    return Err(PbError::PairingInvalid {
                        reason: format!("unsupported pairing version {value:?} (expected 1)"),
                    });
                }
            }
            "pub" => {
                let decoded = URL_SAFE_NO_PAD.decode(value)?;
                if decoded.len() != STATIC_PUBKEY_LEN {
                    return Err(PbError::PairingInvalid {
                        reason: format!(
                            "pub must be {STATIC_PUBKEY_LEN} bytes, got {}",
                            decoded.len()
                        ),
                    });
                }
                let mut key = [0u8; STATIC_PUBKEY_LEN];
                key.copy_from_slice(&decoded);
                bridge_pub = Some(key);
            }
            "adv" => {
                let bytes = value.as_bytes();
                if bytes.len() != ADV_ID_LEN {
                    return Err(PbError::PairingInvalid {
                        reason: format!("adv must be {ADV_ID_LEN} chars, got {}", bytes.len()),
                    });
                }
                if !bytes.iter().all(u8::is_ascii_hexdigit) {
                    return Err(PbError::PairingInvalid {
                        reason: "adv must be hexadecimal characters".into(),
                    });
                }
                let mut id = [0u8; ADV_ID_LEN];
                id.copy_from_slice(bytes);
                adv_id = Some(id);
            }
            "n" => {
                let decoded = URL_SAFE_NO_PAD.decode(value)?;
                if decoded.len() != NONCE_LEN {
                    return Err(PbError::PairingInvalid {
                        reason: format!("n must be {NONCE_LEN} bytes, got {}", decoded.len()),
                    });
                }
                let mut buf = [0u8; NONCE_LEN];
                buf.copy_from_slice(&decoded);
                nonce = Some(buf);
            }
            other => {
                return Err(PbError::PairingInvalid {
                    reason: format!("unknown pairing field {other:?}"),
                });
            }
        }
    }

    let bridge_pub = bridge_pub.ok_or_else(|| PbError::PairingInvalid {
        reason: "missing pub".into(),
    })?;
    let adv_id = adv_id.ok_or_else(|| PbError::PairingInvalid {
        reason: "missing adv".into(),
    })?;
    let nonce = nonce.ok_or_else(|| PbError::PairingInvalid {
        reason: "missing n".into(),
    })?;

    // Timestamp is not carried in the URI; the receiver stamps it at scan
    // time. created_at therefore means "when the URI was parsed".
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    Ok(PairingPayload {
        bridge_pub: bridge_pub.to_vec(),
        adv_id: adv_id.to_vec(),
        nonce: nonce.to_vec(),
        created_at: now,
    })
}

/// Returns true when `payload` is still within the 120-second window.
pub fn is_pairing_fresh(payload: &PairingPayload, now_unix: u64) -> bool {
    let age = now_unix.saturating_sub(payload.created_at);
    Duration::from_secs(age) <= PAIRING_TTL
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_payload() -> PairingPayload {
        PairingPayload::new([0x11u8; 32], *b"1A2B3C4D", [0x22u8; 16], 1_700_000_000)
    }

    #[test]
    fn uri_roundtrip() {
        let payload = sample_payload();
        let uri = payload.to_uri();
        assert_eq!(uri, "pbridge://pair?v=1&pub=ERERERERERERERERERERERERERERERERERERERERERE&adv=1A2B3C4D&n=IiIiIiIiIiIiIiIiIiIiIg");
        let parsed = parse_pair_uri(&uri).unwrap();
        assert_eq!(parsed.bridge_pub, payload.bridge_pub);
        assert_eq!(parsed.adv_id, payload.adv_id);
        assert_eq!(parsed.nonce, payload.nonce);
    }

    #[test]
    fn rejects_wrong_scheme() {
        assert!(parse_pair_uri("https://example.com").is_err());
    }

    #[test]
    fn rejects_unknown_version() {
        let payload = sample_payload();
        let uri = payload.to_uri().replacen("v=1", "v=2", 1);
        assert!(parse_pair_uri(&uri).is_err());
    }

    #[test]
    fn rejects_short_pub() {
        let payload = sample_payload();
        let uri = payload
            .to_uri()
            .replace("ERERERERERERERERERERERERERERERERERERERERERE", "ExEx");
        assert!(parse_pair_uri(&uri).is_err());
    }

    #[test]
    fn rejects_bad_adv() {
        let payload = sample_payload();
        let uri = payload.to_uri().replace("adv=1A2B3C4D", "adv=1A2B3C");
        assert!(parse_pair_uri(&uri).is_err());
        let uri2 = payload.to_uri().replace("adv=1A2B3C4D", "adv=1A2B3C4Z");
        assert!(parse_pair_uri(&uri2).is_err());
    }

    #[test]
    fn expiry_window() {
        let payload = sample_payload();
        assert!(is_pairing_fresh(&payload, 1_700_000_000 + 60));
        assert!(is_pairing_fresh(&payload, 1_700_000_000 + 120));
        assert!(!is_pairing_fresh(&payload, 1_700_000_000 + 121));
        assert!(!is_pairing_fresh(&payload, 1_700_000_000 + 600));
    }
}
