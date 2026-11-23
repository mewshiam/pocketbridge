use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::CoreError;

pub const HEADER_LEN: usize = 6;
pub const FLAG_START: u8 = 0x01;
pub const FLAG_END: u8 = 0x02;
pub const FLAG_HANDSHAKE: u8 = 0x04;
pub const MAX_REASSEMBLED_LEN: usize = 65_535;
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

/// Splits a payload into MTU-sized chunks with the 6-byte PocketBridge GATT header.
///
/// `att_mtu` is the negotiated BLE ATT MTU (typically 23..=517). Each ATT notification/write
/// holds up to `att_mtu - 3` bytes, leaving `att_mtu - 3 - 6` bytes for chunk payload.
pub fn chunk_message(
    msg_id: u16,
    payload: &[u8],
    att_mtu: usize,
    is_handshake: bool,
) -> Result<Vec<Vec<u8>>, CoreError> {
    if payload.len() > MAX_REASSEMBLED_LEN {
        return Err(CoreError::MessageTooLarge {
            len: payload.len() as u64,
            max: MAX_REASSEMBLED_LEN as u64,
        });
    }
    let att_payload_cap = att_mtu.saturating_sub(3);
    if att_payload_cap <= HEADER_LEN {
        return Err(CoreError::InvalidMtu {
            mtu: att_mtu as u32,
        });
    }
    let max_chunk_data = att_payload_cap - HEADER_LEN;

    if payload.is_empty() {
        let mut flags = FLAG_START | FLAG_END;
        if is_handshake {
            flags |= FLAG_HANDSHAKE;
        }
        let mut frame = Vec::with_capacity(HEADER_LEN);
        frame.push(flags);
        frame.push(0x00);
        frame.extend_from_slice(&msg_id.to_be_bytes());
        frame.extend_from_slice(&0u16.to_be_bytes());
        return Ok(vec![frame]);
    }

    let total_chunks = payload.len().div_ceil(max_chunk_data);
    if total_chunks > u16::MAX as usize {
        return Err(CoreError::MessageTooLarge {
            len: payload.len() as u64,
            max: MAX_REASSEMBLED_LEN as u64,
        });
    }

    let mut out = Vec::with_capacity(total_chunks);
    for (idx, slice) in payload.chunks(max_chunk_data).enumerate() {
        let mut flags = 0u8;
        if idx == 0 {
            flags |= FLAG_START;
        }
        if idx + 1 == total_chunks {
            flags |= FLAG_END;
        }
        if is_handshake {
            flags |= FLAG_HANDSHAKE;
        }

        let mut frame = Vec::with_capacity(HEADER_LEN + slice.len());
        frame.push(flags);
        frame.push(0x00);
        frame.extend_from_slice(&msg_id.to_be_bytes());
        frame.extend_from_slice(&(idx as u16).to_be_bytes());
        frame.extend_from_slice(slice);
        out.push(frame);
    }

    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReassembledMessage {
    pub msg_id: u16,
    pub is_handshake: bool,
    pub payload: Vec<u8>,
}

struct PartialBuffer {
    is_handshake: bool,
    next_index: u16,
    data: Vec<u8>,
    updated_at: Instant,
}

/// Stateful GATT chunk reassembler with 65,535-byte hard cap and timeout eviction.
pub struct FrameReassembler {
    inflight: HashMap<u16, PartialBuffer>,
    timeout: Duration,
}

impl Default for FrameReassembler {
    fn default() -> Self {
        Self::new(DEFAULT_TIMEOUT)
    }
}

impl FrameReassembler {
    pub fn new(timeout: Duration) -> Self {
        Self {
            inflight: HashMap::new(),
            timeout,
        }
    }

    pub fn evict_expired(&mut self, now: Instant) {
        self.inflight
            .retain(|_, buf| now.saturating_duration_since(buf.updated_at) <= self.timeout);
    }

    pub fn push_chunk(&mut self, chunk: &[u8]) -> Result<Option<ReassembledMessage>, CoreError> {
        let now = Instant::now();
        self.evict_expired(now);

        if chunk.len() < HEADER_LEN {
            return Err(CoreError::MalformedFrame("chunk shorter than 6-byte header".into()));
        }

        let flags = chunk[0];
        let _reserved = chunk[1];
        let msg_id = u16::from_be_bytes([chunk[2], chunk[3]]);
        let chunk_idx = u16::from_be_bytes([chunk[4], chunk[5]]);
        let body = &chunk[HEADER_LEN..];

        let is_start = (flags & FLAG_START) != 0;
        let is_end = (flags & FLAG_END) != 0;
        let is_handshake = (flags & FLAG_HANDSHAKE) != 0;

        if is_start && is_end {
            if chunk_idx != 0 {
                return Err(CoreError::MalformedFrame("single-chunk frame must have index 0".into()));
            }
            if body.len() > MAX_REASSEMBLED_LEN {
                return Err(CoreError::MessageTooLarge {
                    len: body.len() as u64,
                    max: MAX_REASSEMBLED_LEN as u64,
                });
            }
            self.inflight.remove(&msg_id);
            return Ok(Some(ReassembledMessage {
                msg_id,
                is_handshake,
                payload: body.to_vec(),
            }));
        }

        if is_start {
            if chunk_idx != 0 {
                return Err(CoreError::MalformedFrame("START chunk must have index 0".into()));
            }
            if body.len() > MAX_REASSEMBLED_LEN {
                return Err(CoreError::MessageTooLarge {
                    len: body.len() as u64,
                    max: MAX_REASSEMBLED_LEN as u64,
                });
            }
            self.inflight.insert(
                msg_id,
                PartialBuffer {
                    is_handshake,
                    next_index: 1,
                    data: body.to_vec(),
                    updated_at: now,
                },
            );
            return Ok(None);
        }

        let entry = self.inflight.get_mut(&msg_id).ok_or_else(|| {
            CoreError::MalformedFrame(format!("unexpected continuation chunk for msg_id {msg_id}"))
        })?;

        if entry.next_index != chunk_idx {
            self.inflight.remove(&msg_id);
            return Err(CoreError::MalformedFrame(format!(
                "out-of-order chunk index {chunk_idx} for msg_id {msg_id}"
            )));
        }

        if entry.is_handshake != is_handshake {
            self.inflight.remove(&msg_id);
            return Err(CoreError::MalformedFrame(
                "handshake flag mismatch across chunks".into(),
            ));
        }

        if entry.data.len().saturating_add(body.len()) > MAX_REASSEMBLED_LEN {
            self.inflight.remove(&msg_id);
            return Err(CoreError::MessageTooLarge {
                len: (entry.data.len() + body.len()) as u64,
                max: MAX_REASSEMBLED_LEN as u64,
            });
        }

        entry.data.extend_from_slice(body);
        entry.next_index = entry.next_index.wrapping_add(1);
        entry.updated_at = now;

        if is_end {
            let finished = self.inflight.remove(&msg_id).expect("entry exists");
            return Ok(Some(ReassembledMessage {
                msg_id,
                is_handshake: finished.is_handshake,
                payload: finished.data,
            }));
        }

        Ok(None)
    }
}
