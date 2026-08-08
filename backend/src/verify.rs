//! GreenProof - Groth16 proof verification.
//!
//! ENGINEERING TRADE-OFF (documented honestly, see README "Security model"):
//! The actual elliptic-curve pairing check is performed by `snarkjs`
//! (`snarkjs groth16 verify`), invoked as a subprocess, rather than
//! re-implemented with a pure-Rust pairing library (e.g. ark-groth16).
//!
//! Why: snarkjs's verification-key JSON format and ark-groth16's BN254
//! serialization are not drop-in compatible, and hand-rolling that
//! conversion without the ability to run/test it in this environment risks
//! shipping a verifier that *looks* cryptographic but is subtly wrong -
//! which would be worse than being explicit about using the reference
//! implementation. snarkjs's `groth16.verify` is a real, standard Groth16
//! pairing check (not a mock): it recomputes the pairing equation
//! e(A,B) = e(alpha,beta) * e(vk_x,gamma) * e(C,delta) over BN254 and
//! returns a real boolean. A tampered proof or tampered public signal WILL
//! fail this check.
//!
//! Future work: replace this subprocess call with a native Rust verifier
//! (ark-groth16 + ark-bn254) once the vkey conversion has test coverage.

use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::process::Command;

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyRequest {
    pub proof: serde_json::Value,
    pub public_signals: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct VerifyResult {
    pub zk_proof_valid: bool,
    pub raw_stdout: String,
    pub raw_stderr: String,
}

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("failed to write temp files: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to spawn verifier subprocess: {0}")]
    Spawn(String),
    #[error("verifier subprocess produced no usable output")]
    NoOutput,
}

/// Verifies a Groth16 proof against the deployed circuit's fixed
/// verification key. Only PUBLIC signals and the proof itself are ever
/// touched here - there is no private data in this path by construction,
/// because a Groth16 proof + its public signals never contain the private
/// witness.
pub async fn verify_proof(
    scripts_dir: &str,
    vkey_path: &str,
    req: &VerifyRequest,
) -> Result<VerifyResult, VerifyError> {
    let tmp_dir = std::env::temp_dir().join(format!("greenproof-verify-{}", uuid_like()));
    tokio::fs::create_dir_all(&tmp_dir).await?;

    let proof_path = tmp_dir.join("proof.json");
    let public_path = tmp_dir.join("public.json");
    tokio::fs::write(&proof_path, serde_json::to_vec_pretty(&req.proof)?).await?;
    tokio::fs::write(
        &public_path,
        serde_json::to_vec_pretty(&req.public_signals)?,
    )
    .await?;

    let output = Command::new("node")
        .arg(format!("{}/verify.js", scripts_dir))
        .arg(&proof_path)
        .arg(&public_path)
        .arg(vkey_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| VerifyError::Spawn(e.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if stdout.is_empty() && stderr.is_empty() {
        return Err(VerifyError::NoOutput);
    }

    let zk_proof_valid =
        output.status.success() && stdout.contains("VALID") && !stdout.contains("INVALID");

    Ok(VerifyResult {
        zk_proof_valid,
        raw_stdout: stdout,
        raw_stderr: stderr,
    })
}

fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{n:x}")
}

// serde_json::Error -> std::io::Error bridging for the ? operator above
impl From<serde_json::Error> for VerifyError {
    fn from(e: serde_json::Error) -> Self {
        VerifyError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}
