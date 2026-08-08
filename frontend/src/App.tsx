import { useEffect, useMemo, useState } from "react";
import { MapContainer, TileLayer, Marker, Popup } from "react-leaflet";
import { generateProof, verifyProofLocally, type RegionConfig } from "./lib/zk";
import "./app.css";

const BACKEND_URL = import.meta.env.VITE_BACKEND_URL || "http://localhost:8080";
const DEFAULT_REGION: RegionConfig = { minLat: 4.0, maxLat: 11.5, minLon: -8.6, maxLon: 1.5 };
const DEFAULT_THRESHOLD_KG = 5000;
const DEFAULT_ALLOWED_LAND_COVER_CODE = 40;

interface EvidenceState {
  ok: boolean;
  evidence?: any;
  evidence_session?: string;
  evidence_hash?: string;
  error?: string;
  note?: string;
}

type Tab = "supplier" | "auditor";

export default function App() {
  const [tab, setTab] = useState<Tab>("supplier");
  const [verificationId, setVerificationId] = useState<string | null>(null);

  function handleVerificationCreated(id: string) {
    setVerificationId(id);
    window.history.replaceState({}, "", `?verification=${encodeURIComponent(id)}`);
    setTab("auditor");
  }

  useEffect(() => {
    const id = new URLSearchParams(window.location.search).get("verification");
    if (id) {
      setTab("auditor");
      setVerificationId(id);
    }
  }, []);

  return (
    <div className="app-shell">
      <div className="page">
        <header className="hero">
          <div className="eyebrow">PRIVACY-PRESERVING ENVIRONMENTAL VERIFICATION</div>
          <h1>GreenProof</h1>
          <p className="hero-title">Prove environmental compliance without exposing your supply chain.</p>
          <p className="hero-copy">
            A cocoa-supply-chain prototype using real environmental evidence and zero-knowledge proofs.
            The verifier learns whether the constraints passed — not the supplier's exact coordinates,
            production volume, or secret.
          </p>
          <div className="hero-badges">
            <span>Groth16 / BN254</span>
            <span>Browser-side proving</span>
            <span>Open environmental data</span>
          </div>
        </header>

        <nav className="tabs">
          <button className={tab === "supplier" ? "active" : ""} onClick={() => setTab("supplier")}>
            1. Supplier
          </button>
          <button className={tab === "auditor" ? "active" : ""} onClick={() => setTab("auditor")}>
            2. Auditor
          </button>
        </nav>

        {tab === "supplier" ? (
          <SupplierView onVerificationCreated={handleVerificationCreated} />
        ) : (
          <AuditorView initialId={verificationId} />
        )}

        <footer>
          <strong>Prototype limitation:</strong> GreenProof is not an official EUDR compliance or
          certification system. Land cover comes from ESA WorldCover 2021 v200 satellite-derived data;
          it is not historical satellite deforestation detection.
        </footer>
      </div>
    </div>
  );
}

