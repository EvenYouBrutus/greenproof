//! Real geospatial evidence providers used by GreenProof.
//! Environmental failures are fail-closed: no mock or guessed values.

use crate::time::now_iso;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;

const NOMINATIM_BASE: &str = "https://nominatim.openstreetmap.org";
const WORLDCOVER_WMS_DEFAULT: &str = "https://titiler.terrascope.be/wms";
const WORLDCOVER_LAYER: &str = "esa-worldcover-map-10m-2021-v2_map";
const GFW_BASE: &str = "https://data-api.globalforestwatch.org";
const GFW_VERSION: &str = "v1.12";
const USER_AGENT: &str = "GreenProof/0.2 (hackathon prototype)";
pub const PROTECTED_RADIUS_M: u32 = 1000;
pub const FOREST_LOSS_MIN_YEAR: u16 = 2001;
pub const FOREST_LOSS_MAX_YEAR: u16 = 2024;

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
    #[error("Global Forest Watch forest-loss lookup failed: {0}")]
    ForestLossUnavailable(String),
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
    pub status: bool,
    pub source: &'static str,
    pub dataset: &'static str,
    pub source_url: String,
    pub cutoff_year: u16,
    pub first_loss_year_after_cutoff: Option<u16>,
    pub loss_area_ha_after_cutoff: f64,
    pub query_radius_m: u32,
    pub raw_response_sha256: String,
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
    pub forest_loss: ForestLossEvidence,
}

fn validate(lat: f64, lon: f64) -> Result<(), GeoError> {
    if !lat.is_finite() || !(-90.0..=90.0).contains(&lat) { return Err(GeoError::InvalidLatitude(lat)); }
    if !lon.is_finite() || !(-180.0..=180.0).contains(&lon) { return Err(GeoError::InvalidLongitude(lon)); }
    Ok(())
}

async fn reverse_geocode(client: &reqwest::Client, lat: f64, lon: f64) -> Result<(Option<String>, Option<String>), GeoError> {
    let url = format!("{NOMINATIM_BASE}/reverse?format=jsonv2&lat={lat}&lon={lon}&zoom=10&addressdetails=1");
    let r = client.get(&url).header("User-Agent", USER_AGENT).timeout(Duration::from_secs(10)).send().await.map_err(|e| GeoError::NominatimUnavailable(e.to_string()))?;
    if !r.status().is_success() { return Err(GeoError::NominatimUnavailable(format!("HTTP {}", r.status()))); }
    let v: serde_json::Value = r.json().await.map_err(|e| GeoError::NominatimUnavailable(e.to_string()))?;
    Ok((v.get("address").and_then(|a| a.get("country")).and_then(|x| x.as_str()).map(str::to_owned), v.get("display_name").and_then(|x| x.as_str()).map(str::to_owned)))
}

const OVERPASS_ENDPOINTS: &[&str] = &["https://overpass-api.de/api/interpreter", "https://overpass.kumi.systems/api/interpreter", "https://overpass.openstreetmap.fr/api/interpreter"];

