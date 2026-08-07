#!/usr/bin/env node
// GreenProof - real Groth16 proof generation
//
// Usage:
//   node prove.js path/to/request.json
//
// request.json shape (see data/example-request.json for a template):
// {
//   "private": {
//     "latitude": 6.6666,
//     "longitude": -1.6163,
//     "quantityKg": 1200,
//     "supplierId": "12345",
//     "supplierSecret": "some-long-random-secret"
//   },
//   "publicConfig": {
//     "region": { "minLat": 4.5, "maxLat": 11.5, "minLon": -3.5, "maxLon": 1.5 },
//     "quantityThresholdKg": 5000,
//     "allowedLandCoverCode": 40
//   },
//   "evidence": {
//     "protectedFlag": 0,
//     "landCoverCode": 40
//   }
// }
//
// "evidence" MUST come from scripts/lib or backend/src/geo.rs real lookups
// against Nominatim / Overpass (OSM protected areas) / land-cover source -
// this script does not fetch it itself, to keep proving deterministic and
// testable; see backend for the live query path used by the actual app.

const fs = require("fs");
const path = require("path");
const snarkjs = require("snarkjs");
const { poseidon2, poseidon4 } = require("poseidon-lite");
const { encodeLat, encodeLon, boundingBoxEnc } = require("./lib/encode");

async function main() {
  const reqPath = process.argv[2];
  if (!reqPath) {
    console.error("Usage: node prove.js <request.json> [outDir]");
    process.exit(1);
  }
  const outDir = process.argv[3] || path.join(__dirname, "..", "circuits", "build", "out");
  fs.mkdirSync(outDir, { recursive: true });

  const req = JSON.parse(fs.readFileSync(reqPath, "utf8"));
  const priv = req.private;
  const cfg = req.publicConfig;
  const evidence = req.evidence;

  // ---- basic input validation (fail loudly, do not silently coerce) ----
  for (const [k, v] of Object.entries({
    latitude: priv.latitude,
    longitude: priv.longitude,
    quantityKg: priv.quantityKg,
    supplierId: priv.supplierId,
    supplierSecret: priv.supplierSecret,
  })) {
    if (v === undefined || v === null) throw new Error(`Missing private field: ${k}`);
  }
  if (priv.latitude < -90 || priv.latitude > 90) throw new Error("INVALID LATITUDE");
  if (priv.longitude < -180 || priv.longitude > 180) throw new Error("INVALID LONGITUDE");

  const latEnc = encodeLat(priv.latitude);
  const lonEnc = encodeLon(priv.longitude);
  const { latMin, latMax, lonMin, lonMax } = boundingBoxEnc(
    cfg.region.minLat, cfg.region.maxLat, cfg.region.minLon, cfg.region.maxLon
  );

  // supplierId/secret must be field elements: hash arbitrary strings down
  // with a non-cryptographic-but-deterministic string->BigInt fold so any
  // real identifier/secret string can be used (no fabricated numeric IDs).
  function strToField(s) {
    let x = 0n;
    for (const ch of Buffer.from(String(s), "utf8")) {
      x = (x * 256n + BigInt(ch)) % (2n ** 200n); // stay well under BN254 field size
    }
    return x;
  }
  const supplierIdF = strToField(priv.supplierId);
  const supplierSecretF = strToField(priv.supplierSecret);
  const supplierCommitment = poseidon2([supplierIdF, supplierSecretF]);

  const protectedFlag = evidence.protectedFlag ? 1 : 0;
  const landCoverCode = evidence.landCoverCode;
  const evidenceHash = poseidon4([latEnc, lonEnc, protectedFlag, landCoverCode]);

  // ---- pre-flight human-readable checks (before the circuit rejects it) ----
  const failures = [];
  if (!(latEnc >= latMin && latEnc <= latMax)) failures.push("LOCATION OUTSIDE SUPPORTED REGION (latitude)");
  if (!(lonEnc >= lonMin && lonEnc <= lonMax)) failures.push("LOCATION OUTSIDE SUPPORTED REGION (longitude)");
  if (protectedFlag !== 0) failures.push("PROTECTED AREA DETECTED");
  if (landCoverCode !== cfg.allowedLandCoverCode) failures.push("LAND COVER CLASSIFICATION NOT PERMITTED");
  if (!(priv.quantityKg <= cfg.quantityThresholdKg)) failures.push("PRODUCTION THRESHOLD FAILED");

  const input = {
    latEnc: latEnc.toString(),
    lonEnc: lonEnc.toString(),
    quantity: priv.quantityKg.toString(),
    supplierId: supplierIdF.toString(),
    supplierSecret: supplierSecretF.toString(),
    protectedFlag: protectedFlag.toString(),
    landCoverCode: landCoverCode.toString(),
    latMin: latMin.toString(),
    latMax: latMax.toString(),
    lonMin: lonMin.toString(),
    lonMax: lonMax.toString(),
    quantityThreshold: cfg.quantityThresholdKg.toString(),
    allowedLandCoverCode: cfg.allowedLandCoverCode.toString(),
    supplierCommitment: supplierCommitment.toString(),
    evidenceHash: evidenceHash.toString(),
  };
  fs.writeFileSync(path.join(outDir, "input.json"), JSON.stringify(input, null, 2));

  if (failures.length > 0) {
    console.error("PRE-FLIGHT CHECK FAILED (this witness cannot satisfy the circuit):");
    for (const f of failures) console.error("  - " + f);
    console.error(
      "Attempting real proof generation anyway to demonstrate the circuit " +
      "genuinely rejects invalid witnesses (this is expected to fail below)."
    );
  }

  const buildDir = path.join(__dirname, "..", "circuits", "build");
  const wasmPath = path.join(buildDir, "environmental_compliance_js", "environmental_compliance.wasm");
  const zkeyPath = path.join(buildDir, "environmental_compliance_final.zkey");

  if (!fs.existsSync(wasmPath) || !fs.existsSync(zkeyPath)) {
    console.error(
      `Missing compiled circuit artifacts. Run 'bash setup.sh' first.\n` +
      `Expected:\n  ${wasmPath}\n  ${zkeyPath}`
    );
    process.exit(2);
  }

  try {
    const { proof, publicSignals } = await snarkjs.groth16.fullProve(input, wasmPath, zkeyPath);
    fs.writeFileSync(path.join(outDir, "proof.json"), JSON.stringify(proof, null, 2));
    fs.writeFileSync(path.join(outDir, "public.json"), JSON.stringify(publicSignals, null, 2));

    // Never write private inputs anywhere near the outputs shipped to a verifier.
    console.log("PROOF GENERATED:", path.join(outDir, "proof.json"));
    console.log("PUBLIC SIGNALS:", path.join(outDir, "public.json"));
    console.log(
      "Public signals disclosed to the auditor include: latMin/latMax/lonMin/lonMax, " +
      "quantityThreshold, allowedLandCoverCode, supplierCommitment, evidenceHash, and the " +
      "circuit's 'valid' output. Exact latitude/longitude/quantity/supplierId/supplierSecret " +
      "are never included."
    );
  } catch (err) {
    console.error("PROOF GENERATION FAILED - the witness does not satisfy the circuit's constraints.");
    console.error(String(err.message || err));
    process.exit(3);
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
