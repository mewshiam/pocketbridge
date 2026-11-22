use thiserror::Error;

/// Errors surfaced across the PocketBridge UniFFI surface.
///
/// Every variant maps onto a recoverable situation the platform apps can render
/// to the user (pairing expired, peer key mismatch, malformed chunk stream…).
#[derive(Debug, Error, uniffi::Error)]
pub enum PbError {
    /// A QR pairing URI could not be parsed or has expired (> 120 s).
    #[error("pairing URI invalid or expired: {reason}")]
    PairingInvalid { reason: String },

    /// The Noise_XX handshake failed (bad message, wrong key, tampering).
    #[error("noise handshake failed: {reason}")]
    HandshakeFailed { reason: String },

    /// The remote static key does not match the pinned peer key.
    #[error("peer static key mismatch: expected pin, got different key")]
    PeerKeyMismatch,

    /// A transport operation ran before the session was established.
    #[error("session not established (complete the handshake first)")]
    NotEstablished,

    /// An AEAD operation rejected the ciphertext (tampering or wrong key).
    #[error("decrypt failed (tampered ciphertext or wrong key)")]
    DecryptFailed { reason: String },

    /// The chunk reassembler rejected the stream (bad flags, gap, oversize…).
    #[error("framing error: {reason}")]
    Framing { reason: String },

    /// L2CAP audio key derivation or frame handling failed.
    #[error("audio crypto error: {reason}")]
    AudioCrypto { reason: String },

    /// Input arguments are out of range (empty keys, bad PSM…).
    #[error("invalid argument: {reason}")]
    InvalidArgument { reason: String },
}

impl From<snow::Error> for PbError {
    fn from(e: snow::Error) -> Self {
        PbError::HandshakeFailed {
            reason: e.to_string(),
        }
    }
}

impl From<chacha20poly1305::Error> for PbError {
    fn from(e: chacha20poly1305::Error) -> Self {
        PbError::DecryptFailed {
            reason: e.to_string(),
        }
    }
}

impl From<base64::DecodeError> for PbError {
    fn from(e: base64::DecodeError) -> Self {
        PbError::PairingInvalid {
            reason: e.to_string(),
        }
    }
}

pub type PbResult<T> = Result<T, PbError>;
