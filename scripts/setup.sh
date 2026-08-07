#!/usr/bin/env bash
# GreenProof - circuit compilation + Groth16 trusted setup
#
# Requires network access and these tools installed locally:
#   - circom >= 2.1.6   (https://docs.circom.io/getting-started/installation/)
#   - node >= 18 with npm
# Run once from the scripts/ directory:  bash setup.sh
set -euo pipefail

cd "$(dirname "$0")"
ROOT_DIR="$(cd .. && pwd)"
CIRCUITS_DIR="$ROOT_DIR/circuits"
BUILD_DIR="$ROOT_DIR/circuits/build"
mkdir -p "$BUILD_DIR"

echo "==> Installing script dependencies (snarkjs, circomlib)"
npm install --no-audit --no-fund

echo "==> Vendoring circomlib into circuits/ so 'include \"circomlib/...\"' resolves"
mkdir -p "$CIRCUITS_DIR/circomlib"
cp -r node_modules/circomlib/circuits/* "$CIRCUITS_DIR/circomlib/"

echo "==> Compiling environmental_compliance.circom"
circom "$CIRCUITS_DIR/environmental_compliance.circom" \
  --r1cs --wasm --sym \
  -l "$CIRCUITS_DIR" \
  -o "$BUILD_DIR"

echo "==> Circuit stats"
npx snarkjs r1cs info "$BUILD_DIR/environmental_compliance.r1cs"

# ---- Groth16 trusted setup (Powers of Tau, MVP-scale ceremony) ----
# NOTE: for a real production deployment this ceremony must involve multiple
# independent, mutually-distrusting contributors (a real multi-party
# computation "Powers of Tau" ceremony). For this hackathon MVP we run a
# single local contribution, which is NOT trustworthy for production use.
# This limitation is documented in README "Security model / Trust
# assumptions" and must not be silently glossed over.
#
# Domain size: this circuit currently has ~1,458 constraints, which needs a
# power-of-two domain of 2^11 (2048). We use 2^12 (4096) for headroom as the
# circuit grows. A much larger degree (e.g. 2^15) does still produce a valid
# ceremony, but snarkjs's pure-JS Groth16 setup reads/processes the *entire*
# ptau file regardless of how few constraints the circuit actually has, so an
# oversized degree here makes `groth16 setup` take many minutes for no
# benefit - keep this matched to the circuit's real size.
PTAU_DEGREE=12
PTAU="$BUILD_DIR/pot${PTAU_DEGREE}_final.ptau"
if [ ! -f "$PTAU" ]; then
  echo "==> Powers of Tau phase 1 (local, MVP-only - see note above)"
  npx snarkjs powersoftau new bn128 "$PTAU_DEGREE" "$BUILD_DIR/pot${PTAU_DEGREE}_0000.ptau" -v
  npx snarkjs powersoftau contribute "$BUILD_DIR/pot${PTAU_DEGREE}_0000.ptau" "$BUILD_DIR/pot${PTAU_DEGREE}_0001.ptau" \
    --name="greenproof mvp contribution" -v -e="$(date +%s)-$RANDOM"
  npx snarkjs powersoftau prepare phase2 "$BUILD_DIR/pot${PTAU_DEGREE}_0001.ptau" "$PTAU" -v
fi

echo "==> Phase 2 (circuit-specific) setup"
npx snarkjs groth16 setup "$BUILD_DIR/environmental_compliance.r1cs" "$PTAU" \
  "$BUILD_DIR/environmental_compliance_0000.zkey"
npx snarkjs zkey contribute "$BUILD_DIR/environmental_compliance_0000.zkey" \
  "$BUILD_DIR/environmental_compliance_final.zkey" \
  --name="greenproof mvp contribution" -v -e="$(date +%s)-$RANDOM"

echo "==> Exporting verification key"
npx snarkjs zkey export verificationkey "$BUILD_DIR/environmental_compliance_final.zkey" \
  "$BUILD_DIR/verification_key.json"

echo "==> Done. Artifacts in $BUILD_DIR:"
ls -la "$BUILD_DIR"
