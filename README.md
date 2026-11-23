# PocketBridge

*Your second phone stays in your bag. Your calls stay yours.*

PocketBridge is an open-source, peer-to-peer, end-to-end encrypted Bluetooth Low Energy (BLE) bridge that lets you make and receive cellular calls, send and receive SMS, and sync contacts and call logs from a SIM-bearing **Android phone (Android 12+)** directly on an **iPhone (iOS 17+)**.

## Highlights

- **Zero Cloud, Zero Telemetry:** Strictly peer-to-peer over BLE GATT and L2CAP CoC. No relay servers, analytics, or user accounts.
- **End-to-End Encrypted:** QR-authenticated `Noise_XX_25519_ChaChaPoly_SHA256` handshake with static key pinning and HKDF-derived L2CAP AEAD audio keys.
- **Shared Rust Core:** Framing, Noise sessions, pairing URIs, and audio packet AEAD live in `core/` and bind to Kotlin and Swift via UniFFI.
- **Bilingual (EN / FA):** Designed for English (LTR) and Persian (RTL) with Vazirmatn typography and OEM battery guides.

## Repository Layout

- `protocol/` — Wire protocol specification (`SPEC.md`) and Protobuf v3 schema (`proto/pocketbridge.proto`, Apache-2.0).
- `core/` — Shared Rust cryptography, GATT chunking, QR pairing, and L2CAP audio AEAD library (UniFFI).
- `android/` — Android 12+ Bridge app (Kotlin, Jetpack Compose, Foreground Service, SQLCipher).
- `ios/` — iOS 17+ Client app (Swift 6, SwiftUI, CoreBluetooth, CallKit, AVAudioEngine).
- `scripts/` — Protobuf and UniFFI binding generation scripts.
- `docs/` — Architecture blueprint (`POCKETBRIDGE_ALL_IN_ONE.md`), device compatibility matrix, and Persian guides (`docs/fa/`).

## Quick Verification

```bash
# 1. Validate shell scripts
bash -n scripts/gen-proto.sh scripts/build-uniffi.sh

# 2. Build & test Rust core
cd core && cargo fmt --check && cargo clippy -- -D warnings && cargo test

# 3. Generate Protobuf bindings
./scripts/gen-proto.sh
```

## License

- Apps & shared core: **GPL-3.0** (`LICENSE`)
- Protocol specification & Protobuf schema: **Apache-2.0** (`protocol/LICENSE`)
