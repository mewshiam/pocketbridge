# PocketBridge — All-in-One Project Blueprint

> **Single Source of Truth:** This document contains everything required for a human engineering team or an autonomous coding agent to bootstrap, build, verify, and ship **PocketBridge** from zero to v1.0.

**Tagline:** *Your second phone stays in your bag. Your calls stay yours.*  
**License:** **GPL-3.0** (Android/iOS apps & core) / **Apache-2.0** (Protocol specification & Protobuf schema)

---

## 1. Executive Summary & Hard Constraints

**PocketBridge** is a 100% open-source, peer-to-peer, end-to-end encrypted Bluetooth Low Energy (BLE) bridge that lets users make/receive cellular calls, send/receive SMS, and access contacts and call logs from a SIM-bearing **Android phone (Android 12+)** directly on an **iPhone (iOS 17+)**.

### Non-Negotiable Constraints
1. **Clean-Room Implementation:** Write 100% original code, assets, and copy. Never reference competing brands or use private/undocumented Apple or Android APIs.
2. **Zero Cloud / Zero Telemetry:** Never add analytics, crash-reporting SDKs, HTTP sync servers, or mandatory user accounts. All data flows strictly peer-to-peer over BLE.
3. **Security Hygiene:**
   * Zeroize secret key material in memory after use (`zeroize` crate in Rust).
   * Store long-term static private keys exclusively in **Android Keystore** (or `EncryptedSharedPreferences` backed by Keystore) and **iOS Keychain** (`kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`).
   * Never log plaintext SMS bodies, phone numbers, or cryptographic keys in release builds.
4. **Commit Discipline:** Use atomic Conventional Commits (`chore:`, `feat(core):`, `feat(android):`, `feat(ios):`, `test:`).

---

## 2. Brand Identity & Design System

| Element | Specification |
|---|---|
| **Name** | **PocketBridge** |
| **Identifiers** | Android: `app.pocketbridge.bridge` / iOS: `app.pocketbridge.client` |
| **Color Palette** | Deep Slate `#0F172A` (headers/dark bg), Signal Emerald `#10B981` (connected/E2EE), Amber `#F59E0B` (reconnecting/warning), Crimson `#EF4444` (error/disconnected), Warm Off-White `#F8FAFC` (light surface) |
| **Typography** | English (LTR): SF Pro (iOS) / Roboto or Inter (Android). Persian (RTL): **Vazirmatn** (bundled in both apps). |
| **Visual Motif** | Two minimalist rounded rectangles connected by an encrypted wave/bridge icon. Utility-first aesthetic (inspired by Signal / LocalSend). |
| **Localization** | Bilingual v1.0: English (`en`, LTR) and Persian (`fa`, full RTL layout mirroring + localized OEM battery guides). |

---

## 3. Repository Bootstrap & Target Layout

### 3.1 Quick Bootstrap Script
Run this locally with a fresh GitHub token (never commit tokens to Git) and the GitHub CLI (`gh`):

```bash
#!/usr/bin/env bash
set -euo pipefail

export GH_TOKEN="YOUR_FRESH_GITHUB_TOKEN"
REPO_NAME="pocketbridge"

mkdir -p "$REPO_NAME"/{.github/workflows,.github/ISSUE_TEMPLATE,protocol/proto,core/src,core/tests,android,ios,docs/fa,scripts}
cd "$REPO_NAME"

git init -b main
# Add initial files (README, LICENSE, SPEC, proto, this blueprint), then:
# git add . && git commit -m "chore: initial PocketBridge monorepo scaffold"
# gh repo create "$REPO_NAME" --public --source=. --remote=origin --push
```

