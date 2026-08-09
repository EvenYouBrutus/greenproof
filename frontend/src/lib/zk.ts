// Browser-side Groth16 proof generation. Private coordinates, quantity,
// supplier ID and secret remain local. Environmental evidence is issued by
// the backend and checked against a Poseidon(5) commitment before proving.
//
// Public signals: 8 circuit outputs + 9 public inputs.
// [0] valid, [1] regionOk, [2] protectedAreaOk, [3] landCoverOk,
// [4] quantityOk, [5] supplierOk, [6] forestLossOk, [7] evidenceOk,
// [8..16] policy + supplier commitment + evidence hash.

// @ts-expect-error - snarkjs ships without types
import * as snarkjs from "snarkjs";
import { poseidon2, poseidon5 } from "poseidon-lite";
import { boundingBoxEnc, encodeLat, encodeLon, strToField } from "./encode";

export interface RegionConfig { minLat:number; maxLat:number; minLon:number; maxLon:number; }
export interface ProveParams { latitude:number; longitude:number; quantityKg:number; supplierId:string; supplierSecret:string; region:RegionConfig; quantityThresholdKg:number; allowedLandCoverCode:number; cutoffYear:number; evidenceProtectedFlag:boolean; evidenceLandCoverCode:number; evidenceFirstLossYearAfterCutoff:number; evidenceHash:string; }
export interface ComplianceChecks { valid:boolean; regionOk:boolean; protectedAreaOk:boolean; landCoverOk:boolean; quantityOk:boolean; supplierOk:boolean; forestLossOk:boolean; evidenceOk:boolean; }
export interface ProveResult { proof:unknown; publicSignals:string[]; preflightFailures:string[]; compliant:boolean; checks:ComplianceChecks; }

export function parseComplianceChecks(publicSignals:string[]):ComplianceChecks{return{valid:publicSignals[0]==="1",regionOk:publicSignals[1]==="1",protectedAreaOk:publicSignals[2]==="1",landCoverOk:publicSignals[3]==="1",quantityOk:publicSignals[4]==="1",supplierOk:publicSignals[5]==="1",forestLossOk:publicSignals[6]==="1",evidenceOk:publicSignals[7]==="1"};}

export async function generateProof(p:ProveParams):Promise<ProveResult>{
  if(!Number.isInteger(p.cutoffYear)||p.cutoffYear<2001||p.cutoffYear>2024)throw new Error("Forest-loss cutoff year must be between 2001 and 2024.");
  if(!Number.isInteger(p.evidenceFirstLossYearAfterCutoff)||p.evidenceFirstLossYearAfterCutoff<0)throw new Error("Invalid forest-loss evidence returned by backend.");
  const latEnc=encodeLat(p.latitude),lonEnc=encodeLon(p.longitude);const{latMin,latMax,lonMin,lonMax}=boundingBoxEnc(p.region.minLat,p.region.maxLat,p.region.minLon,p.region.maxLon);
  const supplierIdF=strToField(p.supplierId),supplierSecretF=strToField(p.supplierSecret),supplierCommitment=poseidon2([supplierIdF,supplierSecretF]);
  const protectedFlag=p.evidenceProtectedFlag?1n:0n,landCoverCode=BigInt(p.evidenceLandCoverCode),firstLossYear=BigInt(p.evidenceFirstLossYearAfterCutoff);
  const localHash=poseidon5([latEnc,lonEnc,protectedFlag,landCoverCode,firstLossYear]);if(localHash.toString()!==p.evidenceHash)throw new Error("Environmental evidence commitment does not match the private coordinate and backend-issued claim. Run the lookup again.");
  const failures:string[]=[];if(!(latEnc>=latMin&&latEnc<=latMax))failures.push("LOCATION OUTSIDE SUPPORTED REGION (latitude)");if(!(lonEnc>=lonMin&&lonEnc<=lonMax))failures.push("LOCATION OUTSIDE SUPPORTED REGION (longitude)");if(protectedFlag!==0n)failures.push("PROTECTED AREA DETECTED");if(Number(landCoverCode)!==p.allowedLandCoverCode)failures.push("LAND COVER CLASSIFICATION NOT PERMITTED");if(p.quantityKg>p.quantityThresholdKg)failures.push("PRODUCTION THRESHOLD FAILED");if(firstLossYear!==0n)failures.push(`FOREST LOSS DETECTED AFTER CUTOFF (first detected year: ${firstLossYear})`);
  const input={latEnc:latEnc.toString(),lonEnc:lonEnc.toString(),quantity:String(p.quantityKg),supplierId:supplierIdF.toString(),supplierSecret:supplierSecretF.toString(),protectedFlag:protectedFlag.toString(),landCoverCode:landCoverCode.toString(),firstLossYearAfterCutoff:firstLossYear.toString(),latMin:latMin.toString(),latMax:latMax.toString(),lonMin:lonMin.toString(),lonMax:lonMax.toString(),quantityThreshold:String(p.quantityThresholdKg),allowedLandCoverCode:String(p.allowedLandCoverCode),cutoffYear:String(p.cutoffYear),supplierCommitment:supplierCommitment.toString(),evidenceHash:p.evidenceHash};
  const{proof,publicSignals}=await snarkjs.groth16.fullProve(input,"/circuit/environmental_compliance.wasm","/circuit/circuit_final.zkey");const checks=parseComplianceChecks(publicSignals);return{proof,publicSignals,preflightFailures:failures,compliant:checks.valid,checks};
}
export async function verifyProofLocally(proof:unknown,publicSignals:string[]):Promise<boolean>{const v=await(await fetch("/circuit/verification_key.json")).json();return snarkjs.groth16.verify(v,publicSignals,proof);}
