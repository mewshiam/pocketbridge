//! UniFFI exports — the surface consumed by the Android (Kotlin) and iOS
//! (Swift) apps. Byte-identical crypto, framing, and pairing on both sides.
//!

pub mod audio_crypto;
pub mod error;
pub mod framing;
pub mod noise;
pub mod pairing;

pub use audio_crypto::{
    AudioDirection, AudioKeys, AudioRole, CHANNEL_PREFIX_LEN, FRAME_HEADER_LEN, MAX_OPUS_FRAME_LEN,
    TAG_LEN,
};
pub use error::{PbError, PbResult};
pub use framing::{
    chunk_handshake, chunk_message, parse_header, Reassembler, FLAG_END, FLAG_HANDSHAKE,
    FLAG_SINGLE, FLAG_START, MAX_MESSAGE_SIZE,
};
pub use noise::{NoiseHandshake, NoiseTransportSession, StaticKey, StaticKeypair};
pub use pairing::{is_pairing_fresh, parse_pair_uri, PairingPayload, PAIRING_TTL};

uniffi::setup_scaffolding!();

/// Parsed header of one framed chunk.
#[derive(uniffi::Record)]
pub struct ChunkHeader {
    pub flags: u8,
    pub message_id: u16,
    pub chunk_index: u16,
}

/// Result of one envelope exchange, as seen by the apps.
#[derive(uniffi::Record)]
pub struct ExchangeOutcome {
    pub message_id: u16,
    pub chunks_sent: u32,
    pub plaintext_len: u32,
}

/// A full pairing + handshake + transport round-trip helper used by the apps
/// during onboarding self-tests (and heavily by the integration tests).
#[derive(uniffi::Record)]
pub struct SelfTestReport {
    pub pairing_uri_valid: bool,
    pub handshake_complete: bool,
    pub peer_key_pinned: bool,
    pub transport_roundtrip_bytes: u32,
    pub audio_frames_ok: u32,
}

/// Generates a fresh static X25519 keypair (secret zeroized on drop).
#[uniffi::export]
fn generate_static_keypair() -> StaticKeypair {
    StaticKeypair::generate()
}

/// Builds the `pbridge://pair` QR URI for the current bridge identity.
#[uniffi::export]
fn build_pairing_uri(
    bridge_public: Vec<u8>,
    adv_id: String,
    nonce: Vec<u8>,
    now_unix: u64,
) -> Result<String, PbError> {
    if bridge_public.len() != pairing::STATIC_PUBKEY_LEN {
        return Err(PbError::InvalidArgument {
            reason: format!(
                "bridge_public must be {} bytes, got {}",
                pairing::STATIC_PUBKEY_LEN,
                bridge_public.len()
            ),
        });
    }
    let adv_bytes = adv_id.as_bytes();
    if adv_bytes.len() != pairing::ADV_ID_LEN || !adv_bytes.iter().all(u8::is_ascii_hexdigit) {
        return Err(PbError::InvalidArgument {
            reason: format!("adv_id must be {} hex chars", pairing::ADV_ID_LEN),
        });
    }
    if nonce.len() != pairing::NONCE_LEN {
        return Err(PbError::InvalidArgument {
            reason: format!(
                "nonce must be {} bytes, got {}",
                pairing::NONCE_LEN,
                nonce.len()
            ),
        });
    }
    let mut pubkey = [0u8; pairing::STATIC_PUBKEY_LEN];
    pubkey.copy_from_slice(&bridge_public);
    let mut adv = [0u8; pairing::ADV_ID_LEN];
    adv.copy_from_slice(adv_bytes);
    let mut n = [0u8; pairing::NONCE_LEN];
    n.copy_from_slice(&nonce);
    let payload = PairingPayload::new(pubkey, adv, n, now_unix);
    Ok(payload.to_uri())
}

/// Parses and validates a scanned `pbridge://pair` URI.
#[uniffi::export]
fn parse_pairing_uri(uri: String) -> Result<PairingPayload, PbError> {
    parse_pair_uri(&uri)
}

/// Checks whether a scanned pairing URI is still inside the 120 s window.
#[uniffi::export]
fn pairing_uri_fresh(payload: &PairingPayload, now_unix: u64) -> bool {
    is_pairing_fresh(payload, now_unix)
}