### 3.2 Target Monorepo Structure
```text
pocketbridge/
├── .github/
│   ├── workflows/
│   │   ├── core-ci.yml             # Rust fmt, clippy, cargo test
│   │   ├── android-ci.yml          # Gradle lint, unit tests, debug APK
│   │   ├── ios-ci.yml              # XcodeGen + xcodebuild test on macOS
│   │   └── release.yml             # Tag-triggered signed APK, SHA256SUMS, AltStore JSON
│   └── ISSUE_TEMPLATE/
│       ├── device-compatibility.yml
│       └── bug-report.yml
├── protocol/
│   ├── LICENSE                     # Apache-2.0
│   ├── SPEC.md                     # Wire protocol specification
│   └── proto/
│       └── pocketbridge.proto      # Protobuf v3 schema
├── core/                           # Shared Rust library (UniFFI)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs                  # UniFFI exports
│   │   ├── framing.rs              # 6-byte GATT chunker & reassembler
│   │   ├── noise.rs                # Noise_XX handshake & transport session
│   │   ├── audio_crypto.rs         # HKDF key derivation & L2CAP AEAD framing
│   │   └── pairing.rs              # pbridge://pair URI builder & parser
│   └── tests/
│       └── integration_tests.rs
├── android/                        # Kotlin / Jetpack Compose (Android 12+)
│   ├── build.gradle.kts
│   ├── settings.gradle.kts
│   └── app/src/main/java/app/pocketbridge/bridge/
│       ├── ble/                    # GattServerManager, L2capServer, BleAdvertiser
│       ├── service/                # BridgeForegroundService
│       ├── telephony/              # SmsSender, SmsReceiver, BridgeInCallService
│       ├── audio/                  # AudioBridgeStrategy (Tiers 1-4), OpusBridge, SelfTest
│       ├── data/                   # EncryptedOfflineQueue (SQLCipher), KeyStoreRepo
│       └── ui/                     # Compose UI (Onboarding, QR, Status, Diagnostics)
├── ios/                            # Swift / SwiftUI (iOS 17+)
│   ├── project.yml                 # XcodeGen manifest
│   └── PocketBridge/
│       ├── BLE/                    # BluetoothCentralManager, L2capAudioStream
│       ├── CallKit/                # CallKitManager, AudioEngineManager, JitterBuffer
│       ├── Storage/                # EncryptedStore (SQLCipher), KeychainRepo
│       └── UI/                     # SwiftUI tabs (Dashboard, Messages, Dialer, Recents, Contacts)
├── docs/
│   ├── POCKETBRIDGE_ALL_IN_ONE.md
│   ├── compatibility.md
│   └── fa/
├── scripts/
│   ├── gen-proto.sh
│   └── build-uniffi.sh
├── LICENSE                         # GPL-3.0
└── README.md
```

---

## 4. Locked Technical Stack

All cryptography, chunking, QR URI parsing, and audio packet AEAD framing live in a **shared Rust core (`core/`)** compiled into native Kotlin and Swift bindings via **UniFFI**. This eliminates cross-platform crypto drift and allows deterministic CI testing without physical phones.

| Layer | Technology | Key Libraries / Frameworks |
|---|---|---|
| **Shared Core (`core/`)** | Rust (Edition 2021) | `snow` (`Noise_XX_25519_ChaChaPoly_SHA256`), `chacha20poly1305`, `hkdf`, `sha2`, `rand_core`, `zeroize`, `uniffi`, `base64`, `thiserror` |
| **Serialization (`protocol/`)** | Protocol Buffers v3 | `protobuf-javalite` (Android), `swift-protobuf` (iOS) |
| **Android Bridge (`android/`)** | Kotlin 2.x, Min SDK 31, Target SDK 35 | Jetpack Compose (Material 3), Coroutines/Flow, SQLCipher (`net.zetetic:sqlcipher-android`), ZXing (QR gen), `libopus` (NDK/JNI) |
| **iOS Client (`ios/`)** | Swift 6, Min iOS 17.0 | SwiftUI, CoreBluetooth, CallKit, AVFAudio (`AVAudioEngine`), AVFoundation (QR scan), SQLCipher, `libopus` XCFramework |

---

## 5. Complete BLE & E2EE Protocol Specification

