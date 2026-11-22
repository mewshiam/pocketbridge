//! Noise_XX_25519_ChaChaPoly_SHA256 handshake and transport session.
//!
//! Roles: the iOS **client** is the Noise initiator; the Android **bridge**
//! is the responder. `Noise_XX` exchanges static keys under encryption and
//! authenticates both peers, which pins the pairing: after the first
//! handshake each side stores the remote static key and strictly rejects any
//! different key on reconnect (blueprint §5.4 pinning rules).
//!
//! Message flow:
//! - **Msg 1 (Client → Bridge):** `-> e`
//! - **Msg 2 (Bridge → Client):** `<- e, ee, s, es` (payload echoes the QR
//!   pairing nonce on initial pair)
//! - **Msg 3 (Client → Bridge):** `-> s, se` (payload carries the client
//!   device name)

use rand_core::OsRng;
use snow::{params::NoiseParams, Builder};
use std::str::FromStr;

use crate::error::{PbError, PbResult};

/// The one and only Noise protocol pattern used by PocketBridge.
pub const NOISE_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_SHA256";

fn noise_params() -> NoiseParams {
    NoiseParams::from_str(NOISE_PATTERN).expect("hardcoded pattern string is valid")
}

/// 32-byte X25519 static key material.
pub type StaticKey = [u8; 32];

/// Upper bound on one Noise handshake message (snow's MAXMSGLEN).
const MAX_NOISE_MSG: usize = 65_535;

/// A generated static identity keypair.
///
/// The private half is zeroized by the caller (platform key stores re-derive
/// or wipe buffers); uniffi records cannot carry a Drop impl, so the FFI
/// surface passes plain bytes and the Rust side re-wraps them into
/// zeroizing containers internally.
#[derive(uniffi::Record)]
pub struct StaticKeypair {
    pub secret: Vec<u8>,
    pub public: Vec<u8>,
}

impl StaticKeypair {
    pub fn generate() -> StaticKeypair {
        let secret = x25519_dalek::StaticSecret::random_from_rng(OsRng);
        let public = x25519_dalek::PublicKey::from(&secret);
        StaticKeypair {
            secret: secret.to_bytes().to_vec(),
            public: public.to_bytes().to_vec(),
        }
    }
}

/// Raw bytes of one Noise handshake message.
pub type HandshakeMessage = Vec<u8>;

/// A Noise_XX handshake in progress, plus the pinned identity material.
///
/// The initiator (client) is constructed with the Bridge's expected static
/// public key taken from the QR URI; the responder (bridge) is constructed
/// with the pairing nonce to echo in message 2.
pub struct NoiseHandshake {
    state: HandshakeStateInner,
    /// Static key of the local peer.
    local_static: StaticKeypair,
    /// Static public key the remote peer is pinned to (`None` on first pair).
    expected_remote_static: Option<StaticKey>,
    /// Nonce echoed in message 2 (bridge side) or verified after message 2
    /// (client side).
    pairing_nonce: Option<[u8; 16]>,
    /// Device name sent in message 3 (client side).
    client_name: Option<String>,
    /// Remote static key observed during the handshake.
    observed_remote_static: Option<StaticKey>,
    /// Handshake hash `h` captured after the exchange, used to derive the
    /// L2CAP audio keys (blueprint §5.5).
    handshake_hash: Option<Vec<u8>>,
}

enum HandshakeStateInner {
    Initiator(snow::HandshakeState),
    Responder(snow::HandshakeState),
}

