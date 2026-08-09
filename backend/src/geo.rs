//! GreenProof real environmental evidence layer.
//!
//! Every environmental value returned by this module is derived from a live
//! provider response. Provider failures are returned explicitly; no mock or
//! fallback environmental values are permitted.

use crate::time::now_iso;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;

const NOMINATIM_BASE: &str = "https://nominatim.openstreetmap.org";
const WORLDCOVER_WMS_DEFAULT: &str = "https://titiler.terrascope.be/wms";
const WORLDCOVER_LAYER: &str = "esa-worldcover-map-10m-2021-v2_map";
const GFW_TILE_BASE: &str = "https://storage.googleapis.com/earthenginepartners-hansen/GFC-2024-v1.12";
const USER_AGENT: &str = "GreenProof/0.2 (hackathon prototype)";
pub const PROTECTED_RADIUS_M: u32 = 1000;
pub const FOREST_LOSS_YEAR_START: u16 = 2001;
pub const FOREST_LOSS_YEAR_END: u16 = 2024;

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
    #[error("Hansen forest-loss lookup failed: {0}")]
    ForestLossUnavailable(String),
    #[error("Hansen forest-loss tile is unavailable for this coordinate")]
    ForestLossTileUnavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectedAreaEvidence {
    pub status: bool,
    pub matched_features: Vec<String>,
    pub source: &'static str,
    pub dataset: &'static str,
    pub query_radius_m: u32,
    pub source_url: String,
    pub retrieved_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LandCoverEvidence {
    pub classification: String,
    pub code: i64,
    pub source: &'static str,
    pub dataset: &'static str,
    pub source_url: String,
    pub retrieved_at: String,
    pub note: &'static str,
    pub raw_response_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForestLossEvidence {
    pub dataset: &'static str,
    pub source: &'static str,
    pub source_url: String,
    pub tree_cover_baseline_percent: u8,
    pub first_loss_year: Option<u16>,
    pub cutoff_year: u16,
    pub no_loss_after_cutoff: bool,
    pub tile: String,
    pub tile_response_sha256: String,
    pub retrieved_at: String,
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
    pub forest_loss: ForestLossEvidence,
}

#[derive(Debug, Clone)]
struct RawLandCoverResponse { body: String, source_url: String }

fn validate(lat: f64, lon: f64) -> Result<(), GeoError> {
    if !lat.is_finite() || !(-90.0..=90.0).contains(&lat) { return Err(GeoError::InvalidLatitude(lat)); }
    if !lon.is_finite() || !(-180.0..=180.0).contains(&lon) { return Err(GeoError::InvalidLongitude(lon)); }
    Ok(())
}

fn gfw_tile_name(lat: f64, lon: f64) -> String {
    let ns = if lat >= 0.0 { 'N' } else { 'S' };
    let ew = if lon >= 0.0 { 'E' } else { 'W' };
    format!("Hansen_GFC-2024-v1.12_lossyear_{}{:03}_{}{:03}.tif", ns, lat.abs().floor() as u32, ew, lon.abs().floor() as u32)
}

fn gfw_tile_url(tile: &str) -> String { format!("{GFW_TILE_BASE}/Hansen_GFC-2024-v1.12_lossyear_10N_000E.tif") }

/// Deterministic forest-loss evidence provider.
///
/// The Hansen GFC collection stores `lossyear` as 1=2001, ..., 24=2024.
/// A production implementation should decode the requested GeoTIFF pixel;
/// this hackathon implementation deliberately fails closed until a decodable
/// tile response is available rather than inventing a loss year.
async fn fetch_forest_loss(
    client: &reqwest::Client,
    lat: f64,
    lon: f64,
    cutoff_year: u16,
) -> Result<ForestLossEvidence, GeoError> {
    if !(FOREST_LOSS_YEAR_START..=FOREST_LOSS_YEAR_END).contains(&cutoff_year) {
        return Err(GeoError::ForestLossUnavailable(format!("cutoff year must be between {FOREST_LOSS_YEAR_START} and {FOREST_LOSS_YEAR_END}")));
    }
    let tile = gfw_tile_name(lat, lon);
    let url = gfw_tile_url(&tile);
    let response = client.get(&url).header("User-Agent", USER_AGENT).timeout(Duration::from_secs(20)).send().await
        .map_err(|e| GeoError::ForestLossUnavailable(e.to_string()))?;
    if !response.status().is_success() { return Err(GeoError::ForestLossUnavailable(format!("HTTP {} for {}", response.status(), tile))); }
    let bytes = response.bytes().await.map_err(|e| GeoError::ForestLossUnavailable(e.to_string()))?;
    if bytes.len() < 4 || &bytes[..4] != b"II*\0" && &bytes[..4] != b"MM\0*" { return Err(GeoError::ForestLossTileUnavailable); }
    Err(GeoError::ForestLossTileUnavailable)
}

// Kept as a small, testable parser boundary. GeoTIFF decoding is intentionally
// not hidden behind a fake value: unsupported tile decoding fails closed.

async fn reverse_geocode(client: &reqwest::Client, lat: f64, lon: f64) -> Result<(Option<String>, Option<String>), GeoError> {
    let url = format!("{NOMINATIM_BASE}/reverse?format=jsonv2&lat={lat}&lon={lon}&zoom=10&addressdetails=1");
    let resp = client.get(&url).header("User-Agent", USER_AGENT).timeout(Duration::from_secs(10)).send().await
        .map_err(|e| GeoError::NominatimUnavailable(e.to_string()))?;
    if !resp.status().is_success() { return Err(GeoError::NominatimUnavailable(format!("HTTP {}", resp.status()))); }
    let body: serde_json::Value = resp.json().await.map_err(|e| GeoError::NominatimUnavailable(e.to_string()))?;
    Ok((body.get("address").and_then(|a| a.get("country")).and_then(|v| v.as_str()).map(str::to_string), body.get("display_name").and_then(|v| v.as_str()).map(str::to_string)))
}

const OVERPASS_ENDPOINTS: &[&str] = &["https://overpass-api.de/api/interpreter", "https://overpass.kumi.systems/api/interpreter", "https://overpass.openstreetmap.fr/api/interpreter"];

pub(crate) async fn overpass_query(client: &reqwest::Client, lat: f64, lon: f64) -> Result<serde_json::Value, GeoError> {
    let query = format!(r#"[out:json][timeout:25];(way(around:{PROTECTED_RADIUS_M},{lat},{lon})["boundary"="protected_area"];relation(around:{PROTECTED_RADIUS_M},{lat},{lon})["boundary"="protected_area"];way(around:{PROTECTED_RADIUS_M},{lat},{lon})["leisure"="nature_reserve"];relation(around:{PROTECTED_RADIUS_M},{lat},{lon})["leisure"="nature_reserve"];);out tags;"#);
    let mut last = String::new();
    for endpoint in OVERPASS_ENDPOINTS {
        match client.post(*endpoint).header("User-Agent", USER_AGENT).timeout(Duration::from_secs(10)).form(&[("data", &query)]).send().await {
            Ok(resp) if resp.status().is_success() => return resp.json().await.map_err(|e| GeoError::OverpassUnavailable(e.to_string())),
            Ok(resp) => last = format!("HTTP {} from {endpoint}", resp.status()),
            Err(e) => last = format!("{endpoint}: {e}"),
        }
    }
    Err(GeoError::OverpassUnavailable(last))
}

async fn fetch_worldcover(client: &reqwest::Client, lat: f64, lon: f64) -> Result<LandCoverEvidence, GeoError> {
    let epsilon = 0.00005_f64;
    let url = reqwest::Url::parse_with_params(&std::env::var("GREENPROOF_WORLDCOVER_WMS_URL").unwrap_or_else(|_| WORLDCOVER_WMS_DEFAULT.into()), [
        ("SERVICE", "WMS"), ("VERSION", "1.3.0"), ("REQUEST", "GetFeatureInfo"),
        ("LAYERS", WORLDCOVER_LAYER), ("QUERY_LAYERS", WORLDCOVER_LAYER), ("STYLES", ""),
        ("CRS", "EPSG:4326"), ("BBOX", &format!("{},{},{},{}", lat-epsilon, lon-epsilon, lat+epsilon, lon+epsilon)),
        ("WIDTH", "1"), ("HEIGHT", "1"), ("I", "0"), ("J", "0"), ("TIME", "2021-01-01"),
        ("INFO_FORMAT", "application/geo+json"), ("FORMAT", "image/png")
    ]).map_err(|e| GeoError::WorldCoverUnavailable(e.to_string()))?;
    let resp = client.get(url.clone()).header("User-Agent", USER_AGENT).timeout(Duration::from_secs(15)).send().await.map_err(|e| GeoError::WorldCoverUnavailable(e.to_string()))?;
    if !resp.status().is_success() { return Err(GeoError::WorldCoverUnavailable(format!("HTTP {}", resp.status()))); }
    let body = resp.text().await.map_err(|e| GeoError::WorldCoverUnavailable(e.to_string()))?;
    let json: serde_json::Value = serde_json::from_str(&body).map_err(|_| GeoError::WorldCoverUnparseable)?;
    let rgb = find_rgb(&json).ok_or(GeoError::WorldCoverUnparseable)?;
    let (code, classification) = ESA_WORLDCOVER_LEGEND.iter().find(|(r,g,b,_,_)| (*r,*g,*b)==rgb).map(|&(_,_,_,c,n)|(c,n)).ok_or(GeoError::WorldCoverUnparseable)?;
    Ok(LandCoverEvidence { classification: classification.into(), code, source: "ESA WorldCover", dataset: "ESA WorldCover 2021 v200 (10 m)", source_url: url.to_string(), retrieved_at: now_iso(), note: "Satellite-derived 2021 land-cover observation; not a historical forest-loss determination.", raw_response_sha256: format!("{:x}", Sha256::digest(body.as_bytes())) })
}

const ESA_WORLDCOVER_LEGEND: &[(u8,u8,u8,i64,&str)] = &[(0,100,0,10,"tree cover"),(255,187,34,20,"shrubland"),(255,255,76,30,"grassland"),(240,150,255,40,"cropland"),(250,0,0,50,"built-up"),(180,180,180,60,"bare / sparse vegetation"),(240,240,240,70,"snow and ice"),(0,100,200,80,"permanent water bodies"),(0,150,160,90,"herbaceous wetland"),(0,207,117,95,"mangroves"),(250,230,160,100,"moss and lichen")];
fn find_rgb(v: &serde_json::Value) -> Option<(u8,u8,u8)> { match v { serde_json::Value::Object(m) => { let g=|k:&str| m.get(k).and_then(|x|x.as_i64()).and_then(|x|u8::try_from(x).ok()); match (g("band_1"),g("band_2"),g("band_3")){(Some(r),Some(g),Some(b))=>Some((r,g,b)),_=>m.values().find_map(find_rgb)} }, serde_json::Value::Array(a)=>a.iter().find_map(find_rgb), _=>None } }

pub async fn check_location(client: &reqwest::Client, lat: f64, lon: f64, cutoff_year: u16) -> Result<LocationEvidence, GeoError> {
    validate(lat, lon)?;
    let (country, display_name) = reverse_geocode(client, lat, lon).await?;
    let overpass = overpass_query(client, lat, lon).await?;
    let land_cover = fetch_worldcover(client, lat, lon).await?;
    let forest_loss = fetch_forest_loss(client, lat, lon, cutoff_year).await?;
    let elements = overpass.get("elements").and_then(|v|v.as_array()).cloned().unwrap_or_default();
    let matched_protected: Vec<String> = elements.iter().filter_map(|el| { let t=el.get("tags")?; let hit=t.get("boundary").and_then(|v|v.as_str())==Some("protected_area") || t.get("leisure").and_then(|v|v.as_str())==Some("nature_reserve"); if !hit{return None;} Some(t.get("name").and_then(|v|v.as_str()).unwrap_or("(unnamed protected feature)").to_string()) }).collect();
    Ok(LocationEvidence { latitude: lat, longitude: lon, country, display_name, reverse_geocode_source:"Nominatim (OpenStreetMap)", reverse_geocode_source_url:NOMINATIM_BASE.into(), protected_area:ProtectedAreaEvidence { status:!matched_protected.is_empty(), matched_features:matched_protected, source:"OpenStreetMap via Overpass API", dataset:"protected_area / nature_reserve tags", query_radius_m:PROTECTED_RADIUS_M, source_url:OVERPASS_ENDPOINTS[0].into(), retrieved_at:now_iso() }, land_cover, forest_loss })
}
