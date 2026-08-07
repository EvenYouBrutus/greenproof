#!/usr/bin/env node
// GreenProof - real Groth16 proof verification.
//
// This IS the verification implementation used in the running app: the Rust
// backend (backend/src/verify.rs) invokes this script as a subprocess for
// every /api/verify-proof call, rather than re-implementing the BN254
// pairing check in native Rust (see backend/src/verify.rs module docs for
// why - short version: a hand-rolled vkey-format conversion we can't test
// here would be riskier than being explicit about using snarkjs directly).
// `snarkjs.groth16.verify` performs a real Groth16 pairing check, not a
// mock - a tampered proof or public signal will fail it.
//
// Usage: node verify.js <proof.json> <public.json> [verification_key.json]

const fs = require("fs");
const path = require("path");
const snarkjs = require("snarkjs");

async function main() {
  const [proofPath, publicPath, vkPathArg] = process.argv.slice(2);
  if (!proofPath || !publicPath) {
    console.error("Usage: node verify.js <proof.json> <public.json> [verification_key.json]");
    process.exit(1);
  }
  const vkPath = vkPathArg || path.join(__dirname, "..", "circuits", "build", "verification_key.json");

  const proof = JSON.parse(fs.readFileSync(proofPath, "utf8"));
  const publicSignals = JSON.parse(fs.readFileSync(publicPath, "utf8"));
  const vk = JSON.parse(fs.readFileSync(vkPath, "utf8"));

  const ok = await snarkjs.groth16.verify(vk, publicSignals, proof);

  console.log("GREENPROOF VERIFICATION");
  console.log("ZK proof cryptographic check:", ok ? "VALID" : "INVALID");
  console.log("Private coordinates disclosed: NO");
  console.log("Private quantity disclosed: NO");
  console.log("Supplier secret disclosed: NO");
  console.log("Public signals used:", publicSignals);

  process.exit(ok ? 0 : 4);
}

main().catch((e) => {
  console.error("Verification error:", e);
  process.exit(1);
});
