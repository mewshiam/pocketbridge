pub mod audio_crypto;
pub mod framing;
pub mod noise;
pub mod pairing;

use thiserror::Error;

uniffi::setup_scaffolding!();

#[derive(Debug, Error, uniffi::Error)]
pub enum CoreError {
    #[error("message too large: {len} bytes (max {max})")]
    MessageTooLarge { len: u64, max: u64 },
    #[error("invalid ATT MTU: {mtu}")]
    InvalidMtu { mtu: u32 },
    #[error("malformed frame: {0}")]
    MalformedFrame(String),
    #[error("invalid pairing URI: {0}")]
    InvalidPairingUri(String),
    #[error("pairing verification failed: {0}")]
    PairingVerificationFailed(String),
    #[error("peer static public key does not match pinned key")]
    PeerKeyMismatch,
    #[error("cryptographic error: {0}")]
    Crypto(String),
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiPairingPayload {
    pub version: u8,
    pub bridge_static_pub: Vec<u8>,
    pub adv_id: String,
    pub nonce: Vec<u8>,
}

#[uniffi::export]
pub fn build_pairing_uri(
    bridge_static_pub: Vec<u8>,
    adv_id: String,
    nonce: Vec<u8>,
) -> Result<String, CoreError> {
    let pub_arr: [u8; 32] = bridge_static_pub
        .try_into()
        .map_err(|_| CoreError::InvalidPairingUri("bridge_static_pub must be 32 bytes".into()))?;
    let nonce_arr: [u8; 16] = nonce
        .try_into()
        .map_err(|_| CoreError::InvalidPairingUri("nonce must be 16 bytes".into()))?;
    let payload = pairing::PairingPayload {
        version: pairing::PAIRING_VERSION,
        bridge_static_pub: pub_arr,
        adv_id,
        nonce: nonce_arr,
    };
    payload.to_uri()
}

#[uniffi::export]
pub fn parse_pairing_uri(uri: String) -> Result<FfiPairingPayload, CoreError> {
    let parsed = pairing::PairingPayload::from_uri(&uri)?;
    Ok(FfiPairingPayload {
        version: parsed.version,
        bridge_static_pub: parsed.bridge_static_pub.to_vec(),
        adv_id: parsed.adv_id,
        nonce: parsed.nonce.to_vec(),
    })
}

#[uniffi::export]
pub fn chunk_gatt_message(
    msg_id: u16,
    payload: Vec<u8>,
    att_mtu: u32,
    is_handshake: bool,
) -> Result<Vec<Vec<u8>>, CoreError> {
    framing::chunk_message(msg_id, &payload, att_mtu as usize, is_handshake)
}
