//! Parsing and writing single objects: the one input path every JSON object takes into the model.
//!
//! [`from_json`] runs, in order: the depth limit ([`crate::input::MAX_MODEL_DEPTH`]), number normalisation (integral
//! doubles become integers), the schema-directed input normalisation of [`crate::input`] (NFC strings, trimmed
//! names, the swatch `id` alias, canonical key order; opaque `meta`/`ext` values untouched), schema validation
//! against the type's generated schema (10 §2.8), typed deserialisation, canonicalisation (exposure runs, spline
//! handle clamping; 10 §4.7, §4.12) and the object-local semantic rules of [`crate::validate`]. A value that passes
//! is canonical: [`to_json`] of it is the persisted JSON (10 DM-1), whatever order its input keys had.

use serde_json::Value;

use crate::canonical;
use crate::entities::ModelObject;
use crate::entities::element::{Element, ExposureRun};
use crate::error::{ModelError, codes};
use crate::schema;
use crate::validate::{Check, Issues};

/// Maximum number of errors reported for one object.
pub const MAX_ERRORS: usize = 100;

/// Applies the input normalisation of 10 §1.1/§1.3 for an object of type `ty` to a JSON value in place (see
/// [`crate::input`]).
pub fn normalize_input(ty: crate::types::ObjectType, value: &mut Value) {
    canonical::normalize_numbers(value);
    crate::input::prepare(ty, value);
}

/// Parses and validates one object of type `T` from JSON.
///
/// # Errors
/// Every problem found, as `doc.schemaViolation` (schema and shape) or `arg.invalid` (semantic rules), each with
/// `details.pointer`; at most [`MAX_ERRORS`]. `doc.limitExceeded` for nesting deeper than
/// [`crate::input::MAX_MODEL_DEPTH`].
pub fn from_json<T: ModelObject + Check>(value: &Value) -> Result<T, Vec<ModelError>> {
    let limit = crate::input::MAX_MODEL_DEPTH;
    if crate::input::depth(value, limit) > limit {
        return Err(vec![ModelError::with_details(
            codes::LIMIT_EXCEEDED,
            format!("object nested deeper than {limit} levels"),
            serde_json::json!({ "pointer": "", "maxDepth": limit }),
        )]);
    }
    let mut v = value.clone();
    normalize_input(T::TYPE, &mut v);
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

/// Runs a typed value through the validated input path again: [`from_json`] of its [`to_json`]. Used where typed
/// values arrive without having passed [`from_json`] (public `Deserialize`, document parts; C11-R1-02).
///
/// # Errors
/// As [`to_json`] and [`from_json`].
pub fn revalidate<T: ModelObject + Check>(object: &T) -> Result<T, Vec<ModelError>> {
    from_json(&to_json(object).map_err(|e| vec![e])?)
}

/// The persisted JSON of an object (its own entry form: children stored in other entries appear as ID-only stubs;
/// the document serialiser adds names and expands them).
///
/// # Errors
/// As [`canonical::to_value`]: a non-finite number is an error, never `null` (10 §1.1).
pub fn to_json<T: ModelObject>(object: &T) -> Result<Value, ModelError> {
    canonical::to_value(object)
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
        for ch in self.channels.iter_mut().flat_map(|m| m.values_mut()) {
            if ch.curve_type == Some(crate::entities::channel::CurveType::Spline)
                && let Some(keys) = ch.keys.as_mut()
            {
                crate::entities::channel::clamp_handles(keys);
            }
        }
        self
    }
}

impl Canonicalize for crate::entities::channel::Channel {
    fn canonicalized(mut self) -> Self {
        if self.curve_type == Some(crate::entities::channel::CurveType::Spline)
            && let Some(keys) = self.keys.as_mut()
        {
            crate::entities::channel::clamp_handles(keys);
        }
        self
    }
}

macro_rules! plain_canonical {
    ($($t:ty),+) => { $( impl Canonicalize for $t {} )+ };
}

plain_canonical!(
    crate::entities::project::Project,
    crate::entities::palette::Palette,
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
