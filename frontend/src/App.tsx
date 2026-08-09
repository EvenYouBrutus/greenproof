import { useEffect, useMemo, useState, type ReactNode } from "react";
import { MapContainer, TileLayer, Marker, Popup, useMap, useMapEvents } from "react-leaflet";
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
type StageStatus = "idle" | "active" | "processing" | "completed" | "failed";

const WORKFLOW_STAGES = [
  { id: 1, label: "Private inputs" },
  { id: 2, label: "Environmental evidence" },
  { id: 3, label: "Generate proof" },
  { id: 4, label: "Verify" },
  { id: 5, label: "Result" },
] as const;

export default function App() {
  const [tab, setTab] = useState<Tab>("supplier");
  const [verificationId, setVerificationId] = useState<string | null>(null);

  function handleVerificationCreated(id: string) {
    setVerificationId(id);
    window.history.replaceState({}, "", `?verification=${encodeURIComponent(id)}`);
    setTab("auditor");
  }

  function startVerification() {
    setTab("supplier");
    document.getElementById("workflow")?.scrollIntoView({ behavior: "smooth", block: "start" });
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
        <header className="topbar">
          <div className="brand">
            <span className="brand-mark" aria-hidden="true" />
            <div>
              <strong>GreenProof</strong>
              <span>Environmental compliance · Privacy</span>
            </div>
          </div>
          <nav className="tabs" aria-label="Role">
            <button
              type="button"
              className={tab === "supplier" ? "active" : ""}
              onClick={() => setTab("supplier")}
            >
              Supplier
            </button>
            <button
              type="button"
              className={tab === "auditor" ? "active" : ""}
              onClick={() => setTab("auditor")}
            >
              Auditor
            </button>
          </nav>
        </header>

        <section className="hero">
          <div className="hero-copy-block">
            <p className="eyebrow">Privacy-preserving environmental verification</p>
            <h1>
              Prove environmental compliance.
              <br />
              Keep your location private.
            </h1>
            <p className="hero-lead">
              Generate a zero-knowledge proof that an environmental policy is satisfied
              without revealing the exact coordinates of the underlying site.
            </p>
            <div className="hero-actions">
              <button type="button" className="btn btn-primary" onClick={startVerification}>
                Start verification
              </button>
              <button
                type="button"
                className="btn btn-ghost"
                onClick={() => {
                  setTab("auditor");
                  document.getElementById("workflow")?.scrollIntoView({ behavior: "smooth" });
                }}
              >
                View as auditor
              </button>
            </div>
          </div>

          <div className="concept-flow" aria-label="How GreenProof works">
            <ConceptStep index="01" title="Private site" detail="Exact coordinates stay protected" />
            <div className="concept-arrow" aria-hidden="true" />
            <ConceptStep index="02" title="Environmental evidence" detail="Real land-cover data" />
            <div className="concept-arrow" aria-hidden="true" />
            <ConceptStep index="03" title="Zero-knowledge proof" detail="Compliance without disclosure" />
            <div className="concept-arrow" aria-hidden="true" />
            <ConceptStep index="04" title="Verified compliance" detail="Cryptographically checkable" highlight />
          </div>
        </section>

        <section className="privacy-split" aria-label="What each party sees">
          <article className="split-card split-knows">
            <p className="split-label">Supplier knows</p>
            <ul>
              <li>Exact coordinates</li>
              <li>Production data</li>
              <li>Secret</li>
            </ul>
          </article>
          <article className="split-card split-receives">
            <p className="split-label">Auditor receives</p>
            <ul>
              <li>Verification result</li>
              <li>Environmental evidence</li>
              <li>Cryptographic proof</li>
            </ul>
          </article>
          <article className="split-card split-hidden">
            <p className="split-label">Auditor does not receive</p>
            <ul>
              <li>Exact coordinates</li>
              <li>Private inputs</li>
            </ul>
          </article>
        </section>

        <section className="pillars" aria-label="Product pillars">
          <article className="pillar">
            <span className="pillar-index">01</span>
            <h2>Environment</h2>
            <p>Real environmental evidence from open geospatial sources.</p>
          </article>
          <article className="pillar">
            <span className="pillar-index">02</span>
            <h2>Privacy</h2>
            <p>Sensitive location remains protected by the proof system.</p>
          </article>
          <article className="pillar">
            <span className="pillar-index">03</span>
            <h2>Verification</h2>
            <p>The result is cryptographically verifiable.</p>
          </article>
        </section>

        <div id="workflow">
          {tab === "supplier" ? (
            <SupplierView onVerificationCreated={handleVerificationCreated} />
          ) : (
            <AuditorView initialId={verificationId} />
          )}
        </div>

        <footer className="site-footer">
          <p>
            <strong>Prototype limitation.</strong> GreenProof is not an official certification system.
            Land cover comes from ESA WorldCover 2021 v200 satellite-derived data; it is not historical
            satellite deforestation detection.
          </p>
        </footer>
      </div>
    </div>
  );
}

