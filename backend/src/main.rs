mod ai;
mod geo;
mod time;
mod verify;

use crate::time::now_iso;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::process::Command;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

#[derive(Clone)]
struct AppState {
    http_client: reqwest::Client,
    scripts_dir: String,
    vkey_path: String,
    verifications: Arc<Mutex<HashMap<String, StoredVerification>>>,
    evidence_sessions: Arc<Mutex<HashMap<String, EvidenceSession>>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckLocationRequest {
    latitude: f64,
    longitude: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredVerification {
    verification_id: String,
    created_at: String,
    zk_proof_valid: bool,
    compliance_status: String,
    failed_checks: Vec<String>,
    proof: serde_json::Value,
    public_signals: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    evidence: Option<PublicEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PublicEvidence {
    evidence_hash: String,
    protected_area: PublicProtectedEvidence,
    land_cover: PublicLandCoverEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PublicProtectedEvidence {
    status: bool,
    source: String,
    dataset: String,
    query_radius_m: u32,
    source_url: String,
    retrieved_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PublicLandCoverEvidence {
    classification: String,
    code: i64,
    source: String,
    dataset: String,
    source_url: String,
    retrieved_at: String,
    note: String,
    raw_response_sha256: String,
}

#[derive(Debug, Clone)]
struct EvidenceSession {
    evidence_hash: String,
    evidence: PublicEvidence,
    created_unix_secs: u64,
}

#[derive(Debug, Deserialize)]
struct VerifyProofRequest {
    proof: serde_json::Value,
    public_signals: serde_json::Value,
    evidence_session: String,
}

async fn check_location(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CheckLocationRequest>,
) -> impl IntoResponse {
    match geo::check_location(&state.http_client, req.latitude, req.longitude).await {
        Ok(evidence) => match create_evidence_session(&state, &evidence).await {
            Ok((session_id, evidence_hash)) => (
                StatusCode::OK,
                Json(serde_json::json!({
                    "ok": true,
                    "evidence": evidence,
                    "evidence_session": session_id,
                    "evidence_hash": evidence_hash,
                    "evidence_commitment": {
                        "algorithm": "Poseidon(4) over encoded coordinate + normalized claim",
                        "evidence_hash": evidence_hash
                    }
                })),
            ),
            Err(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "ok": false, "error": error })),
            ),
        },
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "ok": false,
                "error": e.to_string(),
                "note": "No fallback data was substituted. Fix connectivity to the failing source and retry."
            })),
        ),
    }
}

async fn verify_proof_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VerifyProofRequest>,
) -> impl IntoResponse {
    let session = match take_evidence_session(&state, &req.evidence_session) {
        Ok(session) => session,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "error": error })),
            )
        }
    };
    if let Err(error) = validate_public_signals(&req.public_signals, &session.evidence_hash) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "ok": false, "error": error })),
        );
    }

    // Extract compliance status from circuit output signals before verification
    let (compliance_status, failed_checks) =
        extract_compliance_status(&req.public_signals);

    match verify::verify_proof(
        &state.scripts_dir,
        &state.vkey_path,
        &verify::VerifyRequest {
            proof: req.proof.clone(),
            public_signals: req.public_signals.clone(),
        },
    )
    .await
    {
        Ok(result) => {
            if !result.zk_proof_valid {
                return (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "ok": true,
                        "zk_proof_valid": false,
                        "private_coordinates_disclosed": false,
                        "private_quantity_disclosed": false,
                        "supplier_secret_disclosed": false,
                        "log": result.raw_stdout
                    })),
                );
            }

            let verification_id = create_verification_id();
            let stored = StoredVerification {
                verification_id: verification_id.clone(),
                created_at: now_iso(),
                zk_proof_valid: true,
                compliance_status: compliance_status.clone(),
                failed_checks: failed_checks.clone(),
                proof: req.proof,
                public_signals: req.public_signals,
                evidence: Some(session.evidence),
            };

            match state.verifications.lock() {
                Ok(mut db) => {
                    db.insert(verification_id.clone(), stored.clone());
                }
                Err(_) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(
                            serde_json::json!({ "ok": false, "error": "verification store unavailable" }),
                        ),
                    );
                }
            }

            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "ok": true,
                    "zk_proof_valid": true,
                    "verification_id": verification_id,
                    "created_at": stored.created_at,
                    "compliance_status": compliance_status,
                    "failed_checks": failed_checks,
                    "private_coordinates_disclosed": false,
                    "private_quantity_disclosed": false,
                    "supplier_secret_disclosed": false
                })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

const SESSION_TTL_SECS: u64 = 10 * 60;
// 7 output signals + 8 public input signals = 15 total
const EXPECTED_PUBLIC_SIGNALS: usize = 15;
const DEFAULT_LAT_MIN: u64 = 94_000_000;
const DEFAULT_LAT_MAX: u64 = 101_500_000;
const DEFAULT_LON_MIN: u64 = 171_400_000;
const DEFAULT_LON_MAX: u64 = 181_500_000;
const DEFAULT_QUANTITY_THRESHOLD: u64 = 5_000;
const DEFAULT_ALLOWED_LAND_COVER: u64 = 40;

