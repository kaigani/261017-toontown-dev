//! Value types of 10 §1.1: colours, points, matrices, timestamps, blobs, frame ranges.

use std::borrow::Cow;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{ModelError, codes};

/// An sRGB colour with straight alpha, stored as `#RRGGBBAA` upper case (10 §1.1). `#RRGGBB` is accepted on input
/// and stored with `FF` alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Colour([u8; 4]);

impl Colour {
    /// Opaque black.
    pub const BLACK: Colour = Colour([0, 0, 0, 255]);
    /// Opaque white.
    pub const WHITE: Colour = Colour([255, 255, 255, 255]);

    /// A colour from its four channels.
    #[must_use]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self([r, g, b, a])
    }

    /// The four channels `[r, g, b, a]`.
    #[must_use]
    pub const fn channels(self) -> [u8; 4] {
        self.0
    }

    /// Parses `#RRGGBB` or `#RRGGBBAA` (either case).
    ///
    /// # Errors
    /// `arg.invalid` for any other string.
    pub fn parse(s: &str) -> Result<Self, ModelError> {
        let hex = s.strip_prefix('#').filter(|h| (h.len() == 6 || h.len() == 8) && h.is_ascii());
        let bad = || ModelError::new(codes::ARG_INVALID, format!("`{s}` is not a colour (#RRGGBB or #RRGGBBAA)"));
        let hex = hex.ok_or_else(bad)?;
        let byte = |i: usize| hex.get(i..i + 2).and_then(|p| u8::from_str_radix(p, 16).ok()).ok_or_else(bad);
        let a = if hex.len() == 8 { byte(6)? } else { 255 };
        Ok(Self([byte(0)?, byte(2)?, byte(4)?, a]))
    }
}

/// The JSON Schema pattern of a colour string.
pub const COLOUR_PATTERN: &str = "^#([0-9A-Fa-f]{6}|[0-9A-Fa-f]{8})$";

impl fmt::Display for Colour {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [r, g, b, a] = self.0;
        write!(f, "#{r:02X}{g:02X}{b:02X}{a:02X}")
    }
}

impl Serialize for Colour {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Colour {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = Cow::<str>::deserialize(d)?;
        Colour::parse(&s).map_err(|e| serde::de::Error::custom(e.message()))
    }
}

impl JsonSchema for Colour {
    fn schema_name() -> Cow<'static, str> {
        "Colour".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "pattern": COLOUR_PATTERN,
            "description": "sRGB colour with straight alpha, #RRGGBBAA (written upper case; #RRGGBB accepted on input)."
        })
    }
}

/// A 2-D point `{x, y}` (10 §1.1).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Point2 {
    /// X coordinate.
    pub x: f64,
    /// Y coordinate.
    pub y: f64,
}

/// A 3-D point `{x, y, z}` (10 §1.1).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Point3 {
    /// X coordinate.
    pub x: f64,
    /// Y coordinate.
    pub y: f64,
    /// Z coordinate.
    pub z: f64,
}

/// A 2×3 affine matrix `[a, b, c, d, tx, ty]`: (x, y) ↦ (a·x + c·y + tx, b·x + d·y + ty) (10 §1.1).
pub type Matrix2d = [f64; 6];

/// The identity [`Matrix2d`].
pub const IDENTITY: Matrix2d = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// An inclusive frame range `{from, to}` (10 §1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FrameRange {
    /// First frame (1-based).
    #[schemars(range(min = 1))]
    pub from: u32,
    /// Last frame (inclusive).
    #[schemars(range(min = 1))]
    pub to: u32,
}

/// An ISO-8601 UTC timestamp with milliseconds, e.g. `2026-10-17T09:30:12.345Z` (10 §1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Timestamp(i64);

/// The JSON Schema pattern of a timestamp string.
pub const TIMESTAMP_PATTERN: &str = r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]{3}Z$";

impl Timestamp {
    /// Milliseconds since the Unix epoch.
    #[must_use]
    pub const fn from_unix_ms(ms: i64) -> Self {
        Self(ms)
    }

    /// The current time.
    #[must_use]
    pub fn now() -> Self {
        let ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        Self(i64::try_from(ms).unwrap_or(i64::MAX))
    }

    /// Milliseconds since the Unix epoch.
    #[must_use]
    pub const fn unix_ms(self) -> i64 {
        self.0
    }

