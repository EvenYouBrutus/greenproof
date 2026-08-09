use crate::geo::ForestLossEvidence;
use crate::time::now_iso;
use reqwest::Client;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{path::{Path, PathBuf}, time::Duration};
use thiserror::Error;
use tokio::fs;

const BASE: &str = "https://storage.googleapis.com/earthenginepartners-hansen/GFC-2024-v1.12";
pub const MIN_YEAR: u16 = 2001;
pub const MAX_YEAR: u16 = 2024;

#[derive(Debug, Error)]
pub enum HansenError {
    #[error("Hansen cutoff year must be between {MIN_YEAR} and {MAX_YEAR}")]
    InvalidCutoff,
    #[error("Hansen download failed: {0}")]
    Download(String),
    #[error("Hansen cache error: {0}")]
    Cache(String),
    #[error("Hansen TIFF error: {0}")]
    Tiff(String),
}

fn tile(lat: f64, lon: f64) -> (i32, i32) {
    ((lat.floor() as i32 / 10) * 10, (lon.floor() as i32 / 10) * 10)
}
fn lat_name(v: i32) -> String { format!("{:02}{}", v.abs(), if v >= 0 { "N" } else { "S" }) }
fn lon_name(v: i32) -> String { format!("{:03}{}", v.abs(), if v >= 0 { "E" } else { "W" }) }
fn filename(lat: i32, lon: i32) -> String { format!("Hansen_GFC-2024-v1.12_lossyear_{}_{}.tif", lat_name(lat), lon_name(lon)) }
fn url(lat: i32, lon: i32) -> String { format!("{BASE}/{}.zip", filename(lat, lon).trim_end_matches(".tif")) }

async fn cached_tile(client: &Client, cache: &Path, lat: i32, lon: i32) -> Result<PathBuf, HansenError> {
    fs::create_dir_all(cache).await.map_err(|e| HansenError::Cache(e.to_string()))?;
    let tif = cache.join(filename(lat, lon));
    if fs::try_exists(&tif).await.unwrap_or(false) { return Ok(tif); }
    let zip_path = cache.join(format!("{}.zip", filename(lat, lon).trim_end_matches(".tif")));
    let response = client.get(url(lat, lon)).header("User-Agent", "GreenProof/0.3").timeout(Duration::from_secs(60)).send().await.map_err(|e| HansenError::Download(e.to_string()))?;
    let status = response.status();
    if !status.is_success() { return Err(HansenError::Download(format!("HTTP {} for {}", status, url(lat, lon)))); }
    let bytes = response.bytes().await.map_err(|e| HansenError::Download(e.to_string()))?;
    fs::write(&zip_path, &bytes).await.map_err(|e| HansenError::Cache(e.to_string()))?;
    let file = std::fs::File::open(&zip_path).map_err(|e| HansenError::Cache(e.to_string()))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| HansenError::Cache(e.to_string()))?;
    let mut found = false;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| HansenError::Cache(e.to_string()))?;
        let name = Path::new(entry.name()).file_name().and_then(|x| x.to_str()).unwrap_or("").to_owned();
        if name == filename(lat, lon) {
            let mut out = std::fs::File::create(&tif).map_err(|e| HansenError::Cache(e.to_string()))?;
            std::io::copy(&mut entry, &mut out).map_err(|e| HansenError::Cache(e.to_string()))?;
            found = true;
            break;
        }
    }
    let _ = std::fs::remove_file(&zip_path);
    if !found { return Err(HansenError::Cache(format!("archive did not contain {}", filename(lat, lon)))); }
    Ok(tif)
}

/// Read the nearest 30m Hansen loss-year pixel. GeoTIFF is north-up EPSG:4326.
fn read_lossyear(path: &Path, lat: f64, lon: f64) -> Result<u8, HansenError> {
    let mut decoder = tiff::decoder::Decoder::new(std::fs::File::open(path).map_err(|e| HansenError::Tiff(e.to_string()))?).map_err(|e| HansenError::Tiff(e.to_string()))?;
    let (w, h) = decoder.dimensions().map_err(|e| HansenError::Tiff(e.to_string()))?;
    let bounds = decoder.get_tag(tiff::tags::Tag::ModelTiepointTag).map_err(|e| HansenError::Tiff(e.to_string()))?;
    let scale = decoder.get_tag(tiff::tags::Tag::ModelPixelScaleTag).map_err(|e| HansenError::Tiff(e.to_string()))?;
    let tie = match bounds { tiff::decoder::ifd::Value::Double(v) => v, _ => return Err(HansenError::Tiff("unexpected ModelTiepointTag".into())) };
    let pix = match scale { tiff::decoder::ifd::Value::Double(v) => v, _ => return Err(HansenError::Tiff("unexpected ModelPixelScaleTag".into())) };
    if tie.len() < 6 || pix.len() < 2 { return Err(HansenError::Tiff("invalid GeoTIFF georeferencing tags".into())); }
    let x = ((lon - tie[3]) / pix[0]).floor().max(0.0) as u32;
    let y = ((tie[4] - lat) / pix[1]).floor().max(0.0) as u32;
    if x >= w || y >= h { return Err(HansenError::Tiff("coordinate outside tile".into())); }
    decoder.seek_to_image(0).map_err(|e| HansenError::Tiff(e.to_string()))?;
    let image = decoder.read_image().map_err(|e| HansenError::Tiff(e.to_string()))?;
    match image {
        tiff::decoder::DecodingResult::U8(data) => Ok(data[(y * w + x) as usize]),
        _ => Err(HansenError::Tiff("lossyear raster is not 8-bit".into())),
    }
}

pub async fn check(client: &Client, cache_dir: &str, lat: f64, lon: f64, cutoff: u16) -> Result<ForestLossEvidence, HansenError> {
    if !(MIN_YEAR..=MAX_YEAR).contains(&cutoff) { return Err(HansenError::InvalidCutoff); }
    let (tile_lat, tile_lon) = tile(lat, lon);
    let path = cached_tile(client, Path::new(cache_dir), tile_lat, tile_lon).await?;
    let value = read_lossyear(&path, lat, lon)?;
    let first = if value == 0 { None } else { Some(2000 + value as u16) };
    let loss_after_cutoff = first.filter(|y| *y > cutoff);
    let raw = fs::read(&path).await.map_err(|e| HansenError::Cache(e.to_string()))?;
    let digest = format!("{:x}", Sha256::digest(&raw));
    Ok(ForestLossEvidence {
        status: loss_after_cutoff.is_none(),
        source: "Hansen Global Forest Change via official UMD GLAD archive",
        dataset: "Hansen/UMD GFC 2024 v1.12 loss year, 30 m",
        source_url: format!("{BASE}/download.html"),
        cutoff_year: cutoff,
        first_loss_year_after_cutoff: loss_after_cutoff,
        loss_area_ha_after_cutoff: 0.0,
        query_radius_m: 15,
        raw_response_sha256: digest,
        retrieved_at: now_iso(),
        note: "The official Hansen loss-year raster is read at the private coordinate. A non-zero pixel value is the detected gross forest-cover loss year; this predicate checks whether that year is after the selected cutoff. It does not establish legal land status or prove that no loss occurred elsewhere on a larger parcel.",
    })
}
