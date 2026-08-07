// GreenProof - coordinate encoding (browser side).
// MUST stay numerically identical to scripts/lib/encode.js and the encoding
// documented in circuits/environmental_compliance.circom, or proofs
// generated here will not match evidence hashes computed elsewhere.

const LAT_OFFSET = 90;
const LON_OFFSET = 180;
const SCALE = 1_000_000;

export function encodeLat(lat: number): bigint {
  if (Number.isNaN(lat) || lat < -90 || lat > 90) {
    throw new Error(`Invalid latitude: ${lat}`);
  }
  return BigInt(Math.round((lat + LAT_OFFSET) * SCALE));
}

export function encodeLon(lon: number): bigint {
  if (Number.isNaN(lon) || lon < -180 || lon > 180) {
    throw new Error(`Invalid longitude: ${lon}`);
  }
  return BigInt(Math.round((lon + LON_OFFSET) * SCALE));
}

export function boundingBoxEnc(minLat: number, maxLat: number, minLon: number, maxLon: number) {
  return {
    latMin: encodeLat(minLat),
    latMax: encodeLat(maxLat),
    lonMin: encodeLon(minLon),
    lonMax: encodeLon(maxLon),
  };
}

export function strToField(s: string): bigint {
  let x = 0n;
  const bytes = new TextEncoder().encode(s);
  for (const b of bytes) {
    x = (x * 256n + BigInt(b)) % (2n ** 200n);
  }
  return x;
}
