#!/usr/bin/env node
// Canonical bridge between the Rust evidence service and Circom Poseidon(5).
// The fifth value is the normalized first detected forest-loss year after the
// selected cutoff, or 0 when no loss was detected.
const { poseidon5 } = require("poseidon-lite");

const [lat, lon, protectedFlag, landCoverCode, firstLossYear] = process.argv.slice(2);
if ([lat, lon, protectedFlag, landCoverCode, firstLossYear].some((v) => v === undefined || !/^\d+$/.test(v))) {
  console.error("Usage: node evidence-hash.js <latEnc> <lonEnc> <protectedFlag> <landCoverCode> <firstLossYear>");
  process.exit(1);
}
console.log(poseidon5([BigInt(lat), BigInt(lon), BigInt(protectedFlag), BigInt(landCoverCode), BigInt(firstLossYear)]).toString());
