# GreenProof — 2 minute 30 second demo script

## 0:00–0:15 — Problem

Show the supplier screen.

Say:

> Environmental compliance needs evidence about where commodities come from. But exact farm coordinates and production volumes can be commercially sensitive. GreenProof asks a different question: can we prove compliance without disclosing the underlying data?

## 0:15–0:30 — Solution

Show the GreenProof landing screen.

Say:

> GreenProof is a privacy-preserving environmental verification layer for commodity supply chains. The supplier checks live environmental evidence, generates a Groth16 zero-knowledge proof locally, and gives the auditor a verification ID instead of the private witness.

## 0:30–1:05 — Valid supplier

Use the valid demo coordinate and a quantity below 5,000 kg.

Fill:

- Commodity: Cocoa
- Coordinate: the known working demo coordinate
- Quantity: 1,200 kg
- Supplier ID: any demo identifier
- Supplier secret: a random demo secret

Click:

`CHECK ENVIRONMENTAL EVIDENCE`

Point out:

- live data source;
- protected-area check;
- land-cover classification;
- public constraint.

Then click:

`GENERATE PRIVATE ZK PROOF`

Say:

> The quantity and supplier secret are processed in the browser. The circuit proves the constraints without exposing those values to the auditor.

## 1:05–1:25 — Verification ID

Click:

`VERIFY & CREATE SHARE LINK`

Show:

`GP-XXXXXXXX`

Open the generated verification URL.

Say:

> The auditor receives a verification ID. The stored verification record contains the cryptographic proof and sanitized provenance, not the exact plot coordinate.

## 1:25–1:50 — Auditor view

Show:

`CRYPTOGRAPHIC STATUS — VERIFIED`

Point to the verification certificate:

- Commodity: Cocoa
- Result: PASS
- ZK verification (Groth16 / BN254): VALID
- Proof ID and creation timestamp
- Evidence source and retrieval timestamp
- Exact coordinates: PRIVATE — never disclosed

And below it:

- Exact coordinates disclosed: NOT DISCLOSED
- Production quantity disclosed: NOT DISCLOSED
- Supplier secret disclosed: NOT DISCLOSED

Say:

> The auditor can verify the statement while the sensitive witness stays hidden.

## 1:50–2:10 — Rejection

Return to supplier and change quantity to something above the public threshold, for example 6,000 kg.

Click:

`GENERATE PRIVATE ZK PROOF`

Show the circuit rejection.

Say:

> This is not just a UI status. The circuit is constrained so an invalid witness cannot produce a valid proof.

## 2:10–2:25 — Tampering

If time permits, use the advanced auditor panel.

Modify a public signal or proof artifact.

Show:

`CRYPTOGRAPHIC STATUS — NOT VERIFIED` (certificate: Result FAIL, ZK verification INVALID)

Say:

> If the proof or public signals are modified, the Groth16 verification fails.

## 2:25–2:30 — Closing

Say:

> GreenProof turns environmental compliance from data disclosure into verifiable proof. The current prototype uses OpenStreetMap evidence; the production roadmap replaces that proxy with authenticated satellite and protected-area datasets.
