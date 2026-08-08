//! GreenProof - real geospatial evidence layer.
//!
//! CRITICAL: every field returned here must come from an actual HTTP
//! response from a real public data source. If a source is unreachable or
//! returns nothing usable, this module returns an explicit `GeoError`
//! describing which source failed - it must never substitute a guessed or
//! hardcoded value.
//!
//! Sources used by this MVP:
//!   - Nominatim (OpenStreetMap) - reverse geocoding.
//!     https://nominatim.org/release-docs/latest/api/Reverse/
//!     Usage policy requires a descriptive User-Agent and rate limiting
//!     (max ~1 req/s for the public instance): https://operations.osmfoundation.org/policies/nominatim/
//!   - Overpass API (OpenStreetMap) - protected-area proxy tags.
//!     https://wiki.openstreetmap.org/wiki/Overpass_API
//!     Tags used: boundary=protected_area, leisure=nature_reserve,
//!   - ESA WorldCover 2021 v200 - satellite-derived, 10 m global land-cover
//!     classification from Copernicus Sentinel-1 and Sentinel-2 data. The
//!     public Terrascope WMS endpoint is queried server-side for a point
//!     feature value. This is the land-cover source used by the proof flow.
//!
//! NOT integrated in this MVP (documented as future work, see README):
//!   - Copernicus Land Monitoring Service tree-cover density: requires a
//!     registered Copernicus Data Space Ecosystem (CDSE) account/API
//!     credentials. Wiring point is left in `.env.example`
//!     (COPERNICUS_CLIENT_ID / COPERNICUS_CLIENT_SECRET). No fallback data is
//!     substituted when ESA WorldCover is unavailable: the lookup fails
//!     explicitly rather than emitting a fabricated environmental claim.

use crate::time::now_iso;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const NOMINATIM_BASE: &str = "https://nominatim.openstreetmap.org";
const OVERPASS_BASE: &str = "https://overpass-api.de/api/interpreter";
const WORLDCOVER_WMS_DEFAULT: &str = "https://titiler.terrascope.be/wms";
const WORLDCOVER_LAYER: &str = "WORLDCOVER_2021_MAP";
const USER_AGENT: &str = "GreenProof-MVP/0.1 (hackathon prototype; contact: set-your-contact-here)";

