//! L2CAP voice-pipeline crypto: HKDF key derivation and AEAD framing.
//!
//! Voice frames travel over a BLE L2CAP CoC channel instead of GATT. The
//! channel itself is unencrypted at the L2CAP layer, so PocketBridge enforces
//! its own AEAD (ChaCha20-Poly1305) with keys derived from the completed
//! Noise_XX handshake hash `h` (blueprint §5.5):
//!
//! - Keys: `HKDF-SHA256(ikm = h, salt = "", info = b"pocketbridge-audio-v1")`
//!   split into two 32-byte keys — one for sending, one for receiving — each
//!   side deriving them in opposite directions.
//! - Frame format: `[uint16 frame_len][uint32 seq_num][ciphertext + 16-byte tag]`.
//! - Nonce: 96-bit, composed of a fixed 8-byte channel prefix plus the
//!   big-endian 32-bit `seq_num`, so every sequence number maps to exactly
//!   one nonce per direction and replay is impossible without nonce reuse.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use hkdf::Hkdf;
use sha2::Sha256;

use crate::error::{PbError, PbResult};

/// HKDF `info` binding the derived keys to the audio pipeline (v1).
pub const AUDIO_INFO: &[u8] = b"pocketbridge-audio-v1";
/// Length of a raw ChaCha20-Poly1305 key.
pub const AUDIO_KEY_LEN: usize = 32;
/// ChaCha20-Poly1305 nonce length (96-bit).
pub const NONCE_LEN: usize = 12;
/// Poly1305 authentication tag length.
pub const TAG_LEN: usize = 16;
/// 8-byte channel prefix of the AEAD nonce (random per session).
pub const CHANNEL_PREFIX_LEN: usize = NONCE_LEN - 4;
/// L2CAP frame header: 2-byte length + 4-byte sequence number.
pub const FRAME_HEADER_LEN: usize = 6;
/// Hard cap on one Opus frame carried in an L2CAP frame.
pub const MAX_OPUS_FRAME_LEN: usize = 1024;
/// Maximum value of the 32-bit sequence counter (wraps around in tests only).
pub const MAX_SEQ: u32 = u32::MAX;

/// Which peer this key set belongs to; the two peers derive mirrored keys.
#[derive(uniffi::Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum AudioRole {
    /// The bridge (Android) — takes the first half of the HKDF output as its send key.
    Bridge,
    /// The client (iOS) — takes the second half of the HKDF output as its send key.
    Client,
}

/// The two direction-specific keys for one voice session.
#[derive(Clone, uniffi::Record)]
pub struct AudioKeys {
    /// Key used for encrypting frames we send.
    pub send_key: Vec<u8>,
    /// Key used for decrypting frames we receive.
    pub recv_key: Vec<u8>,
    /// Nonce prefix (8 bytes, big-endian) for frames we send.
    pub send_prefix: Vec<u8>,
    /// Nonce prefix (8 bytes, big-endian) for frames we receive.
    pub recv_prefix: Vec<u8>,
}

impl AudioKeys {
    /// Derives the directional keys from the Noise handshake hash `h`.
    ///
    /// Both peers expand the same OKM; the bridge sends with the first half,
    /// the client sends with the second half, so their send/recv keys mirror
    /// each other without any extra coordination.
    pub fn derive(
        handshake_hash: &[u8],
        role: AudioRole,
        send_prefix: [u8; CHANNEL_PREFIX_LEN],
        recv_prefix: [u8; CHANNEL_PREFIX_LEN],
    ) -> PbResult<Self> {
        if handshake_hash.len() < 32 {
            return Err(PbError::AudioCrypto {
                reason: format!(
                    "handshake hash must be at least 32 bytes, got {}",
                    handshake_hash.len()
                ),
            });
        }
        let hk: Hkdf<Sha256> = Hkdf::new(None, handshake_hash);
        let mut okm = [0u8; 2 * AUDIO_KEY_LEN];
        hk.expand(AUDIO_INFO, &mut okm)
            .map_err(|_| PbError::AudioCrypto {
                reason: "HKDF expand failed".into(),
            })?;
        // Bridge sends with OKM[0..32], client sends with OKM[32..64].
        let (send_range, recv_range) = match role {
            AudioRole::Bridge => (0..AUDIO_KEY_LEN, AUDIO_KEY_LEN..2 * AUDIO_KEY_LEN),
            AudioRole::Client => (AUDIO_KEY_LEN..2 * AUDIO_KEY_LEN, 0..AUDIO_KEY_LEN),
        };
        let mut send_key = [0u8; AUDIO_KEY_LEN];
        send_key.copy_from_slice(&okm[send_range]);
        let mut recv_key = [0u8; AUDIO_KEY_LEN];
        recv_key.copy_from_slice(&okm[recv_range]);
        Ok(AudioKeys {
            send_key: send_key.to_vec(),
            recv_key: recv_key.to_vec(),
            send_prefix: send_prefix.to_vec(),
            recv_prefix: recv_prefix.to_vec(),
        })
    }
}

