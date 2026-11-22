#!/usr/bin/env bash
# build-uniffi.sh — cross-compile the Rust core and emit UniFFI bindings.
#
# Usage:
#   bash scripts/build-uniffi.sh --android   # cargo-ndk: arm64-v8a + armeabi-v7a -> jniLibs + .kt bindings
#   bash scripts/build-uniffi.sh --ios       # XcodeGen targets: aarch64-apple-ios + simulator -> XCFramework + .swift bindings
#   bash scripts/build-uniffi.sh --host      # host cdylib + bindings (dev loop, no NDK/Xcode needed)
#
# Requirements (per flag):
#   --android: cargo-ndk + Android NDK (ANDROID_NDK_HOME / NDK_HOME)
#   --ios:     Xcode + `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`
#   --host:    plain cargo + uniffi-bindgen (bundled via core/src/bin)
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CORE_DIR="${ROOT_DIR}/core"
LIB_NAME="pocketbridge_core"
MODULE_NAME="pocketbridge"
BINDINGS_OUT_ANDROID="${ROOT_DIR}/android/app/src/main/java/uniffi"
BINDINGS_OUT_IOS="${ROOT_DIR}/ios/PocketBridge/Generated/Uniffi"

log() { printf '[build-uniffi] %s\n' "$*" >&2; }
die() { printf '[build-uniffi] ERROR: %s\n' "$*" >&2; exit 1; }

emit_bindings() {
  local target_kind="$1"   # kotlin | swift
  local lib_path="$2"
  local out_dir="$3"
  command -v cargo >/dev/null 2>&1 || die "cargo not found"
  case "${target_kind}" in
    kotlin)
      mkdir -p "${out_dir}"
      (cd "${CORE_DIR}" && cargo run --features bindgen-cli --bin uniffi-bindgen -- \
        generate --library "${lib_path}" --language kotlin --out-dir "${out_dir}") ||
        die "kotlin binding generation failed"
      ;;
    swift)
      mkdir -p "${out_dir}"
      (cd "${CORE_DIR}" && cargo run --features bindgen-cli --bin uniffi-bindgen -- \
        generate --library "${lib_path}" --language swift --out-dir "${out_dir}") ||
        die "swift binding generation failed"
      ;;
  esac
}

case "${1:-}" in
  --android)
    [[ -n "${ANDROID_NDK_HOME:-${NDK_HOME:-}}" ]] || die "ANDROID_NDK_HOME or NDK_HOME not set"
    command -v cargo-ndk >/dev/null 2>&1 || die "cargo-ndk not installed (cargo install cargo-ndk)"
    rustup target add aarch64-linux-android armv7-linux-androideabi 2>/dev/null || true

    for target in aarch64-linux-android armv7-linux-androideabi; do
      log "building ${target}"
      (cd "${CORE_DIR}" && cargo ndk -t "${target}" -o "${ROOT_DIR}/android/app/src/main/jniLibs" build --release) ||
        die "cargo ndk build failed for ${target}"
    done

    emit_bindings kotlin \
      "${CORE_DIR}/target/aarch64-linux-android/release/lib${LIB_NAME}.so" \
      "${BINDINGS_OUT_ANDROID}"
    log "android outputs: jniLibs/*/lib${LIB_NAME}.so + ${BINDINGS_OUT_ANDROID}/uniffi/${MODULE_NAME}.kt"
    ;;

  --ios)
    rustup target add aarch64-apple-ios aarch64-apple-ios-sim 2>/dev/null || true

    (cd "${CORE_DIR}" && cargo build --release --target aarch64-apple-ios) || die "iOS device build failed"
    (cd "${CORE_DIR}" && cargo build --release --target aarch64-apple-ios-sim) || die "iOS simulator build failed"

    emit_bindings swift \
      "${CORE_DIR}/target/aarch64-apple-ios/release/lib${LIB_NAME}.a" \
      "${BINDINGS_OUT_IOS}"

    FRAMEWORK_DIR="${ROOT_DIR}/ios/Frameworks"
    mkdir -p "${FRAMEWORK_DIR}"
    xcodebuild -create-xcframework \
      -library "${CORE_DIR}/target/aarch64-apple-ios/release/lib${LIB_NAME}.a" \
      -headers "${BINDINGS_OUT_IOS}" \
      -library "${CORE_DIR}/target/aarch64-apple-ios-sim/release/lib${LIB_NAME}.a" \
      -headers "${BINDINGS_OUT_IOS}" \
      -output "${FRAMEWORK_DIR}/${LIB_NAME}.xcframework" || die "xcframework creation failed"
    log "ios outputs: ${FRAMEWORK_DIR}/${LIB_NAME}.xcframework + ${BINDINGS_OUT_IOS}"
    ;;

  --host)
    log "building host cdylib"
    (cd "${CORE_DIR}" && cargo build) || die "host build failed"
    lib_ext="$(uname -s | grep -qi darwin && echo dylib || echo so)"
    emit_bindings kotlin "${CORE_DIR}/target/debug/lib${LIB_NAME}.${lib_ext}" "${BINDINGS_OUT_ANDROID}/host"
    emit_bindings swift "${CORE_DIR}/target/debug/lib${LIB_NAME}.${lib_ext}" "${BINDINGS_OUT_IOS}/host"
    ;;

  --bindings-only)
    # Regenerate both binding sets from an existing host build.
    (cd "${CORE_DIR}" && cargo build) || die "host build failed"
    lib_ext="$(uname -s | grep -qi darwin && echo dylib || echo so)"
    emit_bindings kotlin "${CORE_DIR}/target/debug/lib${LIB_NAME}.${lib_ext}" "${BINDINGS_OUT_ANDROID}"
    emit_bindings swift "${CORE_DIR}/target/debug/lib${LIB_NAME}.${lib_ext}" "${BINDINGS_OUT_IOS}"
    ;;

  *)
    echo "usage: $0 --android | --ios | --host | --bindings-only" >&2
    exit 1
    ;;
esac
log "done"