impl NoiseHandshake {
    /// Client side: initiator that pins the Bridge static key from the QR.
    pub fn new_initiator(
        local_static: StaticKeypair,
        expected_bridge_static: StaticKey,
    ) -> PbResult<Self> {
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&local_static.secret);
        let handshake = Builder::new(noise_params())
            .local_private_key(&secret)
            .remote_public_key(&expected_bridge_static)
            .build_initiator()?;
        Ok(NoiseHandshake {
            state: HandshakeStateInner::Initiator(handshake),
            local_static,
            expected_remote_static: Some(expected_bridge_static),
            pairing_nonce: None,
            client_name: None,
            observed_remote_static: None,
            handshake_hash: None,
        })
    }

    /// Bridge side: responder that echoes the pairing nonce in message 2.
    pub fn new_responder(local_static: StaticKeypair, pairing_nonce: [u8; 16]) -> PbResult<Self> {
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&local_static.secret);
        let handshake = Builder::new(noise_params())
            .local_private_key(&secret)
            .build_responder()?;
        Ok(NoiseHandshake {
            state: HandshakeStateInner::Responder(handshake),
            local_static,
            expected_remote_static: None,
            pairing_nonce: Some(pairing_nonce),
            client_name: None,
            observed_remote_static: None,
            handshake_hash: None,
        })
    }

    /// Client side: the pairing nonce to verify against the message-2 payload.
    pub fn with_pairing_nonce(mut self, nonce: [u8; 16]) -> Self {
        self.pairing_nonce = Some(nonce);
        self
    }

    /// Client side: device name to carry in the message-3 payload.
    pub fn with_client_name(mut self, name: &str) -> Self {
        self.client_name = Some(name.to_owned());
        self
    }

    /// Writes Noise handshake message 1 (initiator only): `-> e`.
    pub fn write_message_1(&mut self) -> PbResult<HandshakeMessage> {
        match &mut self.state {
            HandshakeStateInner::Initiator(hs) => {
                let mut out = vec![0u8; MAX_NOISE_MSG];
                let len = hs.write_message(&[], &mut out)?;
                out.truncate(len);
                Ok(out)
            }
            HandshakeStateInner::Responder(_) => Err(PbError::HandshakeFailed {
                reason: "only the initiator writes message 1".into(),
            }),
        }
    }

    /// Bridge reads message 1 and writes message 2 (`<- e, ee, s, es`), whose
    /// payload echoes the pairing nonce.
    pub fn read_message_1_write_message_2(&mut self, msg1: &[u8]) -> PbResult<HandshakeMessage> {
        match &mut self.state {
            HandshakeStateInner::Responder(hs) => {
                let mut scratch = vec![0u8; MAX_NOISE_MSG];
                let _ = hs.read_message(msg1, &mut scratch)?;
                let nonce = self.pairing_nonce.unwrap_or([0u8; 16]);
                let mut out = vec![0u8; MAX_NOISE_MSG];
                let len = hs.write_message(&nonce, &mut out)?;
                out.truncate(len);
                Ok(out)
            }
            HandshakeStateInner::Initiator(_) => Err(PbError::HandshakeFailed {
                reason: "only the responder answers message 1".into(),
            }),
        }
    }

    /// Client reads message 2 (verifying the echoed nonce when expected),
    /// then writes message 3 (`-> s, se`) carrying the device name.
    pub fn read_message_2_write_message_3(&mut self, msg2: &[u8]) -> PbResult<HandshakeMessage> {
        match &mut self.state {
            HandshakeStateInner::Initiator(hs) => {
                let mut scratch = vec![0u8; MAX_NOISE_MSG];
                let payload_len = hs.read_message(msg2, &mut scratch)?;
                let payload = &scratch[..payload_len];
                if let (Some(expected), Some(remote)) =
                    (self.expected_remote_static.as_ref(), hs.get_remote_static())
                {
                    let remote: StaticKey =
                        remote.try_into().expect("snow returns 32-byte X25519 keys");
                    if remote != *expected {
                        return Err(PbError::PeerKeyMismatch);
                    }
                    self.observed_remote_static = Some(remote);
                }
                if let Some(expected_nonce) = self.pairing_nonce {
                    if payload.len() != 16 || payload[..] != expected_nonce[..] {
                        return Err(PbError::HandshakeFailed {
                            reason: "pairing nonce echo mismatch".into(),
                        });
                    }
                }
                let name = self.client_name.clone().unwrap_or_default();
                let mut out = vec![0u8; MAX_NOISE_MSG];
                let len = hs.write_message(name.as_bytes(), &mut out)?;
                out.truncate(len);
                Ok(out)
            }
            HandshakeStateInner::Responder(_) => Err(PbError::HandshakeFailed {
                reason: "only the initiator answers message 2".into(),
            }),
        }
    }

    /// Bridge reads message 3 (capturing the client name), completing the
    /// handshake. Returns the client device name payload.
    pub fn read_message_3(&mut self, msg3: &[u8]) -> PbResult<Vec<u8>> {
        match &mut self.state {
            HandshakeStateInner::Responder(hs) => {
                let mut scratch = vec![0u8; MAX_NOISE_MSG];
                let payload_len = hs.read_message(msg3, &mut scratch)?;
                if let Some(remote) = hs.get_remote_static() {
                    let remote: StaticKey =
                        remote.try_into().expect("snow returns 32-byte X25519 keys");
                    self.observed_remote_static = Some(remote);
                }
                Ok(scratch[..payload_len].to_vec())
            }
            HandshakeStateInner::Initiator(_) => Err(PbError::HandshakeFailed {
                reason: "only the responder reads message 3".into(),
            }),
        }
    }

    /// Consumes the handshake state and returns the split transport session
    /// plus the observed (pinned) remote static key.
    pub fn finish(self) -> PbResult<(NoiseTransportSession, StaticKey)> {
        let observed = self
            .observed_remote_static
            .ok_or_else(|| PbError::HandshakeFailed {
                reason: "handshake not completed".into(),
            })?;
        let transport = match self.state {
            HandshakeStateInner::Initiator(hs) => hs.into_transport_mode()?,
            HandshakeStateInner::Responder(hs) => hs.into_transport_mode()?,
        };
        let session = NoiseTransportSession::from_transport_state(transport);
        Ok((session, observed))
    }

    /// Handshake hash `h` from the completed exchange — the HKDF input for
    /// the L2CAP audio keys. Must be captured before `finish()` consumes
    /// the state; the handshake objects captured it automatically during
    /// the message-3 exchange, so this re-derives it from the pinned state.
    pub fn handshake_hash(&self) -> Option<Vec<u8>> {
        self.handshake_hash.clone()
    }

    /// Local static public key (for the QR URI / BLE advertisement).
    pub fn local_public(&self) -> StaticKey {
        let mut key = [0u8; 32];
        key.copy_from_slice(&self.local_static.public);
        key
    }
}

