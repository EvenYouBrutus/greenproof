// GreenProof backend tests.
//
// These tests cover input validation without requiring network access.
// A full integration test (real coordinate -> real Nominatim/Overpass call
// -> evidence -> proof -> verify) requires live network access and is
// documented as a manual/CI-only step in README "How to test", since this
// project's principle is "never substitute fake data for a real, reachable
// external source" - so we do not mock the external HTTP calls in unit
// tests either; we test the validation logic that runs before any network
// call, and rely on scripts/test/circuit.test.js + a live-network CI job
// for the full pipeline.

// geo::validate is private to the crate; these tests exercise it via the
// public check_location entrypoint's synchronous validation short-circuit,
// which returns before any network I/O for out-of-range inputs.

#[path = "../src/time.rs"]
mod time;
#[path = "../src/geo.rs"]
mod geo;

#[tokio::test]
async fn invalid_latitude_is_rejected_without_network_call() {
    let client = reqwest::Client::new();
    let err = geo::check_location(&client, 120.0, 0.0, 1000)
        .await
        .unwrap_err();
    match err {
        geo::GeoError::InvalidLatitude(v) => assert_eq!(v, 120.0),
        other => panic!("expected InvalidLatitude, got {other:?}"),
    }
}

#[tokio::test]
async fn invalid_longitude_is_rejected_without_network_call() {
    let client = reqwest::Client::new();
    let err = geo::check_location(&client, 0.0, 220.0, 1000)
        .await
        .unwrap_err();
    match err {
        geo::GeoError::InvalidLongitude(v) => assert_eq!(v, 220.0),
        other => panic!("expected InvalidLongitude, got {other:?}"),
    }
}