### 5.1 Role Assignment
* **Bridge (Android 12+):** BLE GATT Server / Peripheral + L2CAP Listener. Holds the SIM(s), advertises the primary service UUID, and acts as the single source of truth for cellular call states.
* **Client (iOS 17+):** BLE GATT Client / Central + L2CAP Initiator. Uses `CBCentralManagerOptionRestoreIdentifierKey` for background wake, renders native CallKit UI, and stores synced SMS locally.

### 5.2 GATT Service & Characteristics
* **Primary Service UUID:** `8F2B0001-6A4E-4C1D-9B3A-1E5F7D2C8A01`

| Characteristic | UUID | Properties | Direction | Purpose |
|---|---|---|---|---|
| `CTRL_TX` | `8F2B0002-6A4E-4C1D-9B3A-1E5F7D2C8A01` | Notify | Bridge $\rightarrow$ Client | Encrypted Noise handshake & Protobuf control frames |
| `CTRL_RX` | `8F2B0003-6A4E-4C1D-9B3A-1E5F7D2C8A01` | Write, Write Without Response | Client $\rightarrow$ Bridge | Encrypted Noise handshake & Protobuf control frames |
| `L2CAP_PSM` | `8F2B0004-6A4E-4C1D-9B3A-1E5F7D2C8A01` | Read | Bridge $\rightarrow$ Client | 2-byte little-endian `uint16` dynamic PSM for voice L2CAP |
| `DEVICE_INFO` | `8F2B0005-6A4E-4C1D-9B3A-1E5F7D2C8A01` | Read | Bridge $\rightarrow$ Client | Unencrypted header: `[uint8 proto_major, uint8 proto_minor, uint16 feature_flags]` |

**`feature_flags` (`uint16` bitmask):**
* `0x0001`: L2CAP Voice Audio
* `0x0002`: Dual-SIM Selection
* `0x0004`: Incremental Contact Sync
* `0x0008`: Opus FEC / DTX

### 5.3 GATT Framing & Chunking (6-Byte Header)
Every encrypted message sent over `CTRL_TX` or `CTRL_RX` is segmented into MTU-sized chunks (`mtu - 3` bytes max per ATT packet, targeting 517-byte MTU).

```text
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|     Flags     |   Reserved    |          Message ID           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|          Chunk Index          |         Payload Bytes...      |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

* **`Flags` (1 byte):** `0x01` = `START`, `0x02` = `END` (`0x03` for single-chunk), `0x04` = `HANDSHAKE`.
* **`Message ID` (2 bytes, big-endian):** Monotonically incrementing per sender.
* **`Chunk Index` (2 bytes, big-endian):** Zero-based sequence index.
* **Hard Cap:** `65,535` bytes max reassembled message size; incomplete buffers evict on timeout.

### 5.4 QR Pairing & `Noise_XX` Handshake
1. **QR Code URI (displayed by Android Bridge):**
   ```text
   pbridge://pair?v=1&pub=<base64url(S_pub)>&adv=<8-char-hex-id>&n=<base64url(16-byte-nonce)>
   ```
   * `pub`: Bridge's 32-byte X25519 static public key ($S_{\text{pub}}$).
   * `adv`: Ephemeral 8-char hex ID placed in BLE Manufacturer Data to disambiguate nearby devices.
   * `n`: One-time 16-byte pairing nonce valid for **120 seconds**.
2. **Handshake (`Noise_XX_25519_ChaChaPoly_SHA256`, `HANDSHAKE` flag set):**
   * **Msg 1 (Client $\rightarrow$ Bridge):** `-> e`
   * **Msg 2 (Bridge $\rightarrow$ Client):** `<- e, ee, s, es` (payload echoes `nonce` on initial pair)
   * **Msg 3 (Client $\rightarrow$ Bridge):** `-> s, se` (payload carries Client device name)
3. **Pinning Rules:**
   * *Initial pair:* Client verifies Bridge's $s$ equals `pub` from QR and echoed nonce equals `n`. Bridge prompts user to tap **Approve** to pin Client's static key.
   * *Reconnects:* Both peers strictly reject any static key not matching their pinned peer key.
4. **Post-Handshake Transport:** Splits into unidirectional ChaCha20-Poly1305 cipher states ($C_{\text{c2b}}, C_{\text{b2c}}$). Each serialized `Envelope` Protobuf message is encrypted as one Noise transport message.

### 5.5 Voice Audio Pipeline (L2CAP CoC + Opus)
Opened only when a call enters `RINGING` or `ACTIVE` state.
* **Codec:** Opus (RFC 6716), 16,000 Hz wideband, 1 channel (mono), 20 ms frames (50 pkt/s), 14 kbps VBR (8–20 kbps), in-band FEC + DTX enabled.
* **Key Derivation:** Derive separate 32-byte send/receive L2CAP keys from the active Noise handshake hash $h$ via HKDF-SHA256 with `info = b"pocketbridge-audio-v1"`.
* **L2CAP Frame Format:**
  ```text
  [ uint16 frame_len ][ uint32 seq_num ][ ChaCha20-Poly1305(opus_frame) + 16-byte tag ]
  ```
* **Jitter Buffer:** Adaptive 40–120 ms buffer on both peers invoking Opus PLC on missing `seq_num`.

### 5.6 Reliability & Offline Queue
* **Heartbeat:** Client sends `Ping` every 30 s; Bridge replies with `Pong` + `DeviceStatus`.
* **Offline Queue:** Android persists undelivered incoming SMS and missed-call events in SQLCipher (up to 7 days) and flushes chronologically on reconnect.

---

## 6. Complete Protobuf v3 Schema (`protocol/proto/pocketbridge.proto`)

```protobuf
syntax = "proto3";

