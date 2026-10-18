//! Identifiers (10 §2.3, ID-01…ID-07; design decision D1, ADR-0010).
//!
//! An [`Id`] is an opaque string of 1–64 characters from `[A-Za-z0-9_-]` (ID-02). Generated IDs are a type prefix,
//! an underscore and a 26-character ULID (48-bit millisecond time + 80 random bits, Crockford base 32), e.g.
//! `el_01JA8Q6V8Z3K7M2N5P4R9S0T1W`. Clients MUST NOT parse IDs; this crate parses them only to validate the
//! character set and to split the compound form `<containerId>/<localId>` of ID-03.

use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{ModelError, codes};
use crate::types::ObjectType;

/// Maximum length of an ID (ID-02).
pub const MAX_ID_LEN: usize = 64;

/// The regular expression an ID matches (ID-02), as used in the JSON Schemas.
pub const ID_PATTERN: &str = "^[A-Za-z0-9_-]{1,64}$";

/// Whether `s` is a valid ID (ID-02).
#[must_use]
pub fn is_valid_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= MAX_ID_LEN && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// A validated, immutable, cheaply clonable object identifier.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(Arc<str>);

impl Id {
    /// Validates and wraps an ID.
    ///
    /// # Errors
    /// `arg.invalid` when `s` is not 1–64 characters of `[A-Za-z0-9_-]`.
    pub fn parse(s: &str) -> Result<Self, ModelError> {
        if is_valid_id(s) {
            Ok(Self(Arc::from(s)))
        } else {
            Err(ModelError::with_details(
                codes::ARG_INVALID,
                format!("`{}` is not a valid id (1–64 characters of A–Z, a–z, 0–9, `_`, `-`)", truncate(s)),
                serde_json::json!({ "allowed": ID_PATTERN }),
            ))
        }
    }

    /// The ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn truncate(s: &str) -> Cow<'_, str> {
    if s.chars().count() > 80 {
        Cow::Owned(s.chars().take(80).chain("…".chars()).collect())
    } else {
        Cow::Borrowed(s)
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id({})", self.0)
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::borrow::Borrow<str> for Id {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Id {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Serialize for Id {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = Cow::<str>::deserialize(d)?;
        Id::parse(&s).map_err(|e| serde::de::Error::custom(e.message()))
    }
}

impl JsonSchema for Id {
    fn schema_name() -> Cow<'static, str> {
        "Id".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({ "type": "string", "pattern": ID_PATTERN })
    }
}

/// An ID as accepted by every interface that takes an `id`: a plain ID or the compound form
/// `<containerId>/<localId>` of objects with container-local IDs (ID-03).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdRef {
    container: Option<Id>,
    local: Id,
}

impl IdRef {
    /// Parses `id` or `containerId/localId`.
    ///
    /// # Errors
    /// `arg.invalid` when either part is not a valid ID or there is more than one `/`.
    pub fn parse(s: &str) -> Result<Self, ModelError> {
        match s.split_once('/') {
            None => Ok(Self { container: None, local: Id::parse(s)? }),
            Some((c, l)) if !l.contains('/') => Ok(Self { container: Some(Id::parse(c)?), local: Id::parse(l)? }),
            Some(_) => Err(ModelError::new(codes::ARG_INVALID, format!("`{}` has more than one `/`", truncate(s)))),
        }
    }

    /// A plain (non-compound) reference.
    #[must_use]
    pub fn plain(id: Id) -> Self {
        Self { container: None, local: id }
    }

    /// A compound reference.
    #[must_use]
    pub fn compound(container: Id, local: Id) -> Self {
        Self { container: Some(container), local }
    }

    /// The container ID of a compound reference.
    #[must_use]
    pub fn container(&self) -> Option<&Id> {
        self.container.as_ref()
    }

    /// The (local or plain) ID.
    #[must_use]
    pub fn local(&self) -> &Id {
        &self.local
    }
}

impl fmt::Display for IdRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.container {
            Some(c) => write!(f, "{c}/{}", self.local),
            None => write!(f, "{}", self.local),
        }
    }
}

const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Encodes a 128-bit ULID (48-bit time, 80-bit randomness) as 26 Crockford base-32 characters.
#[must_use]
pub fn encode_ulid(time_ms: u64, random: u128) -> String {
    let value: u128 = (u128::from(time_ms & 0xFFFF_FFFF_FFFF) << 80) | (random & ((1u128 << 80) - 1));
    let mut out = [0u8; 26];
    for (i, slot) in out.iter_mut().enumerate() {
        let shift = 125 - 5 * i as u32;
        let idx = ((value >> shift) & 0x1F) as usize;
        *slot = CROCKFORD.get(idx).copied().unwrap_or(b'0');
    }
    out.iter().map(|&b| char::from(b)).collect()
}

#[derive(Debug, Clone)]
enum Source {
    /// Operating-system randomness and the wall clock.
    System,
    /// Deterministic: a SplitMix64 stream and a logical clock (1 ms per ID). For replay and tests (ADR-0010).
    Seeded { state: u64, clock_ms: u64 },
}

