# GreenProof

**Privacy-preserving environmental verification for commodity supply chains.**

GreenProof is a hackathon prototype that lets a supplier prove an environmental policy about a private site without revealing the exact coordinates to the verifier.

## What changed

The environmental flow now includes a real historical forest-loss predicate:

> **No detected Hansen/UMD forest loss occurred in the queried site after the selected cutoff year.**

The forest-loss evidence comes from the **Global Forest Watch Data API**, querying the **Hansen/UMD Global Forest Change 2024 v1.12 tree-cover-loss dataset**. The dataset is derived from Landsat time series and covers forest-loss events from 2001 through 2024. The official dataset describes loss as a stand-replacement disturbance / forest-to-nonforest change indicator. urlHansen/UMD Global Forest Change 2024 v1.12 documentationhttps://storage.googleapis.com/earthenginepartners-hansen/GFC-2024-v1.12/download.html

The GFW Data API requires an account/API key. The application fails closed when the key is missing or the provider fails. It never substitutes a successful or fabricated result. urlGlobal Forest Watch Data API documentationhttps://data-api.globalforestwatch.org/

## Architecture

```text
Private coordinate + cutoff year
            │
            ▼
Backend live environmental lookup
            │
            ├── Hansen/UMD GFC via GFW Data API
            │     └── forest-loss years after cutoff
            ├── ESA WorldCover 2021
            │     └── land-cover class
            └── OSM Overpass
                  └── protected-area tags
            │
            ▼
Deterministic normalized evidence
            │
            ├── first detected loss year after cutoff (or 0)
            ├── protected flag
            └── land-cover code
            │
            ▼
Poseidon(5) evidence commitment
            │
            ▼
Browser Groth16 proof
            │
            ├── private coordinate
            ├── private quantity
            ├── private supplier secret
            └── private environmental witness
            │
            ▼
Auditor sees proof + public policy + provenance
```

### Evidence commitment

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

`firstLossYearAfterCutoff = 0` means the live GFW query returned no detected loss after the selected cutoff. Otherwise it is the earliest detected loss year in the queried area after the cutoff.

The browser recomputes the same Poseidon commitment from its private coordinate and the backend-issued normalized evidence. A mismatch stops proof generation.

### What the circuit proves

The Circom/Groth16 circuit proves all of the following for the private witness:

1. the coordinate is inside the configured public sourcing region;
2. the protected-area flag is compliant;
3. the land-cover code matches the public policy;
4. private quantity is below the public threshold;
5. supplier commitment is correct;
6. `firstLossYearAfterCutoff` does not represent a loss after the selected cutoff;
7. the private coordinate/environmental witness matches the public Poseidon evidence commitment.

The exact coordinate, quantity, supplier ID and secret are private witness values and are not contained in the proof or public signals.

## Important cryptographic boundary

The SNARK does **not** download or independently verify satellite data inside the circuit. That would be impractical for this hackathon architecture.

Instead:

```text
real provider response
       ↓
deterministic backend normalization
       ↓
Poseidon commitment
       ↓
SNARK proves private witness == committed evidence
       ↓
SNARK proves policy predicate
```

Therefore the cryptographic guarantee is:

> Given the backend-issued evidence commitment, the Groth16 proof demonstrates that the private coordinate/environmental witness bound to that commitment satisfies the configured policy.

It does **not** prove that Hansen/GFW is complete, that every physical tree was observed, that the provider cannot be wrong, or that a regulator would accept the result as legal EUDR certification.

## Environmental sources

| Source | Role | Real data? |
|---|---|---|
| Hansen/UMD Global Forest Change 2024 v1.12 via GFW Data API | Historical forest-loss verification | Yes |
| ESA WorldCover 2021 v200 | Satellite-derived 10 m land-cover classification | Yes |
| OpenStreetMap Overpass | Protected-area tag proxy | Yes |
| Nominatim | Reverse geocoding | Yes |

ESA WorldCover remains a secondary land-cover signal. It is not used as a substitute for historical forest-loss data.

## Cutoff semantics

The UI accepts a cutoff year from **2001 through 2024**.