package pocketbridge.v1;

option java_package = "app.pocketbridge.protocol.v1";
option java_multiple_files = true;
option swift_prefix = "PB";

message Envelope {
  uint64 id = 1;
  int64 timestamp_ms = 2;

  oneof payload {
    Ping ping = 10;
    Pong pong = 11;
    DeviceStatus device_status = 12;
    Ack ack = 13;

    SmsMessage sms_received = 20;
    SendSmsRequest send_sms_request = 21;
    SendSmsResult send_sms_result = 22;
    SmsSyncRequest sms_sync_request = 23;
    SmsSyncBatch sms_sync_batch = 24;

    CallStateEvent call_state_event = 30;
    PlaceCallRequest place_call_request = 31;
    CallActionRequest call_action_request = 32;
    SendDtmfRequest send_dtmf_request = 33;

    ContactsSyncRequest contacts_sync_request = 40;
    ContactsSyncBatch contacts_sync_batch = 41;
    CallLogSyncRequest call_log_sync_request = 42;
    CallLogSyncBatch call_log_sync_batch = 43;
  }
}

message Ping { uint32 seq = 1; }
message Pong { uint32 seq = 1; }
message Ack { uint64 ref_envelope_id = 1; bool ok = 2; string error_message = 3; }

message SimInfo {
  int32 subscription_id = 1;
  int32 slot_index = 2;
  string carrier_name = 3;
  string phone_number = 4;
  int32 signal_level = 5; // 0 to 4
}

message DeviceStatus {
  int32 battery_percent = 1;
  bool is_charging = 2;
  repeated SimInfo sims = 3;
  string active_audio_strategy = 4;
}

enum SmsDirection {
  SMS_DIRECTION_UNSPECIFIED = 0;
  SMS_DIRECTION_INCOMING = 1;
  SMS_DIRECTION_OUTGOING = 2;
}

message SmsMessage {
  string message_id = 1;
  string thread_id = 2;
  string address = 3;
  string body = 4;
  int64 date_ms = 5;
  SmsDirection direction = 6;
  int32 subscription_id = 7;
  bool is_read = 8;
}

message SendSmsRequest {
  string client_temp_id = 1;
  string address = 2;
  string body = 3;
  int32 subscription_id = 4; // 0 = default SIM
}

message SendSmsResult {
  string client_temp_id = 1;
  bool success = 2;
  string message_id = 3;
  string error_reason = 4;
}

message SmsSyncRequest { int64 since_timestamp_ms = 1; uint32 max_messages = 2; }
message SmsSyncBatch { repeated SmsMessage messages = 1; bool has_more = 2; }

