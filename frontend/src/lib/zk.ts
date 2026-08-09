// GreenProof - browser-side proof generation.
//
// Per the architecture principle "private data should not need to leave the
// browser/local machine if it can be processed locally", the supplier's
// private witness (exact lat/lon, quantity, supplierId, supplierSecret)
// stays in this module's local variables and is fed straight into snarkjs's
// WASM witness calculator + Groth16 prover running in the browser. Only the
// resulting proof + public signals ever leave this function.
//
// The circuit outputs the EVALUATION RESULT (compliant or not) rather than
// rejecting non-compliant inputs. A proof is always generated; the public
// signals include individual check outcomes (regionOk, protectedAreaOk,
// landCoverOk, quantityOk, supplierOk, evidenceOk) so that verifiers can
// see which checks passed/failed without learning the private inputs.
//
// Requires the compiled circuit artifacts to be present at
// /circuit/environmental_compliance.wasm and /circuit/circuit_final.zkey
// (served as static files - see scripts/copy-artifacts-to-frontend.sh).

// @ts-expect-error - snarkjs ships without types
import * as snarkjs from "snarkjs";
import { poseidon2, poseidon4 } from "poseidon-lite";
import { boundingBoxEnc, encodeLat, encodeLon, strToField } from "./encode";

export interface RegionConfig {
  minLat: number;
  maxLat: number;
  minLon: number;
  maxLon: number;
}

export interface ProveParams {
  latitude: number;
  longitude: number;
  quantityKg: number;
  supplierId: string;
  supplierSecret: string;
  region: RegionConfig;
  quantityThresholdKg: number;
  allowedLandCoverCode: number;
  evidenceProtectedFlag: boolean;
  evidenceLandCoverCode: number;
  // Calculated by the backend from its own environmental lookup. Passing this
  // through preserves the existing Poseidon/Circom relation while preventing
  // the client from inventing a replacement commitment at verification time.
  evidenceHash: string;
}

/** Individual check results parsed from the circuit's public output signals. */
export interface ComplianceChecks {
  valid: boolean;
  regionOk: boolean;
  protectedAreaOk: boolean;
  landCoverOk: boolean;
  quantityOk: boolean;
  supplierOk: boolean;
  evidenceOk: boolean;
}

export interface ProveResult {
  proof: unknown;
  publicSignals: string[];
  /** Pre-flight human-readable failure reasons (informational). */
  preflightFailures: string[];
  /** Whether all compliance checks passed (derived from circuit output). */
  compliant: boolean;
  /** Individual check results from the circuit's public outputs. */
  checks: ComplianceChecks;
}

/**
 * Parse the circuit's public output signals into individual check results.
 *
 * Public signals layout (outputs first, then public inputs):
 *   [0] valid, [1] regionOk, [2] protectedAreaOk, [3] landCoverOk,
 *   [4] quantityOk, [5] supplierOk, [6] evidenceOk,
 *   [7..14] public inputs (latMin, latMax, lonMin, lonMax,
 *           quantityThreshold, allowedLandCoverCode, supplierCommitment, evidenceHash)
 */
export function parseComplianceChecks(publicSignals: string[]): ComplianceChecks {
  return {
    valid: publicSignals[0] === "1",
    regionOk: publicSignals[1] === "1",
    protectedAreaOk: publicSignals[2] === "1",
    landCoverOk: publicSignals[3] === "1",
    quantityOk: publicSignals[4] === "1",
    supplierOk: publicSignals[5] === "1",
    evidenceOk: publicSignals[6] === "1",
  };
}

export async function generateProof(params: ProveParams): Promise<ProveResult> {
  const latEnc = encodeLat(params.latitude);
  const lonEnc = encodeLon(params.longitude);
  const { latMin, latMax, lonMin, lonMax } = boundingBoxEnc(
    params.region.minLat,
    params.region.maxLat,
    params.region.minLon,
    params.region.maxLon
  );

  const supplierIdF = strToField(params.supplierId);
  const supplierSecretF = strToField(params.supplierSecret);
  const supplierCommitment = poseidon2([supplierIdF, supplierSecretF]);

  const protectedFlag = params.evidenceProtectedFlag ? 1n : 0n;
  const landCoverCode = BigInt(params.evidenceLandCoverCode);
  const locallyComputedEvidenceHash = poseidon4([latEnc, lonEnc, protectedFlag, landCoverCode]);
  if (locallyComputedEvidenceHash.toString() !== params.evidenceHash) {
    throw new Error("Environmental evidence commitment does not match the private coordinate and normalized claim. Run the lookup again.");
  }

  const preflightFailures: string[] = [];
  if (!(latEnc >= latMin && latEnc <= latMax)) preflightFailures.push("LOCATION OUTSIDE SUPPORTED REGION (latitude)");
  if (!(lonEnc >= lonMin && lonEnc <= lonMax)) preflightFailures.push("LOCATION OUTSIDE SUPPORTED REGION (longitude)");
  if (protectedFlag !== 0n) preflightFailures.push("PROTECTED AREA DETECTED");
  if (Number(landCoverCode) !== params.allowedLandCoverCode) preflightFailures.push("LAND COVER CLASSIFICATION NOT PERMITTED");
  if (!(params.quantityKg <= params.quantityThresholdKg)) preflightFailures.push("PRODUCTION THRESHOLD FAILED");

  const input = {
    latEnc: latEnc.toString(),
    lonEnc: lonEnc.toString(),
    quantity: params.quantityKg.toString(),
    supplierId: supplierIdF.toString(),
    supplierSecret: supplierSecretF.toString(),
    protectedFlag: protectedFlag.toString(),
    landCoverCode: landCoverCode.toString(),
    latMin: latMin.toString(),
    latMax: latMax.toString(),
    lonMin: lonMin.toString(),
    lonMax: lonMax.toString(),
    quantityThreshold: params.quantityThresholdKg.toString(),
    allowedLandCoverCode: params.allowedLandCoverCode.toString(),
    supplierCommitment: supplierCommitment.toString(),
    evidenceHash: params.evidenceHash,
  };

  // The circuit now always succeeds: it outputs the evaluation result
  // rather than rejecting non-compliant witnesses. Proof generation
  // will only fail for truly invalid inputs (e.g., wrong evidence hash).
  const { proof, publicSignals } = await snarkjs.groth16.fullProve(
    input,
    "/circuit/environmental_compliance.wasm",
    "/circuit/circuit_final.zkey"
  );

  const checks = parseComplianceChecks(publicSignals);

  return { proof, publicSignals, preflightFailures, compliant: checks.valid, checks };
}

export async function verifyProofLocally(
  proof: unknown,
  publicSignals: string[]
): Promise<boolean> {
  const vkeyResp = await fetch("/circuit/verification_key.json");
  const vkey = await vkeyResp.json();
  return snarkjs.groth16.verify(vkey, publicSignals, proof);
}