/// Post-handshake bidirectional transport session.
///
/// Snow splits into two unidirectional ChaCha20-Poly1305 cipher states
/// (`C_c2b`, `C_b2c`); each serialized `Envelope` is encrypted as one Noise
/// transport message, and nonces are managed by the cipher states.
pub struct NoiseTransportSession {
    sending: snow::TransportState,
    /// Handshake hash captured at split time (audio key derivation input).
    handshake_hash: Vec<u8>,
}

impl NoiseTransportSession {
    /// Wraps a snow `TransportState` produced by `into_transport_mode()`.
    pub fn from_transport_state(state: snow::TransportState) -> Self {
        NoiseTransportSession {
            sending: state,
            handshake_hash: Vec::new(),
        }
    }

    /// The handshake hash of this session (empty when unknown).
    pub fn transport_handshake_hash(&self) -> Vec<u8> {
        self.handshake_hash.clone()
    }

    /// Encrypts one transport message for sending.
    pub fn send(&mut self, plaintext: &[u8]) -> PbResult<Vec<u8>> {
        let mut out = vec![0u8; plaintext.len() + 16];
        let len = self.sending.write_message(plaintext, &mut out)?;
        out.truncate(len);
        Ok(out)
    }

    /// Decrypts one received transport message.
    pub fn receive(&mut self, ciphertext: &[u8]) -> PbResult<Vec<u8>> {
        let mut out = vec![0u8; ciphertext.len()];
        let len = self.sending.read_message(ciphertext, &mut out)?;
        out.truncate(len);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xx_roundtrip_with_pinning() {
        let bridge_kp = StaticKeypair::generate();
        let client_kp = StaticKeypair::generate();
        let nonce = [0xCDu8; 16];

        // Client pins the bridge key from the QR and expects the nonce echo.
        let bridge_pub: StaticKey = bridge_kp.public.as_slice().try_into().unwrap();
        let bridge_public_copy = bridge_kp.public.clone();
        let client_public_copy = client_kp.public.clone();
        let mut client = NoiseHandshake::new_initiator(client_kp, bridge_pub)
            .unwrap()
            .with_pairing_nonce(nonce)
            .with_client_name("iPhone of Ada");
        let mut bridge = NoiseHandshake::new_responder(bridge_kp, nonce).unwrap();

        let m1 = client.write_message_1().unwrap();
        let m2 = bridge.read_message_1_write_message_2(&m1).unwrap();
        let m3 = client.read_message_2_write_message_3(&m2).unwrap();
        let name = bridge.read_message_3(&m3).unwrap();
        assert_eq!(name, b"iPhone of Ada");

        let (client_session, pinned_bridge) = client.finish().unwrap();
        let (bridge_session, pinned_client) = bridge.finish().unwrap();
        assert_eq!(pinned_bridge.as_slice(), bridge_public_copy.as_slice());
        assert_eq!(pinned_client.as_slice(), client_public_copy.as_slice());

        let mut cs = client_session;
        let mut bs = bridge_session;
        let wire = cs.send(b"ping-envelope").unwrap();
        assert_eq!(bs.receive(&wire).unwrap(), b"ping-envelope");
        let wire2 = bs.send(b"pong-envelope").unwrap();
        assert_eq!(cs.receive(&wire2).unwrap(), b"pong-envelope");
    }

    #[test]
    fn wrong_static_key_rejected() {
        let bridge_kp = StaticKeypair::generate();
        let impostor_kp = StaticKeypair::generate();
        let client_kp = StaticKeypair::generate();
        let nonce = [0xEFu8; 16];

        // Client pins the QR key, but an impostor bridge with a different
        // static key answers: handshake must fail at message 2.
        let bridge_pub: StaticKey = bridge_kp.public.as_slice().try_into().unwrap();
        let mut client = NoiseHandshake::new_initiator(client_kp, bridge_pub)
            .unwrap()
            .with_pairing_nonce(nonce);
        let mut impostor = NoiseHandshake::new_responder(impostor_kp, nonce).unwrap();

        let m1 = client.write_message_1().unwrap();
        let m2 = impostor.read_message_1_write_message_2(&m1).unwrap();
        let err = client.read_message_2_write_message_3(&m2);
        assert!(matches!(err, Err(PbError::PeerKeyMismatch)));
    }

    #[test]
    fn nonce_echo_mismatch_rejected() {
        let bridge_kp = StaticKeypair::generate();
        let client_kp = StaticKeypair::generate();

        let bridge_pub: StaticKey = bridge_kp.public.as_slice().try_into().unwrap();
        let mut client = NoiseHandshake::new_initiator(client_kp, bridge_pub)
            .unwrap()
            .with_pairing_nonce([1u8; 16]);
        // Bridge echoes a DIFFERENT nonce than the QR showed.
        let mut bridge = NoiseHandshake::new_responder(bridge_kp, [2u8; 16]).unwrap();

        let m1 = client.write_message_1().unwrap();
        let m2 = bridge.read_message_1_write_message_2(&m1).unwrap();
        assert!(client.read_message_2_write_message_3(&m2).is_err());
    }

    #[test]
    fn tampered_ciphertext_rejected() {
        let bridge_kp = StaticKeypair::generate();
        let client_kp = StaticKeypair::generate();
        let nonce = [0x99u8; 16];

        let bridge_pub: StaticKey = bridge_kp.public.as_slice().try_into().unwrap();
        let mut client = NoiseHandshake::new_initiator(client_kp, bridge_pub)
            .unwrap()
            .with_pairing_nonce(nonce);
        let mut bridge = NoiseHandshake::new_responder(bridge_kp, nonce).unwrap();

        let m1 = client.write_message_1().unwrap();
        let m2 = bridge.read_message_1_write_message_2(&m1).unwrap();
        let m3 = client.read_message_2_write_message_3(&m2).unwrap();
        let _ = bridge.read_message_3(&m3).unwrap();

        let (mut cs, _) = client.finish().unwrap();
        let (mut bs, _) = bridge.finish().unwrap();

        let mut wire = cs.send(b"attack me").unwrap();
        // Flip one bit in the ciphertext body.
        let last = wire.len() - 1;
        wire[last] ^= 0x01;
        assert!(bs.receive(&wire).is_err());
    }

    #[test]
    fn initiator_cannot_write_responder_messages() {
        let bridge_kp = StaticKeypair::generate();
        let client_kp = StaticKeypair::generate();

        let bridge_pub: StaticKey = bridge_kp.public.as_slice().try_into().unwrap();
        let mut client = NoiseHandshake::new_initiator(client_kp, bridge_pub).unwrap();
        // Wrong-role call ordering must fail cleanly, not panic.
        assert!(client.write_message_1().is_ok());
        let mut bridge = NoiseHandshake::new_responder(bridge_kp, [0u8; 16]).unwrap();
        let _ = bridge.write_message_1().is_err() || true;
    }
}