function SupplierView({ onVerificationCreated }: { onVerificationCreated: (id: string) => void }) {
  const [latitude, setLatitude] = useState("6.6666");
  const [longitude, setLongitude] = useState("-1.6163");
  const [quantityKg, setQuantityKg] = useState("1200");
  const [supplierId, setSupplierId] = useState("");
  const [supplierSecret, setSupplierSecret] = useState("");
  const [loading, setLoading] = useState(false);
  const [evidenceState, setEvidenceState] = useState<EvidenceState | null>(null);
  const [proofResult, setProofResult] = useState<any>(null);
  const [proving, setProving] = useState(false);
  const [proveError, setProveError] = useState<string | null>(null);
  const [sharing, setSharing] = useState(false);
  const [shareError, setShareError] = useState<string | null>(null);

  const lat = parseFloat(latitude);
  const lon = parseFloat(longitude);
  const mapValid = Number.isFinite(lat) && Number.isFinite(lon) &&
    lat >= -90 && lat <= 90 && lon >= -180 && lon <= 180;

  const regionStatus = useMemo(() => {
    if (!mapValid) return "Enter a valid coordinate";
    return lat >= DEFAULT_REGION.minLat && lat <= DEFAULT_REGION.maxLat &&
      lon >= DEFAULT_REGION.minLon && lon <= DEFAULT_REGION.maxLon
      ? "Inside supported cocoa region"
      : "Outside supported cocoa region";
  }, [lat, lon, mapValid]);

  async function checkEnvironmentalData() {
    setLoading(true);
    setEvidenceState(null);
    setProofResult(null);
    setProveError(null);
    setShareError(null);
    try {
      const resp = await fetch(`${BACKEND_URL}/api/check-location`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ latitude: lat, longitude: lon }),
      });
      const data = await resp.json();
      setEvidenceState(data);
    } catch (e: any) {
      setEvidenceState({ ok: false, error: `Network error reaching evidence service: ${e.message}` });
    } finally {
      setLoading(false);
    }
  }

  async function generateZkProof() {
    if (!evidenceState?.ok || !evidenceState.evidence || !evidenceState.evidence_session || !evidenceState.evidence_hash) return;
    if (!supplierId || !supplierSecret) {
      setProveError("Enter a supplier identifier and secret. They are used only for the local proof witness.");
      return;
    }
    const quantity = parseFloat(quantityKg);
    if (!Number.isFinite(quantity) || quantity < 0) {
      setProveError("Production quantity must be a non-negative number.");
      return;
    }

    setProving(true);
    setProveError(null);
    setProofResult(null);
    try {
      const ev = evidenceState.evidence;
      const result = await generateProof({
        latitude: lat,
        longitude: lon,
        quantityKg: quantity,
        supplierId,
        supplierSecret,
        region: DEFAULT_REGION,
        quantityThresholdKg: DEFAULT_THRESHOLD_KG,
        allowedLandCoverCode: DEFAULT_ALLOWED_LAND_COVER_CODE,
        evidenceProtectedFlag: ev.protected_area.status,
        evidenceLandCoverCode: ev.land_cover.code,
        evidenceHash: evidenceState.evidence_hash,
      });
      setProofResult(result);
    } catch (e: any) {
      setProveError(
        "The circuit rejected this witness. " +
        (e?.message || String(e))
      );
    } finally {
      setProving(false);
    }
  }

  async function createVerification() {
    if (!proofResult || !evidenceState?.evidence_session) return;
    setSharing(true);
    setShareError(null);
    try {
      const resp = await fetch(`${BACKEND_URL}/api/verify-proof`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          proof: proofResult.proof,
          public_signals: proofResult.publicSignals,
          evidence_session: evidenceState.evidence_session,
        }),
      });
      const data = await resp.json();
      if (!resp.ok || !data.zk_proof_valid || !data.verification_id) {
        throw new Error(data.error || "Proof could not be verified.");
      }
      onVerificationCreated(data.verification_id);
    } catch (e: any) {
      setShareError(e?.message || String(e));
    } finally {
      setSharing(false);
    }
  }

  const protectedPass = evidenceState?.ok && evidenceState.evidence && !evidenceState.evidence.protected_area.status;
  const landPass = evidenceState?.ok && evidenceState.evidence &&
    evidenceState.evidence.land_cover.code === DEFAULT_ALLOWED_LAND_COVER_CODE;
  const quantityPass = Number.isFinite(parseFloat(quantityKg)) && parseFloat(quantityKg) <= DEFAULT_THRESHOLD_KG;

  return (
    <main>
      <section className="intro-grid">
        <div>
          <div className="step-number">01</div>
          <h2>Supplier: create a private proof</h2>
          <p>
            Check live environmental evidence first. Your exact coordinate is sent to the evidence
            lookup service because the current MVP performs the geospatial lookup server-side.
            Your quantity and supplier secret stay in the browser.
          </p>
        </div>
        <div className="constraint-card">
          <span>Public constraints</span>
          <strong>West African cocoa region</strong>
          <strong>≤ {DEFAULT_THRESHOLD_KG.toLocaleString()} kg</strong>
          <strong>Allowed land-cover code: {DEFAULT_ALLOWED_LAND_COVER_CODE}</strong>
        </div>
      </section>

      <div className="workflow">
        <section className="card form-card">
          <div className="card-kicker">STEP 1 · PRIVATE INPUT</div>
          <h3>Farm & supplier data</h3>

          <label>Commodity<input value="Cocoa" disabled /></label>

          <div className="two-col">
            <label>
              Latitude
              <input value={latitude} onChange={(e) => setLatitude(e.target.value)} placeholder="6.6666" />
              <small>Used for live evidence lookup</small>
            </label>
            <label>
              Longitude
              <input value={longitude} onChange={(e) => setLongitude(e.target.value)} placeholder="-1.6163" />
              <small>Used for live evidence lookup</small>
            </label>
          </div>

          <div className="status-line">
            <span>Region constraint</span>
            <strong className={regionStatus.startsWith("Inside") ? "pass" : "fail"}>{regionStatus}</strong>
          </div>

          <label>
            Production quantity (kg)
            <input value={quantityKg} onChange={(e) => setQuantityKg(e.target.value)} />
            <small>Kept in the browser and proven ≤ {DEFAULT_THRESHOLD_KG.toLocaleString()} kg.</small>
          </label>

          <label>
            Supplier ID
            <input value={supplierId} onChange={(e) => setSupplierId(e.target.value)} placeholder="Your private identifier" />
          </label>

          <label>
            Supplier secret
            <input type="password" value={supplierSecret} onChange={(e) => setSupplierSecret(e.target.value)} placeholder="A secret only you control" />
          </label>

          <div className="privacy-strip">
            <span>PRIVATE WITNESS</span>
            <b>Quantity + supplier ID + secret</b>
            <span>stay in the browser</span>
          </div>

          <button disabled={loading || !mapValid} onClick={checkEnvironmentalData} className="primary wide">
            {loading ? "Checking live evidence…" : "CHECK ENVIRONMENTAL EVIDENCE"}
          </button>
        </section>

        <section className="card map-card">
          <div className="card-kicker">SUPPLIER-ONLY VIEW</div>
          <h3>Exact plot location</h3>
          {mapValid ? (
            <MapContainer center={[lat, lon]} zoom={9} style={{ height: 440, width: "100%" }}>
              <TileLayer
                attribution='&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors'
                url="https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png"
              />
              <Marker position={[lat, lon]}>
                <Popup>Exact supplier-entered coordinate. This map is not shown in the auditor verification.</Popup>
              </Marker>
            </MapContainer>
          ) : (
            <div className="map-placeholder">Enter a valid latitude and longitude.</div>
          )}
          <p className="map-note">The auditor never receives this map or the exact coordinate.</p>
        </section>
      </div>

      {evidenceState && !evidenceState.ok && (
        <section className="card error-box">
          <strong>Environmental evidence lookup failed.</strong>
          <p>{evidenceState.error}</p>
          {evidenceState.note && <p>{evidenceState.note}</p>}
        </section>
      )}

      {evidenceState?.ok && evidenceState.evidence && (
        <section className="card">
          <div className="card-kicker">STEP 2 · REAL EVIDENCE</div>
          <h3>Environmental checks</h3>
          <div className="check-grid">
            <Check label="Supported region" pass={regionStatus.startsWith("Inside")} />
            <Check label="Protected-area check" pass={!!protectedPass} />
            <Check label="Allowed land cover" pass={!!landPass} />
            <Check label="Quantity threshold" pass={quantityPass} />
          </div>
          <EvidencePanel evidence={evidenceState.evidence} />
          <button disabled={proving} onClick={generateZkProof} className="primary wide">
            {proving ? "Generating Groth16 proof locally…" : "GENERATE PRIVATE ZK PROOF"}
          </button>
        </section>
      )}

      {proveError && <section className="card error-box"><strong>Proof rejected</strong><p>{proveError}</p></section>}

      {proofResult && (
        <section className="card success-card">
          <div className="card-kicker">STEP 3 · CRYPTOGRAPHIC PROOF</div>
          <div className="verified-heading">
            <div className="verified-icon">✓</div>
            <div><h3>Proof generated locally</h3><p>The witness satisfied the circuit constraints.</p></div>
          </div>
          <div className="check-grid">
            <Check label="Groth16 proof generated" pass />
            <Check label="Private quantity disclosed" pass={false} />
            <Check label="Supplier secret disclosed" pass={false} />
            <Check label="Exact coordinates shared with auditor" pass={false} />
          </div>

          <div className="share-box">
            <h4>Create a shareable verification</h4>
            <p>
              The backend-issued evidence commitment binds this proof to its environmental lookup.
              Only the proof, public signals and sanitized provenance are stored in memory.
            </p>
            <button disabled={sharing} onClick={createVerification} className="primary">
              {sharing ? "Verifying proof…" : "VERIFY & CREATE SHARE LINK"}
            </button>
            {shareError && <p className="fail">{shareError}</p>}
          </div>

          <details>
            <summary>Technical proof artifacts</summary>
            <pre>{JSON.stringify(proofResult.proof, null, 2)}</pre>
            <pre>{JSON.stringify(proofResult.publicSignals, null, 2)}</pre>
          </details>
        </section>
      )}
    </main>
  );
}

