// Manual runner for circuit.test.js's assertions, for environments without
// mocha/circom_tester installed (no network to `npm install`). Same circuit,
// same real Groth16 artifacts, same 13 cases - just run without a test
// framework. Prefer `npx mocha test/circuit.test.js` when npm install is
// available; this file exists purely as a sandbox workaround.
const assert = require("assert");
const fs = require("fs");
const path = require("path");
const snarkjs = require("snarkjs");
const { poseidon2, poseidon4 } = require("poseidon-lite");
const { encodeLat, encodeLon, boundingBoxEnc } = require("../lib/encode");

const BUILD_DIR = path.join(__dirname, "..", "..", "circuits", "build");
const WASM = path.join(BUILD_DIR, "environmental_compliance_js", "environmental_compliance.wasm");
const ZKEY = path.join(BUILD_DIR, "environmental_compliance_final.zkey");
const VKEY = JSON.parse(fs.readFileSync(path.join(BUILD_DIR, "verification_key.json"), "utf8"));

const REGION = { minLat: 4.0, maxLat: 11.5, minLon: -8.6, maxLon: 1.5 };
const THRESHOLD = 5000;
const ALLOWED_LAND_COVER = 40;

function strToField(s) {
  let x = 0n;
  for (const ch of Buffer.from(String(s), "utf8")) x = (x * 256n + BigInt(ch)) % 2n ** 200n;
  return x;
}

function buildInput({ lat, lon, qty, supplierId, supplierSecret, protectedFlag, landCoverCode, region, threshold, allowedLandCover, evidenceHashOverride, commitmentOverride }) {
  const latEnc = encodeLat(lat);
  const lonEnc = encodeLon(lon);
  const bbox = boundingBoxEnc(region.minLat, region.maxLat, region.minLon, region.maxLon);
  const idF = strToField(supplierId);
  const secF = strToField(supplierSecret);
  const commitment = commitmentOverride ?? poseidon2([idF, secF]);
  const evidenceHash = evidenceHashOverride ?? poseidon4([latEnc, lonEnc, BigInt(protectedFlag), BigInt(landCoverCode)]);
  return {
    latEnc: latEnc.toString(), lonEnc: lonEnc.toString(), quantity: qty.toString(),
    supplierId: idF.toString(), supplierSecret: secF.toString(),
    protectedFlag: protectedFlag.toString(), landCoverCode: landCoverCode.toString(),
    latMin: bbox.latMin.toString(), latMax: bbox.latMax.toString(),
    lonMin: bbox.lonMin.toString(), lonMax: bbox.lonMax.toString(),
    quantityThreshold: threshold.toString(), allowedLandCoverCode: allowedLandCover.toString(),
    supplierCommitment: commitment.toString(), evidenceHash: evidenceHash.toString(),
  };
}

async function expectProveFails(input) {
  await assert.rejects(() => snarkjs.groth16.fullProve(input, WASM, ZKEY));
}

const base = {
  lat: 6.6666, lon: -1.6163, qty: 1200,
  supplierId: "supplier-42", supplierSecret: "correct-horse-battery-staple",
  protectedFlag: 0, landCoverCode: ALLOWED_LAND_COVER,
  region: REGION, threshold: THRESHOLD, allowedLandCover: ALLOWED_LAND_COVER,
};

const tests = [
  ["1. valid coordinate + valid everything -> proof generates and verifies", async () => {
    const input = buildInput(base);
    const { proof, publicSignals } = await snarkjs.groth16.fullProve(input, WASM, ZKEY);
    assert.strictEqual(await snarkjs.groth16.verify(VKEY, publicSignals, proof), true);
  }],
  ["2. invalid latitude (>90) is rejected before circuit input construction", async () => {
    assert.throws(() => encodeLat(120));
  }],
  ["3. invalid longitude (>180) is rejected before circuit input construction", async () => {
    assert.throws(() => encodeLon(220));
  }],
  ["4. protected-area location (protectedFlag=1) -> proof generation fails", async () => {
    await expectProveFails(buildInput({ ...base, protectedFlag: 1 }));
  }],
  ["5. non-protected location -> witness satisfies that condition", async () => {
    const input = buildInput({ ...base, protectedFlag: 0 });
    const { proof, publicSignals } = await snarkjs.groth16.fullProve(input, WASM, ZKEY);
    assert.strictEqual(await snarkjs.groth16.verify(VKEY, publicSignals, proof), true);
  }],
  ["6. production <= threshold -> passes", async () => {
    const input = buildInput({ ...base, qty: THRESHOLD });
    const { proof, publicSignals } = await snarkjs.groth16.fullProve(input, WASM, ZKEY);
    assert.strictEqual(await snarkjs.groth16.verify(VKEY, publicSignals, proof), true);
  }],
  ["7. production > threshold -> proof generation fails", async () => {
    await expectProveFails(buildInput({ ...base, qty: THRESHOLD + 1 }));
  }],
  ["8. valid supplier commitment -> passes", async () => {
    const input = buildInput(base);
    const { proof, publicSignals } = await snarkjs.groth16.fullProve(input, WASM, ZKEY);
    assert.strictEqual(await snarkjs.groth16.verify(VKEY, publicSignals, proof), true);
  }],
  ["9. invalid supplier commitment -> proof generation fails", async () => {
    await expectProveFails(buildInput({ ...base, commitmentOverride: 123456789n }));
  }],
  ["10. valid ZK proof verifies", async () => {
    const input = buildInput(base);
    const { proof, publicSignals } = await snarkjs.groth16.fullProve(input, WASM, ZKEY);
    assert.strictEqual(await snarkjs.groth16.verify(VKEY, publicSignals, proof), true);
  }],
  ["11. modified/tampered proof fails verification", async () => {
    const input = buildInput(base);
    const { proof, publicSignals } = await snarkjs.groth16.fullProve(input, WASM, ZKEY);
    const tampered = JSON.parse(JSON.stringify(proof));
    tampered.pi_a[0] = (BigInt(tampered.pi_a[0]) + 1n).toString();
    assert.strictEqual(await snarkjs.groth16.verify(VKEY, publicSignals, tampered), false);
  }],
  ["12. modified public signals fail verification", async () => {
    const input = buildInput(base);
    const { proof, publicSignals } = await snarkjs.groth16.fullProve(input, WASM, ZKEY);
    const tampered = [...publicSignals];
    tampered[0] = (BigInt(tampered[0]) + 1n).toString();
    assert.strictEqual(await snarkjs.groth16.verify(VKEY, tampered, proof), false);
  }],
  ["13. auditor-facing artifacts contain no private data", async () => {
    const input = buildInput(base);
    const { proof, publicSignals } = await snarkjs.groth16.fullProve(input, WASM, ZKEY);
    const serialized = JSON.stringify({ proof, publicSignals });
    assert.ok(!serialized.includes(input.latEnc));
    assert.ok(!serialized.includes(input.lonEnc));
    assert.ok(!serialized.includes(input.supplierId));
    assert.ok(!serialized.includes(input.supplierSecret));
  }],
];

(async () => {
  let pass = 0, fail = 0;
  for (const [name, fn] of tests) {
    const start = Date.now();
    try {
      await fn();
      console.log(`  \u2713 ${name} (${Date.now() - start}ms)`);
      pass++;
    } catch (e) {
      console.log(`  \u2717 ${name}`);
      console.log(`      ${e.message}`);
      fail++;
    }
  }
  console.log(`\n${pass} passing, ${fail} failing`);
  process.exit(fail > 0 ? 1 : 0);
})();
