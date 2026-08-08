# GreenProof

**Privacy-preserving environmental verification for commodity supply chains.**

GreenProof is a hackathon prototype for cocoa supply chains. It demonstrates how a supplier can prove that a private set of facts satisfies public environmental constraints using a real Groth16 zero-knowledge proof, while an auditor receives only the verification result and public evidence provenance.

> **Prototype limitation:** GreenProof is not an official EUDR compliance or certification system. It does not independently establish legal compliance or deforestation-free status. Land cover is obtained from ESA WorldCover 2021 v200; this is a satellite-derived land-cover observation, not historical deforestation detection.

## The problem

Environmental due diligence often requires sensitive supply-chain information:

- exact farm/plot coordinates;
- production volume;
- supplier identity;
- environmental evidence tied to that location.

Suppliers may not want to expose commercially sensitive coordinates and volumes to every buyer, auditor, platform, or shared database.

The key question is:

> **Can a supplier prove that a claim about a hidden plot is true without handing over the hidden plot data?**

GreenProof demonstrates that workflow with zero-knowledge proofs.

## The solution

The demo uses a single cocoa-supply-chain scenario.

A supplier:

1. enters a private plot coordinate;
2. checks live environmental evidence;
3. enters private production quantity and supplier credentials;
4. generates a Groth16 proof locally in the browser;
5. creates a verification ID.

An auditor can then open the verification ID and see:

- whether the cryptographic proof is valid;
- environmental evidence provenance;
- the public verification metadata.

The auditor does **not** receive the exact coordinate, production quantity, or supplier secret.

## What the prototype actually proves

The Circom circuit constrains:

1. private latitude/longitude are inside the public West African cocoa-region bounding box;
2. the live evidence lookup did not detect a protected-area tag;
3. the reported land-cover code equals the public allowed code;
4. private production quantity is below the public threshold;
5. the supplier knows the secret corresponding to the public supplier commitment;
6. the private environmental witness is bound to the public evidence hash.

This is a proof of the stated computational conditions. It is **not** a proof that OpenStreetMap is complete, that the evidence provider is truthful, or that the physical world matches the dataset.

## Demo workflow

```text
SUPPLIER
  │
  ├─ private coordinates ────────────────┐
  ├─ private quantity                    │
  ├─ private supplier secret             │
  │                                      │
  ▼                                      │
Live environmental lookup                │
  │                                      │
  ├─ protected-area evidence             │
  └─ land-cover evidence                 │
         │                               │
         └──────────────┐                │
                        ▼                │
                 Browser-side ZK proof ◄─┘
                        │
                        ▼
                 Groth16 proof
                        │
                        ▼
                GreenProof ID
                        │
                        ▼
                    AUDITOR
                        │
              ┌─────────┴─────────┐
              ▼                   ▼
        cryptographic        provenance
        result: VALID        metadata
              │
              └── private witness remains hidden
```

The exact coordinate is currently sent temporarily to the backend because the MVP performs the live geospatial lookup server-side. The production quantity and supplier secret never enter that endpoint and are processed locally in the browser.

## Environmental data sources

| Source | Current role |
|---|---|
| Nominatim / OpenStreetMap | Reverse geocoding and place metadata |
| Overpass API / OpenStreetMap | Protected-area tag proxy |
| ESA WorldCover 2021 v200 | Primary satellite-derived 10 m land-cover class, based on Sentinel-1/Sentinel-2 |

The app does not fabricate evidence when a live source fails. It returns an explicit error instead.

### Important limitation

ESA WorldCover classifies land cover for 2021; it is **not** a historical deforestation analysis. Do not describe the MVP as proving "deforestation-free" in the legal EUDR sense.

A production version should replace/supplement this preprocessing step with authoritative environmental datasets, signed evidence attestations, and historical satellite analysis.

## Why zero-knowledge proofs?

A normal database can prove that disclosed data was stored or signed correctly. It does not let a verifier check a property of data that the verifier never receives.

For example:

```text
Private:
  exact plot = X
  quantity = 1,200 kg
  supplier secret = Y

Public requirement:
  quantity <= 5,000 kg

ZK result:
  "Yes, this hidden quantity satisfies the constraint."
```

That is the privacy property GreenProof is demonstrating.

## Architecture

### Frontend

- React
- TypeScript
- Vite
- Leaflet / React-Leaflet
- snarkjs
- Poseidon

### Backend

- Rust
- Axum
- reqwest
- Nominatim
- Overpass API

### Cryptography

