#!/usr/bin/env bash
# Copies compiled circuit artifacts (produced by setup.sh) into
# frontend/public/circuit/ so the browser can fetch them for local,
# private, in-browser proof generation via snarkjs.
set -euo pipefail
cd "$(dirname "$0")"
ROOT_DIR="$(cd .. && pwd)"
BUILD_DIR="$ROOT_DIR/circuits/build"
DEST="$ROOT_DIR/frontend/public/circuit"

mkdir -p "$DEST"
cp "$BUILD_DIR/environmental_compliance_js/environmental_compliance.wasm" "$DEST/"
cp "$BUILD_DIR/environmental_compliance_final.zkey" "$DEST/circuit_final.zkey"
cp "$BUILD_DIR/verification_key.json" "$DEST/"

echo "Copied circuit artifacts to $DEST"
