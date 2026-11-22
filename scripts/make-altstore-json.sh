#!/usr/bin/env bash
# make-altstore-json.sh <tag> — emits altstore-source.json for AltStore/SideStore.
set -euo pipefail
TAG="${1:?usage: make-altstore-json.sh <tag>}"
VERSION="${TAG#v}"
cat <<JSON
{
  "name": "PocketBridge",
  "identifier": "app.pocketbridge.client",
  "version": ${VERSION:-1},
  "apps": [{
    "name": "PocketBridge",
    "bundleIdentifier": "app.pocketbridge.client",
    "developerName": "PocketBridge contributors",
    "version": "${VERSION}",
    "versions": [],
    "localizations": {},
    "tintColor": "10B981"
  }]
}
JSON
