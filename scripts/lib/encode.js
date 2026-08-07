// GreenProof - shared coordinate encoding
//
// Circom circuits only work over a prime field and circomlib's comparators
// operate on non-negative integers of a fixed bit width. Latitude/longitude
// are signed decimals, so we deterministically encode them as non-negative
// integers before they ever become circuit witnesses:
//
//   latEnc = round((latitude + 90)  * 1_000_000)   -> range [0, 180_000_000]
//   lonEnc = round((longitude + 180) * 1_000_000)  -> range [0, 360_000_000]
//
// 1e6 scale keeps ~0.11m precision at the equator, which is far finer than
// the resolution of the underlying land-cover/protected-area datasets, so no
// meaningful precision is lost relative to the real data's own resolution.
//
// This module is imported by BOTH:
//   - the geospatial evidence layer (to compute evidenceHash inputs), and
//   - the witness generator (to build the private circuit inputs)
// so the two never drift apart.

const LAT_OFFSET = 90;
const LON_OFFSET = 180;
const SCALE = 1_000_000;

function encodeLat(lat) {
  if (typeof lat !== "number" || Number.isNaN(lat) || lat < -90 || lat > 90) {
    throw new Error(`Invalid latitude: ${lat}`);
  }
  return Math.round((lat + LAT_OFFSET) * SCALE);
}

function encodeLon(lon) {
  if (typeof lon !== "number" || Number.isNaN(lon) || lon < -180 || lon > 180) {
    throw new Error(`Invalid longitude: ${lon}`);
  }
  return Math.round((lon + LON_OFFSET) * SCALE);
}

function boundingBoxEnc(minLat, maxLat, minLon, maxLon) {
  return {
    latMin: encodeLat(minLat),
    latMax: encodeLat(maxLat),
    lonMin: encodeLon(minLon),
    lonMax: encodeLon(maxLon),
  };
}

module.exports = { encodeLat, encodeLon, boundingBoxEnc, SCALE, LAT_OFFSET, LON_OFFSET };