/// Splits an encrypted (or plaintext handshake) message into framed GATT
/// chunks for `CTRL_TX` / `CTRL_RX`.
#[uniffi::export]
fn chunk_control_message(
    message_id: u16,
    data: Vec<u8>,
    mtu: u32,
    handshake: bool,
) -> Vec<Vec<u8>> {
    if handshake {
        chunk_handshake(message_id, &data)
    } else {
        chunk_message(message_id, &data, mtu as usize)
    }
}

/// Reads the 6-byte framing header of one chunk.
#[uniffi::export]
fn read_chunk_header(frame: Vec<u8>) -> Result<ChunkHeader, PbError> {
    let (flags, message_id, chunk_index) = parse_header(&frame)?;
    Ok(ChunkHeader {
        flags,
        message_id,
        chunk_index,
    })
}

/// Streaming control-message reassembler bound to the FFI surface.
///
/// uniffi objects live behind `Arc`, so the inner state uses a lock for
/// interior mutability.
#[derive(uniffi::Object)]
pub struct ControlReassembler {
    inner: std::sync::Mutex<Reassembler>,
}

#[uniffi::export]
impl ControlReassembler {
    #[uniffi::constructor]
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        ControlReassembler {
            inner: std::sync::Mutex::new(Reassembler::new()),
        }
    }

    /// Feeds one framed chunk; returns the completed message bytes if this
    /// chunk finished it.
    pub fn feed(&self, frame: Vec<u8>) -> Result<Option<Vec<u8>>, PbError> {
        self.inner.lock().expect("reassembler lock").feed(&frame)
    }

    /// Drops buffers older than the reassembly timeout; returns evictions.
    pub fn evict_stale(&self) -> u32 {
        self.inner.lock().expect("reassembler lock").evict_stale() as u32
    }

    /// Number of partially received messages currently held.
    pub fn in_flight(&self) -> u32 {
        self.inner.lock().expect("reassembler lock").in_flight() as u32
    }
}

/// Derives the directional L2CAP audio keys from a Noise handshake hash.
#[uniffi::export]
fn derive_audio_keys(
    handshake_hash: Vec<u8>,
    role: AudioRole,
    send_prefix: Vec<u8>,
    recv_prefix: Vec<u8>,
) -> Result<AudioKeys, PbError> {
    if send_prefix.len() != CHANNEL_PREFIX_LEN || recv_prefix.len() != CHANNEL_PREFIX_LEN {
        return Err(PbError::InvalidArgument {
            reason: format!(
                "nonce prefixes must be {CHANNEL_PREFIX_LEN} bytes each, got {}/{}",
                send_prefix.len(),
                recv_prefix.len()
            ),
        });
    }
    let mut send = [0u8; CHANNEL_PREFIX_LEN];
    send.copy_from_slice(&send_prefix);
    let mut recv = [0u8; CHANNEL_PREFIX_LEN];
    recv.copy_from_slice(&recv_prefix);
    AudioKeys::derive(&handshake_hash, role, send, recv)
}

/// Encrypts one Opus frame into the L2CAP wire format.
#[uniffi::export]
fn seal_audio_frame(
    send_key: Vec<u8>,
    send_prefix: Vec<u8>,
    seq: u32,
    opus_frame: Vec<u8>,
) -> Result<Vec<u8>, PbError> {
    if send_key.len() != audio_crypto::AUDIO_KEY_LEN {
        return Err(PbError::InvalidArgument {
            reason: format!("send_key must be 32 bytes, got {}", send_key.len()),
        });
    }
    let mut key = [0u8; audio_crypto::AUDIO_KEY_LEN];
    key.copy_from_slice(&send_key);
    let mut prefix = [0u8; CHANNEL_PREFIX_LEN];
    prefix.copy_from_slice(&send_prefix);
    AudioDirection::new(&key, prefix).seal_frame(seq, &opus_frame)
}