/// Binds a symmetric key + nonce prefix for one direction of the voice path.
pub struct AudioDirection {
    cipher: ChaCha20Poly1305,
    prefix: [u8; CHANNEL_PREFIX_LEN],
}

impl AudioDirection {
    pub fn new(key: &[u8; AUDIO_KEY_LEN], prefix: [u8; CHANNEL_PREFIX_LEN]) -> Self {
        let mut key_bytes = [0u8; AUDIO_KEY_LEN];
        key_bytes.copy_from_slice(key);
        AudioDirection {
            cipher: ChaCha20Poly1305::new(Key::from_slice(&key_bytes)),
            prefix,
        }
    }

    fn nonce_for(&self, seq: u32) -> Nonce {
        let mut nonce = [0u8; NONCE_LEN];
        nonce[..CHANNEL_PREFIX_LEN].copy_from_slice(&self.prefix);
        nonce[CHANNEL_PREFIX_LEN..].copy_from_slice(&seq.to_be_bytes());
        Nonce::from(nonce)
    }

    /// Encrypts one Opus frame into the L2CAP wire format.
    pub fn seal_frame(&self, seq: u32, opus_frame: &[u8]) -> PbResult<Vec<u8>> {
        if opus_frame.len() > MAX_OPUS_FRAME_LEN {
            return Err(PbError::AudioCrypto {
                reason: format!(
                    "opus frame of {} bytes exceeds cap {MAX_OPUS_FRAME_LEN}",
                    opus_frame.len()
                ),
            });
        }
        let ciphertext = self
            .cipher
            .encrypt(&self.nonce_for(seq), Payload::from(opus_frame))
            .map_err(|_| PbError::DecryptFailed {
                reason: "audio seal failed".into(),
            })?;
        let mut frame = Vec::with_capacity(FRAME_HEADER_LEN + ciphertext.len());
        let frame_len = (FRAME_HEADER_LEN + ciphertext.len()) as u16;
        frame.extend_from_slice(&frame_len.to_be_bytes());
        frame.extend_from_slice(&seq.to_be_bytes());
        frame.extend_from_slice(&ciphertext);
        Ok(frame)
    }

    /// Decrypts one L2CAP wire frame, returning `(seq, opus_frame)`.
    pub fn open_frame(&self, wire: &[u8]) -> PbResult<(u32, Vec<u8>)> {
        if wire.len() < FRAME_HEADER_LEN + TAG_LEN {
            return Err(PbError::AudioCrypto {
                reason: format!(
                    "frame too short: {} bytes (minimum {})",
                    wire.len(),
                    FRAME_HEADER_LEN + TAG_LEN
                ),
            });
        }
        let frame_len = u16::from_be_bytes([wire[0], wire[1]]) as usize;
        if frame_len != wire.len() as u16 as usize || frame_len < FRAME_HEADER_LEN + TAG_LEN {
            return Err(PbError::AudioCrypto {
                reason: format!(
                    "frame_len {frame_len} does not match wire length {}",
                    wire.len()
                ),
            });
        }
        let seq = u32::from_be_bytes([wire[2], wire[3], wire[4], wire[5]]);
        let ciphertext = &wire[FRAME_HEADER_LEN..];
        let plaintext = self
            .cipher
            .decrypt(&self.nonce_for(seq), Payload::from(ciphertext))
            .map_err(|e| PbError::DecryptFailed {
                reason: format!("audio open failed at seq {seq}: {e}"),
            })?;
        Ok((seq, plaintext))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_hash() -> [u8; 32] {
        let mut h = [0u8; 32];
        for (i, b) in h.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(7).wrapping_add(11);
        }
        h
    }

    #[test]
    fn derive_produces_directional_keys() {
        let h = fake_hash();
        let bridge = AudioKeys::derive(&h, AudioRole::Bridge, [1u8; 8], [2u8; 8]).unwrap();
        let client = AudioKeys::derive(&h, AudioRole::Client, [2u8; 8], [1u8; 8]).unwrap();
        // Mirror prefixes: bridge's send key must equal client's recv key.
        assert_eq!(bridge.send_key, client.recv_key);
        assert_eq!(bridge.recv_key, client.send_key);
        assert_ne!(bridge.send_key, bridge.recv_key);
        // Deriving twice yields identical keys (deterministic HKDF).
        let again = AudioKeys::derive(&h, AudioRole::Bridge, [1u8; 8], [2u8; 8]).unwrap();
        assert_eq!(bridge.send_key, again.send_key);
    }

    fn key(k: &[u8]) -> [u8; AUDIO_KEY_LEN] {
        k.try_into().unwrap()
    }

