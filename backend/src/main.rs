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
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

#[derive(Clone)]
struct AppState {
    http_client: reqwest::Client,
    scripts_dir: String,
    vkey_path: String,
    verifications: Arc<Mutex<HashMap<String, StoredVerification>>>,
}

#[derive(Deserialize)]
struct CheckLocationRequest {
    latitude: f64,
    longitude: f64,
    #[serde(default = "default_radius")]
    protected_radius_m: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredVerification {
    verification_id: String,
    created_at: String,
    zk_proof_valid: bool,
    proof: serde_json::Value,
    public_signals: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    evidence: Option<PublicEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PublicEvidence {
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
}

#[derive(Debug, Deserialize)]
struct VerifyProofRequest {
    proof: serde_json::Value,
    public_signals: serde_json::Value,
    #[serde(default)]
    evidence: Option<PublicEvidenceInput>,
}

#[derive(Debug, Deserialize)]
struct PublicEvidenceInput {
    protected_area: PublicProtectedEvidence,
    land_cover: PublicLandCoverEvidence,
}

fn default_radius() -> u32 { 1000 }

async fn check_location(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CheckLocationRequest>,
) -> impl IntoResponse {
    match geo::check_location(
        &state.http_client,
        req.latitude,
        req.longitude,
        req.protected_radius_m,
    ).await {
        Ok(evidence) => (
            StatusCode::OK,
            Json(serde_json::json!({ "ok": true, "evidence": evidence })),
        ),
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
    match verify::verify_proof(&state.scripts_dir, &state.vkey_path, &verify::VerifyRequest {
        proof: req.proof.clone(),
        public_signals: req.public_signals.clone(),
    }).await {
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
                proof: req.proof,
                public_signals: req.public_signals,
                evidence: req.evidence.map(|e| PublicEvidence { protected_area: e.protected_area, land_cover: e.land_cover }),
            };

            match state.verifications.lock() {
                Ok(mut db) => {
                    db.insert(verification_id.clone(), stored.clone());
                }
                Err(_) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({ "ok": false, "error": "verification store unavailable" })),
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
                    "evidence": v.evidence,
                    "public_signals": v.public_signals
                })),
            ),
            None => (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "ok": false, "error": "Verification ID not found or expired." })),
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
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    format!("GP-{:08X}", (nanos as u64) & 0xFFFF_FFFF)
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info,greenproof_backend=info".into()))
        .init();

    let scripts_dir = std::env::var("GREENPROOF_SCRIPTS_DIR").unwrap_or_else(|_| "../scripts".to_string());
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
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = std::env::var("GREENPROOF_BACKEND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    tracing::info!("GreenProof backend listening on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("bind failed");
    axum::serve(listener, app).await.expect("server error");
}
