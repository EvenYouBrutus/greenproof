// GreenProof backend tests.
//
// These tests cover input validation without requiring network access.
// The public geo::check_location entrypoint now also needs the Hansen cache
// directory and cutoff year because environmental verification is part of the
// request. Validation happens before any network call, so invalid coordinates
// still return immediately.

#[path = "../src/geo.rs"]
mod geo;
#[path = "../src/hansen.rs"]
mod hansen;
#[path = "../src/time.rs"]
mod time;

#[tokio::test]
async fn invalid_latitude_is_rejected_without_network_call() {
    let client = reqwest::Client::new();
    let err = geo::check_location(&client, "../data/hansen", 120.0, 0.0, 2020)
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
    let err = geo::check_location(&client, "../data/hansen", 0.0, 220.0, 2020)
        .await
        .unwrap_err();

    match err {
        geo::GeoError::InvalidLongitude(v) => assert_eq!(v, 220.0),
        other => panic!("expected InvalidLongitude, got {other:?}"),
    }
}

#[tokio::test]
async fn test_real_coordinate_overpass() {
    let client = reqwest::Client::new();
    let result = geo::overpass_query(&client, 6.6666, -1.6163).await;
    assert!(result.is_ok(), "overpass_query failed: {:?}", result.err());
}