    fn prefix(p: &[u8]) -> [u8; CHANNEL_PREFIX_LEN] {
        p.try_into().unwrap()
    }

    #[test]
    fn seal_open_roundtrip() {
        let h = fake_hash();
        let bridge = AudioKeys::derive(&h, AudioRole::Bridge, [9u8; 8], [8u8; 8]).unwrap();
        let client = AudioKeys::derive(&h, AudioRole::Client, [8u8; 8], [9u8; 8]).unwrap();
        // Bridge sends with its send key; client receives with its recv key.
        let sender = AudioDirection::new(&key(&bridge.send_key), prefix(&bridge.send_prefix));
        let receiver = AudioDirection::new(&key(&client.recv_key), prefix(&client.recv_prefix));

        let opus = b"\xCE\xF3\x11\x00fake-opus-payload";
        let wire = sender.seal_frame(7, opus).unwrap();
        let (seq, decoded) = receiver.open_frame(&wire).unwrap();
        assert_eq!(seq, 7);
        assert_eq!(decoded, opus);
    }

    #[test]
    fn multiple_frames_incremental_nonce() {
        let h = fake_hash();
        let bridge = AudioKeys::derive(&h, AudioRole::Bridge, [7u8; 8], [7u8; 8]).unwrap();
        let client = AudioKeys::derive(&h, AudioRole::Client, [7u8; 8], [7u8; 8]).unwrap();
        let sender = AudioDirection::new(&key(&bridge.send_key), prefix(&bridge.send_prefix));
        let receiver = AudioDirection::new(&key(&client.recv_key), prefix(&client.recv_prefix));
        for seq in [0u32, 1, 2, 51, 1024, u32::MAX] {
            let wire = sender.seal_frame(seq, b"tiny").unwrap();
            let (got_seq, got) = receiver.open_frame(&wire).unwrap();
            assert_eq!(got_seq, seq);
            assert_eq!(got, b"tiny");
        }
    }

    #[test]
    fn tampered_frame_rejected() {
        let h = fake_hash();
        let bridge = AudioKeys::derive(&h, AudioRole::Bridge, [5u8; 8], [6u8; 8]).unwrap();
        let client = AudioKeys::derive(&h, AudioRole::Client, [6u8; 8], [5u8; 8]).unwrap();
        let sender = AudioDirection::new(&key(&bridge.send_key), prefix(&bridge.send_prefix));
        let receiver = AudioDirection::new(&key(&client.recv_key), prefix(&client.recv_prefix));
        let mut wire = sender.seal_frame(3, b"payload").unwrap();
        let last = wire.len() - 1;
        wire[last] ^= 0x40;
        assert!(receiver.open_frame(&wire).is_err());
    }

    #[test]
    fn wrong_key_rejected() {
        let h = fake_hash();
        let bridge = AudioKeys::derive(&h, AudioRole::Bridge, [5u8; 8], [6u8; 8]).unwrap();
        let sender = AudioDirection::new(&key(&bridge.send_key), prefix(&bridge.send_prefix));
        // Receiver derived from a DIFFERENT handshake hash entirely.
        let mut other_hash = [0xFFu8; 32];
        other_hash[0] = 0x01;
        let other = AudioKeys::derive(&other_hash, AudioRole::Client, [6u8; 8], [5u8; 8]).unwrap();
        let wrong_receiver = AudioDirection::new(&key(&other.recv_key), prefix(&other.recv_prefix));
        let wire = sender.seal_frame(1, b"payload").unwrap();
        assert!(wrong_receiver.open_frame(&wire).is_err());
    }

    #[test]
    fn truncated_frame_rejected() {
        let h = fake_hash();
        let keys = AudioKeys::derive(&h, AudioRole::Bridge, [5u8; 8], [6u8; 8]).unwrap();
        let sender = AudioDirection::new(&key(&keys.send_key), prefix(&keys.send_prefix));
        let wire = sender.seal_frame(9, b"payload").unwrap();
        assert!(
            AudioDirection::new(&key(&keys.recv_key), prefix(&keys.recv_prefix))
                .open_frame(&wire[..wire.len() - 1])
                .is_err()
        );
    }

    #[test]
    fn oversize_opus_frame_rejected() {
        let h = fake_hash();
        let keys = AudioKeys::derive(&h, AudioRole::Bridge, [1u8; 8], [2u8; 8]).unwrap();
        let sender = AudioDirection::new(&key(&keys.send_key), prefix(&keys.send_prefix));
        let huge = vec![0u8; MAX_OPUS_FRAME_LEN + 1];
        assert!(sender.seal_frame(0, &huge).is_err());
    }

    #[test]
    fn short_handshake_hash_rejected() {
        assert!(AudioKeys::derive(&[0u8; 16], AudioRole::Bridge, [0u8; 8], [0u8; 8]).is_err());
    }
}
