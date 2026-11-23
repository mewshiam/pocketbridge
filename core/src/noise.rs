use snow::{Builder, HandshakeState, TransportState};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::CoreError;

pub const NOISE_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_SHA256";
const MAX_NOISE_MSG: usize = 65_535;

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct StaticKeypair {
    pub private: [u8; 32],
    #[zeroize(skip)]
    pub public: [u8; 32],
}

pub fn generate_static_keypair() -> Result<StaticKeypair, CoreError> {
    let params = NOISE_PATTERN
        .parse()
        .map_err(|e| CoreError::Crypto(format!("invalid noise pattern: {e}")))?;
    let kp = Builder::new(params)
        .generate_keypair()
        .map_err(|e| CoreError::Crypto(format!("keypair generation failed: {e}")))?;
    let private: [u8; 32] = kp
        .private
        .try_into()
        .map_err(|_| CoreError::Crypto("unexpected private key length".into()))?;
    let public: [u8; 32] = kp
        .public
        .try_into()
        .map_err(|_| CoreError::Crypto("unexpected public key length".into()))?;
    Ok(StaticKeypair { private, public })
}

pub struct NoiseHandshake {
    state: HandshakeState,
    expected_remote_pub: Option<[u8; 32]>,
    expected_nonce: Option<[u8; 16]>,
}

impl NoiseHandshake {
    pub fn new_initiator(
        local_private: &[u8; 32],
        expected_bridge_pub: [u8; 32],
        expected_nonce: Option<[u8; 16]>,
    ) -> Result<Self, CoreError> {
        let params = NOISE_PATTERN
            .parse()
            .map_err(|e| CoreError::Crypto(format!("invalid noise pattern: {e}")))?;
        let state = Builder::new(params)
            .local_private_key(local_private)
            .build_initiator()
            .map_err(|e| CoreError::Crypto(format!("initiator init failed: {e}")))?;
        Ok(Self {
            state,
            expected_remote_pub: Some(expected_bridge_pub),
            expected_nonce,
        })
    }

    pub fn new_responder(
        local_private: &[u8; 32],
        pinned_client_pub: Option<[u8; 32]>,
    ) -> Result<Self, CoreError> {
        let params = NOISE_PATTERN
            .parse()
            .map_err(|e| CoreError::Crypto(format!("invalid noise pattern: {e}")))?;
        let state = Builder::new(params)
            .local_private_key(local_private)
            .build_responder()
            .map_err(|e| CoreError::Crypto(format!("responder init failed: {e}")))?;
        Ok(Self {
            state,
            expected_remote_pub: pinned_client_pub,
            expected_nonce: None,
        })
    }

    pub fn write_message(&mut self, payload: &[u8]) -> Result<Vec<u8>, CoreError> {
        let mut buf = vec![0u8; MAX_NOISE_MSG];
        let len = self
            .state
            .write_message(payload, &mut buf)
            .map_err(|e| CoreError::Crypto(format!("handshake write failed: {e}")))?;
        buf.truncate(len);
        Ok(buf)
    }

    pub fn read_message(&mut self, message: &[u8]) -> Result<Vec<u8>, CoreError> {
        let mut buf = vec![0u8; MAX_NOISE_MSG];
        let len = self
            .state
            .read_message(message, &mut buf)
            .map_err(|e| CoreError::Crypto(format!("handshake read failed: {e}")))?;
        buf.truncate(len);

        if let Some(expected_nonce) = self.expected_nonce {
            if buf.as_slice() != expected_nonce.as_slice() {
                return Err(CoreError::PairingVerificationFailed(
                    "bridge nonce echo mismatch".into(),
                ));
            }
            self.expected_nonce = None;
        }

        if let (Some(expected_pub), Some(remote)) =
            (self.expected_remote_pub, self.state.get_remote_static())
        {
            if remote != expected_pub.as_slice() {
                return Err(CoreError::PeerKeyMismatch);
            }
        }

        Ok(buf)
    }

    pub fn is_finished(&self) -> bool {
        self.state.is_handshake_finished()
    }

    pub fn into_transport(self) -> Result<NoiseSession, CoreError> {
        if !self.state.is_handshake_finished() {
            return Err(CoreError::Crypto("handshake not finished".into()));
        }
        let remote_slice = self
            .state
            .get_remote_static()
            .ok_or_else(|| CoreError::Crypto("missing remote static key".into()))?;
        let remote_static_pub: [u8; 32] = remote_slice
            .try_into()
            .map_err(|_| CoreError::Crypto("unexpected remote key length".into()))?;

        if let Some(expected_pub) = self.expected_remote_pub {
            if remote_static_pub != expected_pub {
                return Err(CoreError::PeerKeyMismatch);
            }
        }

        let hash_slice = self.state.get_handshake_hash();
        let handshake_hash: [u8; 32] = hash_slice
            .try_into()
            .map_err(|_| CoreError::Crypto("unexpected handshake hash length".into()))?;

        let transport = self
            .state
            .into_transport_mode()
            .map_err(|e| CoreError::Crypto(format!("transport transition failed: {e}")))?;

        Ok(NoiseSession {
            transport,
            remote_static_pub,
            handshake_hash,
        })
    }
}

pub struct NoiseSession {
    transport: TransportState,
    remote_static_pub: [u8; 32],
    handshake_hash: [u8; 32],
}

impl NoiseSession {
    pub fn remote_static_pub(&self) -> [u8; 32] {
        self.remote_static_pub
    }

    pub fn handshake_hash(&self) -> [u8; 32] {
        self.handshake_hash
    }

    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, CoreError> {
        let mut buf = vec![0u8; plaintext.len() + 64];
        let len = self
            .transport
            .write_message(plaintext, &mut buf)
            .map_err(|e| CoreError::Crypto(format!("transport encrypt failed: {e}")))?;
        buf.truncate(len);
        Ok(buf)
    }

    pub fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>, CoreError> {
        let mut buf = vec![0u8; ciphertext.len()];
        let len = self
            .transport
            .read_message(ciphertext, &mut buf)
            .map_err(|e| CoreError::Crypto(format!("transport decrypt failed: {e}")))?;
        buf.truncate(len);
        Ok(buf)
    }
}