- Circom 2.1.6
- Groth16
- BN254
- Poseidon
- circomlib comparators

Proof generation happens in the browser using WASM.

Proof verification uses the standard `snarkjs groth16.verify` implementation through the Rust backend.

## Verification IDs

After a valid proof is generated, the frontend calls:

```text
POST /api/verify-proof
```

The backend verifies the proof and creates an ID such as:

```text
GP-8F3A91C2
```

The auditor can then open:

```text
?verification=GP-8F3A91C2
```

The demo backend stores only the proof, public signals and sanitized evidence provenance in an **in-memory store**. Exact coordinates are deliberately removed before the verification record is stored.

This is suitable for a hackathon demo, not production persistence.

## Security / trust model

GreenProof deliberately separates three things:

### 1. Cryptographic correctness

The Groth16 proof can be independently verified.

Tampering with the proof or public signals makes verification fail.

### 2. Evidence consistency

The backend normalizes the ESA WorldCover class and protected-area result, computes a Poseidon evidence hash over encoded coordinates and that compact claim, and issues a short-lived one-time evidence session. It accepts a proof only when the proof's public evidence hash and fixed public policy signals match that backend-issued session.

### 3. Environmental truth

This is **outside** the circuit.

The system currently trusts the live environmental data returned by the configured public sources. A production deployment should require authenticated/signed evidence from trusted data providers.

## What is intentionally not claimed

GreenProof does not claim to:

- provide official EUDR certification;
- replace a Due Diligence Statement;
- prove legality of harvest;
- prove deforestation-free status from satellite imagery;
- guarantee that OpenStreetMap tags are complete;
- provide production-grade multi-party Groth16 setup;
- replace established EUDR compliance platforms.

The point of the prototype is narrower:

> **Demonstrate minimal-disclosure environmental verification using zero-knowledge proofs.**

## Running locally

Requirements:

- Rust stable + Cargo
- Node.js >= 18
- npm
- Circom >= 2.1.6
- network access for public environmental APIs

```bash
cp .env.example .env

cd scripts
npm install
bash setup.sh
cd ..

bash scripts/copy-artifacts-to-frontend.sh

cd backend
cargo run
```

In another terminal:

```bash
cd frontend
npm install
npm run dev
```

Then open the Vite URL.

### Circuit artifacts

The browser needs:

```text
frontend/public/circuit/environmental_compliance.wasm
frontend/public/circuit/circuit_final.zkey
frontend/public/circuit/verification_key.json
```

These are generated by the setup/copy scripts and are intentionally not committed as source files.

## Tests

Circuit tests use real Groth16 proving/verification rather than mocked cryptographic results:

```bash
cd scripts
npx mocha test/circuit.test.js --timeout 12000
```

Backend tests:

```bash
cd backend
cargo test
```

Frontend production build:

```bash
cd frontend
npm run build
```

## Hackathon demo

The strongest demo is:

### 1. Valid supplier

```text
Environmental checks
✓ Supported region
✓ Protected-area check
✓ Allowed land cover
✓ Quantity threshold

Generate ZK proof
        ↓
VALID
```

### 2. Invalid supplier

Change one condition, such as production quantity above the public threshold:

```text
Quantity threshold
✕

Groth16 proof generation
REJECTED
```

### 3. Tampering

Modify a public signal or proof artifact:

```text
Original proof
✓ VALID

Modified proof/public signal
✕ INVALID
```

This demonstrates that ZK is part of the verification mechanism rather than decorative cryptography.

## Roadmap

### Hackathon MVP

- [x] Real live geospatial lookup
- [x] Browser-side Groth16 proof generation
- [x] Real proof verification
- [x] Privacy-preserving supplier witness
- [x] Auditor verification flow
- [x] Verification IDs
- [x] Sanitized evidence provenance
- [x] Valid/invalid demonstration path

### Production direction

- [ ] Authenticated/signed environmental evidence
- [ ] Protected Planet / WDPA integration
- [ ] Copernicus / Sentinel-derived land-cover and change detection
- [ ] Historical baseline analysis
- [ ] Polygon-level plot geometry
- [ ] Multi-plot and aggregation proofs
- [ ] Persistent verification registry
- [ ] Multi-party trusted setup or a transparent proving system
- [ ] Native Rust Groth16 verifier
- [ ] Full EUDR due-diligence workflow and legal review

## License / attribution

OpenStreetMap data is © OpenStreetMap contributors and subject to the Open Database License.

This repository is a hackathon prototype and should not be treated as a production compliance service.