enum CallState {
  CALL_STATE_UNSPECIFIED = 0;
  CALL_STATE_DIALING = 1;
  CALL_STATE_RINGING = 2;
  CALL_STATE_ACTIVE = 3;
  CALL_STATE_HOLDING = 4;
  CALL_STATE_DISCONNECTED = 5;
}

enum CallDirection {
  CALL_DIRECTION_UNSPECIFIED = 0;
  CALL_DIRECTION_INCOMING = 1;
  CALL_DIRECTION_OUTGOING = 2;
}

enum CallAction {
  CALL_ACTION_UNSPECIFIED = 0;
  CALL_ACTION_ANSWER = 1;
  CALL_ACTION_REJECT = 2;
  CALL_ACTION_HANGUP = 3;
  CALL_ACTION_HOLD = 4;
  CALL_ACTION_UNHOLD = 5;
  CALL_ACTION_MUTE = 6;
  CALL_ACTION_UNMUTE = 7;
}

message CallStateEvent {
  string call_id = 1;
  string phone_number = 2;
  string contact_name = 3;
  CallState state = 4;
  CallDirection direction = 5;
  int32 subscription_id = 6;
  uint32 l2cap_psm = 7;
  string disconnect_reason = 8;
}

message PlaceCallRequest { string client_call_id = 1; string phone_number = 2; int32 subscription_id = 3; }
message CallActionRequest { string call_id = 1; CallAction action = 2; }
message SendDtmfRequest { string call_id = 1; string digit = 2; }

message Contact {
  string contact_id = 1;
  string display_name = 2;
  repeated string phone_numbers = 3;
  int64 updated_at_ms = 4;
}

message ContactsSyncRequest { int64 since_timestamp_ms = 1; }
message ContactsSyncBatch { repeated Contact contacts = 1; uint32 page_index = 2; uint32 total_pages = 3; }

enum CallLogType {
  CALL_LOG_TYPE_UNSPECIFIED = 0;
  CALL_LOG_TYPE_INCOMING = 1;
  CALL_LOG_TYPE_OUTGOING = 2;
  CALL_LOG_TYPE_MISSED = 3;
  CALL_LOG_TYPE_REJECTED = 4;
}

message CallLogEntry {
  string log_id = 1;
  string phone_number = 2;
  string contact_name = 3;
  CallLogType type = 4;
  int64 date_ms = 5;
  uint32 duration_seconds = 6;
  int32 subscription_id = 7;
}