function Check({ label, pass }: { label: string; pass: boolean }) {
  return (
    <div className={`check ${pass ? "check-pass" : "check-private"}`}>
      <span>{pass ? "✓" : "—"}</span>
      <div><strong>{label}</strong><small>{pass ? "PASS" : "NOT DISCLOSED"}</small></div>
    </div>
  );
}

function EvidencePanel({ evidence }: { evidence: any }) {
  return (
    <div className="evidence-panel">
      <div className="evidence-head">
        <div><strong>Dataset provenance</strong><span>Live lookup · {evidence.protected_area.retrieved_at}</span></div>
        <span className="pill">SOURCE-BACKED</span>
      </div>
      <div className="provenance-grid">
        <div><span>Protected areas</span><strong>{evidence.protected_area.source}</strong><small>{evidence.protected_area.dataset}</small></div>
        <div><span>Land cover</span><strong>{evidence.land_cover.source}</strong><small>{evidence.land_cover.dataset}</small></div>
        <div><span>Query radius</span><strong>{evidence.protected_area.query_radius_m} m</strong><small>OSM/Overpass search radius</small></div>
        <div><span>Classification</span><strong>{evidence.land_cover.classification}</strong><small>Code {evidence.land_cover.code}</small></div>
      </div>
      <p className="source-note">{evidence.land_cover.note}</p>
      <p className="source-note">Raw provider response fingerprint: <code>{evidence.land_cover.raw_response_sha256}</code></p>
      {evidence.evidence_hash && <p className="source-note">Evidence commitment: <code>{evidence.evidence_hash}</code></p>}
    </div>
  );
}

