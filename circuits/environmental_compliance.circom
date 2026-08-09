pragma circom 2.1.6;

// GreenProof environmental compliance circuit.
//
// The environmental provider is external to the SNARK. The backend queries
// real environmental data, normalizes it deterministically, and commits to
// the result. The circuit proves that the private coordinate and environmental
// witness are bound to that backend-issued commitment and that the committed
// forest-loss result satisfies the public cutoff policy.
//
// It does NOT prove that the external dataset is truthful or complete.

include "circomlib/circuits/poseidon.circom";
include "circomlib/circuits/comparators.circom";

// Private witness:
//   latEnc, lonEnc, quantity, supplierId, supplierSecret,
//   protectedFlag, landCoverCode, firstLossYearAfterCutoff
//
// Public policy/evidence:
//   region bounds, quantity threshold, allowed land-cover code,
//   cutoffYear, supplier commitment, evidence hash.
template EnvironmentalCompliance(bits) {
    signal input latEnc;
    signal input lonEnc;
    signal input quantity;
    signal input supplierId;
    signal input supplierSecret;
    signal input protectedFlag;
    signal input landCoverCode;
    signal input firstLossYearAfterCutoff;

    signal input latMin;
    signal input latMax;
    signal input lonMin;
    signal input lonMax;
    signal input quantityThreshold;
    signal input allowedLandCoverCode;
    signal input cutoffYear;
    signal input supplierCommitment;
    signal input evidenceHash;

    signal output regionOk;
    signal output protectedAreaOk;
    signal output landCoverOk;
    signal output quantityOk;
    signal output supplierOk;
    signal output forestLossOk;
    signal output evidenceOk;
    signal output valid;

    protectedFlag * (protectedFlag - 1) === 0;
    firstLossYearAfterCutoff * (firstLossYearAfterCutoff - 0) === firstLossYearAfterCutoff * firstLossYearAfterCutoff;

    component geLat = GreaterEqThan(bits);
    geLat.in[0] <== latEnc;
    geLat.in[1] <== latMin;
    component leLat = LessEqThan(bits);
    leLat.in[0] <== latEnc;
    leLat.in[1] <== latMax;

    component geLon = GreaterEqThan(bits);
    geLon.in[0] <== lonEnc;
    geLon.in[1] <== lonMin;
    component leLon = LessEqThan(bits);
    leLon.in[0] <== lonEnc;
    leLon.in[1] <== lonMax;

    component qOk = LessEqThan(bits);
    qOk.in[0] <== quantity;
    qOk.in[1] <== quantityThreshold;

    component landOk = IsEqual();
    landOk.in[0] <== landCoverCode;
    landOk.in[1] <== allowedLandCoverCode;

    component commitHasher = Poseidon(2);
    commitHasher.inputs[0] <== supplierId;
    commitHasher.inputs[1] <== supplierSecret;
    component commitEq = IsEqual();
    commitEq.in[0] <== commitHasher.out;
    commitEq.in[1] <== supplierCommitment;

    // Backend encodes "no detected loss after cutoff" as firstLossYear=0.
    // If a loss exists after the cutoff, firstLossYear is its earliest year.
    // Therefore forestLossOk = NOT(firstLossYear > cutoffYear).
    component lossAfterCutoff = GreaterThan(bits);
    lossAfterCutoff.in[0] <== firstLossYearAfterCutoff;
    lossAfterCutoff.in[1] <== cutoffYear;
    forestLossOk <== 1 - lossAfterCutoff.out;

    // Bind all environmental facts, including the loss-year result, to the
    // public evidence commitment. The exact coordinates remain private.
    component evidenceHasher = Poseidon(5);
    evidenceHasher.inputs[0] <== latEnc;
    evidenceHasher.inputs[1] <== lonEnc;
    evidenceHasher.inputs[2] <== protectedFlag;
    evidenceHasher.inputs[3] <== landCoverCode;
    evidenceHasher.inputs[4] <== firstLossYearAfterCutoff;
    component evidenceEq = IsEqual();
    evidenceEq.in[0] <== evidenceHasher.out;
    evidenceEq.in[1] <== evidenceHash;

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

    signal c1;
    signal c2;
    signal c3;
    signal c4;
    signal c5;
    signal c6;
    c1 <== regionOk * protectedAreaOk;
    c2 <== c1 * landCoverOk;
    c3 <== c2 * quantityOk;
    c4 <== c3 * supplierOk;
    c5 <== c4 * forestLossOk;
    valid <== c5 * evidenceOk;
}

component main {public [latMin, latMax, lonMin, lonMax, quantityThreshold, allowedLandCoverCode, cutoffYear, supplierCommitment, evidenceHash]} = EnvironmentalCompliance(32);
