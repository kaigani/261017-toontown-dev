//! Wall-clock timestamps (ISO 8601 UTC with milliseconds, 10 §1.1) for manifests, locks, backups and recovery data.

use std::time::{SystemTime, UNIX_EPOCH};

/// Milliseconds since the Unix epoch now.
#[must_use]
pub fn now_ms() -> i64 {
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    i64::try_from(ms).unwrap_or(i64::MAX)
}

/// Now as `YYYY-MM-DDTHH:MM:SS.mmmZ`.
#[must_use]
pub fn now_iso() -> String {
    iso_from_ms(now_ms())
}

/// Formats milliseconds since the Unix epoch (civil-from-days, proleptic Gregorian).
#[must_use]
pub fn iso_from_ms(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let milli = ms.rem_euclid(1000);
    let days = secs.div_euclid(86_400);
    let sod = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{milli:03}Z", sod / 3600, (sod % 3600) / 60, sod % 60)
}

/// Parses `YYYY-MM-DDTHH:MM:SS(.mmm)?Z` into milliseconds since the Unix epoch; `None` for anything else.
#[must_use]
pub fn ms_from_iso(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> { s.get(r)?.parse::<i64>().ok() };
    let (y, mo, d, h, mi, se) = (num(0..4)?, num(5..7)?, num(8..10)?, num(11..13)?, num(14..16)?, num(17..19)?);
    let rest = s.get(19..)?;
    let milli = match rest {
        "Z" => 0,
        r if r.len() == 5 && r.starts_with('.') && r.ends_with('Z') => r.get(1..4)?.parse::<i64>().ok()?,
        _ => return None,
    };
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 59 {
        return None;
    }
    // days-from-civil
    let y = if mo <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if mo > 2 { mo - 3 } else { mo + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 86_400 + h * 3600 + mi * 60 + se) * 1000) + milli)
}

/// A file-name-safe form of a timestamp (`2026-10-17T09-30-12.345Z`): `:` is not allowed in Windows file names.
#[must_use]
pub fn file_stamp(iso: &str) -> String {
    iso.replace(':', "-")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test")]
    use super::*;

    #[test]
    fn round_trips() {
        for ms in [0, 951_782_400_000, 1_792_310_412_345, -1] {
            assert_eq!(ms_from_iso(&iso_from_ms(ms)), Some(ms));
        }
        assert_eq!(ms_from_iso("2026-10-17T09:30:12Z"), Some(ms_from_iso("2026-10-17T09:30:12.000Z").unwrap()));
        for bad in ["", "2026-10-17", "2026-13-01T00:00:00.000Z", "2026-10-17T09:30:12.34Z", "x026-10-17T09:30:12Z"] {
            assert_eq!(ms_from_iso(bad), None, "{bad}");
        }
        assert_eq!(file_stamp("2026-10-17T09:30:12.345Z"), "2026-10-17T09-30-12.345Z");
    }
}
