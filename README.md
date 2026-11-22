# PocketBridge

> **Your second phone stays in your bag. Your calls stay yours.**

PocketBridge is a 100% open-source, peer-to-peer, end-to-end encrypted Bluetooth Low Energy (BLE)
bridge that lets you make and receive cellular calls, send and receive SMS, and access contacts and
call logs from a SIM-bearing **Android phone (Android 12+)** directly on an **iPhone (iOS 17+)**.

- **Zero cloud. Zero telemetry.** All data flows strictly peer-to-peer over BLE. No analytics, no
  crash-reporting SDKs, no accounts, no HTTP sync servers.
- **End-to-end encryption** via the Noise framework (`Noise_XX_25519_ChaChaPoly_SHA256`) with
  static key pinning from a QR pairing flow.
- **Shared Rust core** (`core/`) compiled into Kotlin and Swift bindings via UniFFI, so both
  platforms run byte-identical cryptography, framing, and audio-packet AEAD.
- **Bilingual v1.0:** English (LTR) and Persian (فارسی, full RTL).

## Licensing

| Component | License |
|---|---|
| Android/iOS apps & `core/` | **GPL-3.0** (see [`LICENSE`](LICENSE)) |
| Protocol specification & Protobuf schema (`protocol/`) | **Apache-2.0** (see [`protocol/LICENSE`](protocol/LICENSE)) |

## Repository layout

```text
pocketbridge/
├── .github/workflows/     # core-ci, android-ci, ios-ci, release
├── protocol/              # Wire protocol spec + Protobuf v3 schema (Apache-2.0)
├── core/                   # Shared Rust library (UniFFI): framing, Noise, audio crypto, pairing
├── android/                # Kotlin / Jetpack Compose bridge app (Android 12+, minSdk 31)
├── ios/                    # Swift / SwiftUI client app (iOS 17+)
├── docs/                   # Documentation + Persian (fa) docs
└── scripts/                # gen-proto.sh, build-uniffi.sh
```

## Build

| Piece | Command |
|---|---|
| Rust core (fmt + clippy + test) | `cd core && cargo fmt --check && cargo clippy -- -D warnings && cargo test` |
| Protobuf codegen | `bash scripts/gen-proto.sh` |
| UniFFI native builds | `bash scripts/build-uniffi.sh --android` / `--ios` |
| Android app | `cd android && ./gradlew lintDebug testDebugUnitTest assembleDebug` |
| iOS app | `cd ios && xcodegen generate && xcodebuild test -scheme PocketBridge` |

Full engineering detail lives in
[`docs/POCKETBRIDGE_ALL_IN_ONE.md`](docs/POCKETBRIDGE_ALL_IN_ONE.md); the wire protocol is
specified in [`protocol/SPEC.md`](protocol/SPEC.md).

## Verification snapshots

CI runs the full verification chain on every push:

- **core-ci:** `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`
- **android-ci:** Gradle lint, unit tests, debug APK
- **ios-ci:** XcodeGen + `xcodebuild test` on macOS
- **release:** tag-triggered signed APK, `SHA256SUMS.txt`, AltStore source JSON
