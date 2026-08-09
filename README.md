# GreenProof

**Privacy-preserving environmental verification for commodity supply chains.**

GreenProof lets a supplier prove an environmental policy about a private site without revealing the exact coordinates to the final verifier.

## Forest-loss verification

The environmental flow now includes a real historical forest-loss predicate:

> **No detected Hansen/UMD forest loss occurred at the queried site after the selected cutoff year.**

GreenProof uses the **Hansen/UMD Global Forest Change 2024 v1.12** `lossyear` raster from the official University of Maryland GLAD archive. It is derived from Landsat time-series imagery, covers 2000-2024, uses 10° x 10° tiles, and has approximately 30 m pixels at the equator. The official dataset defines `lossyear` as a gross forest-cover loss indicator, with values 1-24 representing detected loss primarily in 2001-2024 and 0 meaning no detected loss. The dataset is publicly downloadable and does not require a GFW API key. urlOfficial Hansen GFC 2024 v1.12 documentationhttps://storage.googleapis.com/earthenginepartners-hansen/GFC-2024-v1.12/download.html

The backend downloads only the relevant `lossyear` tile on first use and caches it under `data/hansen`. It then reads a bounded 1x1 GeoTIFF window at the private coordinate instead of decoding the entire raster into memory.

## Architecture

```text
Private coordinate + cutoff year
            │
            ▼
Rust backend
            │
            ├── Hansen/UMD GFC 2024 v1.12
            │     └── loss-year GeoTIFF pixel
            ├── ESA WorldCover 2021
            │     └── land-cover class
            └── OSM Overpass
                  └── protected-area tags
            │
            ▼
Deterministic normalized evidence
            │
            ├── Hansen loss year after cutoff (or 0)
            ├── protected flag
            └── land-cover code
            │
            ▼
Poseidon(5) evidence commitment
            │
            ▼
Groth16 proof
            │
            ├── private coordinate
            ├── private quantity
            ├── private supplier secret
            └── private environmental witness
            │
            ▼
Verifier receives proof + public policy
```

## Evidence commitment

The backend computes:

```text
Poseidon(
  encodedLatitude,
  encodedLongitude,
  protectedFlag,
  landCoverCode,
  firstLossYearAfterCutoff
)
```

`firstLossYearAfterCutoff = 0` means the Hansen pixel contains no detected loss after the selected cutoff. Otherwise it contains the detected loss year.

The browser recomputes the same commitment from the private coordinate and backend-issued evidence. A mismatch stops proof generation.

## What the ZK circuit proves

The Circom/Groth16 circuit proves that the private witness:

1. lies inside the configured public sourcing region;
2. satisfies the protected-area policy;
3. has the permitted land-cover code;
4. satisfies the quantity threshold;
5. satisfies the supplier commitment;
6. has no Hansen loss year after the selected cutoff;
7. matches the public Poseidon evidence commitment.

The exact coordinate, quantity, supplier ID and secret are private witness values.

## Cryptographic boundary

The SNARK does **not** independently verify the external satellite dataset inside the circuit. The practical architecture is:

```text
real Hansen GeoTIFF
       ↓
deterministic pixel extraction
       ↓
normalized evidence
       ↓
Poseidon commitment
       ↓
Groth16 proves private witness == committed evidence
       ↓
Groth16 proves policy predicate
```

Therefore the proof guarantees that the committed environmental observation and private coordinate are internally consistent and satisfy the circuit policy. It does not prove that Hansen is complete or error-free, that every tree on a parcel was observed, or that a regulator would accept the result as legal certification.

## Environmental sources

| Source | Role | Real data | Credential |
|---|---|---:|---|
| Hansen/UMD GFC 2024 v1.12 | Historical forest-loss verification | Yes | None |
| ESA WorldCover 2021 v200 | Satellite-derived land-cover classification | Yes | None |
| OpenStreetMap Overpass | Protected-area tag proxy | Yes | None |
| Nominatim | Reverse geocoding | Yes | None |