For example:

```text
cutoff = 2020
GFW detected loss years = none after 2020
=> forestLossOk = 1

cutoff = 2020
GFW detected loss year = 2023
=> forestLossOk = 0
```

The GFW query uses a small deterministic polygon around the supplied coordinate because the raster query API requires a geometry. The public verification result does not expose that coordinate.

## Setup

Requirements:

- Rust stable + Cargo
- Node.js >= 18
- npm
- Circom >= 2.1.6
- network access
- a Global Forest Watch Data API key

### 1. Configure the environment

```bash
cp .env.example .env
```

Set:

```text
GLOBAL_FOREST_WATCH_API_KEY=your_real_key
```

The official GFW documentation explains account/API-key creation. urlGFW Data API authentication guidehttps://developer.openepi.io/how-tos/getting-started-using-global-forest-watch-data-api

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
6. The backend queries live GFW/Hansen forest-loss data, ESA WorldCover and Overpass.
7. Confirm the evidence card shows the actual GFW dataset and detected loss result.
8. Click **Generate zero-knowledge proof**.
9. The browser recomputes the Poseidon evidence commitment and generates a real Groth16 proof.
10. Click **Verify & create share link**.
11. Open the verification ID as an auditor.

If the GFW API is unavailable, the environmental lookup fails. There is no mock-data path.

## Tests

### Backend

```bash
cd backend
cargo test
```

### Frontend

```bash
cd frontend
npm run build
```

### Circuit + Groth16

```bash
cd scripts
npx mocha test/circuit.test.js --timeout 120000
```

The circuit tests cover:

- valid no-loss proof;
- detected loss after cutoff;
- cutoff policy behavior;
- protected-area failure;
- quantity failure;
- evidence-commitment mismatch;
- proof tampering;
- public-signal tampering;
- absence of private witness values from auditor artifacts.

## Direct environmental API test

After configuring `GLOBAL_FOREST_WATCH_API_KEY`, test the same GFW endpoint independently:

```bash
curl -X POST \
  'https://data-api.globalforestwatch.org/dataset/umd_tree_cover_loss/v1.12/query/json' \
  -H "x-api-key: $GLOBAL_FOREST_WATCH_API_KEY" \
  -H 'Content-Type: application/json' \
  --data-raw '{
    "sql": "SELECT umd_tree_cover_loss__year AS year, SUM(area__ha) AS area_ha FROM results WHERE umd_tree_cover_loss__year > 2020 GROUP BY umd_tree_cover_loss__year ORDER BY umd_tree_cover_loss__year ASC",
    "geometry": {
      "type": "Polygon",
      "coordinates": [[
        [-1.61635, 6.66655],
        [-1.61625, 6.66655],
        [-1.61625, 6.66665],
        [-1.61635, 6.66665],
        [-1.61635, 6.66655]
      ]]
    }
  }'
```

The API documentation specifies that raster datasets require a geometry and that `umd_tree_cover_loss__year` and `area__ha` are queryable fields. citeturn3view0

## Security and privacy limitations

- The current demo sends the exact coordinate to the backend because the backend performs the live environmental lookup. The final verification record does not store the coordinate.
- The GFW API key must remain server-side. Do not expose it through Vite or browser environment variables.
- Evidence sessions are short-lived and one-time-use in an in-memory store.
- The proof hides private witness values, but public policy values and the evidence commitment remain visible.
- The forest-loss query covers a small deterministic area around the coordinate rather than a full farm polygon. This is a prototype predicate, not parcel-level legal certification.
- Hansen/UMD GFC is a forest-loss indicator derived from Landsat time series. It should not be described as a guarantee of legal deforestation-free status.
- The dataset documentation itself notes methodological differences across years and cautions against treating pixel counts as definitive area estimates.

## Project scope

GreenProof demonstrates a narrower claim than an EUDR certification system:

> **A supplier can prove, without disclosing the exact site, that a real environmental evidence lookup produced a forest-loss result compatible with a public cutoff policy.**

That is the cryptographic/privacy contribution of the prototype.
