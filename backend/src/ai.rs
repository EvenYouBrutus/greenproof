use axum::{response::IntoResponse, Json};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct AiAuditRequest {
    pub compliance_status: String,
    pub failed_checks: Vec<String>,
    pub passed_checks: Option<Vec<String>>,
    pub evidence_dataset: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AiAuditResponse {
    pub ok: bool,
    pub summary: String,
    pub risk_level: String,
    pub recommended_action: String,
    pub ai_model: String,
}

pub async fn ai_audit_handler(
    Json(req): Json<AiAuditRequest>,
) -> impl IntoResponse {
    // Check if an external LLM API Key is configured in environment
    if let Ok(api_key) = std::env::var("GEMINI_API_KEY") {
        if !api_key.is_empty() {
            if let Ok(res) = call_gemini_api(&api_key, &req).await {
                return (StatusCode::OK, Json(res));
            }
        }
    }

    if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
        if !api_key.is_empty() {
            if let Ok(res) = call_openai_api(&api_key, &req).await {
                return (StatusCode::OK, Json(res));
            }
        }
    }

    // Default: Professional Rule-Based AI Environmental Auditor
    let res = generate_fallback_analysis(&req);
    (StatusCode::OK, Json(res))
}

fn generate_fallback_analysis(req: &AiAuditRequest) -> AiAuditResponse {
    let is_compliant = req.compliance_status == "COMPLIANT";
    let failed = &req.failed_checks;
    let dataset = req
        .evidence_dataset
        .as_deref()
        .unwrap_or("ESA WorldCover / OpenStreetMap");

    if is_compliant {
        AiAuditResponse {
            ok: true,
            summary: format!(
                "The zero-knowledge evaluation confirmed full environmental compliance against public policy thresholds ({dataset}). All spatial, land-cover, and quantity constraints were verified without exposing private coordinates."
            ),
            risk_level: "LOW RISK".to_string(),
            recommended_action: "Approve supply batch for EUDR/environmental compliance clearance. Retain cryptographic verification ID for record auditing.".to_string(),
            ai_model: "GreenProof AI Environmental Auditor (Built-in)".to_string(),
        }
    } else {
        let has_protected_fail = failed.iter().any(|f| f.to_lowercase().contains("protected"));
        let has_land_cover_fail = failed.iter().any(|f| f.to_lowercase().contains("land cover"));
        let has_quantity_fail = failed.iter().any(|f| f.to_lowercase().contains("quantity"));
        let has_region_fail = failed.iter().any(|f| f.to_lowercase().contains("region"));

        let mut summary_parts = Vec::new();
        if has_protected_fail {
            summary_parts.push("location falls within a flagged protected or nature-reserve boundary");
        }
        if has_land_cover_fail {
            summary_parts.push("land-cover classification does not match allowed agricultural class");
        }
        if has_quantity_fail {
            summary_parts.push("declared production quantity exceeds regional threshold");
        }
        if has_region_fail {
            summary_parts.push("coordinates lie outside the designated sourcing region bounding box");
        }

        let summary = if !summary_parts.is_empty() {
            format!(
                "Environmental policy failed because: {}. Verified cryptographically via public signals ({dataset}).",
                summary_parts.join("; ")
            )
        } else {
            format!(
                "Environmental compliance evaluation failed for checks: {}. Cryptographic proof is valid but policy requirements were not satisfied.",
                failed.join(", ")
            )
        };

        let risk_level = if has_protected_fail {
            "HIGH RISK (Protected Area Flag)"
        } else if has_land_cover_fail {
            "MEDIUM RISK (Land-Cover Mismatch)"
        } else {
            "MODERATE RISK (Policy Non-Compliance)"
        }
        .to_string();

        let recommended_action = if has_protected_fail {
            "Flag batch for manual environmental review. Request updated polygon boundaries from supplier prior to customs release."
        } else if has_land_cover_fail {
            "Verify satellite land-use classification. Supplier should submit secondary evidence before approval."
        } else {
            "Review batch quantity limits and regional sourcing declarations with supplier."
        }
        .to_string();

        AiAuditResponse {
            ok: true,
            summary,
            risk_level,
            recommended_action,
            ai_model: "GreenProof AI Environmental Auditor (Built-in)".to_string(),
        }
    }
}