#[derive(Debug, thiserror::Error)]
pub enum GeoError {
    #[error("invalid latitude {0}: must be between -90 and 90")]
    InvalidLatitude(f64),
    #[error("invalid longitude {0}: must be between -180 and 180")]
    InvalidLongitude(f64),
    #[error("Nominatim reverse-geocoding request failed: {0}")]
    NominatimUnavailable(String),
    #[error("Overpass API request failed: {0}")]
    OverpassUnavailable(String),
    #[error("ESA WorldCover request failed: {0}")]
    WorldCoverUnavailable(String),
    #[error("ESA WorldCover returned an unsupported land-cover response")]
    WorldCoverUnparseable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectedAreaEvidence {
    pub status: bool,
    pub matched_features: Vec<String>, // names/types of OSM features that triggered `status`
    pub source: &'static str,
    pub dataset: &'static str,
    pub query_radius_m: u32,
    pub source_url: String,
    pub retrieved_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LandCoverEvidence {
    pub classification: String, // human-readable, e.g. "forest (OSM natural=wood)"
    pub code: i64,              // integer code fed into the ZK circuit's landCoverCode
    pub source: &'static str,
    pub dataset: &'static str,
    pub source_url: String,
    pub retrieved_at: String,
    pub note: &'static str,
    pub raw_response_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationEvidence {
    pub latitude: f64,
    pub longitude: f64,
    pub country: Option<String>,
    pub display_name: Option<String>,
    pub reverse_geocode_source: &'static str,
    pub reverse_geocode_source_url: String,
    pub protected_area: ProtectedAreaEvidence,
    pub land_cover: LandCoverEvidence,
}

/// Raw provider output is kept server-side. It is intentionally not made a
/// circuit input and is not returned in verification records.
#[derive(Debug, Clone)]
struct RawLandCoverResponse {
    body: String,
    source_url: String,
}

/// The compact, deterministic claim that crosses the provider/ZK boundary.
#[derive(Debug, Clone)]
struct NormalizedLandCoverClaim {
    source_class: i64,
    code: i64,
    classification: &'static str,
}

/// One provider abstraction: additional providers can implement the same
/// raw-response -> normalized-claim boundary without changing the ZK flow.
struct EsaWorldCoverProvider {
    base_url: String,
}

impl EsaWorldCoverProvider {
    fn from_environment() -> Self {
        Self {
            base_url: std::env::var("GREENPROOF_WORLDCOVER_WMS_URL")
                .unwrap_or_else(|_| WORLDCOVER_WMS_DEFAULT.to_string()),
        }
    }

    async fn fetch_raw(
        &self,
        client: &reqwest::Client,
        lat: f64,
        lon: f64,
    ) -> Result<RawLandCoverResponse, GeoError> {
        // WMS 1.1.1 keeps EPSG:4326 BBOX axis order unambiguous: minLon,
        // minLat, maxLon, maxLat. A tiny deterministic bbox identifies the
        // pixel containing the requested point.
        let epsilon = 0.00005_f64;
        let url = reqwest::Url::parse_with_params(
            &self.base_url,
            [
                ("SERVICE", "WMS".to_string()),
                ("VERSION", "1.1.1".to_string()),
                ("REQUEST", "GetFeatureInfo".to_string()),
                ("LAYERS", WORLDCOVER_LAYER.to_string()),
                ("QUERY_LAYERS", WORLDCOVER_LAYER.to_string()),
                ("SRS", "EPSG:4326".to_string()),
                (
                    "BBOX",
                    format!(
                        "{},{},{},{}",
                        lon - epsilon,
                        lat - epsilon,
                        lon + epsilon,
                        lat + epsilon
                    ),
                ),
                ("WIDTH", "1".to_string()),
                ("HEIGHT", "1".to_string()),
                ("X", "0".to_string()),
                ("Y", "0".to_string()),
                ("INFO_FORMAT", "application/json".to_string()),
                ("FORMAT", "image/png".to_string()),
            ],
        )
        .map_err(|e| GeoError::WorldCoverUnavailable(e.to_string()))?;
        let response = client
            .get(url.clone())
            .header("User-Agent", USER_AGENT)
            .send()
            .await
            .map_err(|e| GeoError::WorldCoverUnavailable(e.to_string()))?;
        if !response.status().is_success() {
            return Err(GeoError::WorldCoverUnavailable(format!(
                "HTTP {}",
                response.status()
            )));
        }
        let body = response
            .text()
            .await
            .map_err(|e| GeoError::WorldCoverUnavailable(e.to_string()))?;
        Ok(RawLandCoverResponse {
            body,
            source_url: url.to_string(),
        })
    }

    fn normalize(raw: &RawLandCoverResponse) -> Result<NormalizedLandCoverClaim, GeoError> {
        let value = serde_json::from_str::<serde_json::Value>(&raw.body)
            .ok()
            .and_then(|json| find_numeric_class(&json))
            .or_else(|| raw.body.trim().parse::<i64>().ok())
            .ok_or(GeoError::WorldCoverUnparseable)?;

        // ESA WorldCover v200 classes. Preserve the existing circuit's
        // allowed agricultural code (40) while making its provenance
        // satellite-derived rather than an OSM tag proxy.
        let (code, classification) = match value {
            10 => (10, "tree cover"),
            20 => (20, "shrubland"),
            30 => (30, "grassland"),
            40 => (40, "cropland"),
            50 => (50, "built-up"),
            60 => (60, "bare / sparse vegetation"),
            70 => (70, "snow and ice"),
            80 => (80, "permanent water bodies"),
            90 => (90, "herbaceous wetland"),
            95 => (95, "mangroves"),
            100 => (100, "moss and lichen"),
            _ => (0, "unclassified"),
        };
        Ok(NormalizedLandCoverClaim {
            source_class: value,
            code,
            classification,
        })
    }
}

fn find_numeric_class(value: &serde_json::Value) -> Option<i64> {
    match value {
        serde_json::Value::Number(n) => n.as_i64(),
        serde_json::Value::String(s) => s.parse().ok(),
        serde_json::Value::Array(values) => values.iter().find_map(find_numeric_class),
        serde_json::Value::Object(map) => ["value", "class", "gray", "band1", "pixelValue"]
            .iter()
            .find_map(|key| map.get(*key).and_then(find_numeric_class))
            .or_else(|| map.values().find_map(find_numeric_class)),
        _ => None,
    }
}

#[cfg(test)]
mod provider_tests {
    use super::*;

    #[test]
    fn worldcover_class_is_normalized_deterministically() {
        let raw = RawLandCoverResponse {
            body: r#"{"features":[{"properties":{"value":40}}]}"#.to_string(),
            source_url: "https://example.invalid/wms".to_string(),
        };
        let claim = EsaWorldCoverProvider::normalize(&raw).unwrap();
        assert_eq!(claim.source_class, 40);
        assert_eq!(claim.code, 40);
        assert_eq!(claim.classification, "cropland");
    }

    #[test]
    fn unknown_worldcover_class_is_not_allowed_cropland() {
        let raw = RawLandCoverResponse {
            body: r#"{"value":255}"#.to_string(),
            source_url: "https://example.invalid/wms".to_string(),
        };
        assert_eq!(EsaWorldCoverProvider::normalize(&raw).unwrap().code, 0);
    }
}

fn validate(lat: f64, lon: f64) -> Result<(), GeoError> {
    if !(-90.0..=90.0).contains(&lat) {
        return Err(GeoError::InvalidLatitude(lat));
    }
    if !(-180.0..=180.0).contains(&lon) {
        return Err(GeoError::InvalidLongitude(lon));
    }
    Ok(())
}

/// Real reverse geocoding via Nominatim. No fallback guessing if it fails.
async fn reverse_geocode(
    client: &reqwest::Client,
    lat: f64,
    lon: f64,
) -> Result<(Option<String>, Option<String>), GeoError> {
    let url = format!(
        "{base}/reverse?format=jsonv2&lat={lat}&lon={lon}&zoom=10&addressdetails=1",
        base = NOMINATIM_BASE
    );
    let resp = client
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .map_err(|e| GeoError::NominatimUnavailable(e.to_string()))?;

    if !resp.status().is_success() {
        return Err(GeoError::NominatimUnavailable(format!(
            "HTTP {}",
            resp.status()
        )));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| GeoError::NominatimUnavailable(e.to_string()))?;

    let display_name = body
        .get("display_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let country = body
        .get("address")
        .and_then(|a| a.get("country"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok((country, display_name))
}

/// Real Overpass query for protected-area tags within
/// `radius_m` metres of the point.
async fn overpass_query(
    client: &reqwest::Client,
    lat: f64,
    lon: f64,
    radius_m: u32,
) -> Result<serde_json::Value, GeoError> {
    let query = format!(
        r#"[out:json][timeout:25];
(
  way(around:{radius},{lat},{lon})["boundary"="protected_area"];
  relation(around:{radius},{lat},{lon})["boundary"="protected_area"];
  way(around:{radius},{lat},{lon})["leisure"="nature_reserve"];
  relation(around:{radius},{lat},{lon})["leisure"="nature_reserve"];
);
out tags center {radius_small};"#,
        radius = radius_m,
        lat = lat,
        lon = lon,
        radius_small = 50
    );

    let resp = client
        .post(OVERPASS_BASE)
        .header("User-Agent", USER_AGENT)
        .form(&[("data", query)])
        .send()
        .await
        .map_err(|e| GeoError::OverpassUnavailable(e.to_string()))?;

    if !resp.status().is_success() {
        return Err(GeoError::OverpassUnavailable(format!(
            "HTTP {}",
            resp.status()
        )));
    }

    resp.json::<serde_json::Value>()
        .await
        .map_err(|e| GeoError::OverpassUnavailable(e.to_string()))
}

/// Runs the full real evidence lookup for a coordinate. Every field in the
/// result is derived from a live HTTP response, never invented.
pub async fn check_location(
    client: &reqwest::Client,
    lat: f64,
    lon: f64,
    protected_radius_m: u32,
) -> Result<LocationEvidence, GeoError> {
    validate(lat, lon)?;

    let (country, display_name) = reverse_geocode(client, lat, lon).await?;
    let overpass = overpass_query(client, lat, lon, protected_radius_m).await?;
    let provider = EsaWorldCoverProvider::from_environment();
    let raw_land_cover = provider.fetch_raw(client, lat, lon).await?;
    let land_claim = EsaWorldCoverProvider::normalize(&raw_land_cover)?;

    let elements = overpass
        .get("elements")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut matched_protected = Vec::new();

    for el in &elements {
        let tags = el.get("tags").cloned().unwrap_or_default();
        let is_protected = tags.get("boundary").and_then(|v| v.as_str()) == Some("protected_area")
            || tags.get("leisure").and_then(|v| v.as_str()) == Some("nature_reserve");

        if is_protected {
            let name = tags
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("(unnamed protected feature)")
                .to_string();
            matched_protected.push(name);
        }
    }

    let protected_status = !matched_protected.is_empty();
    let raw_response_sha256 = format!("{:x}", Sha256::digest(raw_land_cover.body.as_bytes()));

    let evidence = LocationEvidence {
        latitude: lat,
        longitude: lon,
        country,
        display_name,
        reverse_geocode_source: "Nominatim (OpenStreetMap)",
        reverse_geocode_source_url: "https://nominatim.org".to_string(),
        protected_area: ProtectedAreaEvidence {
            status: protected_status,
            matched_features: matched_protected,
            source: "OpenStreetMap via Overpass API",
            dataset: "boundary=protected_area / leisure=nature_reserve tags",
            query_radius_m: protected_radius_m,
            source_url: "https://overpass-api.de".to_string(),
            retrieved_at: now_iso(),
        },
        land_cover: LandCoverEvidence {
            classification: land_claim.classification.to_string(),
            code: land_claim.code,
            source: "ESA WorldCover",
            dataset: "ESA WorldCover 2021 land-cover map (10 m, Sentinel-1/Sentinel-2)",
            source_url: raw_land_cover.source_url,
            retrieved_at: now_iso(),
            note: "Satellite-derived class from ESA WorldCover 2021 v200. This is a land-cover \
                   observation, not a historical deforestation or legal-compliance determination.",
            raw_response_sha256,
        },
    };

    Ok(evidence)
}