Hansen is the primary forest-loss source. WorldCover is only a secondary land-cover signal and is not used as a substitute for historical forest-loss data.

## Cutoff semantics

The UI accepts a cutoff year from **2001 through 2024**.

Example:

```text
lossyear = 0, cutoff = 2020
=> forestLossOk = 1

lossyear = 23, cutoff = 2020
=> detected loss year = 2023
=> forestLossOk = 0
```

The current predicate is pixel-level. It does not claim that an entire farm polygon has no forest loss.

## Setup

Requirements:

- Rust stable + Cargo
- Node.js >= 18
- npm
- Circom >= 2.1.6
- network access

No Global Forest Watch account or API key is required.

### 1. Configure the environment

```bash
cp .env.example .env
```

The default configuration uses the public Hansen archive and caches downloaded tiles under `data/hansen`.

### 2. Build the circuit

```bash
cd scripts
npm install
bash setup.sh
cd ..
bash scripts/copy-artifacts-to-frontend.sh
```

### 3. Start the Rust backend

```bash
cd backend
cargo run
```

### 4. Start the frontend

In another terminal:

```bash
cd frontend
npm install
npm run dev
```

Open the Vite URL.

## Complete demo

1. Open **Supplier**.
2. Enter/select a private coordinate.
3. Select a cutoff year, for example `2020`.
4. Enter quantity, supplier ID and supplier secret.
5. Click **Check environmental evidence**.
6. The backend queries real Hansen/UMD data, ESA WorldCover and Overpass.
7. Review the forest-loss dataset and detected loss result.
8. Click **Generate zero-knowledge proof**.
9. The browser recomputes the Poseidon evidence commitment and generates a Groth16 proof.
10. Click **Verify & create share link**.
11. Open the verification ID as an auditor.

If an environmental source fails, GreenProof fails closed. It never substitutes mock or fabricated environmental evidence.

## Tests

Backend:

```bash
cd backend
cargo test
```

Frontend:

```bash
cd frontend
npm run build
```

Circuit + Groth16:

```bash
cd scripts
npx mocha test/circuit.test.js --timeout 120000
```

The backend also contains a Hansen tile-selection test, including negative-longitude tile handling.

## Direct Hansen source

For manual inspection, the official archive exposes individual 10° x 10° `lossyear` GeoTIFFs. For example, the archive documents URLs such as:

```text
https://storage.googleapis.com/earthenginepartners-hansen/GFC-2024-v1.12/Hansen_GFC-2024-v1.12_lossyear_40N_080W.tif
```

GreenProof constructs the corresponding tile URL from the private coordinate. urlHansen GFC download pagehttps://storage.googleapis.com/earthenginepartners-hansen/GFC-2024-v1.12/download.html

## Security and privacy limitations

- The exact coordinate is sent to the backend because the backend performs the environmental lookup. The final verification record does not expose it.
- The Hansen tile cache contains environmental raster data, not a database of submitted coordinates.
- Evidence sessions are short-lived and one-time-use in the in-memory store.
- The proof hides private witness values, but public policy values and the evidence commitment remain visible.
- The forest-loss query samples the Hansen pixel containing the coordinate rather than a full farm polygon.
- Hansen/UMD GFC is a forest-loss indicator from Landsat time-series analysis, not a legal determination of deforestation-free status. The dataset documentation also warns about temporal methodological differences and limitations in using pixel counts as definitive area estimates. citeturn0search0

## Project scope

GreenProof demonstrates this narrower claim:

> **A supplier can prove, without disclosing the exact site to the final verifier, that a real Hansen/UMD environmental lookup produced a forest-loss observation compatible with a public cutoff policy.**

The environmental observation comes from a real public dataset; the ZK layer proves consistency of the private witness with the committed observation and proves the policy predicate.