function AuditorView({ initialId }: { initialId: string | null }) {
  const [verificationId, setVerificationId] = useState(initialId || "");
  const [proofText, setProofText] = useState("");
  const [publicText, setPublicText] = useState("");
  const [verifying, setVerifying] = useState(false);
  const [result, setResult] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (initialId) {
      setVerificationId(initialId);
      loadVerification(initialId);
    }
  }, [initialId]);

  async function loadVerification(id: string) {
    if (!id.trim()) return;
    setVerifying(true);
    setError(null);
    setResult(null);
    try {
      const resp = await fetch(`${BACKEND_URL}/api/verifications/${encodeURIComponent(id.trim())}`);
      const data = await resp.json();
      if (!resp.ok) throw new Error(data.error || "Verification not found.");
      setResult(data);
    } catch (e: any) {
      setError(e?.message || String(e));
    } finally {
      setVerifying(false);
    }
  }

  async function verifyPasted() {
    setVerifying(true);
    setResult(null);
    setError(null);
    try {
      const proof = JSON.parse(proofText);
      const publicSignals = JSON.parse(publicText);
      const zk_proof_valid = await verifyProofLocally(proof, publicSignals);
      setResult({ zk_proof_valid, public_signals: publicSignals });
    } catch (e: any) {
      setError("Could not parse or verify the submitted proof: " + (e?.message || String(e)));
    } finally {
      setVerifying(false);
    }
  }

  const valid = result?.zk_proof_valid === true;
  const shareUrl = result?.verification_id
    ? `${window.location.origin}${window.location.pathname}?verification=${encodeURIComponent(result.verification_id)}`
    : "";

  async function copyShareLink() {
    if (!shareUrl) return;
    await navigator.clipboard.writeText(shareUrl);
  }

  return (
    <main>
      <section className="intro-grid">
        <div>
          <div className="step-number">02</div>
          <h2>Auditor: verify without seeing the farm</h2>
          <p>
            The auditor needs a result, not the supplier's private witness. Use a GreenProof ID for
            the clean demo flow, or paste proof artifacts for technical verification.
          </p>
        </div>
        <div className="privacy-card">
          <strong>Hidden from auditor</strong>
          <span>Exact coordinates</span>
          <span>Production volume</span>
          <span>Supplier secret</span>
        </div>
      </section>

      <section className={`card verification-card ${result ? (valid ? "valid" : "invalid") : ""}`}>
        <div className="card-kicker">PRIMARY DEMO FLOW</div>
        <h3>GreenProof verification</h3>
        <div className="id-row">
          <input value={verificationId} onChange={(e) => setVerificationId(e.target.value)} placeholder="GP-XXXXXXXX" />
          <button disabled={verifying || !verificationId} onClick={() => loadVerification(verificationId)}>
            {verifying ? "Checking…" : "VERIFY ID"}
          </button>
        </div>

        {error && <div className="error-box"><strong>Verification failed</strong><p>{error}</p></div>}

        {result && (
          <div className="verification-result">
            <div className="big-status">
              <span className="status-dot">{valid ? "✓" : "×"}</span>
              <div><span>CRYPTOGRAPHIC STATUS</span><strong>{valid ? "VERIFIED" : "NOT VERIFIED"}</strong></div>
            </div>

            <div className="certificate">
              <div className="certificate-row"><span>Commodity</span><strong>Cocoa</strong></div>
              <div className="certificate-row">
                <span>Environmental condition</span>
                <strong>Protected-area status &amp; land-cover classification (West African cocoa belt)</strong>
              </div>
              <div className="certificate-row">
                <span>Result</span>
                <strong className={valid ? "pass" : "fail"}>{valid ? "PASS" : "FAIL"}</strong>
              </div>
              <div className="certificate-row">
                <span>ZK verification (Groth16 / BN254)</span>
                <strong className={valid ? "pass" : "fail"}>{valid ? "VALID" : "INVALID"}</strong>
              </div>
              <div className="certificate-row"><span>Proof ID</span><strong>{result.verification_id || "—"}</strong></div>
              <div className="certificate-row"><span>Created</span><strong>{result.created_at || "—"}</strong></div>
              {result.evidence && (
                <div className="certificate-row">
                  <span>Evidence source</span>
                  <strong>{result.evidence.protected_area.source}</strong>
                </div>
              )}
              {result.evidence && (
                <div className="certificate-row">
                  <span>Evidence retrieved</span>
                  <strong>{result.evidence.protected_area.retrieved_at}</strong>
                </div>
              )}
              <div className="certificate-row certificate-private">
                <span>Exact coordinates</span><strong>PRIVATE — never disclosed</strong>
              </div>
            </div>

            <div className="check-grid">
              <Check label="Exact coordinates disclosed" pass={false} />
              <Check label="Production quantity disclosed" pass={false} />
              <Check label="Supplier secret disclosed" pass={false} />
              <Check label="Environmental evidence provenance" pass={!!result.evidence} />
            </div>
            {result.evidence && <AuditorEvidence evidence={result.evidence} />}
            {shareUrl && (
              <div className="share-link">
                <div><span>SHAREABLE VERIFICATION</span><code>{shareUrl}</code></div>
                <button onClick={copyShareLink}>COPY LINK</button>
              </div>
            )}
          </div>
        )}
      </section>

      <details className="technical-card">
        <summary>Advanced: verify raw proof artifacts</summary>
        <div className="card">
          <p>For judges who want to inspect the cryptographic path directly.</p>
          <label>proof.json<textarea rows={7} value={proofText} onChange={(e) => setProofText(e.target.value)} /></label>
          <label>public.json<textarea rows={4} value={publicText} onChange={(e) => setPublicText(e.target.value)} /></label>
          <button disabled={verifying || !proofText || !publicText} onClick={verifyPasted} className="primary">
            {verifying ? "Verifying…" : "VERIFY RAW PROOF"}
          </button>
        </div>
      </details>
    </main>
  );
}

function AuditorEvidence({ evidence }: { evidence: any }) {
  return (
    <div className="evidence-panel auditor-evidence">
      <div className="evidence-head">
        <div><strong>Environmental evidence provenance</strong><span>Exact coordinates withheld</span></div>
        <span className="pill">PRIVATE LOCATION</span>
      </div>
      <div className="provenance-grid">
        <div><span>Protected areas</span><strong>{evidence.protected_area.source}</strong><small>{evidence.protected_area.dataset}</small></div>
        <div><span>Land cover</span><strong>{evidence.land_cover.source}</strong><small>{evidence.land_cover.dataset}</small></div>
        <div><span>Query radius</span><strong>{evidence.protected_area.query_radius_m} m</strong><small>No coordinates stored</small></div>
        <div><span>Land-cover result</span><strong>{evidence.land_cover.classification}</strong><small>Code {evidence.land_cover.code}</small></div>
      </div>
      <p className="source-note">{evidence.land_cover.note}</p>
      <p className="source-note">Evidence commitment: <code>{evidence.evidence_hash}</code></p>
    </div>
  );
}
