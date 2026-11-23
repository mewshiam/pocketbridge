#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROTO_DIR="${ROOT_DIR}/protocol/proto"
PROTO_FILE="${PROTO_DIR}/pocketbridge.proto"

if ! command -v protoc >/dev/null 2>&1; then
  echo "error: protoc is not installed. Install protobuf compiler first." >&2
  exit 1
fi

ANDROID_OUT="${ROOT_DIR}/android/app/src/main/java"
IOS_OUT="${ROOT_DIR}/ios/Generated"

mkdir -p "${ANDROID_OUT}" "${IOS_OUT}"

echo "Generating Java-lite Protobuf bindings for Android..."
protoc   -I "${PROTO_DIR}"   --java_out="lite:${ANDROID_OUT}"   "${PROTO_FILE}"

if command -v protoc-gen-swift >/dev/null 2>&1; then
  echo "Generating Swift Protobuf bindings for iOS..."
  protoc     -I "${PROTO_DIR}"     --swift_out="${IOS_OUT}"     --swift_opt=Visibility=Public     "${PROTO_FILE}"
else
  echo "warning: protoc-gen-swift not found; skipping Swift Protobuf generation." >&2
fi

echo "Protobuf generation completed."
