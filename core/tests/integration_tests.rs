//! End-to-end integration tests: QR pair → Noise XX → chunked Protobuf
//! control traffic → L2CAP audio round-trips, plus tampered-ciphertext and
//! wrong-key rejections (blueprint Task 1 verification).

use pocketbridge_core::audio_crypto::{AudioDirection, AudioKeys, AudioRole};
use pocketbridge_core::framing::{chunk_message, Reassembler};
use pocketbridge_core::noise::{NoiseHandshake, StaticKeypair};
use pocketbridge_core::pairing::{is_pairing_fresh, parse_pair_uri, PairingPayload};

/// A fake serialized `Envelope` Protobuf message (hand-rolled field encoding:
/// field 1 varint id, field 2 varint timestamp). Good enough to prove the
/// transport path moves real bytes deterministically without pulling in the
/// full protobuf crate as a dev-dependency of the core.
fn fake_envelope(id: u64, timestamp_ms: u64, body: &str) -> Vec<u8> {
    let mut out = Vec::new();
    // field 1, wire type 0 (varint): tag = 0x08
    out.push(0x08);
    let mut v = id;
    loop {
        let mut b = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            b |= 0x80;
        }
        out.push(b);
        if v == 0 {
            break;
        }
    }
    // field 2, wire type 0 (varint): tag = 0x10
    out.push(0x10);
    let mut v = timestamp_ms;
    loop {
        let mut b = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            b |= 0x80;
        }
        out.push(b);
        if v == 0 {
            break;
        }
    }
    // field 10 (oneof ping), wire type 2 (length-delimited): tag = 0x52
    out.push(0x52);
    let bytes = body.as_bytes();
    let mut v = bytes.len() as u64;
    loop {
        let mut b = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            b |= 0x80;
        }
        out.push(b);
        if v == 0 {
            break;
        }
    }
    out.extend_from_slice(bytes);
    out
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Drives a full Noise_XX exchange between a client (initiator) and a bridge
/// (responder) that pins the key from the scanned QR URI.
fn full_handshake(client_name: &str) -> (NoiseTransportPair, PairingPayload) {
    let bridge_kp = StaticKeypair::generate();
    let client_kp = StaticKeypair::generate();
    let nonce = [0x5Au8; 16];

    let bridge_pub_arr: [u8; 32] = bridge_kp.public.as_slice().try_into().unwrap();
    let payload = PairingPayload::new(bridge_pub_arr, *b"DEADBEEF", nonce, now());
    let uri = payload.to_uri();
    let scanned = parse_pair_uri(&uri).unwrap();
    assert!(is_pairing_fresh(&scanned, now()));

    let bridge_pub: [u8; 32] = scanned.bridge_pub.as_slice().try_into().unwrap();
    let nonce_arr: [u8; 16] = scanned.nonce.as_slice().try_into().unwrap();
    let mut client = NoiseHandshake::new_initiator(client_kp, bridge_pub)
        .unwrap()
        .with_pairing_nonce(nonce_arr)
        .with_client_name(client_name);
    let mut bridge = NoiseHandshake::new_responder(bridge_kp, nonce_arr).unwrap();

    let m1 = client.write_message_1().unwrap();
    let m2 = bridge.read_message_1_write_message_2(&m1).unwrap();
    let m3 = client.read_message_2_write_message_3(&m2).unwrap();
    let name = bridge.read_message_3(&m3).unwrap();
    assert_eq!(name, client_name.as_bytes());

    let pair = NoiseTransportPair {
        client: client.finish().unwrap().0,
        bridge: bridge.finish().unwrap().0,
    };
    (pair, scanned)
}

struct NoiseTransportPair {
    client: pocketbridge_core::noise::NoiseTransportSession,
    bridge: pocketbridge_core::noise::NoiseTransportSession,
}

#[test]
fn integration_pair_noise_chunked_roundtrip() {
    let (mut pair, scanned) = full_handshake("iPhone of Ada");

    // Envelope: client → bridge over the control characteristics.
    let envelope = fake_envelope(1_234, 1_770_000_000_000, "ping-body");
    let wire = pair.client.send(&envelope).unwrap();

    // Simulate the 6-byte-header chunker at MTU 517 (512-byte payloads).
    let chunks = chunk_message(2, &wire, 517);
    assert!(!chunks.is_empty());

    let mut re = Reassembler::new();
    let mut reassembled = Vec::new();
    for chunk in &chunks {
        if let Some(msg) = re.feed(chunk).unwrap() {
            reassembled = msg;
        }
    }
    assert_eq!(reassembled, wire);

    let decrypted = pair.bridge.receive(&reassembled).unwrap();
    assert_eq!(decrypted, envelope);

    // And the reply direction: bridge → client.
    let reply = fake_envelope(1_235, 1_770_000_000_100, "pong-body");
    let wire_back = pair.bridge.send(&reply).unwrap();
    let chunks_back = chunk_message(3, &wire_back, 517);
    let mut re2 = Reassembler::new();
    let mut reassembled_back = Vec::new();
    for chunk in &chunks_back {
        if let Some(msg) = re2.feed(chunk).unwrap() {
            reassembled_back = msg;
        }
    }
    assert_eq!(reassembled_back, wire_back);
    let decrypted_back = pair.client.receive(&reassembled_back).unwrap();
    assert_eq!(decrypted_back, reply);

    // The pinned key must equal the key from the QR URI.
    assert_eq!(scanned.bridge_pub.len(), 32);
    assert_eq!(scanned.adv_id.len(), 8);
    assert_eq!(scanned.nonce.len(), 16);
}

