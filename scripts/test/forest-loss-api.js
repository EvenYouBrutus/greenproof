#!/usr/bin/env node
// Independent smoke test for the exact GFW/Hansen query used by GreenProof.
// Usage: GLOBAL_FOREST_WATCH_API_KEY=... node test/forest-loss-api.js 6.6666 -1.6163 2020
const cutoff = Number(process.argv[4] || 2020);
const lat = Number(process.argv[2] || 6.6666);
const lon = Number(process.argv[3] || -1.6163);
const key = process.env.GLOBAL_FOREST_WATCH_API_KEY;
if (!key) throw new Error("GLOBAL_FOREST_WATCH_API_KEY is required");
if (!Number.isFinite(lat) || lat < -90 || lat > 90) throw new Error("Invalid latitude");
if (!Number.isFinite(lon) || lon < -180 || lon > 180) throw new Error("Invalid longitude");
if (!Number.isInteger(cutoff) || cutoff < 2001 || cutoff > 2024) throw new Error("Cutoff must be 2001..2024");

const e = 0.00005;
const geometry = { type:"Polygon", coordinates:[[[lon-e,lat-e],[lon+e,lat-e],[lon+e,lat+e],[lon-e,lat+e],[lon-e,lat-e]]] };
const sql = `SELECT umd_tree_cover_loss__year AS year, SUM(area__ha) AS area_ha FROM results WHERE umd_tree_cover_loss__year > ${cutoff} GROUP BY umd_tree_cover_loss__year ORDER BY umd_tree_cover_loss__year ASC`;

(async()=>{
  const response = await fetch("https://data-api.globalforestwatch.org/dataset/umd_tree_cover_loss/v1.12/query/json",{method:"POST",headers:{"x-api-key":key,"Content-Type":"application/json","User-Agent":"GreenProof/0.2 smoke-test"},body:JSON.stringify({sql,geometry})});
  const body = await response.text();
  if(!response.ok) throw new Error(`GFW HTTP ${response.status}: ${body.slice(0,500)}`);
  const json=JSON.parse(body);const rows=Array.isArray(json.data)?json.data:(Array.isArray(json.data?.rows)?json.data.rows:[]);
  console.log(JSON.stringify({ok:true,cutoff,rows},null,2));
})().catch(e=>{console.error(e.message);process.exit(1)});