async fn call_gemini_api(api_key: &str, req: &AiAuditRequest) -> Result<AiAuditResponse, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent?key={api_key}"
    );

    let prompt = format!(
        "You are an expert AI Environmental Auditor for EUDR supply chains. Analyze the following PUBLIC ZK verification audit result:\n\n\
        Status: {}\n\
        Failed checks: {:?}\n\
        Passed checks: {:?}\n\
        Evidence dataset: {}\n\n\
        Provide a JSON response strictly matching this structure (no markdown formatting, raw JSON):\n\
        {{\n  \"summary\": \"Concise 2-sentence explanation of the compliance status\",\n  \"risk_level\": \"LOW RISK | MEDIUM RISK | HIGH RISK\",\n  \"recommended_action\": \"Single clear actionable recommendation for the auditor\"\n}}",
        req.compliance_status,
        req.failed_checks,
        req.passed_checks,
        req.evidence_dataset.as_deref().unwrap_or("ESA WorldCover")
    );

    let body = serde_json::json!({
        "contents": [{
            "parts": [{"text": prompt}]
        }]
    });

    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err("Gemini API call failed".into());
    }

    let res_json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let text = res_json["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .ok_or_else(|| "Empty AI response".to_string())?;

    let cleaned_text = text.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    let parsed: serde_json::Value = serde_json::from_str(cleaned_text).map_err(|e| e.to_string())?;

    Ok(AiAuditResponse {
        ok: true,
        summary: parsed["summary"].as_str().unwrap_or("Evaluation completed.").to_string(),
        risk_level: parsed["risk_level"].as_str().unwrap_or("UNKNOWN").to_string(),
        recommended_action: parsed["recommended_action"].as_str().unwrap_or("Inspect proof results.").to_string(),
        ai_model: "Gemini 1.5 Flash (Live AI API)".to_string(),
    })
}

async fn call_openai_api(api_key: &str, req: &AiAuditRequest) -> Result<AiAuditResponse, String> {
    let client = reqwest::Client::new();
    let url = "https://api.openai.com/v1/chat/completions";

    let prompt = format!(
        "You are an expert AI Environmental Auditor for EUDR supply chains. Analyze the following PUBLIC ZK verification audit result:\n\n\
        Status: {}\n\
        Failed checks: {:?}\n\
        Passed checks: {:?}\n\
        Evidence dataset: {}\n\n\
        Provide a JSON response strictly matching this structure (raw JSON):\n\
        {{\n  \"summary\": \"Concise 2-sentence explanation of the compliance status\",\n  \"risk_level\": \"LOW RISK | MEDIUM RISK | HIGH RISK\",\n  \"recommended_action\": \"Single clear actionable recommendation for the auditor\"\n}}",
        req.compliance_status,
        req.failed_checks,
        req.passed_checks,
        req.evidence_dataset.as_deref().unwrap_or("ESA WorldCover")
    );

    let body = serde_json::json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": prompt}],
        "response_format": { "type": "json_object" }
    });

    let resp = client
        .post(url)
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err("OpenAI API call failed".into());
    }

    let res_json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let content = res_json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| "Empty AI response".to_string())?;

    let parsed: serde_json::Value = serde_json::from_str(content).map_err(|e| e.to_string())?;

    Ok(AiAuditResponse {
        ok: true,
        summary: parsed["summary"].as_str().unwrap_or("Evaluation completed.").to_string(),
        risk_level: parsed["risk_level"].as_str().unwrap_or("UNKNOWN").to_string(),
        recommended_action: parsed["recommended_action"].as_str().unwrap_or("Inspect proof results.").to_string(),
        ai_model: "OpenAI GPT-4o-mini (Live AI API)".to_string(),
    })
}
