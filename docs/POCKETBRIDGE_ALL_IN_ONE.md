# PocketBridge — All-in-One Project Blueprint (build copy)

This is the working copy of the project blueprint used to bootstrap this repository.
It is kept in `docs/` as the historical reference for the v1.0 scope: brand identity,
locked stack, BLE + Noise E2EE protocol, platform architecture deep-dives, ordered
execution backlog, HITL checkpoints, distribution channels, and risk register.

For the live documents, see:

- [`protocol/SPEC.md`](../protocol/SPEC.md) — wire protocol specification
- [`compatibility.md`](compatibility.md) — device compatibility matrix and OEM notes
- [`../README.md`](../README.md) — build instructions and repository layout

## Ordered execution backlog (tracked in CHECKPOINTS.md)

| Task | Scope | Verification |
|---|---|---|
| 0 | Repo hygiene & CI workflows | `bash -n scripts/gen-proto.sh` + workflow YAML validation |
| 1 | Shared Rust core | `cargo fmt --check && cargo clippy -- -D warnings && cargo test` |
| 2 | UniFFI & Protobuf pipeline | `uniffi-bindgen` emits bindings; `protoc` zero errors |
| 3 | Android bridge MVP | `./gradlew lintDebug testDebugUnitTest assembleDebug` |
| 4 | iOS client MVP | `xcodegen generate && xcodebuild test` |
| 5 | SMS / contacts / call logs | unit tests on both platforms |
| 6 | L2CAP voice + CallKit + audio routing | jitter/PLC, CallKit, L2CAP AEAD tests |
| 7 | EN/FA localization + hardening | all CI workflows green, release dry-run |