/// Generates ULID-style IDs (ID-02). Within one generator IDs are strictly increasing (monotonic ULIDs): when two IDs
/// fall in the same millisecond the random part of the later one is the earlier one's plus one.
#[derive(Debug, Clone)]
pub struct IdGenerator {
    source: Source,
    last_ms: u64,
    last_random: u128,
}

impl IdGenerator {
    /// A generator using operating-system randomness and the system clock (the product default).
    #[must_use]
    pub fn system() -> Self {
        Self { source: Source::System, last_ms: 0, last_random: 0 }
    }

    /// A deterministic generator: the same seed and start time always produce the same IDs. IDs from seeded
    /// generators are unique within a project but not across projects; the product uses [`IdGenerator::system`].
    #[must_use]
    pub fn seeded(seed: u64, start_ms: u64) -> Self {
        Self { source: Source::Seeded { state: seed, clock_ms: start_ms }, last_ms: 0, last_random: 0 }
    }

    /// The next ID for an object of type `ty`: `<prefix>_<ULID>`.
    ///
    /// # Errors
    /// `internal` when the operating system's random source fails.
    pub fn next(&mut self, ty: ObjectType) -> Result<Id, ModelError> {
        let (mut ms, random) = match &mut self.source {
            Source::System => {
                let mut buf = [0u8; 10];
                getrandom::fill(&mut buf)
                    .map_err(|e| ModelError::new(codes::INTERNAL, format!("random source failed: {e}")))?;
                let r = buf.iter().fold(0u128, |acc, &b| (acc << 8) | u128::from(b));
                let ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
                (u64::try_from(ms).unwrap_or(u64::MAX), r)
            }
            Source::Seeded { state, clock_ms } => {
                let hi = splitmix64(state);
                let lo = splitmix64(state);
                let ms = *clock_ms;
                *clock_ms = clock_ms.saturating_add(1);
                (ms, (u128::from(hi) << 64 | u128::from(lo)) & ((1u128 << 80) - 1))
            }
        };
        let mut random = random;
        if ms <= self.last_ms {
            // Same (or an earlier) millisecond: keep the previous time and increment the random part.
            ms = self.last_ms;
            random = self.last_random.wrapping_add(1) & ((1u128 << 80) - 1);
            if random == 0 {
                ms = ms.saturating_add(1);
            }
        }
        self.last_ms = ms;
        self.last_random = random;
        Id::parse(&format!("{}_{}", ty.id_prefix(), encode_ulid(ms, random)))
    }
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_character_set_and_length() {
        assert!(Id::parse("el_01JA8Q6V8Z3K7M2N5P4R9S0T1W").is_ok());
        assert!(Id::parse("s42").is_ok());
        assert!(Id::parse(&"a".repeat(64)).is_ok());
        for bad in ["", "a b", "a/b", "é", &"a".repeat(65), "a.b", "a\u{0}"] {
            assert_eq!(Id::parse(bad).unwrap_err().code(), "arg.invalid", "{bad:?}");
        }
    }

    #[test]
    fn compound_form() {
        let r = IdRef::parse("dr_01JA/s42").unwrap();
        assert_eq!(r.container().unwrap().as_str(), "dr_01JA");
        assert_eq!(r.local().as_str(), "s42");
        assert_eq!(r.to_string(), "dr_01JA/s42");
        assert!(IdRef::parse("a/b/c").is_err());
        assert!(IdRef::parse("/b").is_err());
    }

    #[test]
    fn ulid_encoding_is_26_crockford_chars_and_ordered() {
        let a = encode_ulid(1, 0);
        let b = encode_ulid(1, 1);
        let c = encode_ulid(2, 0);
        assert_eq!(a.len(), 26);
        assert!(a < b && b < c);
        assert_eq!(encode_ulid(0, 0), "00000000000000000000000000");
        assert_eq!(encode_ulid(0xFFFF_FFFF_FFFF, u128::MAX), "7ZZZZZZZZZZZZZZZZZZZZZZZZZ");
    }

    #[test]
    fn generated_ids_are_valid_prefixed_and_increasing() {
        let mut g = IdGenerator::system();
        let mut prev = String::new();
        for _ in 0..1000 {
            let id = g.next(ObjectType::Element).unwrap();
            let s = id.as_str();
            assert!(s.starts_with("el_") && s.len() == 29, "{s}");
            assert!(s > prev.as_str(), "{s} after {prev}");
            prev = s.to_owned();
        }
    }

    #[test]
    fn seeded_generators_are_reproducible() {
        let ids = |seed| {
            let mut g = IdGenerator::seeded(seed, 1_700_000_000_000);
            (0..5).map(|_| g.next(ObjectType::Scene).unwrap()).collect::<Vec<_>>()
        };
        assert_eq!(ids(7), ids(7));
        assert_ne!(ids(7), ids(8));
    }
}