    /// Parses `YYYY-MM-DDThh:mm:ss.sssZ` (exactly this form, years 0000–9999, a real calendar date).
    ///
    /// # Errors
    /// `arg.invalid` for any other string.
    pub fn parse(s: &str) -> Result<Self, ModelError> {
        let bad =
            || ModelError::new(codes::ARG_INVALID, format!("`{s}` is not a timestamp (YYYY-MM-DDThh:mm:ss.sssZ)"));
        let b = s.as_bytes();
        if b.len() != 24 {
            return Err(bad());
        }
        let shape_ok = b.iter().enumerate().all(|(i, &c)| match i {
            4 | 7 => c == b'-',
            10 => c == b'T',
            13 | 16 => c == b':',
            19 => c == b'.',
            23 => c == b'Z',
            _ => c.is_ascii_digit(),
        });
        if !shape_ok {
            return Err(bad());
        }
        let num = |r: std::ops::Range<usize>| s.get(r).and_then(|p| p.parse::<i64>().ok()).ok_or_else(bad);
        let (y, mo, d, h, mi, se, ms) =
            (num(0..4)?, num(5..7)?, num(8..10)?, num(11..13)?, num(14..16)?, num(17..19)?, num(20..23)?);
        if !(1..=12).contains(&mo) || d < 1 || d > days_in_month(y, mo) || h > 23 || mi > 59 || se > 59 {
            return Err(bad());
        }
        let days = days_from_civil(y, mo, d);
        Ok(Self(((days * 24 + h) * 60 + mi) * 60_000 + se * 1000 + ms))
    }
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if is_leap(y) => 29,
        _ => 28,
    }
}

/// Days since 1970-01-01 of a proleptic Gregorian date (H. Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ms_total = self.0;
        let days = ms_total.div_euclid(86_400_000);
        let rem = ms_total.rem_euclid(86_400_000);
        let (y, m, d) = civil_from_days(days);
        let (h, mi, s, ms) = (rem / 3_600_000, rem / 60_000 % 60, rem / 1000 % 60, rem % 1000);
        write!(f, "{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{ms:03}Z")
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = Cow::<str>::deserialize(d)?;
        Timestamp::parse(&s).map_err(|e| serde::de::Error::custom(e.message()))
    }
}

impl JsonSchema for Timestamp {
    fn schema_name() -> Cow<'static, str> {
        "Timestamp".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "pattern": TIMESTAMP_PATTERN,
            "description": "ISO-8601 UTC timestamp with milliseconds."
        })
    }
}

/// A binary payload stored as a package entry (10 §1.1). Never inlined in object JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Blob {
    /// Package entry path.
    #[schemars(length(min = 1, max = 1024))]
    pub entry: String,
    /// IANA media type.
    #[schemars(length(min = 1, max = 255))]
    pub media_type: String,
    /// Size in bytes (64-bit: entries larger than 4 GiB are supported, 10 §5).
    pub size: u64,
    /// Lower-case hex SHA-256 of the bytes.
    #[schemars(regex(pattern = "^[0-9a-f]{64}$"))]
    pub sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_normalise_to_upper_rgba() {
        assert_eq!(Colour::parse("#a1b2c3").unwrap().to_string(), "#A1B2C3FF");
        assert_eq!(Colour::parse("#A1B2C380").unwrap().to_string(), "#A1B2C380");
        for bad in ["", "#", "#12345", "#1234567", "a1b2c3", "#GGGGGG", "#ＡＡＡＡＡＡ"] {
            assert!(Colour::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn timestamps_round_trip_and_reject_impossible_dates() {
        for s in [
            "2026-10-17T09:30:12.345Z",
            "1970-01-01T00:00:00.000Z",
            "2000-02-29T23:59:59.999Z",
            "1969-12-31T23:59:59.999Z",
        ] {
            assert_eq!(Timestamp::parse(s).unwrap().to_string(), s);
        }
        assert_eq!(Timestamp::parse("1970-01-01T00:00:01.000Z").unwrap().unix_ms(), 1000);
        for bad in [
            "2026-02-29T00:00:00.000Z",
            "2026-13-01T00:00:00.000Z",
            "2026-10-17T24:00:00.000Z",
            "2026-10-17T09:30:12Z",
            "2026-10-17 09:30:12.345Z",
            "2026-10-17T09:30:12.345+00:00",
        ] {
            assert!(Timestamp::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn timestamp_display_matches_civil_conversion_for_many_days() {
        // Years 0001–9999, the range the four-digit form can express.
        for days in (-719_162i64..2_932_896).step_by(997) {
            let t = Timestamp::from_unix_ms(days * 86_400_000 + 12_345);
            assert_eq!(Timestamp::parse(&t.to_string()).unwrap(), t);
        }
    }
}
