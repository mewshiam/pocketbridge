#!/usr/bin/env bash
# gen-proto.sh — generate Kotlin-lite and Swift Protobuf sources from protocol/proto/pocketbridge.proto
#
# Usage:
#   bash scripts/gen-proto.sh              # detect and generate for all available targets
#   bash scripts/gen-proto.sh --android     # Kotlin (javalite) only
#   bash scripts/gen-proto.sh --ios         # Swift only
#   bash scripts/gen-proto.sh --check       # verify protoc + proto file only, no output dirs
#
# Requires: protoc (>= 3.19) on PATH plus
#   - Android: protoc-gen-javalite (protoc plugin for lite runtime)
#   - iOS:     protoc-gen-swift   (from swift-protobuf)
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROTO_FILE="${ROOT_DIR}/protocol/proto/pocketbridge.proto"
PROTO_OUT_ANDROID="${ROOT_DIR}/android/app/src/main/java"
PROTO_OUT_IOS="${ROOT_DIR}/ios/PocketBridge/Generated/Proto"

log() { printf '[gen-proto] %s\n' "$*" >&2; }
die() { printf '[gen-proto] ERROR: %s\n' "$*" >&2; exit 1; }

# Locate protoc: native protoc on PATH, else the grpcio-tools shim.
PROTOC_BIN=""
if command -v protoc >/dev/null 2>&1; then
  PROTOC_BIN="$(command -v protoc)"
elif command -v python-grpc-tools-protoc >/dev/null 2>&1; then
  PROTOC_BIN="$(command -v python-grpc-tools-protoc)"
  log "using grpcio-tools protoc shim at ${PROTOC_BIN}"
fi
[[ -n "${PROTOC_BIN}" ]] || die "protoc not found on PATH (install protobuf-compiler or grpcio-tools)"
"${PROTOC_BIN}" --version >&2 || die "protoc --version failed"

MODE="all"
case "${1:-}" in
  --android) MODE="android" ;;
  --ios) MODE="ios" ;;
  --check) MODE="check" ;;
  --help|-h) echo "usage: $0 [--android|--ios|--check]"; exit 0 ;;
  "") MODE="all" ;;
  *) die "unknown option: $1 (expected --android, --ios or --check)" ;;
esac

if [[ "$MODE" == "check" ]]; then
  log "syntax-checking ${PROTO_FILE}"
  "${PROTOC_BIN}" -I "${ROOT_DIR}/protocol/proto" --descriptor_set_out=/dev/null "${PROTO_FILE}" ||
    die "proto file failed syntax check"
  log "check OK"
  exit 0
fi

if [[ "$MODE" == "android" || "$MODE" == "all" ]]; then
  if command -v protoc-gen-javalite >/dev/null 2>&1 || [[ -x "${ROOT_DIR}/scripts/bin/protoc-gen-javalite" ]]; then
    PLUG_JAVALITE="${ROOT_DIR}/scripts/bin/protoc-gen-javalite"
    command -v protoc-gen-javalite >/dev/null 2>&1 && PLUG_JAVALITE="$(command -v protoc-gen-javalite)"
    mkdir -p "${PROTO_OUT_ANDROID}"
    log "generating Kotlin (javalite) sources -> ${PROTO_OUT_ANDROID}"
    "${PROTOC_BIN}" -I "${ROOT_DIR}/protocol/proto" \
      --plugin=protoc-gen-javalite="${PLUG_JAVALITE}" \
      --javalite_out="${PROTO_OUT_ANDROID}" \
      "${PROTO_FILE}" || die "javalite generation failed"
  else
    log "SKIP android (protoc-gen-javalite not installed)"
  fi
fi

if [[ "$MODE" == "ios" || "$MODE" == "all" ]]; then
  if command -v protoc-gen-swift >/dev/null 2>&1; then
    mkdir -p "${PROTO_OUT_IOS}"
    log "generating Swift sources -> ${PROTO_OUT_IOS}"
    "${PROTOC_BIN}" -I "${ROOT_DIR}/protocol/proto" \
      --plugin="$(command -v protoc-gen-swift)" \
      --swift_out="${PROTO_OUT_IOS}" \
      "${PROTO_FILE}" || die "swift generation failed"
  else
    log "SKIP ios (protoc-gen-swift not installed)"
  fi
fi

log "done"
