//! Parsing and writing single objects: the one input path every JSON object takes into the model.
//!
//! [`from_json`] runs, in order: number normalisation (integral doubles become integers), name normalisation (NFC,
//! trimmed; 10 §1.3), schema validation against the type's generated schema (10 §2.8), typed deserialisation,
//! canonicalisation (exposure runs, swatch value order; 10 §4.7, §4.13) and the object-local semantic rules of
//! [`crate::validate`]. A value that passes is canonical: [`to_json`] of it is the persisted JSON (10 DM-1).

use serde_json::Value;

use crate::canonical;
use crate::entities::ModelObject;
use crate::entities::element::{Element, ExposureRun};
use crate::error::{ModelError, codes};
use crate::schema;
use crate::types::ObjectType;
use crate::validate::{Check, Issues};

/// Maximum number of errors reported for one object.
pub const MAX_ERRORS: usize = 100;

/// Applies the input normalisation of 10 §1.1/§1.3 to a JSON value in place: integral doubles become integers and
/// the `name` of every embedded object is NFC-normalised and trimmed.
pub fn normalize_input(value: &mut Value) {
    canonical::normalize_numbers(value);
    normalize_names(value, 0);
}

fn normalize_names(value: &mut Value, depth: usize) {
    if depth > crate::MAX_JSON_DEPTH {
        return;
    }
    match value {
        Value::Object(m) => {
            let is_object = m.get("type").and_then(Value::as_str).and_then(ObjectType::parse).is_some();
            if is_object && let Some(Value::String(n)) = m.get_mut("name") {
                *n = crate::name::normalize(n);
            }
            for v in m.values_mut() {
                normalize_names(v, depth + 1);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|v| normalize_names(v, depth + 1)),
        _ => {}
    }
}

/// Parses and validates one object of type `T` from JSON.
///
/// # Errors
/// Every problem found, as `doc.schemaViolation` (schema and shape) or `arg.invalid` (semantic rules), each with
/// `details.pointer`; at most [`MAX_ERRORS`].
pub fn from_json<T: ModelObject + Check>(value: &Value) -> Result<T, Vec<ModelError>> {
    let mut v = value.clone();
    normalize_input(&mut v);
    let violations = schema::validate(T::TYPE, &v, MAX_ERRORS).map_err(|e| vec![e])?;
    if !violations.is_empty() {
        return Err(violations.iter().map(schema::Violation::to_error).collect());
    }
    let parsed: T = serde_json::from_value(v).map_err(|e| {
        vec![ModelError::with_details(codes::SCHEMA_VIOLATION, e.to_string(), serde_json::json!({"pointer": ""}))]
    })?;
    let parsed = parsed.canonicalized();
    let mut issues = Issues::new(MAX_ERRORS);
    parsed.check("", &mut issues);
    if issues.is_empty() { Ok(parsed) } else { Err(issues.into_errors()) }
}

/// The persisted JSON of an object (its own entry form: children stored in other entries appear as ID-only stubs;
/// the document serialiser adds names and expands them).
///
/// # Errors
/// `internal` if serialisation fails (non-finite numbers cannot be constructed through [`from_json`]).
pub fn to_json<T: ModelObject>(object: &T) -> Result<Value, ModelError> {
    serde_json::to_value(object).map_err(|e| ModelError::new(codes::INTERNAL, format!("serialisation failed: {e}")))
}

/// The canonical text of an object (10 §6.5).
///
/// # Errors
/// As [`to_json`] and [`canonical::to_canonical_string`].
pub fn to_canonical<T: ModelObject>(object: &T) -> Result<String, ModelError> {
    canonical::to_canonical_string(&to_json(object)?)
}

/// Canonical exposure runs (10 §4.7): adjacent runs with equal values merged, a trailing blank run omitted, zero
/// counts dropped.
#[must_use]
pub fn canonical_runs(runs: &[ExposureRun]) -> Vec<ExposureRun> {
    let mut out: Vec<ExposureRun> = Vec::with_capacity(runs.len());
    for (drawing, count) in runs {
        if *count == 0 {
            continue;
        }
        match out.last_mut() {
            Some((d, c)) if d == drawing => *c = c.saturating_add(*count),
            _ => out.push((drawing.clone(), *count)),
        }
    }
    while matches!(out.last(), Some((None, _))) {
        out.pop();
    }
    out
}

/// Canonical form of a value: the normalisation the model applies on input.
pub trait Canonicalize: Sized {
    /// Returns the canonical form.
    #[must_use]
    fn canonicalized(self) -> Self {
        self
    }
}

impl Canonicalize for Element {
    fn canonicalized(mut self) -> Self {
        if let Some(runs) = self.exposure.take() {
            self.exposure = Some(canonical_runs(&runs));
        }
        self
    }
}

impl Canonicalize for crate::entities::palette::Palette {
    fn canonicalized(mut self) -> Self {
        // Swatch values in style order (10 §4.13; written in a deterministic order).
        let order: Vec<_> = self.styles.iter().map(|s| s.id.clone()).collect();
        for sw in &mut self.swatches {
            sw.values.sort_by_key(|k, _| order.iter().position(|o| o == k).unwrap_or(usize::MAX));
        }
        self
    }
}

macro_rules! plain_canonical {
    ($($t:ty),+) => { $( impl Canonicalize for $t {} )+ };
}

plain_canonical!(
    crate::entities::project::Project,
    crate::entities::palette::PaletteStyle,
    crate::entities::palette::Swatch,
    crate::entities::pen::Pen,
    crate::entities::asset::Asset,
    crate::entities::drawing::DrawingBank,
    crate::entities::drawing::Drawing,
    crate::entities::drawing::DrawingNode,
    crate::entities::drawing::Stroke,
    crate::entities::scene::Scene,
    crate::entities::transition::Transition,
    crate::entities::channel::Channel,
    crate::entities::channel::Key,
    crate::entities::channel::MotionPoint,
    crate::entities::effect::ParticleBlock,
    crate::entities::sound::SoundPlacement,
    crate::entities::sound::MediaSequence,
    crate::entities::library::Library,
    crate::entities::library::LibraryFolder,
    crate::entities::library::LibraryItem
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::Id;

    fn id(s: &str) -> Id {
        Id::parse(s).unwrap()
    }

    #[test]
    fn runs_are_merged_and_trailing_blanks_dropped() {
        let runs = vec![(Some(id("a")), 2), (Some(id("a")), 3), (None, 1), (None, 0), (Some(id("b")), 1), (None, 4)];
        assert_eq!(canonical_runs(&runs), vec![(Some(id("a")), 5), (None, 1), (Some(id("b")), 1)]);
        assert_eq!(canonical_runs(&[(None, 3)]), vec![]);
    }
}
