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
// This circuit proves that:
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
    signal notProtected;
    notProtected <== 1 - protectedFlag;

    // 6. landCoverCode must equal the publicly allowed code
    component landOk = IsEqual();
    landOk.in[0] <== landCoverCode;
    landOk.in[1] <== allowedLandCoverCode;

    // 7. supplier commitment: Poseidon(supplierId, supplierSecret) == supplierCommitment
    component commitHasher = Poseidon(2);
    commitHasher.inputs[0] <== supplierId;
    commitHasher.inputs[1] <== supplierSecret;
    component commitOk = IsEqual();
    commitOk.in[0] <== commitHasher.out;
    commitOk.in[1] <== supplierCommitment;

    // 8. evidence binding: Poseidon(latEnc, lonEnc, protectedFlag, landCoverCode) == evidenceHash
    component evidenceHasher = Poseidon(4);
    evidenceHasher.inputs[0] <== latEnc;
    evidenceHasher.inputs[1] <== lonEnc;
    evidenceHasher.inputs[2] <== protectedFlag;
    evidenceHasher.inputs[3] <== landCoverCode;
    component evidenceOk = IsEqual();
    evidenceOk.in[0] <== evidenceHasher.out;
    evidenceOk.in[1] <== evidenceHash;

    // ---- combine all conditions ----
    signal a1;
    signal a2;
    signal a3;
    signal a4;
    signal a5;
    signal a6;
    signal a7;

    a1 <== geLat.out * leLat.out;
    a2 <== a1 * geLon.out;
    a3 <== a2 * leLon.out;
    a4 <== a3 * qOk.out;
    a5 <== a4 * notProtected;
    a6 <== a5 * landOk.out;
    a7 <== a6 * commitOk.out;
    valid <== a7 * evidenceOk.out;

    // The circuit does not merely output "valid" as an informational signal -
    // it is CONSTRAINED to 1, so a witness that fails any condition cannot
    // produce a proof for a "valid == 1" public output at all: the proving
    // key generation below fixes valid=1 as the only satisfiable public
    // output for this circuit's intended use (see scripts/prove.sh which
    // fails proof generation, not just prints a warning, on invalid input).
    valid === 1;
}

// bits=32 is enough for: latEnc in [0, 180_000_000], lonEnc in [0, 360_000_000],
// quantity in [0, ~4.29e9] kg. See scripts/lib/encode.js for the exact
// fixed-point encoding used off-circuit.
component main {public [latMin, latMax, lonMin, lonMax, quantityThreshold, allowedLandCoverCode, supplierCommitment, evidenceHash]} = EnvironmentalCompliance(32);