#[test]
fn integration_tampered_ciphertext_rejected() {
    let (mut pair, _) = full_handshake("Tamper Test");
    let wire = pair.client.send(b"integrity-check").unwrap();

    let mut tampered = wire.clone();
    // Flip one bit near the end (inside the Poly1305 tag or ciphertext).
    let last = tampered.len() - 1;
    tampered[last] ^= 0x01;
    assert!(pair.bridge.receive(&tampered).is_err());

    // Truncation is also rejected.
    assert!(pair.bridge.receive(&wire[..wire.len() - 2]).is_err());
}

#[test]
fn integration_wrong_key_rejected() {
    // Session A: client ↔ bridge_A, both honest.
    let (mut pair_a, _) = full_handshake("Session A");

    // Session B: a different client pairs with a different bridge. Its keys
    // must not decrypt session A's traffic.
    let (mut pair_b, _) = full_handshake("Session B");
    let wire = pair_a.client.send(b"private").unwrap();
    assert!(pair_b.bridge.receive(&wire).is_err());

    // And vice versa.
    let wire_b = pair_b.client.send(b"other").unwrap();
    assert!(pair_a.bridge.receive(&wire_b).is_err());
}

#[test]
fn integration_expired_pairing_rejected() {
    let bridge_kp = StaticKeypair::generate();
    let bridge_pub_arr: [u8; 32] = bridge_kp.public.as_slice().try_into().unwrap();
    let payload = PairingPayload::new(bridge_pub_arr, *b"0BAD0F00", [0x11u8; 16], now());
    // 121 seconds later the QR URI is stale.
    assert!(!is_pairing_fresh(&payload, payload.created_at + 121));
}

#[test]
fn integration_l2cap_audio_roundtrip() {
    // Deterministic handshake-hash stand-in for the derivation input.
    let hash: [u8; 32] = *include_bytes!("../src/self_test_hash.bin");
    let bridge_keys =
        AudioKeys::derive(&hash, AudioRole::Bridge, [0x0Bu8; 8], [0x0Cu8; 8]).unwrap();
    let client_keys =
        AudioKeys::derive(&hash, AudioRole::Client, [0x0Cu8; 8], [0x0Bu8; 8]).unwrap();

    let b_send_key: [u8; 32] = bridge_keys.send_key.as_slice().try_into().unwrap();
    let b_send_prefix: [u8; 8] = bridge_keys.send_prefix.as_slice().try_into().unwrap();
    let b_recv_key: [u8; 32] = bridge_keys.recv_key.as_slice().try_into().unwrap();
    let b_recv_prefix: [u8; 8] = bridge_keys.recv_prefix.as_slice().try_into().unwrap();
    let c_send_key: [u8; 32] = client_keys.send_key.as_slice().try_into().unwrap();
    let c_send_prefix: [u8; 8] = client_keys.send_prefix.as_slice().try_into().unwrap();
    let c_recv_key: [u8; 32] = client_keys.recv_key.as_slice().try_into().unwrap();
    let c_recv_prefix: [u8; 8] = client_keys.recv_prefix.as_slice().try_into().unwrap();

    let bridge_send = AudioDirection::new(&b_send_key, b_send_prefix);
    let bridge_recv = AudioDirection::new(&b_recv_key, b_recv_prefix);
    let client_send = AudioDirection::new(&c_send_key, c_send_prefix);
    let client_recv = AudioDirection::new(&c_recv_key, c_recv_prefix);

    // 20 ms Opus frames at 50 pkt/s: simulate a second of traffic.
    let opus_frame = vec![0xCE, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77];
    let mut frames_ok = 0usize;
    for seq in 0..50u32 {
        // client → bridge
        let wire = client_send.seal_frame(seq, &opus_frame).unwrap();
        let (got_seq, got) = client_recv_open(&bridge_recv, &wire);
        assert_eq!((got_seq, got.clone()), (seq, opus_frame.clone()));
        // bridge → client
        let wire_b = bridge_send.seal_frame(seq, &opus_frame).unwrap();
        let (got_seq_b, got_b) = client_recv_open(&client_recv, &wire_b);
        assert_eq!((got_seq_b, got_b.clone()), (seq, opus_frame.clone()));
        frames_ok += 2;
    }
    assert_eq!(frames_ok, 100);

    // Tampering with any audio frame breaks the Poly1305 tag.
    let mut bad = client_send.seal_frame(0, &opus_frame).unwrap();
    let last = bad.len() - 1;
    bad[last] ^= 0x08;
    assert!(bridge_recv.open_frame(&bad).is_err());
}

fn client_recv_open(dir: &AudioDirection, wire: &[u8]) -> (u32, Vec<u8>) {
    dir.open_frame(wire).unwrap()
}

#[test]
fn integration_audio_wrong_key_rejected() {
    let hash: [u8; 32] = *include_bytes!("../src/self_test_hash.bin");
    let keys = AudioKeys::derive(&hash, AudioRole::Bridge, [1u8; 8], [2u8; 8]).unwrap();
    let send_key: [u8; 32] = keys.send_key.as_slice().try_into().unwrap();
    let send_prefix: [u8; 8] = keys.send_prefix.as_slice().try_into().unwrap();
    let sender = AudioDirection::new(&send_key, send_prefix);

    // Receiver derives keys from a *different* hash (different handshake).
    let other = AudioKeys::derive(&[0xABu8; 32], AudioRole::Client, [2u8; 8], [1u8; 8]).unwrap();
    let other_recv_key: [u8; 32] = other.recv_key.as_slice().try_into().unwrap();
    let other_recv_prefix: [u8; 8] = other.recv_prefix.as_slice().try_into().unwrap();
    let wrong = AudioDirection::new(&other_recv_key, other_recv_prefix);

    let wire = sender.seal_frame(4, b"opus-secret").unwrap();
    assert!(wrong.open_frame(&wire).is_err());
}
