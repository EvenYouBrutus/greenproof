pragma circom 2.1.6;

// =============================================================================
// GreenProof - environmental_compliance.circom
// =============================================================================
//
// WHAT THIS CIRCUIT DOES AND DOES NOT PROVE (read before trusting this proof)
// -----------------------------------------------------------------------------
// This circuit does NOT independently verify satellite imagery, forest
// boundaries, or protected-area polygons. Arbitrary polygon intersection is
// not something we implement inside a SNARK circuit for this MVP - it is
// computed OUTSIDE the circuit, once, by a real geospatial lookup against
// real public datasets (Nominatim / OpenStreetMap Overpass "protected area"
// tags; see backend/src/geo.rs and README "Real data sources").
//
// The off-chain lookup produces an "evidence report":
//   { latEnc, lonEnc, protectedFlag, landCoverCode, source metadata... }
//
// This circuit evaluates whether:
//   1. The supplier's PRIVATE coordinates fall inside a PUBLIC bounding box
//      (the declared "supported geographic region", e.g. Ghana/Cote d'Ivoire
//      cocoa belt bounds).
//   2. The PRIVATE (protectedFlag, landCoverCode) witnessed here are exactly
//      the ones the off-chain evidence report contains, because the circuit
//      recomputes Poseidon(latEnc, lonEnc, protectedFlag, landCoverCode) and
//      constrains it equal to a PUBLIC evidenceHash. The auditor is given
//      the plaintext evidence report (minus exact coordinates) and the
//      dataset provenance, and independently recomputes evidenceHash from
//      the disclosed protectedFlag/landCoverCode + the supplier's committed
//      latEnc/lonEnc to confirm the proof is bound to the real, disclosed
//      evidence, not an arbitrary one.
//   3. protectedFlag == 0 (not inside a detected protected area).
//   4. landCoverCode == an allowed public code (from the real land-cover
//      classification scheme in use).
//   5. quantity <= a public threshold.
//   6. Poseidon(supplierId, supplierSecret) == a public supplier commitment.
//
// Rather than rejecting non-compliant inputs, the circuit outputs the evaluation
// results as public signals (e.g. regionOk, valid), enabling verifiers to
// explicitly see which checks passed or failed.
//
// This circuit trusts that the off-chain evidence report faithfully reflects
// the real datasets queried at query time (see README "Threat model" and
// "Trust assumptions"). It proves internal CONSISTENCY between the private
// witness and the disclosed/committed evidence, plus the arithmetic
// conditions above - it is not a magical guarantee that the real world
// matches the data.
// =============================================================================

include "circomlib/circuits/poseidon.circom";
include "circomlib/circuits/comparators.circom";

template EnvironmentalCompliance(bits) {
    // ---- private inputs (witness only, never disclosed) ----
    signal input latEnc;            // latitude, shifted+scaled to a non-negative integer off-circuit
    signal input lonEnc;            // longitude, shifted+scaled to a non-negative integer off-circuit
    signal input quantity;          // production quantity, kg
    signal input supplierId;
    signal input supplierSecret;
    signal input protectedFlag;     // 0 or 1, from the real off-chain protected-area lookup
    signal input landCoverCode;     // integer land-cover class, from the real off-chain lookup

    // ---- public inputs ----
    signal input latMin;
    signal input latMax;
    signal input lonMin;
    signal input lonMax;
    signal input quantityThreshold;
    signal input allowedLandCoverCode;
    signal input supplierCommitment;
    signal input evidenceHash;

    // ---- output ----
    signal output regionOk;
    signal output protectedAreaOk;
    signal output landCoverOk;
    signal output quantityOk;
    signal output supplierOk;
    signal output evidenceOk;
    signal output valid;

    // 1. protectedFlag must be boolean
    protectedFlag * (protectedFlag - 1) === 0;

    // 2. latitude bounding box: latMin <= latEnc <= latMax
    component geLat = GreaterEqThan(bits);
    geLat.in[0] <== latEnc;
    geLat.in[1] <== latMin;

    component leLat = LessEqThan(bits);
    leLat.in[0] <== latEnc;
    leLat.in[1] <== latMax;

    // 3. longitude bounding box: lonMin <= lonEnc <= lonMax
    component geLon = GreaterEqThan(bits);
    geLon.in[0] <== lonEnc;
    geLon.in[1] <== lonMin;

    component leLon = LessEqThan(bits);
    leLon.in[0] <== lonEnc;
    leLon.in[1] <== lonMax;

    // 4. quantity <= quantityThreshold
    component qOk = LessEqThan(bits);
    qOk.in[0] <== quantity;
    qOk.in[1] <== quantityThreshold;

    // 5. protectedFlag must be exactly 0 (not in a detected protected area)

    // 6. landCoverCode must equal the publicly allowed code
    component landOk = IsEqual();
    landOk.in[0] <== landCoverCode;
    landOk.in[1] <== allowedLandCoverCode;

    // 7. supplier commitment: Poseidon(supplierId, supplierSecret) == supplierCommitment
    component commitHasher = Poseidon(2);
    commitHasher.inputs[0] <== supplierId;
    commitHasher.inputs[1] <== supplierSecret;
    component commitEq = IsEqual();
    commitEq.in[0] <== commitHasher.out;
    commitEq.in[1] <== supplierCommitment;

    // 8. evidence binding: Poseidon(latEnc, lonEnc, protectedFlag, landCoverCode) == evidenceHash
    component evidenceHasher = Poseidon(4);
    evidenceHasher.inputs[0] <== latEnc;
    evidenceHasher.inputs[1] <== lonEnc;
    evidenceHasher.inputs[2] <== protectedFlag;
    evidenceHasher.inputs[3] <== landCoverCode;
    component evidenceEq = IsEqual();
    evidenceEq.in[0] <== evidenceHasher.out;
    evidenceEq.in[1] <== evidenceHash;

    // ---- compute sub-check results ----
    // Region: 4-way AND via bilinear intermediate signals
    signal a1;
    signal a2;
    signal a3;
    signal a4;
    signal a5;
    signal r1;
    signal r2;

    r1 <== geLat.out * leLat.out;
    r2 <== r1 * geLon.out;
    regionOk <== r2 * leLon.out;
    
    protectedAreaOk <== 1 - protectedFlag;
    landCoverOk <== landOk.out;
    quantityOk <== qOk.out;
    supplierOk <== commitEq.out;
    evidenceOk <== evidenceEq.out;

    a1 <== regionOk * protectedAreaOk;
    a2 <== a1 * landCoverOk;
    a3 <== a2 * quantityOk;
    a4 <== a3 * supplierOk;
    valid <== a4 * evidenceOk;

    // The circuit outputs "valid" and sub-check flags as informational signals.
    // It DOES NOT constrain valid to 1, meaning that even non-compliant inputs
    // will produce a valid proof (but with valid=0 and specific sub-flags=0),
    // allowing the verifier to explicitly inspect the evaluation result.
}

// bits=32 is enough for: latEnc in [0, 180_000_000], lonEnc in [0, 360_000_000],
// quantity in [0, ~4.29e9] kg. See scripts/lib/encode.js for the exact
// fixed-point encoding used off-circuit.
component main {public [latMin, latMax, lonMin, lonMax, quantityThreshold, allowedLandCoverCode, supplierCommitment, evidenceHash]} = EnvironmentalCompliance(32);