async fn create_evidence_session(
    state: &AppState,
    evidence: &geo::LocationEvidence,
) -> Result<(String, String), String> {
    let lat_enc = ((evidence.latitude + 90.0) * 1_000_000.0).round() as u64;
    let lon_enc = ((evidence.longitude + 180.0) * 1_000_000.0).round() as u64;
    let protected = if evidence.protected_area.status { 1 } else { 0 };
    let output = Command::new("node")
        .arg(format!("{}/evidence-hash.js", state.scripts_dir))
        .arg(lat_enc.to_string())
        .arg(lon_enc.to_string())
        .arg(protected.to_string())
        .arg(evidence.land_cover.code.to_string())
        .output()
        .await
        .map_err(|e| format!("failed to calculate evidence commitment: {e}"))?;
    if !output.status.success() {
        return Err("failed to calculate evidence commitment".into());
    }
    let evidence_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if evidence_hash.is_empty() {
        return Err("empty evidence commitment".into());
    }
    let evidence = PublicEvidence {
        evidence_hash: evidence_hash.clone(),
        protected_area: PublicProtectedEvidence {
            status: evidence.protected_area.status,
            source: evidence.protected_area.source.to_string(),
            dataset: evidence.protected_area.dataset.to_string(),
            query_radius_m: evidence.protected_area.query_radius_m,
            source_url: evidence.protected_area.source_url.clone(),
            retrieved_at: evidence.protected_area.retrieved_at.clone(),
        },
        land_cover: PublicLandCoverEvidence {
            classification: evidence.land_cover.classification.clone(),
            code: evidence.land_cover.code,
            source: evidence.land_cover.source.to_string(),
            dataset: evidence.land_cover.dataset.to_string(),
            source_url: evidence.land_cover.source_url.clone(),
            retrieved_at: evidence.land_cover.retrieved_at.clone(),
            note: evidence.land_cover.note.to_string(),
            raw_response_sha256: evidence.land_cover.raw_response_sha256.clone(),
        },
    };
    let session_id = random_id("ES");
    let session = EvidenceSession {
        evidence_hash: evidence_hash.clone(),
        evidence,
        created_unix_secs: unix_secs(),
    };
    state
        .evidence_sessions
        .lock()
        .map_err(|_| "evidence session store unavailable".to_string())?
        .insert(session_id.clone(), session);
    Ok((session_id, evidence_hash))
}

fn take_evidence_session(state: &AppState, id: &str) -> Result<EvidenceSession, String> {
    let now = unix_secs();
    let mut sessions = state
        .evidence_sessions
        .lock()
        .map_err(|_| "evidence session store unavailable".to_string())?;
    sessions.retain(|_, value| now.saturating_sub(value.created_unix_secs) <= SESSION_TTL_SECS);
    sessions.remove(id).ok_or_else(|| "Evidence session not found, expired, or already used. Run the environmental lookup again.".to_string())
}

/// Extract compliance status and failed checks from the circuit's public output signals.
///
/// Public signals layout (outputs first, then public inputs):
///   [0] valid, [1] regionOk, [2] protectedAreaOk, [3] landCoverOk,
///   [4] quantityOk, [5] supplierOk, [6] evidenceOk,
///   [7..14] public inputs
fn extract_compliance_status(signals: &serde_json::Value) -> (String, Vec<String>) {
    let signals = match signals.as_array() {
        Some(a) => a,
        None => return ("UNKNOWN".into(), vec![]),
    };
    let is_one = |idx: usize| signals.get(idx).and_then(|v| v.as_str()) == Some("1");

    let check_names = [
        (1, "Region check"),
        (2, "Protected area check"),
        (3, "Land cover check"),
        (4, "Quantity threshold check"),
        (5, "Supplier commitment check"),
        (6, "Evidence binding check"),
    ];
    let failed: Vec<String> = check_names
        .iter()
        .filter(|(idx, _)| !is_one(*idx))
        .map(|(_, name)| name.to_string())
        .collect();

    let status = if is_one(0) {
        "COMPLIANT".to_string()
    } else {
        "NOT_COMPLIANT".to_string()
    };
    (status, failed)
}

