# PocketBridge Build Checkpoints

Execution log of the ordered backlog from `docs/POCKETBRIDGE_ALL_IN_ONE.md`.
A task is complete only when its verification command exits `0`.

| Task | Scope | Status | Verified by | Commit |
|---|---|---|---|---|
| 0 | Repository hygiene & CI workflows | ✅ | `bash -n scripts/gen-proto.sh`, YAML parse of all workflows, `gen-proto.sh --check` | `chore(task0): ...` |
| 1 | Shared Rust core | ⏳ | `cargo fmt --check && cargo clippy -- -D warnings && cargo test` | — |
| 2 | UniFFI & Protobuf pipeline | ⏳ | `uniffi-bindgen` .kt/.swift emit; `protoc` zero errors | — |
| 3 | Android bridge MVP | ⏳ | `./gradlew lintDebug testDebugUnitTest assembleDebug` | — |
| 4 | iOS client MVP | ⏳ | `xcodegen generate && xcodebuild test` | — |
| 5 | Two-way SMS, contacts, call logs | ⏳ | unit tests both platforms | — |
| 6 | L2CAP voice, CallKit, audio routing | ⏳ | jitter/PLC, CallKit, L2CAP AEAD tests | — |
| 7 | EN + FA localization, hardening | ⏳ | all CI green, release dry-run | — |

## Human-in-the-Loop physical device gates

- **Checkpoint A (after Task 4):** QR scan, Noise XX approval, DeviceStatus updates, background
  reconnect after walking out of range — on physical Android + iOS hardware.
- **Checkpoint B (after Task 5):** live dual-SIM SMS send/receive with Wi-Fi/data off; offline
  queue flush after Bluetooth toggle.
- **Checkpoint C (after Task 6):** Audio Self-Test Wizard, live CallKit calls (lock screen +
  AirPods), end-to-end latency < 180 ms, echo cancellation.
