# data/

This directory intentionally contains no third-party datasets, forest
boundaries, or protected-area polygons. GreenProof queries real public data
sources live (Nominatim, Overpass/OpenStreetMap; see `backend/src/geo.rs`
and the main README's "Real data sources" section) rather than shipping a
static snapshot that could go stale or be mistaken for authoritative data.

`example-request.json` is a synthetic, clearly-labelled example input for
`scripts/prove.js` showing the expected shape - it is a test fixture, not
sample "real" supplier data, and is not used by the running application.
