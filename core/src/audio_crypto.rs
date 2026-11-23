use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::CoreError;

pub const AUDIO_HKDF_INFO: &[u8] = b"pocketbridge-audio-v1";
const AEAD_TAG_LEN: usize = 16;

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct AudioKeySet {
    pub client_to_bridge: [u8; 32],
    pub bridge_to_client: [u8; 32],
}

pub fn derive_audio_keys(handshake_hash: &[u8; 32]) -> Result<AudioKeySet, CoreError> {
    let hk = Hkdf::<Sha256>::new(None, handshake_hash);
    let mut okm = [0u8; 64];
    hk.expand(AUDIO_HKDF_INFO, &mut okm)
        .map_err(|_| CoreError::Crypto("HKDF expansion failed".into()))?;

    let mut client_to_bridge = [0u8; 32];
    let mut bridge_to_client = [0u8; 32];
    client_to_bridge.copy_from_slice(&okm[..32]);
    bridge_to_client.copy_from_slice(&okm[32..]);
    okm.zeroize();

    Ok(AudioKeySet {
        client_to_bridge,
        bridge_to_client,
    })
}

fn make_nonce(seq_num: u32) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[8..12].copy_from_slice(&seq_num.to_be_bytes());
    nonce
}

/// Seals an Opus frame into the L2CAP wire format:
/// `[ uint16 frame_len (BE) ][ uint32 seq_num (BE) ][ ciphertext || 16-byte tag ]`
pub fn seal_audio_frame(
    key: &[u8; 32],
    seq_num: u32,
    opus_frame: &[u8],
) -> Result<Vec<u8>, CoreError> {
    let cipher = ChaCha20Poly1305::new(key.into());
    let nonce_bytes = make_nonce(seq_num);
    let aad = seq_num.to_be_bytes();

    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload {
                msg: opus_frame,
                aad: &aad,
            },
        )
        .map_err(|_| CoreError::Crypto("audio AEAD encrypt failed".into()))?;

    let body_len = 4 + ciphertext.len();
    let frame_len = u16::try_from(body_len)
        .map_err(|_| CoreError::MessageTooLarge { len: body_len as u64, max: u16::MAX as u64 })?;

    let mut out = Vec::with_capacity(2 + body_len);
    out.extend_from_slice(&frame_len.to_be_bytes());
    out.extend_from_slice(&aad);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Opens a sealed L2CAP audio frame and returns `(seq_num, opus_frame)`.
pub fn open_audio_frame(key: &[u8; 32], wire_frame: &[u8]) -> Result<(u32, Vec<u8>), CoreError> {
    if wire_frame.len() < 2 + 4 + AEAD_TAG_LEN {
        return Err(CoreError::MalformedFrame("audio frame too short".into()));
    }

    let declared_len = u16::from_be_bytes([wire_frame[0], wire_frame[1]]) as usize;
    if wire_frame.len() != 2 + declared_len {
        return Err(CoreError::MalformedFrame("audio frame length mismatch".into()));
    }

    let seq_bytes = [wire_frame[2], wire_frame[3], wire_frame[4], wire_frame[5]];
    let seq_num = u32::from_be_bytes(seq_bytes);
    let ciphertext = &wire_frame[6..];

    let cipher = ChaCha20Poly1305::new(key.into());
    let nonce_bytes = make_nonce(seq_num);

    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload {
                msg: ciphertext,
                aad: &seq_bytes,
            },
        )
        .map_err(|_| CoreError::Crypto("audio AEAD authentication failed".into()))?;

    Ok((seq_num, plaintext))
}