fn validate_public_signals(signals: &serde_json::Value, evidence_hash: &str) -> Result<(), String> {
    let signals = signals
        .as_array()
        .ok_or_else(|| "public_signals must be an array".to_string())?;
    if signals.len() != EXPECTED_PUBLIC_SIGNALS {
        return Err(format!(
            "unexpected public-signal count: expected {}, got {}",
            EXPECTED_PUBLIC_SIGNALS,
            signals.len()
        ));
    }
    // Output signals [0..6] are the circuit's compliance check results —
    // they can be 0 or 1, and we validate them structurally (must be "0" or "1")
    // but do NOT require them to be "1".
    for i in 0..7 {
        let val = signals[i].as_str().unwrap_or("");
        if val != "0" && val != "1" {
            return Err(format!("output signal {i} is not a valid boolean"));
        }
    }
    // Public input signals [7..14] must match the deployed policy.
    // Index mapping: 7=latMin, 8=latMax, 9=lonMin, 10=lonMax,
    //                11=quantityThreshold, 12=allowedLandCoverCode,
    //                13=supplierCommitment (any value), 14=evidenceHash
    let policy_checks: [(usize, Option<String>); 8] = [
        (7, Some(DEFAULT_LAT_MIN.to_string())),
        (8, Some(DEFAULT_LAT_MAX.to_string())),
        (9, Some(DEFAULT_LON_MIN.to_string())),
        (10, Some(DEFAULT_LON_MAX.to_string())),
        (11, Some(DEFAULT_QUANTITY_THRESHOLD.to_string())),
        (12, Some(DEFAULT_ALLOWED_LAND_COVER.to_string())),
        (13, None), // supplierCommitment — any value is acceptable
        (14, Some(evidence_hash.to_string())),
    ];
    for (index, expected) in &policy_checks {
        if let Some(ref exp) = expected {
            if signals[*index].as_str() != Some(exp) {
                return Err(format!(
                    "public signal {index} does not match the deployed GreenProof policy"
                ));
            }
        }
    }
    Ok(())
}

fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn random_id(prefix: &str) -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    format!(
        "{prefix}-{}",
        bytes.iter().map(|b| format!("{b:02X}")).collect::<String>()
    )
}

async fn get_verification(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.verifications.lock() {
        Ok(db) => match db.get(&id) {
            Some(v) => (
                StatusCode::OK,
                Json(serde_json::json!({
                    "ok": true,
                    "verification_id": v.verification_id,
                    "created_at": v.created_at,
                    "zk_proof_valid": v.zk_proof_valid,
                    "compliance_status": v.compliance_status,
                    "failed_checks": v.failed_checks,
                    "evidence": v.evidence,
                    "public_signals": v.public_signals
                })),
            ),
            None => (
                StatusCode::NOT_FOUND,
                Json(
                    serde_json::json!({ "ok": false, "error": "Verification ID not found or expired." }),
                ),
            ),
        },
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "ok": false, "error": "verification store unavailable" })),
        ),
    }
}

async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let count = state.verifications.lock().map(|db| db.len()).unwrap_or(0);
    Json(serde_json::json!({
        "ok": true,
        "service": "greenproof-backend",
        "verification_count": count,
        "storage": "in-memory demo store"
    }))
}

fn create_verification_id() -> String {
    random_id("GP")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_radius_cannot_be_supplied_by_client() {
        let rejected = serde_json::from_str::<CheckLocationRequest>(
            r#"{"latitude": 6.6666, "longitude": -1.6163, "protected_radius_m": 0}"#,
        );
        assert!(
            rejected.is_err(),
            "client-supplied protected_radius_m must be rejected"
        );

        let accepted = serde_json::from_str::<CheckLocationRequest>(
            r#"{"latitude": 6.6666, "longitude": -1.6163}"#,
        )
        .expect("latitude/longitude-only body must deserialize");
        assert!((accepted.latitude - 6.6666).abs() < f64::EPSILON);
        assert!((accepted.longitude - -1.6163).abs() < f64::EPSILON);

        // Policy radius is server-fixed and non-zero so a zero-radius bypass
        // cannot be reintroduced via request fields.
        assert_eq!(geo::PROTECTED_RADIUS_M, 1000);
        assert!(geo::PROTECTED_RADIUS_M > 0);
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,greenproof_backend=info".into()),
        )
        .init();

    let scripts_dir =
        std::env::var("GREENPROOF_SCRIPTS_DIR").unwrap_or_else(|_| "../scripts".to_string());
    let vkey_path = std::env::var("GREENPROOF_VKEY_PATH")
        .unwrap_or_else(|_| "../circuits/build/verification_key.json".to_string());

    let state = Arc::new(AppState {
        http_client: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .expect("failed to build reqwest client"),
        scripts_dir,
        vkey_path,
        verifications: Arc::new(Mutex::new(HashMap::new())),
        evidence_sessions: Arc::new(Mutex::new(HashMap::new())),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/check-location", post(check_location))
        .route("/api/verify-proof", post(verify_proof_handler))
        .route("/api/verifications/:id", get(get_verification))
        .route("/api/ai-audit", post(ai::ai_audit_handler))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr =
        std::env::var("GREENPROOF_BACKEND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    tracing::info!("GreenProof backend listening on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("bind failed");
    axum::serve(listener, app).await.expect("server error");
}
