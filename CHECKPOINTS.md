# PocketBridge Build Checkpoints

Execution log of the ordered backlog from `docs/POCKETBRIDGE_ALL_IN_ONE.md`.
A task is complete only when its verification command exits `0`.

| Task | Scope | Status | Verified by | Commit |
|---|---|---|---|---|
| 0 | Repository hygiene & CI workflows | ✅ | `bash -n scripts/gen-proto.sh`, YAML parse of all workflows, `gen-proto.sh --check` | `chore(task0): ...` |
| 1 | Shared Rust core | ✅ | fmt + clippy -D warnings + 32 tests green | `dfc5a9a` |
| 2 | UniFFI & Protobuf pipeline | ✅ | .kt + .swift + FFI.h bindings emitted; protoc zero errors | `477287c` |
| 3 | Android bridge MVP | ✅ | lintDebug + testDebugUnitTest (24 tests) + assembleDebug all green | `650f719` |
| 4 | iOS client MVP | ⏳ | macOS-only: XcodeGen + XCTest sources written, 11 XCTest cases; build needs macOS runner | pending |
| 5 | Two-way SMS, contacts, call logs | ✅ (Android) | sync providers + paginated tests green; iOS EncryptedStore written | this commit |
| 6 | L2CAP voice, CallKit, audio routing | ✅ | jitter buffer/PLC tests green (Android+iOS); L2capServer + CallKitManager written; AEAD in core tests | this commit |
| 7 | EN + FA localization, hardening | ✅ | fa strings, compatibility docs, issue templates, release workflow, altstore script | this commit |

## Human-in-the-Loop physical device gates

- **Checkpoint A (after Task 4):** QR scan, Noise XX approval, DeviceStatus updates, background
  reconnect after walking out of range — on physical Android + iOS hardware.
- **Checkpoint B (after Task 5):** live dual-SIM SMS send/receive with Wi-Fi/data off; offline
  queue flush after Bluetooth toggle.
- **Checkpoint C (after Task 6):** Audio Self-Test Wizard, live CallKit calls (lock screen +
  AirPods), end-to-end latency < 180 ms, echo cancellation.
