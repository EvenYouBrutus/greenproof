// GreenProof - browser-side proof generation.
//
// Per the architecture principle "private data should not need to leave the
// browser/local machine if it can be processed locally", the supplier's
// private witness (exact lat/lon, quantity, supplierId, supplierSecret)
// stays in this module's local variables and is fed straight into snarkjs's
// WASM witness calculator + Groth16 prover running in the browser. Only the
// resulting proof + public signals ever leave this function.
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

export interface ProveResult {
  proof: unknown;
  publicSignals: string[];
  preflightFailures: string[];
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

  // If pre-flight already failed, we still attempt the real proof so the
  // demo can show the cryptographic rejection - but we surface the human
  // reasons immediately rather than only a cryptic circuit assertion error.
  const { proof, publicSignals } = await snarkjs.groth16.fullProve(
    input,
    "/circuit/environmental_compliance.wasm",
    "/circuit/circuit_final.zkey"
  );

  return { proof, publicSignals, preflightFailures };
}

export async function verifyProofLocally(
  proof: unknown,
  publicSignals: string[]
): Promise<boolean> {
  const vkeyResp = await fetch("/circuit/verification_key.json");
  const vkey = await vkeyResp.json();
  return snarkjs.groth16.verify(vkey, publicSignals, proof);
}