message CallLogSyncRequest { int64 since_timestamp_ms = 1; uint32 limit = 2; }
message CallLogSyncBatch { repeated CallLogEntry entries = 1; bool has_more = 2; }
```

---

## 7. Platform Architecture Deep-Dives

### 7.1 Android Call Audio Routing (`AudioBridgeStrategy`)
Two-way cellular call audio capture/injection varies heavily across Android OEMs (Qualcomm, Exynos, MediaTek). Implement a pluggable `AudioBridgeStrategy` interface with a **30-second Audio Self-Test Wizard** that probes and locks the best tier:
1. **Tier 1 (`NativeVoiceCaptureStrategy`):** `AudioRecord`/`AudioTrack` via `VOICE_COMMUNICATION` / `VOICE_CALL` where firmware allows.
2. **Tier 2 (`BluetoothScoBridgeStrategy`):** Internal Bluetooth HFP/SCO routing where supported.
3. **Tier 3 (`ShizukuCaptureStrategy`):** Optional opt-in module using Shizuku (`CAPTURE_AUDIO_OUTPUT`) for high reliability on restrictive OEM ROMs without full root.
4. **Tier 4 (`AcousticFallbackStrategy`):** Low-volume speakerphone + mic loop with aggressive WebRTC AEC/NS (last resort, clearly labeled).

### 7.2 Android Background Survival
* Run a persistent **Foreground Service** (`connectedDevice` + `phoneCall` types).
* Prompt for `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` and show in-app OEM-specific autostart guides (Xiaomi HyperOS/MIUI, Samsung Sleeping Apps, Huawei/Honor).
* Tune BLE connection interval to low-power when idle and `CONNECTION_PRIORITY_HIGH` during active calls.

### 7.3 iOS Native Integration
* **CallKit (`CXProvider` + `CXCallController`):** Native lock-screen ringing, AirPods/CarPlay routing, and optional Recents logging.
* **Background Modes:** `bluetooth-central`, `voip`, and `audio` in `Info.plist`, with CoreBluetooth state restoration (`CBCentralManagerOptionRestoreIdentifierKey`).
* **Audio:** `AVAudioEngine` in `.voiceChat` mode (hardware AEC/NS) fed by the Opus decoder + adaptive jitter buffer.

---

## 8. Ordered Execution Backlog (Builder Agent & Dev Team)

Execute tasks strictly in order. A task is complete **only** when its verification command exits `0`.

### Task 0: Repository Hygiene & CI Workflows
* Add `LICENSE` (GPL-3.0), `protocol/LICENSE` (Apache-2.0), and `.gitignore` (Rust, Gradle, Xcode, generated bindings).
* Create `scripts/gen-proto.sh` and `.github/workflows/{core-ci.yml, android-ci.yml, ios-ci.yml}`.
* **Verify:** `bash -n scripts/gen-proto.sh` and validate all workflow YAMLs.

### Task 1: Shared Rust Core (`core/`)
* Implement `framing.rs` (6-byte header chunker/reassembler), `pairing.rs` (`pbridge://pair` builder/parser + 120 s expiry), `noise.rs` (`Noise_XX` initiator/responder + pinning), `audio_crypto.rs` (HKDF + L2CAP AEAD), and `lib.rs` (UniFFI exports).
* Write integration tests covering QR pair $\rightarrow$ Noise XX $\rightarrow$ chunked Protobuf $\rightarrow$ L2CAP audio round-trips, plus tampered ciphertext and wrong-key rejections.
* **Verify:** `cd core && cargo fmt --check && cargo clippy -- -D warnings && cargo test`

### Task 2: UniFFI & Protobuf Build Pipeline
* Create `scripts/build-uniffi.sh` supporting `--android` (`cargo-ndk` for `arm64-v8a`, `armeabi-v7a`) and `--ios` (`aarch64-apple-ios`, `aarch64-apple-ios-sim` XCFramework).
* Wire `scripts/gen-proto.sh` to generate Kotlin-lite and Swift Protobuf sources.
* **Verify:** `uniffi-bindgen` emits `.kt` and `.swift` bindings cleanly; `protoc` succeeds with zero errors.

### Task 3: Android Bridge MVP (`android/`) — BLE Peripheral & Pairing
* Scaffold Gradle Kotlin DSL project with Compose, Coroutines, Protobuf-javalite, SQLCipher, and required Bluetooth/Telephony/SMS permissions in `AndroidManifest.xml`.
* Implement `KeyStoreRepo`, `GattServerManager`, `BleAdvertiser`, and `BridgeForegroundService` wired to Rust `core/`.
* Build Compose UI: Permission Walkthrough, QR Pairing + "Approve Pairing" dialog, Status Dashboard, Diagnostics.
* Add JVM unit tests mocking GATT callbacks for handshake + `Ping`/`Pong` + `DeviceStatus`.
* **Verify:** `cd android && ./gradlew lintDebug testDebugUnitTest assembleDebug`

### Task 4: iOS Client MVP (`ios/`) — CoreBluetooth Central & Pairing
* Create `ios/project.yml` (XcodeGen) with `bluetooth-central`, `voip`, `audio` background modes and usage strings.
* Implement `KeychainRepo` and `BluetoothCentralManager` (with state restoration) wired to Rust `core/`.
* Build SwiftUI QR Scanner, Pairing screen, and Dashboard (connection badge, Android battery %, SIM signal bars).
* Add XCTest unit tests for Keychain mock, Envelope dispatch, and restoration logic.
* **Verify:** `xcodegen generate && xcodebuild test -scheme PocketBridge -destination 'platform=iOS Simulator,name=iPhone 16'`

