use crate::geo::ForestLossEvidence;
use crate::time::now_iso;
use geotiff_reader::GeoTiffFile;
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::{path::{Path, PathBuf}, time::Duration};
use thiserror::Error;
use tokio::fs;

const BASE:&str="https://storage.googleapis.com/earthenginepartners-hansen/GFC-2024-v1.12";
pub const MIN_YEAR:u16=2001;
pub const MAX_YEAR:u16=2024;
#[derive(Debug,Error)] pub enum HansenError{
 #[error("Hansen cutoff year must be between {MIN_YEAR} and {MAX_YEAR}")] InvalidCutoff,
 #[error("Hansen download failed: {0}")] Download(String),
 #[error("Hansen cache error: {0}")] Cache(String),
 #[error("Hansen GeoTIFF error: {0}")] Tiff(String),
}
fn tile_start(v:f64)->i32{((v.floor() as i32).div_euclid(10))*10}
fn lat_name(v:i32)->String{format!("{:02}{}",v.abs(),if v>=0{"N"}else{"S"})}
fn lon_name(v:i32)->String{format!("{:03}{}",v.abs(),if v>=0{"E"}else{"W"})}
fn filename(lat:i32,lon:i32)->String{format!("Hansen_GFC-2024-v1.12_lossyear_{}_{}.tif",lat_name(lat),lon_name(lon))}
fn url(lat:i32,lon:i32)->String{format!("{BASE}/{}",filename(lat,lon))}
async fn cached_tile(client:&Client,cache:&Path,lat:i32,lon:i32)->Result<PathBuf,HansenError>{fs::create_dir_all(cache).await.map_err(|e|HansenError::Cache(e.to_string()))?;let path=cache.join(filename(lat,lon));if fs::try_exists(&path).await.unwrap_or(false){return Ok(path)}let r=client.get(url(lat,lon)).header("User-Agent","GreenProof/0.3").timeout(Duration::from_secs(90)).send().await.map_err(|e|HansenError::Download(e.to_string()))?;let status=r.status();if !status.is_success(){return Err(HansenError::Download(format!("HTTP {status} for {}",url(lat,lon))))}let bytes=r.bytes().await.map_err(|e|HansenError::Download(e.to_string()))?;let tmp=cache.join(format!("{}.part",filename(lat,lon)));fs::write(&tmp,&bytes).await.map_err(|e|HansenError::Cache(e.to_string()))?;fs::rename(&tmp,&path).await.map_err(|e|HansenError::Cache(e.to_string()))?;Ok(path)}
fn read_pixel(path:&Path,lat:f64,lon:f64)->Result<u8,HansenError>{let file=GeoTiffFile::open(path).map_err(|e|HansenError::Tiff(e.to_string()))?;let(col,row)=file.geo_to_pixel(lon,lat).ok_or_else(||HansenError::Tiff("coordinate could not be transformed to pixel coordinates".into()))?;let col=col.floor() as isize;let row=row.floor() as isize;if col<0||row<0||col>=file.width() as isize||row>=file.height() as isize{return Err(HansenError::Tiff("coordinate outside Hansen tile".into()))}let data=file.read_band_window::<u8>(0,row as usize,col as usize,1,1).map_err(|e|HansenError::Tiff(e.to_string()))?;data.into_iter().next().ok_or_else(||HansenError::Tiff("empty pixel window".into()))}
pub async fn check(client:&Client,cache_dir:&str,lat:f64,lon:f64,cutoff:u16)->Result<ForestLossEvidence,HansenError>{if !(MIN_YEAR..=MAX_YEAR).contains(&cutoff){return Err(HansenError::InvalidCutoff)}let(tl_lat,tl_lon)=(tile_start(lat),tile_start(lon));let path=cached_tile(client,Path::new(cache_dir),tl_lat,tl_lon).await?;let path_for_read=path.clone();let value=tokio::task::spawn_blocking(move||read_pixel(&path_for_read,lat,lon)).await.map_err(|e|HansenError::Tiff(e.to_string()))??;let detected=if value==0{None}else{Some(2000+value as u16)};let first=detected.filter(|year|*year>cutoff);let binding=format!("Hansen/UMD GFC 2024 v1.12|{}|{}|pixel={}|cutoff={}",tl_lat,tl_lon,value,cutoff);let digest=format!("{:x}",Sha256::digest(binding.as_bytes()));Ok(ForestLossEvidence{status:first.is_none(),source:"Hansen Global Forest Change via official UMD GLAD archive",dataset:"Hansen/UMD GFC 2024 v1.12 loss year, 30 m",source_url:format!("{BASE}/download.html"),cutoff_year:cutoff,first_loss_year_after_cutoff:first,loss_area_ha_after_cutoff:0.0,query_radius_m:15,raw_response_sha256:digest,retrieved_at:now_iso(),note:"The official Hansen loss-year GeoTIFF is read at the private coordinate. Values 1-24 represent detected gross forest-cover loss primarily in 2001-2024; 0 means no loss detected in that pixel. This is a relative indicator of forest-loss dynamics, not a legal land-status determination or proof about an entire parcel."})}
#[cfg(test)]mod tests{use super::*;#[test]fn negative_longitudes_use_correct_tiles(){assert_eq!(tile_start(-1.6),-10);assert_eq!(tile_start(-10.0),-10);assert_eq!(tile_start(6.6),0);}}