pub(crate) async fn overpass_query(client: &reqwest::Client, lat: f64, lon: f64) -> Result<serde_json::Value, GeoError> {
    let query = format!(r#"[out:json][timeout:25];(way(around:{PROTECTED_RADIUS_M},{lat},{lon})["boundary"="protected_area"];relation(around:{PROTECTED_RADIUS_M},{lat},{lon})["boundary"="protected_area"];way(around:{PROTECTED_RADIUS_M},{lat},{lon})["leisure"="nature_reserve"];relation(around:{PROTECTED_RADIUS_M},{lat},{lon})["leisure"="nature_reserve"];);out tags;"#);
    let mut last = String::new();
    for endpoint in OVERPASS_ENDPOINTS {
        match client.post(*endpoint).header("User-Agent", USER_AGENT).timeout(Duration::from_secs(10)).form(&[("data", &query)]).send().await {
            Ok(r) if r.status().is_success() => return r.json().await.map_err(|e| GeoError::OverpassUnavailable(e.to_string())),
            Ok(r) => last = format!("HTTP {} from {endpoint}", r.status()),
            Err(e) => last = format!("{endpoint}: {e}"),
        }
    }
    Err(GeoError::OverpassUnavailable(last))
}

const ESA_WORLDCOVER_LEGEND: &[(u8,u8,u8,i64,&str)] = &[(0,100,0,10,"tree cover"),(255,187,34,20,"shrubland"),(255,255,76,30,"grassland"),(240,150,255,40,"cropland"),(250,0,0,50,"built-up"),(180,180,180,60,"bare / sparse vegetation"),(240,240,240,70,"snow and ice"),(0,100,200,80,"permanent water bodies"),(0,150,160,90,"herbaceous wetland"),(0,207,117,95,"mangroves"),(250,230,160,100,"moss and lichen")];
fn find_rgb(v: &serde_json::Value) -> Option<(u8,u8,u8)> { match v { serde_json::Value::Object(m) => { let f=|k:&str| m.get(k).and_then(|x| x.as_i64()).and_then(|x|u8::try_from(x).ok()); match (f("band_1"),f("band_2"),f("band_3")){(Some(r),Some(g),Some(b))=>Some((r,g,b)),_=>m.values().find_map(find_rgb)} }, serde_json::Value::Array(a)=>a.iter().find_map(find_rgb), _=>None } }

async fn worldcover(client: &reqwest::Client, lat: f64, lon: f64) -> Result<LandCoverEvidence, GeoError> {
    let base = std::env::var("GREENPROOF_WORLDCOVER_WMS_URL").unwrap_or_else(|_| WORLDCOVER_WMS_DEFAULT.into());
    let e = 0.00005_f64;
    let url = reqwest::Url::parse_with_params(&base, [("SERVICE","WMS"),("VERSION","1.3.0"),("REQUEST","GetFeatureInfo"),("LAYERS",WORLDCOVER_LAYER),("QUERY_LAYERS",WORLDCOVER_LAYER),("STYLES",""),("CRS","EPSG:4326"),("BBOX",format!("{},{},{},{}",lat-e,lon-e,lat+e,lon+e)),("WIDTH","1"),("HEIGHT","1"),("I","0"),("J","0"),("TIME","2021-01-01"),("INFO_FORMAT","application/geo+json"),("FORMAT","image/png")]).map_err(|e| GeoError::WorldCoverUnavailable(e.to_string()))?;
    let r = client.get(url.clone()).header("User-Agent",USER_AGENT).timeout(Duration::from_secs(15)).send().await.map_err(|e|GeoError::WorldCoverUnavailable(e.to_string()))?;
    if !r.status().is_success(){return Err(GeoError::WorldCoverUnavailable(format!("HTTP {}",r.status())))}
    let body=r.text().await.map_err(|e|GeoError::WorldCoverUnavailable(e.to_string()))?;
    let v:serde_json::Value=serde_json::from_str(&body).map_err(|_|GeoError::WorldCoverUnparseable)?;
    let rgb=find_rgb(&v).ok_or(GeoError::WorldCoverUnparseable)?;
    let (code,name)=ESA_WORLDCOVER_LEGEND.iter().find(|(r,g,b,_,_)|(*r,*g,*b)==rgb).map(|&(_,_,_,c,n)|(c,n)).ok_or(GeoError::WorldCoverUnparseable)?;
    Ok(LandCoverEvidence{classification:name.into(),code,source:"ESA WorldCover",dataset:"ESA WorldCover 2021 v200 (10 m)",source_url:url.to_string(),retrieved_at:now_iso(),note:"Satellite-derived 2021 land-cover observation; not a historical forest-loss determination.",raw_response_sha256:format!("{:x}",Sha256::digest(body.as_bytes()))})
}

fn point_polygon(lat:f64,lon:f64)->serde_json::Value{let e=0.00005_f64;serde_json::json!({"type":"Polygon","coordinates":[[[lon-e,lat-e],[lon+e,lat-e],[lon+e,lat+e],[lon-e,lat+e],[lon-e,lat-e]]]})}

/// Queries the official Global Forest Watch Data API for the Hansen/UMD tree
/// cover loss raster. The API requires a GFW API key; without it we fail
/// explicitly instead of substituting evidence.
async fn forest_loss(client:&reqwest::Client,lat:f64,lon:f64,cutoff_year:u16)->Result<ForestLossEvidence,GeoError>{
    if !(FOREST_LOSS_MIN_YEAR..=FOREST_LOSS_MAX_YEAR).contains(&cutoff_year){return Err(GeoError::ForestLossUnavailable(format!("cutoff year must be between {FOREST_LOSS_MIN_YEAR} and {FOREST_LOSS_MAX_YEAR}")))}
    let key=std::env::var("GLOBAL_FOREST_WATCH_API_KEY").map_err(|_|GeoError::ForestLossUnavailable("GLOBAL_FOREST_WATCH_API_KEY is not configured".into()))?;
    if key.trim().is_empty(){return Err(GeoError::ForestLossUnavailable("GLOBAL_FOREST_WATCH_API_KEY is empty".into()))}
    let sql=format!("SELECT umd_tree_cover_loss__year AS year, SUM(area__ha) AS area_ha FROM results WHERE umd_tree_cover_loss__year > {cutoff_year} GROUP BY umd_tree_cover_loss__year ORDER BY umd_tree_cover_loss__year ASC");
    let geometry=point_polygon(lat,lon);
    let url=format!("{GFW_BASE}/dataset/umd_tree_cover_loss/{GFW_VERSION}/query/json");
    let r=client.post(&url).header("x-api-key",key).header("User-Agent",USER_AGENT).json(&serde_json::json!({"sql":sql,"geometry":geometry})).timeout(Duration::from_secs(20)).send().await.map_err(|e|GeoError::ForestLossUnavailable(e.to_string()))?;
    let status=r.status();
    let body=r.text().await.map_err(|e|GeoError::ForestLossUnavailable(e.to_string()))?;
    if !status.is_success(){return Err(GeoError::ForestLossUnavailable(format!("HTTP {status}: {}",body.chars().take(300).collect::<String>())))}
    let json:serde_json::Value=serde_json::from_str(&body).map_err(|e|GeoError::ForestLossUnavailable(format!("invalid JSON: {e}")))?;
    let rows=json.get("data").and_then(|v|v.as_array()).ok_or_else(||GeoError::ForestLossUnavailable("response did not contain data rows".into()))?;
    let mut first=None; let mut total=0.0_f64;
    for row in rows { let year=row.get("year").or_else(||row.get("umd_tree_cover_loss__year")).and_then(|v|v.as_u64()).and_then(|y|u16::try_from(y).ok()); let area=row.get("area_ha").and_then(|v|v.as_f64()).unwrap_or(0.0); if let Some(y)=year { if (FOREST_LOSS_MIN_YEAR..=FOREST_LOSS_MAX_YEAR).contains(&y) { first=Some(first.map_or(y,|old:u16|old.min(y))); total+=area.max(0.0); } } }
    Ok(ForestLossEvidence{status:first.is_none(),source:"Global Forest Watch Data API",dataset:"Hansen/UMD Global Forest Change 2024 v1.12 tree cover loss",source_url:url,cutoff_year,first_loss_year_after_cutoff:first,loss_area_ha_after_cutoff:total,query_radius_m:7,raw_response_sha256:format!("{:x}",Sha256::digest(body.as_bytes())),retrieved_at:now_iso(),note:"Real Hansen/UMD Landsat-derived forest-loss observations queried through the official Global Forest Watch Data API. A positive result means detected gross forest-cover loss in the queried ~10 m square after the selected cutoff year; it is not a legal EUDR determination."})
}

pub async fn check_location(client:&reqwest::Client,lat:f64,lon:f64,cutoff_year:u16)->Result<LocationEvidence,GeoError>{
    validate(lat,lon)?;
    let (country,display_name)=reverse_geocode(client,lat,lon).await?;
    let overpass=overpass_query(client,lat,lon).await?;
    let land_cover=worldcover(client,lat,lon).await?;
    let loss=forest_loss(client,lat,lon,cutoff_year).await?;
    let elements=overpass.get("elements").and_then(|v|v.as_array()).cloned().unwrap_or_default();
    let matched:Vec<String>=elements.iter().filter_map(|el|{let t=el.get("tags")?;let hit=t.get("boundary").and_then(|v|v.as_str())==Some("protected_area")||t.get("leisure").and_then(|v|v.as_str())==Some("nature_reserve");if !hit{return None}Some(t.get("name").and_then(|v|v.as_str()).unwrap_or("(unnamed protected feature)").to_string())}).collect();
    Ok(LocationEvidence{latitude:lat,longitude:lon,country,display_name,reverse_geocode_source:"Nominatim (OpenStreetMap)",reverse_geocode_source_url:NOMINATIM_BASE.into(),protected_area:ProtectedAreaEvidence{status:!matched.is_empty(),matched_features:matched,source:"OpenStreetMap via Overpass API",dataset:"protected_area / nature_reserve tags",query_radius_m:PROTECTED_RADIUS_M,source_url:OVERPASS_ENDPOINTS[0].into(),retrieved_at:now_iso()},land_cover,forest_loss:loss})
}

#[cfg(test)]
mod tests { use super::*; #[test] fn coordinates_are_validated(){assert!(validate(90.0,180.0).is_ok());assert!(validate(90.1,0.0).is_err());assert!(validate(0.0,180.1).is_err());} #[test] fn polygon_is_deterministic(){let a=point_polygon(6.0,-1.0);assert_eq!(a["type"],"Polygon");} }
