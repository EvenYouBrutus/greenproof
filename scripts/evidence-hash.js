#!/usr/bin/env node
// Canonical bridge between the Rust evidence service and the existing
// Circom Poseidon(4) evidence binding. Output is deliberately one decimal
// field element so callers cannot accidentally parse display text as data.
const { poseidon4 } = require("poseidon-lite");

const [lat, lon, protectedFlag, landCoverCode] = process.argv.slice(2);
if ([lat, lon, protectedFlag, landCoverCode].some((v) => v === undefined || !/^\d+$/.test(v))) {
  console.error("Usage: node evidence-hash.js <latEnc> <lonEnc> <protectedFlag> <landCoverCode>");
  process.exit(1);
}
console.log(poseidon4([BigInt(lat), BigInt(lon), BigInt(protectedFlag), BigInt(landCoverCode)]).toString());