/// Decrypts one L2CAP wire frame; returns `(seq, opus_frame)` flattened as
/// `[seq (4 bytes BE)] ++ opus_frame`.
#[uniffi::export]
fn open_audio_frame(
    recv_key: Vec<u8>,
    recv_prefix: Vec<u8>,
    wire: Vec<u8>,
) -> Result<Vec<u8>, PbError> {
    if recv_key.len() != audio_crypto::AUDIO_KEY_LEN {
        return Err(PbError::InvalidArgument {
            reason: format!("recv_key must be 32 bytes, got {}", recv_key.len()),
        });
    }
    let mut key = [0u8; audio_crypto::AUDIO_KEY_LEN];
    key.copy_from_slice(&recv_key);
    let mut prefix = [0u8; CHANNEL_PREFIX_LEN];
    prefix.copy_from_slice(&recv_prefix);
    let (seq, opus) = AudioDirection::new(&key, prefix).open_frame(&wire)?;
    let mut out = Vec::with_capacity(4 + opus.len());
    out.extend_from_slice(&seq.to_be_bytes());
    out.extend_from_slice(&opus);
    Ok(out)
}

/// Runs the full QR pairing → Noise_XX → chunked transport → L2CAP audio
/// self-test used by the onboarding screens on both platforms.
#[uniffi::export]
fn run_pairing_self_test(now_unix: u64) -> Result<SelfTestReport, PbError> {
    use rand_core::RngCore;

    // 1. Bridge identity + pairing URI.
    let bridge_kp = StaticKeypair::generate();
    let client_kp = StaticKeypair::generate();
    let mut nonce = [0u8; pairing::NONCE_LEN];
    rand_core::OsRng.fill_bytes(&mut nonce);
    let uri = build_pairing_uri(
        bridge_kp.public.to_vec(),
        "A1B2C3D4".into(),
        nonce.to_vec(),
        now_unix,
    )?;
    let payload = parse_pair_uri(&uri)?;
    let fresh = is_pairing_fresh(&payload, now_unix);

    // 2. Noise_XX handshake with pinning and nonce echo.
    let mut bridge_pub_arr = [0u8; 32];
    bridge_pub_arr.copy_from_slice(&payload.bridge_pub);
    let mut nonce_arr = [0u8; 16];
    nonce_arr.copy_from_slice(&payload.nonce);
    let mut client = NoiseHandshake::new_initiator(client_kp, bridge_pub_arr)?
        .with_pairing_nonce(nonce_arr)
        .with_client_name("self-test");
    let mut bridge = NoiseHandshake::new_responder(bridge_kp, nonce_arr)?;
    let m1 = client.write_message_1()?;
    let m2 = bridge.read_message_1_write_message_2(&m1)?;
    let m3 = client.read_message_2_write_message_3(&m2)?;
    let _name = bridge.read_message_3(&m3)?;
    let handshake_complete = true;

    // 3. Split transport + chunked ping round-trip.
    let (mut client_session, pinned) = client.finish()?;
    let (mut bridge_session, _pinned_client) = bridge.finish()?;
    let peer_key_pinned = pinned.as_slice() == payload.bridge_pub.as_slice();

    let ping = b"ping-envelope-0001";
    let wire = client_session.send(ping)?;
    let decoded = bridge_session.receive(&wire)?;
    let roundtrip_ok = decoded == ping;

    let mut re = Reassembler::new();
    let mut reassembled_ok = false;
    for frame in chunk_message(5, &wire, 517) {
        if let Some(msg) = re.feed(&frame)? {
            reassembled_ok = msg == wire;
        }
    }

    // 4. L2CAP audio frame round-trip.
    let hash: [u8; 32] = *include_bytes!("self_test_hash.bin");
    let keys = AudioKeys::derive(&hash, AudioRole::Bridge, [3u8; 8], [4u8; 8])?;
    let sender = AudioDirection::new(
        keys.send_key.as_slice().try_into().unwrap(),
        keys.send_prefix.as_slice().try_into().unwrap(),
    );
    let receiver = AudioDirection::new(
        keys.recv_key.as_slice().try_into().unwrap(),
        keys.recv_prefix.as_slice().try_into().unwrap(),
    );
    let mut audio_ok = 0u32;
    for (seq, opus) in [(0u32, b"opus-a"), (1, b"opus-b"), (2, b"opus-c")] {
        let frame = sender.seal_frame(seq, opus)?;
        if let Ok((got_seq, got)) = receiver.open_frame(&frame) {
            if got_seq == seq && got == opus {
                audio_ok += 1;
            }
        }
    }

    Ok(SelfTestReport {
        pairing_uri_valid: fresh,
        handshake_complete,
        peer_key_pinned,
        transport_roundtrip_bytes: if roundtrip_ok && reassembled_ok {
            ping.len() as u32
        } else {
            0
        },
        audio_frames_ok: audio_ok,
    })
}
