#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CORE_DIR="${ROOT_DIR}/core"
MODE="${1:-all}"

build_android() {
  echo "Building Rust core for Android (arm64-v8a, armeabi-v7a)..."
  mkdir -p "${ROOT_DIR}/android/app/src/main/jniLibs" "${ROOT_DIR}/android/app/src/main/java"
  (
    cd "${CORE_DIR}"
    cargo ndk -t arm64-v8a -t armeabi-v7a -o "${ROOT_DIR}/android/app/src/main/jniLibs" build --release
    cargo run --bin uniffi-bindgen generate       --library target/aarch64-linux-android/release/libpocketbridge_core.so       --language kotlin       --out-dir "${ROOT_DIR}/android/app/src/main/java"
  )
}

build_ios() {
  echo "Building Rust core XCFramework for iOS..."
  mkdir -p "${ROOT_DIR}/ios/Generated" "${ROOT_DIR}/ios/Frameworks"
  (
    cd "${CORE_DIR}"
    cargo build --release --target aarch64-apple-ios
    cargo build --release --target aarch64-apple-ios-sim
    cargo run --bin uniffi-bindgen generate       --library target/aarch64-apple-ios/release/libpocketbridge_core.a       --language swift       --out-dir "${ROOT_DIR}/ios/Generated"
    rm -rf "${ROOT_DIR}/ios/Frameworks/PocketBridgeCore.xcframework"
    xcodebuild -create-xcframework       -library target/aarch64-apple-ios/release/libpocketbridge_core.a -headers "${ROOT_DIR}/ios/Generated"       -library target/aarch64-apple-ios-sim/release/libpocketbridge_core.a -headers "${ROOT_DIR}/ios/Generated"       -output "${ROOT_DIR}/ios/Frameworks/PocketBridgeCore.xcframework"
  )
}

case "${MODE}" in
  --android) build_android ;;
  --ios) build_ios ;;
  all)
    build_android
    build_ios
    ;;
  *)
    echo "Usage: $0 [--android|--ios|all]" >&2
    exit 1
    ;;
esac
