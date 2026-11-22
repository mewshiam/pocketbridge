#!/usr/bin/env bash
# make-release-keystore.sh — generate the PocketBridge release keystore.
# The keystore itself is NEVER committed (gitignored); commit the base64 to
# the GitHub secret PB_KEYSTORE_BASE64 and the password to PB_KEYSTORE_PASSWORD.
set -euo pipefail
KS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../android/keystore" && mkdir -p . && pwd)"
KS_FILE="${KS_DIR}/pocketbridge-release.keystore"
if [[ -e "${KS_FILE}" ]]; then echo "refusing to overwrite ${KS_FILE}" >&2; exit 1; fi
keytool -genkeypair -v \
  -keystore "${KS_FILE}" -alias pocketbridge \
  -keyalg RSA -keysize 2048 -validity 10000 \
  -storepass "${KEYSTORE_PASSWORD:?set KEYSTORE_PASSWORD}" \
  -keypass "${KEYSTORE_PASSWORD}" \
  -dname "CN=PocketBridge, OU=Open Source, O=PocketBridge Contributors, L=Berlin, C=DE"
echo "keystore written to ${KS_FILE}"
echo "base64 -w0 ${KS_FILE}  -> GitHub secret PB_KEYSTORE_BASE64"