function ConceptStep({
  index,
  title,
  detail,
  highlight,
}: {
  index: string;
  title: string;
  detail: string;
  highlight?: boolean;
}) {
  return (
    <div className={`concept-step${highlight ? " concept-step-highlight" : ""}`}>
      <span className="concept-index">{index}</span>
      <strong>{title}</strong>
      <span>{detail}</span>
    </div>
  );
}

function WorkflowProgress({
  stages,
}: {
  stages: { id: number; label: string; status: StageStatus }[];
}) {
  return (
    <ol className="workflow-progress" aria-label="Verification progress">
      {stages.map((stage, i) => (
        <li key={stage.id} className={`progress-step progress-${stage.status}`}>
          <div className="progress-node">
            <span className="progress-num">{String(stage.id).padStart(2, "0")}</span>
            {i < stages.length - 1 && <span className="progress-line" aria-hidden="true" />}
          </div>
          <span className="progress-label">{stage.label}</span>
        </li>
      ))}
    </ol>
  );
}

function StatusBadge({
  children,
  tone = "neutral",
}: {
  children: ReactNode;
  tone?: "neutral" | "private" | "success" | "danger" | "info" | "warn";
}) {
  return <span className={`status-badge tone-${tone}`}>{children}</span>;
}

function MetaRow({
  label,
  value,
  mono,
  badge,
}: {
  label: string;
  value: ReactNode;
  mono?: boolean;
  badge?: ReactNode;
}) {
  return (
    <div className="meta-row">
      <span className="meta-label">{label}</span>
      <div className="meta-value">
        {badge}
        <span className={mono ? "mono" : undefined}>{value}</span>
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
  const mapValid =
    Number.isFinite(lat) &&
    Number.isFinite(lon) &&
    lat >= -90 &&
    lat <= 90 &&
    lon >= -180 &&
    lon <= 180;

  const regionStatus = useMemo(() => {
    if (!mapValid) return "Enter a valid coordinate";
    return lat >= DEFAULT_REGION.minLat &&
      lat <= DEFAULT_REGION.maxLat &&
      lon >= DEFAULT_REGION.minLon &&
      lon <= DEFAULT_REGION.maxLon
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
    if (
      !evidenceState?.ok ||
      !evidenceState.evidence ||
      !evidenceState.evidence_session ||
      !evidenceState.evidence_hash
    )
      return;
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
      setProveError("The circuit rejected this witness. " + (e?.message || String(e)));
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

  const protectedPass =
    evidenceState?.ok && evidenceState.evidence && !evidenceState.evidence.protected_area.status;
  const landPass =
    evidenceState?.ok &&
    evidenceState.evidence &&
    evidenceState.evidence.land_cover.code === DEFAULT_ALLOWED_LAND_COVER_CODE;
  const quantityPass =
    Number.isFinite(parseFloat(quantityKg)) && parseFloat(quantityKg) <= DEFAULT_THRESHOLD_KG;
  const policySatisfied = !!(protectedPass && landPass && regionStatus.startsWith("Inside"));

  const stageStatuses: StageStatus[] = useMemo(() => {
    const s1: StageStatus = "completed";
    let s2: StageStatus = "idle";
    if (loading) s2 = "processing";
    else if (evidenceState && !evidenceState.ok) s2 = "failed";
    else if (evidenceState?.ok) s2 = "completed";
    else s2 = "active";

    let s3: StageStatus = "idle";
    if (proving) s3 = "processing";
    else if (proveError) s3 = "failed";
    else if (proofResult) s3 = "completed";
    else if (evidenceState?.ok) s3 = "active";

    let s4: StageStatus = "idle";
    if (sharing) s4 = "processing";
    else if (shareError) s4 = "failed";
    else if (proofResult && !shareError) s4 = "active";

    const s5: StageStatus = "idle";
    return [s1, s2, s3, s4, s5];
  }, [loading, evidenceState, proving, proveError, proofResult, sharing, shareError]);

  const progressStages = WORKFLOW_STAGES.map((stage, i) => ({
    ...stage,
    status: stageStatuses[i],
  }));

  return (
    <main className="main-panel">
      <div className="panel-header">
        <div>
          <p className="eyebrow">Supplier workflow</p>
          <h2>Create a private compliance proof</h2>
          <p className="panel-desc">
            Coordinates are used for the environmental evidence lookup. Quantity and supplier secret
            remain in the browser as private witness inputs.
          </p>
        </div>
        <div className="constraint-card">
          <p className="split-label">Public constraints</p>
          <MetaRow label="Region" value="West African cocoa belt" />
          <MetaRow label="Quantity" value={`≤ ${DEFAULT_THRESHOLD_KG.toLocaleString()} kg`} />
          <MetaRow label="Land cover" value={`Code ${DEFAULT_ALLOWED_LAND_COVER_CODE}`} />
        </div>
      </div>

      <WorkflowProgress stages={progressStages} />

      <div className="workflow-grid">
        <section className={`card stage-card stage-${stageStatuses[0]}`}>
          <div className="card-header">
            <div>
              <p className="card-kicker">01 · Private inputs</p>
              <h3>Sensitive site data</h3>
            </div>
            <StatusBadge tone="private">Private</StatusBadge>
          </div>
          <p className="card-support">Used to generate the proof.</p>

          <div className="private-fields">
            <div className="field-group">
              <div className="field-head">
                <label htmlFor="lat">Exact coordinates</label>
                <StatusBadge tone="private">Private</StatusBadge>
              </div>
              <div className="two-col">
                <input
                  id="lat"
                  value={latitude}
                  onChange={(e) => setLatitude(e.target.value)}
                  placeholder="Latitude"
                  inputMode="decimal"
                />
                <input
                  value={longitude}
                  onChange={(e) => setLongitude(e.target.value)}
                  placeholder="Longitude"
                  inputMode="decimal"
                  aria-label="Longitude"
                />
              </div>
              <div className="status-line">
                <span>Region constraint</span>
                <strong className={regionStatus.startsWith("Inside") ? "pass" : "fail"}>
                  {regionStatus}
                </strong>
              </div>
              <small>Sent to the evidence lookup for geospatial classification.</small>
            </div>

            <div className="field-group">
              <div className="field-head">
                <label htmlFor="qty">Production quantity</label>
                <StatusBadge tone="private">Private</StatusBadge>
              </div>
              <input
                id="qty"
                value={quantityKg}
                onChange={(e) => setQuantityKg(e.target.value)}
                inputMode="decimal"
              />
              <small>
                Kept in the browser. Proven ≤ {DEFAULT_THRESHOLD_KG.toLocaleString()} kg without
                disclosure.
              </small>
            </div>

            <div className="field-group">
              <div className="field-head">
                <label htmlFor="sid">Supplier identifier</label>
                <StatusBadge tone="private">Private</StatusBadge>
              </div>
              <input
                id="sid"
                value={supplierId}
                onChange={(e) => setSupplierId(e.target.value)}
                placeholder="Your private identifier"
              />
            </div>

            <div className="field-group">
              <div className="field-head">
                <label htmlFor="secret">Supplier secret</label>
                <StatusBadge tone="private">Private</StatusBadge>
              </div>
              <input
                id="secret"
                type="password"
                value={supplierSecret}
                onChange={(e) => setSupplierSecret(e.target.value)}
                placeholder="A secret only you control"
                autoComplete="off"
              />
              <small>Local witness only — not shared with the auditor.</small>
            </div>
          </div>

          <div className="privacy-strip">
            <StatusBadge tone="private">Private witness</StatusBadge>
            <span>Quantity, supplier ID, and secret stay in the browser.</span>
          </div>

          <button
            type="button"
            disabled={loading || !mapValid}
            onClick={checkEnvironmentalData}
            className="btn btn-primary wide"
          >
            {loading ? "Checking environmental evidence…" : "Check environmental evidence"}
          </button>
        </section>

        <section className="card map-card">
          <div className="card-header">
            <div>
              <p className="card-kicker">Supplier-only view</p>
              <h3>Exact plot location</h3>
            </div>
            <StatusBadge tone="private">Not shared</StatusBadge>
          </div>
          {mapValid ? (
            <MapContainer center={[lat, lon]} zoom={9} style={{ height: 360, width: "100%" }}>
              <TileLayer
                attribution='&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors'
                url="https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png"
              />
              <MapController
                lat={lat}
                lon={lon}
                onLocationSelect={(newLat, newLon) => {
                  setLatitude(newLat.toFixed(6));
                  setLongitude(newLon.toFixed(6));
                }}
              />
            </MapContainer>
          ) : (
            <div className="map-placeholder">Enter a valid latitude and longitude.</div>
          )}
          <p className="map-note">The auditor never receives this map or the exact coordinate.</p>
        </section>
      </div>

      {loading && (
        <section className="card stage-card stage-processing">
          <div className="card-header">
            <div>
              <p className="card-kicker">02 · Environmental evidence</p>
              <h3>Looking up live data…</h3>
            </div>
            <StatusBadge tone="info">Processing</StatusBadge>
          </div>
          <div className="loading-bar" aria-hidden="true">
            <span />
          </div>
        </section>
      )}

      {evidenceState && !evidenceState.ok && (
        <section className="card failure-card" role="alert">
          <div className="failure-icon">×</div>
          <div>
            <p className="card-kicker">02 · Environmental evidence</p>
            <h3>Environmental evidence unavailable</h3>
            <p>{evidenceState.error}</p>
            {evidenceState.note && <p className="muted">{evidenceState.note}</p>}
          </div>
        </section>
      )}

      {evidenceState?.ok && evidenceState.evidence && (
        <section className={`card stage-card stage-${stageStatuses[1]} evidence-artifact`}>
          <div className="card-header">
            <div>
              <p className="card-kicker">02 · Environmental evidence</p>
              <h3>Verifiable data artifact</h3>
            </div>
            <StatusBadge tone={policySatisfied ? "success" : "warn"}>
              {policySatisfied ? "Verified" : "Not satisfied"}
            </StatusBadge>
          </div>

          <div className="artifact-grid">
            <MetaRow
              label="Source"
              value={evidenceState.evidence.land_cover?.source || "ESA WorldCover"}
            />
            <MetaRow
              label="Land cover"
              value={
                <>
                  {evidenceState.evidence.land_cover.classification}
                  <span className="muted-inline"> · code {evidenceState.evidence.land_cover.code}</span>
                </>
              }
            />
            <MetaRow
              label="Policy"
              value="Protected-area status & allowed land cover (West African cocoa belt)"
            />
            <MetaRow
              label="Status"
              value={policySatisfied ? "Policy checks passed" : "Policy not fully satisfied"}
              badge={
                <StatusBadge tone={policySatisfied ? "success" : "warn"}>
                  {policySatisfied ? "Verified" : "Not satisfied"}
                </StatusBadge>
              }
            />
          </div>

          <div className="check-grid">
            <Check label="Supported region" pass={regionStatus.startsWith("Inside")} />
            <Check label="Protected-area check" pass={!!protectedPass} />
            <Check label="Allowed land cover" pass={!!landPass} />
            <Check label="Quantity threshold" pass={quantityPass} />
          </div>

          {!policySatisfied && (
            <div className="inline-warn" role="status">
              <strong>Policy not satisfied</strong>
              <span>
                Evidence was retrieved, but one or more environmental constraints did not pass.
                Proof generation may still be attempted; the circuit will reject invalid witnesses.
              </span>
            </div>
          )}

          <EvidencePanel
            evidence={evidenceState.evidence}
            evidenceHash={evidenceState.evidence_hash}
          />

          <button
            type="button"
            disabled={proving}
            onClick={generateZkProof}
            className="btn btn-primary wide"
          >
            {proving ? "Generating zero-knowledge proof…" : "Generate zero-knowledge proof"}
          </button>
        </section>
      )}

      {proving && (
        <section className="card stage-card stage-processing">
          <div className="card-header">
            <div>
              <p className="card-kicker">03 · Zero-knowledge proof</p>
              <h3>Generating…</h3>
            </div>
            <StatusBadge tone="info">Processing</StatusBadge>
          </div>
          <p className="card-support">Proving runs locally in the browser. Private inputs are not sent as plain text to the auditor.</p>
          <div className="loading-bar" aria-hidden="true">
            <span />
          </div>
          <div className="proof-summary">
            <MetaRow label="Private inputs" value="Hidden" badge={<StatusBadge tone="private">Hidden</StatusBadge>} />
            <MetaRow label="Proof" value="Generating" badge={<StatusBadge tone="info">In progress</StatusBadge>} />
          </div>
        </section>
      )}

      {proveError && (
        <section className="card failure-card" role="alert">
          <div className="failure-icon">×</div>
          <div>
            <p className="card-kicker">03 · Zero-knowledge proof</p>
            <h3>Proof generation failed</h3>
            <p>{proveError}</p>
          </div>
        </section>
      )}

      {proofResult && (
        <section className="card stage-card stage-completed proof-card">
          <div className="card-header">
            <div>
              <p className="card-kicker">03 · Zero-knowledge proof</p>
              <h3 className="success-title">
                <span className="success-check" aria-hidden="true">
                  ✓
                </span>
                Proof generated
              </h3>
            </div>
            <StatusBadge tone="success">Valid witness</StatusBadge>
          </div>

          <div className="proof-summary">
            <MetaRow
              label="Private inputs"
              value="Hidden"
              badge={<StatusBadge tone="private">Hidden</StatusBadge>}
            />
            <MetaRow
              label="Proof"
              value="Valid"
              badge={<StatusBadge tone="success">Valid</StatusBadge>}
            />
            <MetaRow
              label="Local circuit"
              value="Constraints satisfied"
              badge={<StatusBadge tone="success">Pass</StatusBadge>}
            />
          </div>

          <div className="check-grid">
            <Check label="Groth16 proof generated" pass />
            <Check label="Private quantity disclosed" pass={false} />
            <Check label="Supplier secret disclosed" pass={false} />
            <Check label="Exact coordinates shared with auditor" pass={false} />
          </div>

          <div className={`share-box${sharing ? " is-processing" : ""}${shareError ? " is-failed" : ""}`}>
            <p className="card-kicker">04 · Verify</p>
            <h4>Create a shareable verification</h4>
            <p>
              The backend-issued evidence commitment binds this proof to its environmental lookup.
              Only the proof, public signals, and sanitized provenance are stored in memory.
            </p>
            <button
              type="button"
              disabled={sharing}
              onClick={createVerification}
              className="btn btn-primary"
            >
              {sharing ? "Verifying proof…" : "Verify & create share link"}
            </button>
            {shareError && (
              <div className="inline-fail" role="alert">
                <strong>Verification failed</strong>
                <span>{shareError}</span>
              </div>
            )}
          </div>

          <details className="tech-details">
            <summary>Technical details</summary>
            <div className="tech-body">
              <div className="tech-chips">
                <span>Groth16</span>
                <span>Circom</span>
                <span>Poseidon</span>
                <span>BN254</span>
              </div>
              <p className="muted">Proof artifact</p>
              <pre>{JSON.stringify(proofResult.proof, null, 2)}</pre>
              <p className="muted">Public signals</p>
              <pre>{JSON.stringify(proofResult.publicSignals, null, 2)}</pre>
            </div>
          </details>
        </section>
      )}
    </main>
  );
}

function Check({ label, pass }: { label: string; pass: boolean }) {
  return (
    <div className={`check ${pass ? "check-pass" : "check-private"}`}>
      <span aria-hidden="true">{pass ? "✓" : "—"}</span>
      <div>
        <strong>{label}</strong>
        <small>{pass ? "Pass" : "Not disclosed"}</small>
      </div>
    </div>
  );
}

function MapController({ lat, lon, onLocationSelect }: { lat: number; lon: number; onLocationSelect: (lat: number, lon: number) => void }) {
  const map = useMap();

  useMapEvents({
    click(e) {
      onLocationSelect(e.latlng.lat, e.latlng.lng);
    },
  });

  useEffect(() => {
    map.setView([lat, lon], map.getZoom());
  }, [lat, lon, map]);

  return (
    <Marker position={[lat, lon]}>
      <Popup>
        Exact supplier-entered coordinate. This map is not shown in the auditor verification.
      </Popup>
    </Marker>
  );
}

function EvidencePanel({
  evidence,
  evidenceHash,
}: {
  evidence: any;
  evidenceHash?: string;
}) {
  return (
    <div className="evidence-panel">
      <div className="evidence-head">
        <div>
          <strong>Dataset provenance</strong>
          <span>Live lookup · {evidence.protected_area.retrieved_at}</span>
        </div>
        <StatusBadge tone="info">Source-backed</StatusBadge>
      </div>
      <div className="provenance-grid">
        <div>
          <span>Protected areas</span>
          <strong>{evidence.protected_area.source}</strong>
          <small>{evidence.protected_area.dataset}</small>
        </div>
        <div>
          <span>Land cover</span>
          <strong>{evidence.land_cover.source}</strong>
          <small>{evidence.land_cover.dataset}</small>
        </div>
        <div>
          <span>Query radius</span>
          <strong>{evidence.protected_area.query_radius_m} m</strong>
          <small>OSM/Overpass search radius</small>
        </div>
        <div>
          <span>Classification</span>
          <strong>{evidence.land_cover.classification}</strong>
          <small>Code {evidence.land_cover.code}</small>
        </div>
      </div>
      {evidence.land_cover.note && <p className="source-note">{evidence.land_cover.note}</p>}
      {evidence.land_cover.raw_response_sha256 && (
        <p className="source-note">
          Raw provider response fingerprint:{" "}
          <code className="mono">{evidence.land_cover.raw_response_sha256}</code>
        </p>
      )}
      {(evidenceHash || evidence.evidence_hash) && (
        <p className="source-note">
          Evidence commitment: <code className="mono">{evidenceHash || evidence.evidence_hash}</code>
        </p>
      )}
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
    <main className="main-panel">
      <div className="panel-header">
        <div>
          <p className="eyebrow">Auditor workflow</p>
          <h2>Verify without seeing the farm</h2>
          <p className="panel-desc">
            The auditor needs a result, not the supplier&apos;s private witness. Use a GreenProof ID
            for the demo flow, or paste proof artifacts for technical verification.
          </p>
        </div>
        <div className="constraint-card privacy-side">
          <p className="split-label">Hidden from auditor</p>
          <ul className="plain-list">
            <li>Exact coordinates</li>
            <li>Production volume</li>
            <li>Supplier secret</li>
          </ul>
        </div>
      </div>

      <section
        className={`card verification-card${result ? (valid ? " is-valid" : " is-invalid") : ""}${verifying ? " is-processing" : ""}`}
      >
        <div className="card-header">
          <div>
            <p className="card-kicker">05 · Result</p>
            <h3>GreenProof verification</h3>
          </div>
        </div>

        <div className="id-row">
          <input
            value={verificationId}
            onChange={(e) => setVerificationId(e.target.value)}
            placeholder="GP-XXXXXXXX"
            aria-label="Verification ID"
          />
          <button
            type="button"
            className="btn btn-primary"
            disabled={verifying || !verificationId}
            onClick={() => loadVerification(verificationId)}
          >
            {verifying ? "Checking…" : "Verify ID"}
          </button>
        </div>

        {verifying && !result && (
          <div className="loading-bar" aria-hidden="true">
            <span />
          </div>
        )}

        {error && (
          <div className="failure-card inline-failure" role="alert">
            <div className="failure-icon">×</div>
            <div>
              <h3>Verification failed</h3>
              <p>{error}</p>
            </div>
          </div>
        )}

        {result && (
          <div className={`verification-result${valid ? " reveal-success" : " reveal-fail"}`}>
            {valid ? (
              <div className="payoff">
                <div className="payoff-badge" aria-hidden="true">
                  ✓
                </div>
                <div>
                  <p className="eyebrow success-eyebrow">Environmental policy verified</p>
                  <h3>✓ Environmental policy verified</h3>
                  <p className="payoff-message">
                    The supplier proved compliance without revealing the sensitive location.
                  </p>
                </div>
              </div>
            ) : (
              <div className="payoff payoff-fail">
                <div className="payoff-badge fail-badge" aria-hidden="true">
                  ×
                </div>
                <div>
                  <p className="eyebrow">Proof invalid</p>
                  <h3>Proof invalid</h3>
                  <p className="payoff-message">
                    The cryptographic verification did not succeed for this submission.
                  </p>
                </div>
              </div>
            )}

            <div className="result-matrix">
              <div className="matrix-item">
                <span>Exact location</span>
                <StatusBadge tone="private">Private</StatusBadge>
              </div>
              <div className="matrix-item">
                <span>Private inputs</span>
                <StatusBadge tone="private">Hidden</StatusBadge>
              </div>
              <div className="matrix-item">
                <span>Environmental evidence</span>
                <StatusBadge tone={result.evidence ? "success" : "neutral"}>
                  {result.evidence ? "Verified" : "—"}
                </StatusBadge>
              </div>
              <div className="matrix-item">
                <span>Zero-knowledge proof</span>
                <StatusBadge tone={valid ? "success" : "danger"}>
                  {valid ? "Valid" : "Invalid"}
                </StatusBadge>
              </div>
            </div>

            <div className="certificate">
              <div className="certificate-row">
                <span>Commodity</span>
                <strong>Cocoa</strong>
              </div>
              <div className="certificate-row">
                <span>Environmental condition</span>
                <strong>
                  Protected-area status &amp; land-cover classification (West African cocoa belt)
                </strong>
              </div>
              <div className="certificate-row">
                <span>Result</span>
                <strong className={valid ? "pass" : "fail"}>{valid ? "Pass" : "Fail"}</strong>
              </div>
              <div className="certificate-row">
                <span>ZK verification</span>
                <strong className={valid ? "pass" : "fail"}>{valid ? "Valid" : "Invalid"}</strong>
              </div>
              <div className="certificate-row">
                <span>Proof ID</span>
                <strong className="mono">{result.verification_id || "—"}</strong>
              </div>
              <div className="certificate-row">
                <span>Created</span>
                <strong>{result.created_at || "—"}</strong>
              </div>
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
                <span>Exact coordinates</span>
                <strong>Private — never disclosed</strong>
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
                <div>
                  <span>Shareable verification</span>
                  <code className="mono">{shareUrl}</code>
                </div>
                <button type="button" className="btn btn-secondary" onClick={copyShareLink}>
                  Copy link
                </button>
              </div>
            )}

            <details className="tech-details">
              <summary>Technical details</summary>
              <div className="tech-body">
                <div className="tech-chips">
                  <span>Groth16</span>
                  <span>Circom</span>
                  <span>Poseidon</span>
                  <span>BN254</span>
                </div>
                {result.public_signals && (
                  <>
                    <p className="muted">Public signals</p>
                    <pre>{JSON.stringify(result.public_signals, null, 2)}</pre>
                  </>
                )}
              </div>
            </details>
          </div>
        )}
      </section>

      <details className="technical-card">
        <summary>Advanced: verify raw proof artifacts</summary>
        <div className="card nested-card">
          <p className="muted">For judges who want to inspect the cryptographic path directly.</p>
          <label>
            proof.json
            <textarea
              rows={7}
              value={proofText}
              onChange={(e) => setProofText(e.target.value)}
              spellCheck={false}
            />
          </label>
          <label>
            public.json
            <textarea
              rows={4}
              value={publicText}
              onChange={(e) => setPublicText(e.target.value)}
              spellCheck={false}
            />
          </label>
          <button
            type="button"
            disabled={verifying || !proofText || !publicText}
            onClick={verifyPasted}
            className="btn btn-primary"
          >
            {verifying ? "Verifying…" : "Verify raw proof"}
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
        <div>
          <strong>Environmental evidence provenance</strong>
          <span>Exact coordinates withheld</span>
        </div>
        <StatusBadge tone="private">Private location</StatusBadge>
      </div>
      <div className="provenance-grid">
        <div>
          <span>Protected areas</span>
          <strong>{evidence.protected_area.source}</strong>
          <small>{evidence.protected_area.dataset}</small>
        </div>
        <div>
          <span>Land cover</span>
          <strong>{evidence.land_cover.source}</strong>
          <small>{evidence.land_cover.dataset}</small>
        </div>
        <div>
          <span>Query radius</span>
          <strong>{evidence.protected_area.query_radius_m} m</strong>
          <small>No coordinates stored</small>
        </div>
        <div>
          <span>Land-cover result</span>
          <strong>{evidence.land_cover.classification}</strong>
          <small>Code {evidence.land_cover.code}</small>
        </div>
      </div>
      {evidence.land_cover.note && <p className="source-note">{evidence.land_cover.note}</p>}
      {evidence.evidence_hash && (
        <p className="source-note">
          Evidence commitment: <code className="mono">{evidence.evidence_hash}</code>
        </p>
      )}
    </div>
  );
}
