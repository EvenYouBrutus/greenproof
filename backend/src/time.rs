//! GreenProof - minimal UTC timestamp formatting.
//!
//! The evidence provenance and verification certificate both need a real,
//! human-readable retrieval/creation timestamp (see README "Verification
//! IDs" and the hackathon brief's certificate requirements) - a raw
//! "unix:<secs>" string is not a credible timestamp to show a judge or
//! auditor. Rather than pull in a new dependency (chrono) whose build we
//! cannot verify in this environment, this module implements the
//! well-known, widely-used civil-calendar-from-days-since-epoch algorithm
//! (Howard Hinnant's `civil_from_days`,
//! http://howardhinnant.github.io/date_algorithms.html) using only the
//! standard library. It is UTC-only, which is exactly what a server
//! timestamp should be.

use std::time::{SystemTime, UNIX_EPOCH};

/// Returns the current time as an RFC3339-ish UTC timestamp, e.g.
/// "2026-08-07T19:42:11Z".
pub fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format_unix_secs(secs as i64)
}

/// Formats a Unix timestamp (seconds since epoch, UTC) as
/// "YYYY-MM-DDTHH:MM:SSZ".
pub fn format_unix_secs(total_secs: i64) -> String {
    let days = total_secs.div_euclid(86_400);
    let secs_of_day = total_secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let hh = secs_of_day / 3600;
    let mm = (secs_of_day % 3600) / 60;
    let ss = secs_of_day % 60;
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Civil calendar date (year, month, day) from a day count relative to
/// 1970-01-01 (proleptic Gregorian, UTC). Standard, well-tested algorithm -
/// see module docs for the reference.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_formats_correctly() {
        assert_eq!(format_unix_secs(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn known_date_formats_correctly() {
        // 2024-01-01T00:00:00Z
        assert_eq!(format_unix_secs(1_704_067_200), "2024-01-01T00:00:00Z");
    }

    #[test]
    fn end_of_day_rolls_over() {
        // 2024-02-29T23:59:59Z (leap day)
        assert_eq!(format_unix_secs(1_709_251_199), "2024-02-29T23:59:59Z");
    }
}
