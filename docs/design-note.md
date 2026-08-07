# GreenProof — Internal Design Note

## TARGET USERS
1. Suppliers/producers (cocoa growers/cooperatives) who hold sensitive plot- and volume-level data.
2. Buyers/brands who need verifiable environmental evidence from suppliers without absorbing liability for storing suppliers' exact coordinates.
3. Auditors/compliance teams who need scalable, cryptographically checkable evidence instead of manual document review.

## CORE PROBLEM
Regulations like the EU Deforestation Regulation (EUDR) require operators to collect exact plot geolocation and prove deforestation-free status, legality, and traceability. This creates commercial tension: the data needed to prove compliance (exact coordinates, volumes, supplier identity) is also the data suppliers and buyers most want to protect from competitors and from being aggregated into a single leakable database.

## EXISTING SOLUTIONS (research, Aug 2026)
1. **osapiens** — EUDR due-diligence automation platform; aggregates supplier geolocation/product data into a shared "data network," runs risk assessment, generates and submits Due Diligence Statements to EU TRACES.
2. **IntegrityNext** — full EUDR journey platform; ingests supplier-submitted geolocation and multi-layer satellite imagery (AI-scored), generates DDS, integrates with ERP/SRM systems.
3. **TraceX** — geo-tagging at plot/polygon level, satellite (Sentinel-2/Landsat/Planet Labs) monitoring, positions blockchain as the "tamper-proof ledger" for plot geolocation and ownership data.
4. **Finboot (MARCO)** — blockchain-based track & trace suite; treats on-chain immutability as the trust mechanism for supply-chain records.

## WHAT THEY ALREADY DO WELL
- Real satellite-based deforestation/land-cover monitoring at scale.
- Mature onboarding workflows, ERP integration, DDS/TRACES submission automation.
- Established relationships with hundreds of suppliers.

## WHAT THEY DO NOT SOLVE
- All of the above require the exact plot geolocation (and usually volumes and supplier identity) to be **disclosed** — to a shared platform, an EU competent authority, or an immutable blockchain ledger. None of the reviewed platforms allow a supplier to prove "my plot satisfies the environmental constraint" **without** revealing the plot coordinates and volumes to the verifying party.
- Blockchain-based approaches ("MARCO") advertise immutability/tamper-resistance, not confidentiality — immutable and public are in tension with commercially sensitive data.
- None of the reviewed platforms use zero-knowledge proofs.

## GREENPROOF DIFFERENTIATOR
The MVP does not compete on satellite-analytics sophistication or on scale of EUDR workflow automation — established vendors already do that well. GreenProof demonstrates a narrower, technically distinct capability those platforms do not offer: converting a real environmental-data-derived constraint into a **zero-knowledge proof**, so a verifier learns only "constraint satisfied: yes/no" plus dataset provenance, never the exact coordinates, volume, or supplier identity used to satisfy it.

We do not claim to be "the first" ZK-based sustainability project in an absolute sense (ZK-for-ESG is an active academic/startup research area); we claim, based on the platforms reviewed above, that mainstream EUDR compliance tooling as currently marketed relies on data disclosure/immutability rather than zero-knowledge minimal disclosure.

## WHY ZKP IS ACTUALLY NECESSARY
The verifier (buyer/auditor) needs to know a statement is true ("plot is outside protected areas AND volume ≤ threshold AND commitment matches a registered supplier") but does not need the private facts that make it true. A normal database, digital signature, or blockchain record can prove *that data was submitted and unaltered* — it cannot prove *a property of hidden data* without revealing the data itself. That gap is exactly what a zk-SNARK closes.

## WHY A NORMAL DATABASE / QR CODE / BLOCKCHAIN IS NOT SUFFICIENT
- **Database/QR code**: verifier must be given (or the code must encode) the underlying data to check it — no confidentiality.
- **Digital signature alone**: proves authenticity/integrity of disclosed data, not a property of undisclosed data.
- **Blockchain**: adds immutability and auditability of what was posted, but whatever is posted is either public or requires an off-chain trusted party to interpret — it doesn't hide the private inputs from the verifier while still letting them check a derived fact.

## MVP SCOPE
- Single commodity (cocoa), single narrow claim: plot not in a detected protected area (via real OSM/Overpass protected-area tags) + declared volume ≤ configurable threshold + supplier commitment, proven in zero-knowledge with a real Circom/Groth16 circuit fed by a real (non-fabricated) geospatial preprocessing step.

## FEATURES EXCLUDED FROM MVP
- Full EUDR due-diligence workflow (risk classification tiers, DDS/TRACES submission, legality-of-harvest proof).
- Full-resolution satellite deforestation-change detection (Sentinel-2 time series) — out of scope for a preprocessing step buildable without paid imagery access; documented as future work.
- Multi-commodity support, multi-plot aggregation, organization accounts, persistent central storage of supplier private data.