### Task 5: Two-Way SMS, Contacts & Call Logs
* **Android:** `SmsReceiver`, dual-SIM `SmsSender`, `EncryptedOfflineQueue` (SQLCipher, 7-day retention), incremental `ContactsSyncProvider` & `CallLogSyncProvider`.
* **iOS:** `EncryptedStore` (SQLCipher), `UNUserNotificationCenter` local SMS alerts, and SwiftUI tabs (**Messages**, **Contacts**, **Recents**).
* **Verify:** Unit tests for offline queue ordering, dual-SIM SMS routing, and paginated sync pass on both platforms.

### Task 6: L2CAP Voice, CallKit & Android Audio Routing
* Integrate `libopus` (16 kHz mono, 20 ms, 14 kbps VBR, FEC + DTX) and adaptive 40–120 ms jitter buffer on both platforms.
* Implement L2CAP CoC server (`listenUsingInsecureL2capChannel()` on Android; `openL2CAPChannel(psm)` on iOS—encryption is enforced by our AEAD layer).
* Implement iOS `CallKitManager` + `AudioEngineManager` and Android `BridgeInCallService` + 4-tier `AudioBridgeStrategy` + Audio Self-Test Wizard.
* **Verify:** Unit tests for jitter buffer/PLC, CallKit state transitions, and L2CAP AEAD pass; both debug builds compile with zero warnings.

### Task 7: Bilingual Localization (EN + FA RTL), Hardening & Release CI
* Bundle **Vazirmatn** font; localize all strings to `en` and `fa` with full RTL mirroring.
* Create `docs/compatibility.md`, `.github/ISSUE_TEMPLATE/device-compatibility.yml`, and `.github/workflows/release.yml` (signed APK, `SHA256SUMS.txt`, `altstore-source.json`).
* **Verify:** All CI workflows pass green and release workflow dry-run succeeds.

---

## 9. Human-in-the-Loop (HITL) Physical Device Checkpoints

Pause and test on real hardware at these three gates:
1. **Checkpoint A (After Task 4):** Install both builds on physical Android + iOS devices. Verify QR scan, Noise XX approval, `DeviceStatus` updates, and automatic background reconnect after walking out of range.
2. **Checkpoint B (After Task 5):** Test live dual-SIM SMS send/receive with Wi-Fi/data off (cellular on Android only). Toggle Bluetooth off/on to verify offline queue flush.
3. **Checkpoint C (After Task 6):** Run the Android Audio Self-Test Wizard, place/answer live calls via iOS CallKit (lock screen + AirPods), and verify end-to-end latency (< 180 ms target) and echo cancellation.

---

## 10. Distribution, Risks & Agent Recovery Rules

### Distribution Channels
* **Android:** GitHub Releases (signed APK + SHA-256), Obtainium one-tap config, and IzzyOnDroid / F-Droid (avoiding Google Play's SMS/Call Log policy rejections).
* **iOS:** Apple App Store (framed accurately as a BLE companion utility with a 2-device hardware demo video), TestFlight beta, and official `.ipa` + `altstore-source.json` for AltStore/SideStore.

### Top Risks & Mitigations
| Risk | Mitigation |
|---|---|
| OEM blocks two-way call audio | 4-tier `AudioBridgeStrategy` + optional Shizuku module + crowdsourced `docs/compatibility.md` |
| App Store review rejection | Zero trademark overlap, zero private APIs, clear hardware demo video, official AltStore IPA fallback |
| Android kills background service | `connectedDevice` Foreground Service + battery exemption prompt + in-app OEM guides |
| BLE audio dropouts / battery drain | 20 ms Opus FEC/DTX + adaptive jitter buffer + dynamic BLE connection priority |

### Agent Error-Recovery Rules
* **Compilation/Test Failure:** Fix the root cause; never comment out tests or suppress warnings without explicit justification.
* **Protocol Ambiguity:** Update Section 5 / `protocol/SPEC.md` first, document the change in the commit, then implement in `core/`.