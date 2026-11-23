use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

use crate::CoreError;

pub const PAIRING_SCHEME_PREFIX: &str = "pbridge://pair?";
pub const PAIRING_VERSION: u8 = 1;
pub const PAIRING_TTL_SECS: u64 = 120;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingPayload {
    pub version: u8,
    pub bridge_static_pub: [u8; 32],
    pub adv_id: String,
    pub nonce: [u8; 16],
}

impl PairingPayload {
    pub fn to_uri(&self) -> Result<String, CoreError> {
        validate_adv_id(&self.adv_id)?;
        let pub_b64 = URL_SAFE_NO_PAD.encode(self.bridge_static_pub);
        let nonce_b64 = URL_SAFE_NO_PAD.encode(self.nonce);
        Ok(format!(
            "{PAIRING_SCHEME_PREFIX}v={}&pub={pub_b64}&adv={}&n={nonce_b64}",
            self.version, self.adv_id
        ))
    }

    pub fn from_uri(uri: &str) -> Result<Self, CoreError> {
        let query = uri
            .strip_prefix(PAIRING_SCHEME_PREFIX)
            .ok_or_else(|| CoreError::InvalidPairingUri("missing pbridge://pair? prefix".into()))?;

        let mut version: Option<u8> = None;
        let mut pub_bytes: Option<[u8; 32]> = None;
        let mut adv_id: Option<String> = None;
        let mut nonce_bytes: Option<[u8; 16]> = None;

        for pair in query.split('&') {
            let (k, v) = pair
                .split_once('=')
                .ok_or_else(|| CoreError::InvalidPairingUri("malformed query parameter".into()))?;
            match k {
                "v" => {
                    let parsed = v
                        .parse::<u8>()
                        .map_err(|_| CoreError::InvalidPairingUri("invalid version".into()))?;
                    if parsed != PAIRING_VERSION {
                        return Err(CoreError::InvalidPairingUri(format!(
                            "unsupported pairing version {parsed}"
                        )));
                    }
                    version = Some(parsed);
                }
                "pub" => {
                    let raw = URL_SAFE_NO_PAD
                        .decode(v)
                        .map_err(|_| CoreError::InvalidPairingUri("invalid base64url pub".into()))?;
                    let arr: [u8; 32] = raw.try_into().map_err(|_| {
                        CoreError::InvalidPairingUri("pub key must be 32 bytes".into())
                    })?;
                    pub_bytes = Some(arr);
                }
                "adv" => {
                    validate_adv_id(v)?;
                    adv_id = Some(v.to_ascii_lowercase());
                }
                "n" => {
                    let raw = URL_SAFE_NO_PAD
                        .decode(v)
                        .map_err(|_| CoreError::InvalidPairingUri("invalid base64url nonce".into()))?;
                    let arr: [u8; 16] = raw.try_into().map_err(|_| {
                        CoreError::InvalidPairingUri("nonce must be 16 bytes".into())
                    })?;
                    nonce_bytes = Some(arr);
                }
                _ => {}
            }
        }

        Ok(Self {
            version: version.ok_or_else(|| CoreError::InvalidPairingUri("missing v".into()))?,
            bridge_static_pub: pub_bytes
                .ok_or_else(|| CoreError::InvalidPairingUri("missing pub".into()))?,
            adv_id: adv_id.ok_or_else(|| CoreError::InvalidPairingUri("missing adv".into()))?,
            nonce: nonce_bytes.ok_or_else(|| CoreError::InvalidPairingUri("missing n".into()))?,
        })
    }
}

pub fn is_pairing_expired(created_at_unix_secs: u64, now_unix_secs: u64) -> bool {
    now_unix_secs.saturating_sub(created_at_unix_secs) > PAIRING_TTL_SECS
}

fn validate_adv_id(adv: &str) -> Result<(), CoreError> {
    if adv.len() != 8 || !adv.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(CoreError::InvalidPairingUri(
            "adv must be an 8-character hex identifier".into(),
        ));
    }
    Ok(())
}
