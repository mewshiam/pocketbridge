# PocketBridge Wire Protocol Specification

Version 1.0 — the normative reference for the on-air behavior of PocketBridge. The Rust
implementation in `core/` and the platform bindings are the reference implementations.

## 1. Roles

- **Bridge (Android 12+):** BLE GATT Server / Peripheral + L2CAP CoC Listener. Holds the SIM(s),
  advertises the primary service UUID, and is the single source of truth for cellular call state.
- **Client (iOS 17+):** BLE GATT Client / Central + L2CAP CoC Initiator. Uses
  `CBCentralManagerOptionRestoreIdentifierKey` for background wake, renders native CallKit UI,
  and stores synced SMS locally.

## 2. GATT service and characteristics

- **Primary Service UUID:** `8F2B0001-6A4E-4C1D-9B3A-1E5F7D2C8A01`

| Characteristic | UUID | Properties | Direction | Purpose |
|---|---|---|---|---|
| `CTRL_TX` | `8F2B0002-6A4E-4C1D-9B3A-1E5F7D2C8A01` | Notify | Bridge → Client | Encrypted Noise handshake & Protobuf control frames |
| `CTRL_RX` | `8F2B0003-6A4E-4C1D-9B3A-1E5F7D2C8A01` | Write, Write Without Response | Client → Bridge | Encrypted Noise handshake & Protobuf control frames |
| `L2CAP_PSM` | `8F2B0004-6A4E-4C1D-9B3A-1E5F7D2C8A01` | Read | Bridge → Client | 2-byte little-endian `uint16` dynamic PSM for voice L2CAP |
| `DEVICE_INFO` | `8F2B0005-6A4E-4C1D-9B3A-1E5F7D2C8A01` | Read | Bridge → Client | Unencrypted `[uint8 proto_major, uint8 proto_minor, uint16 feature_flags]` |

`feature_flags` (`uint16` bitmask, little-endian):

| Bit | Value | Feature |
|---|---|---|
| 0 | `0x0001` | L2CAP Voice Audio |
| 1 | `0x0002` | Dual-SIM Selection |
| 2 | `0x0004` | Incremental Contact Sync |
| 3 | `0x0008` | Opus FEC / DTX |

## 3. GATT framing and chunking (6-byte header)

Every encrypted message sent over `CTRL_TX` / `CTRL_RX` is segmented into MTU-sized chunks
(`mtu - 3` bytes max per ATT packet, targeting 517-byte ATT MTU).

```text
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|     Flags     |   Reserved    |          Message ID           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|          Chunk Index          |         Payload Bytes...      |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

- **Flags (1 byte):** `0x01` = `START`, `0x02` = `END`, `0x03` = single-chunk, `0x04` = `HANDSHAKE`.
- **Message ID (2 bytes, big-endian):** monotonically incrementing per sender.
- **Chunk Index (2 bytes, big-endian):** zero-based sequence index.
- **Hard cap:** 65,535 bytes max reassembled message size; incomplete buffers are evicted on
  timeout.

## 4. QR pairing and Noise_XX handshake

1. **QR Code URI (displayed by the Android Bridge):**

   ```text
   pbridge://pair?v=1&pub=<base64url(S_pub)>&adv=<8-char-hex-id>&n=<base64url(16-byte-nonce)>
   ```

   - `pub`: Bridge's 32-byte X25519 static public key (`S_pub`).
   - `adv`: ephemeral 8-character hex ID placed in BLE Manufacturer Data to disambiguate nearby
     devices.
   - `n`: one-time 16-byte pairing nonce valid for **120 seconds**.

2. **Handshake (`Noise_XX_25519_ChaChaPoly_SHA256`, `HANDSHAKE` flag set on every chunk):**

   - **Msg 1 (Client → Bridge):** `-> e`
   - **Msg 2 (Bridge → Client):** `<- e, ee, s, es` (payload echoes `nonce` on initial pair)
   - **Msg 3 (Client → Bridge):** `-> s, se` (payload carries Client device name)

3. **Pinning rules:**

   - *Initial pair:* Client verifies Bridge's `s` equals `pub` from the QR and the echoed nonce
     equals `n`. The Bridge prompts the user to tap **Approve** to pin the Client's static key.
   - *Reconnects:* both peers strictly reject any static key not matching their pinned peer key.

4. **Post-handshake transport:** the session splits into unidirectional ChaCha20-Poly1305 cipher
   states (`C_c2b`, `C_b2c`). Each serialized `Envelope` Protobuf message is encrypted as one
   Noise transport message.

## 5. Voice audio pipeline (L2CAP CoC + Opus)

The L2CAP channel is opened only when a call enters `RINGING` or `ACTIVE` state.

- **Codec:** Opus (RFC 6716), 16,000 Hz wideband, mono, 20 ms frames (50 packets/s), 14 kbps VBR
  (8–20 kbps), in-band FEC + DTX enabled.
- **Key derivation:** separate 32-byte send/receive L2CAP keys are derived from the active Noise
  handshake hash `h` via HKDF-SHA256 with `info = b"pocketbridge-audio-v1"`.
- **L2CAP frame format:**

  ```text
  [ uint16 frame_len ][ uint32 seq_num ][ ChaCha20-Poly1305(opus_frame) + 16-byte tag ]
  ```

- **Jitter buffer:** adaptive 40–120 ms buffer on both peers, invoking Opus PLC on missing
  `seq_num`.

## 6. Reliability and offline queue

- **Heartbeat:** Client sends `Ping` every 30 s; Bridge replies with `Pong` + `DeviceStatus`.
- **Offline queue:** Android persists undelivered incoming SMS and missed-call events in
  SQLCipher (up to 7 days) and flushes chronologically on reconnect.

## 7. Protobuf schema

See [`proto/pocketbridge.proto`](proto/pocketbridge.proto) (Apache-2.0). The `Envelope` oneof
defines all control messages: heartbeat (`Ping`/`Pong`), `DeviceStatus`, `Ack`, SMS
(receive/send/sync), call control (state events, place/action/DTMF), and incremental sync of
contacts and call logs.
