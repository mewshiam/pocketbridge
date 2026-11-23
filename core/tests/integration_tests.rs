use pocketbridge_core::audio_crypto::{derive_audio_keys, open_audio_frame, seal_audio_frame};
use pocketbridge_core::framing::{chunk_message, FrameReassembler};
use pocketbridge_core::noise::{generate_static_keypair, NoiseHandshake};
use pocketbridge_core::pairing::{is_pairing_expired, PairingPayload, PAIRING_VERSION};
use pocketbridge_core::CoreError;

#[test]
fn full_qr_pair_noise_framing_and_audio_roundtrip() {
    let bridge_kp = generate_static_keypair().expect("bridge keypair");
    let client_kp = generate_static_keypair().expect("client keypair");
    let nonce = [0xABu8; 16];

    // 1. QR URI build & parse
    let qr_payload = PairingPayload {
        version: PAIRING_VERSION,
        bridge_static_pub: bridge_kp.public,
        adv_id: "1a2b3c4d".to_string(),
        nonce,
    };
    let uri = qr_payload.to_uri().expect("uri");
    let parsed = PairingPayload::from_uri(&uri).expect("parsed uri");
    assert_eq!(parsed, qr_payload);
    assert!(!is_pairing_expired(1_000, 1_120));
    assert!(is_pairing_expired(1_000, 1_121));

    // 2. Noise_XX handshake (Client -> Bridge -> Client)
    let mut client_hs =
        NoiseHandshake::new_initiator(&client_kp.private, parsed.bridge_static_pub, Some(parsed.nonce))
            .expect("client hs");
    let mut bridge_hs = NoiseHandshake::new_responder(&bridge_kp.private, None).expect("bridge hs");

    let msg1 = client_hs.write_message(&[]).expect("msg1");
    bridge_hs.read_message(&msg1).expect("bridge read msg1");

    let msg2 = bridge_hs.write_message(&nonce).expect("msg2");
    let echoed = client_hs.read_message(&msg2).expect("client read msg2");
    assert_eq!(echoed, nonce);

    let msg3 = client_hs.write_message(b"iPhone 16 Pro").expect("msg3");
    let device_name = bridge_hs.read_message(&msg3).expect("bridge read msg3");
    assert_eq!(device_name, b"iPhone 16 Pro");

    let mut client_sess = client_hs.into_transport().expect("client transport");
    let mut bridge_sess = bridge_hs.into_transport().expect("bridge transport");
    assert_eq!(client_sess.handshake_hash(), bridge_sess.handshake_hash());
    assert_eq!(bridge_sess.remote_static_pub(), client_kp.public);

    // 3. Chunked encrypted transport payload over small MTU
    let big_envelope = vec![0x42u8; 1_200];
    let encrypted = client_sess.encrypt(&big_envelope).expect("encrypt");
    let chunks = chunk_message(7, &encrypted, 247, false).expect("chunk");
    assert!(chunks.len() > 4);

    let mut reassembler = FrameReassembler::default();
    let mut reassembled = None;
    for chunk in chunks {
        reassembled = reassembler.push_chunk(&chunk).expect("push chunk");
    }
    let finished = reassembled.expect("completed frame");
    assert_eq!(finished.msg_id, 7);
    let decrypted = bridge_sess.decrypt(&finished.payload).expect("decrypt");
    assert_eq!(decrypted, big_envelope);

    // 4. L2CAP audio key derivation and AEAD roundtrip
    let client_audio = derive_audio_keys(&client_sess.handshake_hash()).expect("client audio keys");
    let bridge_audio = derive_audio_keys(&bridge_sess.handshake_hash()).expect("bridge audio keys");

    let opus_sample = vec![0x11, 0x22, 0x33, 0x44, 0x55];
    let wire = seal_audio_frame(&client_audio.client_to_bridge, 42, &opus_sample).expect("seal");
    let (seq, opened) = open_audio_frame(&bridge_audio.client_to_bridge, &wire).expect("open");
    assert_eq!(seq, 42);
    assert_eq!(opened, opus_sample);

    // Tampered ciphertext must fail authentication
    let mut tampered = wire.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 0xFF;
    assert!(open_audio_frame(&bridge_audio.client_to_bridge, &tampered).is_err());
}

#[test]
fn wrong_pinned_key_is_rejected() {
    let bridge_kp = generate_static_keypair().expect("bridge keypair");
    let impostor_kp = generate_static_keypair().expect("impostor keypair");
    let client_kp = generate_static_keypair().expect("client keypair");

    let mut client_hs =
        NoiseHandshake::new_initiator(&client_kp.private, bridge_kp.public, None).expect("client");
    let mut impostor_hs =
        NoiseHandshake::new_responder(&impostor_kp.private, None).expect("impostor");

    let msg1 = client_hs.write_message(&[]).expect("msg1");
    impostor_hs.read_message(&msg1).expect("read msg1");
    let msg2 = impostor_hs.write_message(&[]).expect("msg2");

    let err = client_hs.read_message(&msg2).expect_err("must reject wrong static key");
    assert!(matches!(err, CoreError::PeerKeyMismatch));
}
