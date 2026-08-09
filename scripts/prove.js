#!/usr/bin/env node
// GreenProof Groth16 proof generation.
// Environmental evidence must come from the backend's live provider lookup.
const fs = require("fs");
const path = require("path");
const snarkjs = require("snarkjs");
const { poseidon2, poseidon5 } = require("poseidon-lite");
const { boundingBoxEnc, encodeLat, encodeLon } = require("./lib/encode");

async function main() {
  const reqPath = process.argv[2];
  if (!reqPath) { console.error("Usage: node prove.js <request.json> [outDir]"); process.exit(1); }
  const outDir = process.argv[3] || path.join(__dirname, "..", "circuits", "build", "out");
  fs.mkdirSync(outDir, { recursive: true });
  const req = JSON.parse(fs.readFileSync(reqPath, "utf8"));
  const priv = req.private, cfg = req.publicConfig, evidence = req.evidence;
  for (const [k,v] of Object.entries({latitude:priv.latitude,longitude:priv.longitude,quantityKg:priv.quantityKg,supplierId:priv.supplierId,supplierSecret:priv.supplierSecret})) if(v===undefined||v===null) throw new Error(`Missing private field: ${k}`);
  if(priv.latitude < -90 || priv.latitude > 90 || !Number.isFinite(priv.latitude)) throw new Error("INVALID LATITUDE");
  if(priv.longitude < -180 || priv.longitude > 180 || !Number.isFinite(priv.longitude)) throw new Error("INVALID LONGITUDE");
  const cutoffYear = Number(cfg.cutoffYear);
  if(!Number.isInteger(cutoffYear) || cutoffYear < 2001 || cutoffYear > 2024) throw new Error("INVALID FOREST-LOSS CUTOFF YEAR");
  if(!evidence || !Number.isInteger(Number(evidence.firstLossYearAfterCutoff)) || Number(evidence.firstLossYearAfterCutoff) < 0) throw new Error("INVALID FOREST-LOSS EVIDENCE");

  const latEnc=encodeLat(priv.latitude), lonEnc=encodeLon(priv.longitude);
  const {latMin,latMax,lonMin,lonMax}=boundingBoxEnc(cfg.region.minLat,cfg.region.maxLat,cfg.region.minLon,cfg.region.maxLon);
  function strToField(s){let x=0n;for(const ch of Buffer.from(String(s),"utf8"))x=(x*256n+BigInt(ch))%(2n**200n);return x;}
  const supplierIdF=strToField(priv.supplierId), supplierSecretF=strToField(priv.supplierSecret);
  const supplierCommitment=poseidon2([supplierIdF,supplierSecretF]);
  const protectedFlag=evidence.protectedFlag?1n:0n;
  const landCoverCode=BigInt(evidence.landCoverCode);
  const firstLossYear=BigInt(evidence.firstLossYearAfterCutoff);
  const evidenceHash=poseidon5([latEnc,lonEnc,protectedFlag,landCoverCode,firstLossYear]);
  if(req.evidenceHash && req.evidenceHash !== evidenceHash.toString()) throw new Error("ENVIRONMENTAL EVIDENCE COMMITMENT MISMATCH");

  const failures=[];
  if(!(latEnc>=latMin&&latEnc<=latMax))failures.push("LOCATION OUTSIDE SUPPORTED REGION (latitude)");
  if(!(lonEnc>=lonMin&&lonEnc<=lonMax))failures.push("LOCATION OUTSIDE SUPPORTED REGION (longitude)");
  if(protectedFlag!==0n)failures.push("PROTECTED AREA DETECTED");
  if(Number(landCoverCode)!==cfg.allowedLandCoverCode)failures.push("LAND COVER CLASSIFICATION NOT PERMITTED");
  if(priv.quantityKg>cfg.quantityThresholdKg)failures.push("PRODUCTION THRESHOLD FAILED");
  if(firstLossYear!==0n)failures.push(`FOREST LOSS DETECTED AFTER CUTOFF (first detected year: ${firstLossYear})`);

  const input={latEnc:latEnc.toString(),lonEnc:lonEnc.toString(),quantity:String(priv.quantityKg),supplierId:supplierIdF.toString(),supplierSecret:supplierSecretF.toString(),protectedFlag:protectedFlag.toString(),landCoverCode:landCoverCode.toString(),firstLossYearAfterCutoff:firstLossYear.toString(),latMin:latMin.toString(),latMax:latMax.toString(),lonMin:lonMin.toString(),lonMax:lonMax.toString(),quantityThreshold:String(cfg.quantityThresholdKg),allowedLandCoverCode:String(cfg.allowedLandCoverCode),cutoffYear:String(cutoffYear),supplierCommitment:supplierCommitment.toString(),evidenceHash:evidenceHash.toString()};
  fs.writeFileSync(path.join(outDir,"input.json"),JSON.stringify(input,null,2));
  if(failures.length) { console.warn("PRE-FLIGHT POLICY FAILURES:"); failures.forEach(f=>console.warn("  - "+f)); }
  const buildDir=path.join(__dirname,"..","circuits","build");
  const wasmPath=path.join(buildDir,"environmental_compliance_js","environmental_compliance.wasm"), zkeyPath=path.join(buildDir,"environmental_compliance_final.zkey");
  if(!fs.existsSync(wasmPath)||!fs.existsSync(zkeyPath)){console.error("Missing compiled circuit artifacts. Run bash setup.sh first.");process.exit(2);}
  try{
    const {proof,publicSignals}=await snarkjs.groth16.fullProve(input,wasmPath,zkeyPath);
    fs.writeFileSync(path.join(outDir,"proof.json"),JSON.stringify(proof,null,2));fs.writeFileSync(path.join(outDir,"public.json"),JSON.stringify(publicSignals,null,2));
    const names=["valid","regionOk","protectedAreaOk","landCoverOk","quantityOk","supplierOk","forestLossOk","evidenceOk"];
    console.log("PROOF GENERATED:",path.join(outDir,"proof.json"));console.log("PUBLIC SIGNALS:",path.join(outDir,"public.json"));console.log("COMPLIANCE STATUS:",publicSignals[0]==="1"?"COMPLIANT":"NOT COMPLIANT");for(let i=0;i<8;i++)console.log(`  ${names[i]}: ${publicSignals[i]==="1"?"PASS":"FAIL"}`);
  }catch(err){console.error("PROOF GENERATION FAILED:",String(err.message||err));process.exit(3)}
}
main().catch(e=>{console.error(e);process.exit(1)});
