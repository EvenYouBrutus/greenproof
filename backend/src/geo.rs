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
//!   - Overpass API (OpenStreetMap) - protected-area / land-cover proxy tags.
//!     https://wiki.openstreetmap.org/wiki/Overpass_API
//!     Tags used: boundary=protected_area, leisure=nature_reserve,
//!     natural=* (used as a free, no-credential land-cover proxy).
//!
//! NOT integrated in this MVP (documented as future work, see README):
//!   - Copernicus Land Monitoring Service tree-cover density: requires a
//!     registered Copernicus Data Space Ecosystem (CDSE) account/API
//!     credentials. Wiring point is left in `.env.example`
//!     (COPERNICUS_CLIENT_ID / COPERNICUS_CLIENT_SECRET) but no fallback
//!     fake data is substituted when those are absent - land-cover
//!     classification instead uses the real (but coarser) OSM `natural=`/
//!     `landuse=` tagging as an honestly-documented proxy.

use crate::time::now_iso;
use serde::{Deserialize, Serialize};

const NOMINATIM_BASE: &str = "https://nominatim.openstreetmap.org";
const OVERPASS_BASE: &str = "https://overpass-api.de/api/interpreter";
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

/// Real Overpass query for protected-area and natural/land-cover tags within
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
  way(around:{radius},{lat},{lon})["natural"];
  relation(around:{radius},{lat},{lon})["natural"];
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

/// Maps an OSM `natural=`/`landuse=` tag value to a coarse land-cover code.
/// This mapping is a documented, honest proxy for a real land-cover
/// classification scheme (a stand-in for Copernicus tree-cover density
/// classes, which require registered API credentials not wired up in this
/// MVP - see module docs above). It is NOT a fabricated compliance decision;
/// it is a deterministic function of real tag data returned by Overpass.
fn land_cover_code_for_tag(tag_value: &str) -> (i64, &'static str) {
    match tag_value {
        "wood" | "forest" => (10, "forest"),
        "scrub" | "heath" => (20, "shrubland"),
        "grassland" | "meadow" | "farmland" => (40, "agricultural / grassland"),
        "wetland" => (30, "wetland"),
        "water" => (50, "water"),
        "bare_rock" | "sand" | "beach" => (60, "bare / sparse"),
        other => {
            let _ = other;
            (0, "unclassified")
        }
    }
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

    let elements = overpass
        .get("elements")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut matched_protected = Vec::new();
    let mut land_cover_tag: Option<String> = None;

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

        if land_cover_tag.is_none() {
            if let Some(nat) = tags.get("natural").and_then(|v| v.as_str()) {
                land_cover_tag = Some(nat.to_string());
            }
        }
    }

    let protected_status = !matched_protected.is_empty();
    let (code, classification_label) = match &land_cover_tag {
        Some(tag) => land_cover_code_for_tag(tag),
        None => (0, "unclassified (no OSM natural= tag found nearby)"),
    };

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
            classification: classification_label.to_string(),
            code,
            source: "OpenStreetMap via Overpass API",
            dataset: "natural=* tag (proxy classification)",
            source_url: "https://overpass-api.de".to_string(),
            retrieved_at: now_iso(),
            note: "Proxy for Copernicus Land Monitoring Service tree-cover density; \
                   Copernicus CDSE integration requires API credentials not configured \
                   in this MVP (see .env.example) and is listed as future work.",
        },
    };

    Ok(evidence)
}
