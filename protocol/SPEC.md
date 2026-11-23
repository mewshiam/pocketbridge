# PocketBridge Wire Protocol Specification (v1.0)

Licensed under **Apache-2.0** (`protocol/LICENSE`).

## 1. Roles

- **Bridge (Android 12+):** BLE GATT Server / Peripheral and L2CAP CoC Listener. Holds cellular SIM(s) and acts as source of truth for telephony state.
- **Client (iOS 17+):** BLE GATT Client / Central and L2CAP CoC Initiator. Uses CoreBluetooth state restoration for background reconnection.

## 2. GATT Service & Characteristics

Primary Service UUID: `8F2B0001-6A4E-4C1D-9B3A-1E5F7D2C8A01`

| Characteristic | UUID | Properties | Direction | Purpose |
|---|---|---|---|---|
| `CTRL_TX` | `8F2B0002-6A4E-4C1D-9B3A-1E5F7D2C8A01` | Notify | Bridge -> Client | Encrypted Noise handshake & Protobuf control frames |
| `CTRL_RX` | `8F2B0003-6A4E-4C1D-9B3A-1E5F7D2C8A01` | Write, Write Without Response | Client -> Bridge | Encrypted Noise handshake & Protobuf control frames |
| `L2CAP_PSM` | `8F2B0004-6A4E-4C1D-9B3A-1E5F7D2C8A01` | Read | Bridge -> Client | 2-byte little-endian `uint16` dynamic PSM for voice L2CAP |
| `DEVICE_INFO` | `8F2B0005-6A4E-4C1D-9B3A-1E5F7D2C8A01` | Read | Bridge -> Client | `[uint8 proto_major, uint8 proto_minor, uint16 feature_flags]` |

Feature flags (`uint16` bitmask):
- `0x0001`: L2CAP Voice Audio
- `0x0002`: Dual-SIM Selection
- `0x0004`: Incremental Contact Sync
- `0x0008`: Opus FEC / DTX

## 3. GATT 6-Byte Framing Header

Every encrypted payload over `CTRL_TX` / `CTRL_RX` is chunked to `mtu - 3` bytes (targeting 517-byte MTU):

- Byte 0: `Flags` (`0x01` = START, `0x02` = END, `0x03` = single-chunk, `0x04` = HANDSHAKE)
- Byte 1: `Reserved` (`0x00`)
- Bytes 2..3: `Message ID` (`uint16`, big-endian, monotonic per sender)
- Bytes 4..5: `Chunk Index` (`uint16`, big-endian, zero-based)
- Bytes 6..N: Chunk payload bytes

Hard cap: `65,535` bytes maximum reassembled message size.

## 4. Pairing URI & Noise_XX Handshake

QR code URI displayed by Bridge:

```text
pbridge://pair?v=1&pub=<base64url(S_pub)>&adv=<8-char-hex-id>&n=<base64url(16-byte-nonce)>
```

- Pairing nonces expire after **120 seconds**.
- Handshake pattern: `Noise_XX_25519_ChaChaPoly_SHA256` with `HANDSHAKE` (`0x04`) flag set.
  1. `Client -> Bridge`: `-> e`
  2. `Bridge -> Client`: `<- e, ee, s, es` (payload echoes `nonce` on initial pair)
  3. `Client -> Bridge`: `-> s, se` (payload carries UTF-8 client device name)
- Post-handshake transport encrypts each serialized `pocketbridge.v1.Envelope` as one Noise transport message.

## 5. Voice Audio Pipeline (L2CAP CoC + Opus)

- Codec: Opus (RFC 6716), 16,000 Hz mono, 20 ms frames, 14 kbps VBR (8–20 kbps), FEC + DTX enabled.
- Keys: Derived from Noise handshake hash `h` via HKDF-SHA256 with `info = b"pocketbridge-audio-v1"` producing two 32-byte unidirectional keys (`c2b` and `b2c`).
- Wire frame: `[ uint16 frame_len (BE) ][ uint32 seq_num (BE) ][ ChaCha20-Poly1305(opus_frame) + 16-byte tag ]`
  with 12-byte nonce `[0u8; 8] || seq_num.to_be_bytes()` and `seq_num.to_be_bytes()` as AAD.
