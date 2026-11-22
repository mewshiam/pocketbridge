//! BLE GATT framing: the 6-byte chunk header used by `CTRL_TX` / `CTRL_RX`.
//!
//! Every encrypted message sent over the control characteristics is segmented
//! into MTU-sized chunks (`mtu - 3` bytes max payload per ATT packet). The
//! reassembler enforces the 65,535-byte hard cap and evicts incomplete buffers
//! on timeout.
//!
//! Header layout (all multi-byte fields big-endian):
//!
//! ```text
//!  0                   1                   2                   3
//!  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |     Flags     |   Reserved    |          Message ID           |
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |          Chunk Index          |         Payload Bytes...      |
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! ```

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::error::{PbError, PbResult};

/// `0x01` — first chunk of a multi-chunk message.
pub const FLAG_START: u8 = 0x01;
/// `0x02` — last chunk of a multi-chunk message.
pub const FLAG_END: u8 = 0x02;
/// `0x03` — single-chunk message (START | END).
pub const FLAG_SINGLE: u8 = 0x03;
/// `0x04` — handshake traffic (Noise_XX pre-transport).
pub const FLAG_HANDSHAKE: u8 = 0x04;

/// Maximum reassembled message size in bytes (protocol hard cap).
pub const MAX_MESSAGE_SIZE: usize = 65_535;

/// ATT header overhead (op code + handle) subtracted from the negotiated MTU.
pub const ATT_HEADER_OVERHEAD: usize = 3;

/// Time an incomplete reassembly buffer may sit before eviction.
const REASSEMBLY_TIMEOUT: Duration = Duration::from_secs(30);

/// Header size in bytes.
const HEADER_LEN: usize = 6;

/// Splits `message` into framed chunks whose payload fits inside `mtu`.
///
/// `message_id` must be monotonically incrementing per sender; the reassembler
/// on the peer side keys buffers by that ID.
pub fn chunk_message(message_id: u16, data: &[u8], mtu: usize) -> Vec<Vec<u8>> {
    // GATT payloads max out at 512 bytes (517 MTU - 3 overhead) and the
    // reassembled cap is 65,535; clamp defensively either way.
    let payload_max = (mtu.saturating_sub(ATT_HEADER_OVERHEAD)).clamp(1, 512);
    let chunks = if data.is_empty() {
        1
    } else {
        data.len().div_ceil(payload_max)
    };
    let flags_final = if chunks == 1 { FLAG_SINGLE } else { FLAG_END };

    let mut out = Vec::with_capacity(chunks.min(u16::MAX as usize));
    for idx in 0..chunks {
        let start = idx * payload_max;
        let end = ((idx + 1) * payload_max).min(data.len());
        let payload = &data[start..end];

        let mut flags = 0u8;
        if idx == 0 {
            flags |= FLAG_START;
        }
        if idx == chunks - 1 {
            flags |= flags_final & FLAG_END;
        }
        if chunks == 1 {
            flags = FLAG_SINGLE;
        }

        let mut frame = Vec::with_capacity(HEADER_LEN + payload.len());
        frame.push(flags);
        frame.push(0x00); // reserved
        frame.extend_from_slice(&message_id.to_be_bytes());
        frame.extend_from_slice(&(idx as u16).to_be_bytes());
        frame.extend_from_slice(payload);
        out.push(frame);
    }
    out
}

/// Splits `message` into framed chunks flagged as Noise handshake traffic.
///
/// Handshake chunks carry `FLAG_HANDSHAKE` (0x04) instead of START/END framing
/// so the receive side routes them to the Noise session before transport
/// encryption exists. Each handshake message is still delivered as a single
/// chunk, regardless of size — the BLE stack fragments at the ATT layer.
pub fn chunk_handshake(message_id: u16, data: &[u8]) -> Vec<Vec<u8>> {
    let mut frame = Vec::with_capacity(HEADER_LEN + data.len());
    frame.push(FLAG_HANDSHAKE);
    frame.push(0x00); // reserved
    frame.extend_from_slice(&message_id.to_be_bytes());
    frame.extend_from_slice(&0u16.to_be_bytes());
    frame.extend_from_slice(data);
    vec![frame]
}

/// Parses the fixed 6-byte header of a framed chunk.
pub fn parse_header(frame: &[u8]) -> PbResult<(u8, u16, u16)> {
    if frame.len() < HEADER_LEN {
        return Err(PbError::Framing {
            reason: format!("frame too short: {} bytes", frame.len()),
        });
    }
    let flags = frame[0];
    let message_id = u16::from_be_bytes([frame[2], frame[3]]);
    let chunk_index = u16::from_be_bytes([frame[4], frame[5]]);
    Ok((flags, message_id, chunk_index))
}

/// Streaming reassembler keyed by `message_id`.
///
/// Feeds framed chunks and returns `Some(message)` once the END chunk lands,
/// or `None` while the message is still incomplete. Incomplete buffers are
/// evicted when `evict_stale()` is called after `REASSEMBLY_TIMEOUT`.
pub struct Reassembler {
    buffers: HashMap<u16, Buffer>,
}

struct Buffer {
    flags_seen_start: bool,
    flags_seen_end: bool,
    next_index: u16,
    data: Vec<u8>,
    updated: Instant,
}

impl Reassembler {
    pub fn new() -> Self {
        Reassembler {
            buffers: HashMap::new(),
        }
    }

    /// Feeds one framed chunk. Returns the completed message when this chunk
    /// finishes it, otherwise `None`.
    pub fn feed(&mut self, frame: &[u8]) -> PbResult<Option<Vec<u8>>> {
        let (flags, message_id, chunk_index) = parse_header(frame)?;
        let payload = &frame[HEADER_LEN..];
        let is_handshake = flags == FLAG_HANDSHAKE;
        let has_start = flags & FLAG_START != 0 || flags == FLAG_SINGLE;
        let has_end = flags & FLAG_END != 0 || flags == FLAG_SINGLE;

        // Handshake chunks are standalone: emit immediately.
        if is_handshake {
            return Ok(Some(payload.to_vec()));
        }

        let buffer = self.buffers.entry(message_id).or_insert_with(|| Buffer {
            flags_seen_start: false,
            flags_seen_end: false,
            next_index: 0,
            data: Vec::new(),
            updated: Instant::now(),
        });

        if has_start && chunk_index != 0 {
            return Err(PbError::Framing {
                reason: format!("START chunk with index {chunk_index} (must be 0)"),
            });
        }
        if !has_start && buffer.data.is_empty() && !buffer.flags_seen_start {
            return Err(PbError::Framing {
                reason: format!("message {message_id} started mid-stream (no START chunk)"),
            });
        }
        if buffer.flags_seen_end {
            return Err(PbError::Framing {
                reason: format!("message {message_id} already completed"),
            });
        }
        if chunk_index != buffer.next_index {
            return Err(PbError::Framing {
                reason: format!(
                    "message {message_id} chunk gap: expected {}, got {chunk_index}",
                    buffer.next_index
                ),
            });
        }
        if buffer.data.len() + payload.len() > MAX_MESSAGE_SIZE {
            return Err(PbError::Framing {
                reason: format!("message {message_id} exceeds {MAX_MESSAGE_SIZE}-byte cap"),
            });
        }

        if has_start {
            buffer.flags_seen_start = true;
            buffer.data.clear();
            buffer.next_index = 0;
        }
        buffer.data.extend_from_slice(payload);
        buffer.next_index = buffer.next_index.wrapping_add(1);
        buffer.updated = Instant::now();

        if has_end {
            buffer.flags_seen_end = true;
            let message = buffer.data.clone();
            self.buffers.remove(&message_id);
            return Ok(Some(message));
        }
        Ok(None)
    }

    /// Drops every buffer older than the reassembly timeout; returns how many
    /// were evicted. Call this opportunistically from the BLE receive loop.
    pub fn evict_stale(&mut self) -> usize {
        let before = self.buffers.len();
        self.buffers
            .retain(|_, b| b.updated.elapsed() < REASSEMBLY_TIMEOUT);
        before - self.buffers.len()
    }

    /// Number of partially received messages currently held.
    pub fn in_flight(&self) -> usize {
        self.buffers.len()
    }
}

impl Default for Reassembler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_chunk_roundtrip() {
        let data = b"hello pocketbridge";
        let frames = chunk_message(7, data, 517);
        assert_eq!(frames.len(), 1);
        let (flags, mid, idx) = parse_header(&frames[0]).unwrap();
        assert_eq!(flags, FLAG_SINGLE);
        assert_eq!(mid, 7);
        assert_eq!(idx, 0);

        let mut re = Reassembler::new();
        let out = re.feed(&frames[0]).unwrap().unwrap();
        assert_eq!(out, data);
    }

    #[test]
    fn multi_chunk_roundtrip() {
        let data: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8).collect();
        let frames = chunk_message(42, &data, 517);
        assert!(frames.len() > 1);

        let mut re = Reassembler::new();
        let mut final_msg = None;
        for f in &frames {
            if let Some(m) = re.feed(f).unwrap() {
                final_msg = Some(m);
            }
        }
        assert_eq!(final_msg.unwrap(), data);
    }

    #[test]
    fn handshake_chunk_passes_through() {
        let frames = chunk_handshake(1, b"noise-xx-msg");
        assert_eq!(frames.len(), 1);
        let mut re = Reassembler::new();
        let out = re.feed(&frames[0]).unwrap().unwrap();
        assert_eq!(out, b"noise-xx-msg");
    }

    #[test]
    fn rejects_mid_stream_start() {
        let data = vec![0u8; 1024];
        let frames = chunk_message(9, &data, 517);
        let mut re = Reassembler::new();
        // Feed chunk 1 (index 1) before any START.
        assert!(re.feed(&frames[1]).is_err());
    }

    #[test]
    fn rejects_chunk_gap() {
        let data = vec![7u8; 1500];
        let frames = chunk_message(3, &data, 517);
        let mut re = Reassembler::new();
        re.feed(&frames[0]).unwrap();
        // Skip chunk 1, jump straight to chunk 2.
        assert!(re.feed(&frames[2]).is_err());
    }

    #[test]
    fn rejects_oversize_message() {
        // Forge a stream claiming a message larger than the 65,535 cap.
        let mut re = Reassembler::new();
        let max_payload = 512;
        let mut too_big = vec![0u8; MAX_MESSAGE_SIZE + 100];
        too_big[0] = FLAG_START;
        too_big[2..4].copy_from_slice(&5u16.to_be_bytes());
        too_big[4..6].copy_from_slice(&0u16.to_be_bytes());
        let _ = max_payload;
        assert!(re.feed(&too_big).is_err());
    }

    #[test]
    fn empty_message_single_chunk() {
        let frames = chunk_message(1, b"", 517);
        assert_eq!(frames.len(), 1);
        let mut re = Reassembler::new();
        let out = re.feed(&frames[0]).unwrap().unwrap();
        assert!(out.is_empty());
    }
}
