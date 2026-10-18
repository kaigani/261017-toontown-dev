//! Object-local semantic rules (10 §4, §5): the constraints a JSON Schema cannot express (cross-field ranges,
//! uniqueness inside an object, presence by kind, canonical forms). Document-level rules (references, uniqueness
//! across objects, the scene tree) are in [`crate::document`].
//!
//! Every rule reports `arg.invalid` with `details.pointer` (JSON Pointer into the object) and `details.rule`.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde_json::{Value, json};

use crate::entities::channel::{Channel, CurveType, Key, MotionPoint, POSITION};
use crate::entities::drawing::{Drawing, DrawingBank, DrawingNode, NodeKind, Stroke, StrokeKind};
use crate::entities::effect::{self, ParticleBlock};
use crate::entities::element::{Element, ElementType, PivotSet, SPECIFIC_PROPERTIES};
use crate::entities::library::{Library, LibraryFolder, LibraryItem, LibraryNode};
use crate::entities::palette::{Palette, PaletteStyle, Swatch, SwatchValue};
use crate::entities::pen::Pen;
use crate::entities::project::Project;
use crate::entities::scene::{Scene, TreeNode};
use crate::entities::sound::{MediaSequence, SoundPlacement};
use crate::entities::transition::Transition;
use crate::entities::{ModelObject, asset::Asset};
use crate::error::{ModelError, codes};
use crate::name::{NameRule, check as check_name};
use crate::object::Canonicalize;
use crate::types::ObjectType;

/// A bounded list of problems.
#[derive(Debug, Clone)]
pub struct Issues {
    errors: Vec<ModelError>,
    limit: usize,
}

impl Issues {
    /// An empty list holding at most `limit` problems.
    #[must_use]
    pub fn new(limit: usize) -> Self {
        Self { errors: Vec::new(), limit }
    }

    /// Records a problem at a pointer.
    pub fn push(&mut self, pointer: &str, rule: &str, message: impl Into<String>) {
        if self.errors.len() < self.limit {
            self.errors.push(ModelError::with_details(
                codes::ARG_INVALID,
                message,
                json!({ "pointer": pointer, "rule": rule }),
            ));
        }
    }

    /// Records an error as-is.
    pub fn push_error(&mut self, e: ModelError) {
        if self.errors.len() < self.limit {
            self.errors.push(e);
        }
    }

    /// Whether nothing was recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// The recorded problems.
    #[must_use]
    pub fn into_errors(self) -> Vec<ModelError> {
        self.errors
    }

    /// Whether the limit was reached.
    #[must_use]
    pub fn full(&self) -> bool {
        self.errors.len() >= self.limit
    }
}

/// Object-local semantic validation.
pub trait Check: Canonicalize {
    /// Records every violated rule, with pointers prefixed by `at`.
    fn check(&self, at: &str, out: &mut Issues);
}

fn ptr(at: &str, field: &str) -> String {
    format!("{at}/{}", crate::pointer::escape(field))
}

fn finite(v: f64, at: &str, out: &mut Issues) {
    if !v.is_finite() {
        out.push(at, "finite", "NaN and infinities are invalid");
    }
}

fn name_rule<T: ModelObject>(name: &str, at: &str, out: &mut Issues) {
    if let Err(e) = check_name(&NameRule::for_type(T::TYPE), name) {
        out.push(&ptr(at, "name"), "name", e.message().to_owned());
    } else if crate::name::normalize(name) != name {
        out.push(&ptr(at, "name"), "name", "name must be NFC-normalised and trimmed");
    }
}

fn unique<'a, I: IntoIterator<Item = (usize, &'a str)>>(
    items: I,
    at: &str,
    what: &str,
    case_insensitive: bool,
    out: &mut Issues,
) {
    let mut seen = HashSet::new();
    for (i, s) in items {
        let key = if case_insensitive { s.to_lowercase() } else { s.to_owned() };
        if !seen.insert(key) {
            out.push(&format!("{at}/{i}"), "unique", format!("duplicate {what} `{s}`"));
        }
    }
}

fn meta_ext(at: &str, ext: &crate::entities::Ext, out: &mut Issues) {
    for k in ext.as_map().keys() {
        if !crate::entities::is_ext_key(k) {
            out.push(&ptr(&ptr(at, "ext"), k), "ext", format!("`{k}` is not a reverse-DNS namespace"));
        }
    }
}

impl Check for Pen {
    fn check(&self, at: &str, out: &mut Issues) {
        name_rule::<Pen>(&self.name, at, out);
        finite(self.min_size, &ptr(at, "minSize"), out);
        finite(self.max_size, &ptr(at, "maxSize"), out);
        if self.min_size > self.max_size {
            out.push(&ptr(at, "minSize"), "minNotAboveMax", "minSize must not exceed maxSize");
        }
        if self.smoothness > 10 {
            out.push(&ptr(at, "smoothness"), "range", "smoothness is 0–10");
        }
        meta_ext(at, &self.ext, out);
    }
}

impl Check for PaletteStyle {
    fn check(&self, at: &str, out: &mut Issues) {
        name_rule::<PaletteStyle>(&self.name, at, out);
        meta_ext(at, &self.ext, out);
    }
}

fn check_swatch_values(sw: &Swatch, at: &str, out: &mut Issues) {
    for (style, value) in &sw.values {
        let vat = ptr(&ptr(at, "values"), style.as_str());
        if !Swatch::value_matches_kind(sw.kind, value) {
            out.push(&vat, "valueKind", format!("value does not match swatch kind {:?}", sw.kind));
        }
        if let SwatchValue::Gradient(g) = value {
            if g.stops.len() < 2 {
                out.push(&ptr(&vat, "stops"), "stops", "a gradient needs at least two stops");
            }
            if g.stops.windows(2).any(|w| w.first().map(|a| a.position) > w.get(1).map(|b| b.position)) {
                out.push(&ptr(&vat, "stops"), "stopsSorted", "gradient stops must be sorted by position");
            }
        }
    }
}

impl Check for Swatch {
    fn check(&self, at: &str, out: &mut Issues) {
        name_rule::<Swatch>(&self.name, at, out);
        check_swatch_values(self, at, out);
        meta_ext(at, &self.ext, out);
    }
}

impl Check for Palette {
    fn check(&self, at: &str, out: &mut Issues) {
        name_rule::<Palette>(&self.name, at, out);
        if self.styles.is_empty() {
            out.push(&ptr(at, "styles"), "minStyles", "a palette has at least one style");
        }
        let sat = ptr(at, "styles");
        unique(self.styles.iter().enumerate().map(|(i, s)| (i, s.name.as_str())), &sat, "style name", false, out);
        unique(self.styles.iter().enumerate().map(|(i, s)| (i, s.id.as_str())), &sat, "style id", false, out);
        for (i, s) in self.styles.iter().enumerate() {
            s.check(&format!("{sat}/{i}"), out);
        }
        let style_ids: Vec<_> = self.styles.iter().map(|s| &s.id).collect();
        let wat = ptr(at, "swatches");
        unique(self.swatches.iter().enumerate().map(|(i, s)| (i, s.colour_id.as_str())), &wat, "colour id", false, out);
        for (i, sw) in self.swatches.iter().enumerate() {
            let at_i = format!("{wat}/{i}");
            sw.check(&at_i, out);
            let keys: Vec<_> = sw.values.keys().collect();
            if keys != style_ids {
                out.push(
                    &ptr(&at_i, "values"),
                    "valuePerStyle",
                    "a swatch has exactly one value per style, in style order",
                );
            }
        }
        meta_ext(at, &self.ext, out);
    }
}

impl Check for Transition {
    fn check(&self, at: &str, out: &mut Issues) {
        finite(self.angle, &ptr(at, "angle"), out);
        if !self.size.is_multiple_of(5) || !(5..=100).contains(&self.size) {
            out.push(&ptr(at, "size"), "range", "size is 5–100 in steps of 5");
        }
        meta_ext(at, &self.ext, out);
    }
}

impl Check for Asset {
    fn check(&self, at: &str, out: &mut Issues) {
        name_rule::<Asset>(&self.name, at, out);
        use crate::entities::asset::AssetStorage;
        match self.storage {
            AssetStorage::Embedded => {
                if self.entry.is_none() {
                    out.push(&ptr(at, "entry"), "storage", "embedded assets have an entry");
                }
                if self.link.is_some() {
                    out.push(&ptr(at, "link"), "storage", "embedded assets have no link");
                }
            }
            AssetStorage::Linked => {
                if self.link.is_none() {
                    out.push(&ptr(at, "link"), "storage", "linked assets have a link");
                }
                if self.entry.is_some() {
                    out.push(&ptr(at, "entry"), "storage", "linked assets have no entry");
                }
            }
        }
        if let Some(e) = &self.entry {
            check_entry_path(e, &ptr(at, "entry"), out);
        }
        meta_ext(at, &self.ext, out);
    }
}

/// Package entry names: relative, `/`-separated, no `..`, no drive letters, no leading `/` (10 ZIP-01).
pub fn check_entry_path(entry: &str, at: &str, out: &mut Issues) {
    let bad = entry.is_empty()
        || entry.starts_with('/')
        || entry.contains('\\')
        || entry.split('/').any(|seg| seg.is_empty() || seg == ".." || seg == ".")
        || entry.chars().nth(1) == Some(':')
        || entry.chars().any(char::is_control);
    if bad {
        out.push(at, "entryPath", format!("`{entry}` is not a safe relative package entry path"));
    }
}

impl Check for Stroke {
    fn check(&self, at: &str, out: &mut Issues) {
        let min = if self.closed { 1 } else { 2 };
        if self.points.len() < min {
            out.push(
                &ptr(at, "points"),
                "minPoints",
                format!("an {} stroke has at least {min} anchors", if self.closed { "closed" } else { "open" }),
            );
        }
        for (i, p) in self.points.iter().enumerate() {
            if p.iter().any(|v| !v.is_finite()) {
                out.push(&format!("{at}/points/{i}"), "finite", "NaN and infinities are invalid");
            }
        }
        match self.kind {
            StrokeKind::Pencil => {
                if self.line_style.is_none() {
                    out.push(&ptr(at, "lineStyle"), "kind", "pencil strokes have a line style");
                }
                if self.spine.is_some() || self.fill_matrix.is_some() {
                    out.push(at, "kind", "pencil strokes have no spine and no fill matrix");
                }
            }
            StrokeKind::Brush => {
                if self.line_style.is_some() {
                    out.push(&ptr(at, "lineStyle"), "kind", "brush strokes have no line style");
                }
            }
            StrokeKind::Invisible => {
                if self.line_style.is_some()
                    || self.spine.is_some()
                    || self.fill_matrix.is_some()
                    || self.colour_id.is_some()
                {
                    out.push(at, "kind", "invisible strokes have no colour, line style, spine or fill matrix");
                }
            }
        }
        if let Some(sp) = &self.spine {
            if sp.pen.min_size > sp.pen.max_size {
                out.push(&format!("{at}/spine/pen/minSize"), "minNotAboveMax", "minSize must not exceed maxSize");
            }
            let mut last = -1.0;
            for (i, [t, p]) in sp.pressure.iter().enumerate() {
                if !(0.0..=1.0).contains(t) || !(0.0..=1.0).contains(p) || *t < last {
                    out.push(
                        &format!("{at}/spine/pressure/{i}"),
                        "pressure",
                        "pressure samples are (t, p) in [0, 1], ordered by t",
                    );
                }
                last = *t;
            }
        }
        meta_ext(at, &self.ext, out);
    }
}

impl Check for DrawingNode {
    fn check(&self, at: &str, out: &mut Issues) {
        if let Some(n) = &self.name
            && let Err(e) = check_name(&NameRule::for_type(ObjectType::DrawingNode), n)
        {
            out.push(&ptr(at, "name"), "name", e.message().to_owned());
        }
        let present: Vec<&str> = [
            ("hint", self.hint.is_some()),
            ("strokes", self.strokes.is_some()),
            ("children", self.children.is_some()),
            ("transform", self.transform.is_some()),
            ("widthMode", self.width_mode.is_some()),
            ("width", self.width.is_some()),
            ("height", self.height.is_some()),
            ("text", self.text.is_some()),
            ("runs", self.runs.is_some()),
            ("paragraphs", self.paragraphs.is_some()),
            ("image", self.image.is_some()),
        ]
        .into_iter()
        .filter_map(|(k, p)| p.then_some(k))
        .collect();
        let (required, allowed): (&[&str], &[&str]) = match self.kind {
            NodeKind::Layer => (&["hint", "strokes"], &["hint", "strokes"]),
            NodeKind::Group => (&["children"], &["children"]),
            NodeKind::Text => (
                &["transform", "widthMode", "text", "runs", "paragraphs"],
                &["transform", "widthMode", "width", "text", "runs", "paragraphs"],
            ),
            NodeKind::Bitmap => {
                (&["image", "width", "height", "transform"], &["image", "width", "height", "transform"])
            }
        };
        for r in required.iter().filter(|r| !present.contains(r)) {
            out.push(&ptr(at, r), "kind", format!("a {:?} node has `{r}`", self.kind));
        }
        for p in present.iter().filter(|p| !allowed.contains(p)) {
            out.push(&ptr(at, p), "kind", format!("a {:?} node has no `{p}`", self.kind));
        }
        if self.kind == NodeKind::Bitmap {
            for (k, v) in [("width", self.width), ("height", self.height)] {
                if let Some(v) = v
                    && (v.fract() != 0.0 || !(1.0..=16384.0).contains(&v))
                {
                    out.push(&ptr(at, k), "range", format!("bitmap {k} is an integer 1–16384"));
                }
            }
        }
        if let (Some(text), Some(runs)) = (&self.text, &self.runs) {
            let len = u32::try_from(text.chars().count()).unwrap_or(u32::MAX);
            let mut pos = 0u32;
            for (i, r) in runs.iter().enumerate() {
                if r.start != pos {
                    out.push(&format!("{at}/runs/{i}"), "runsCover", "runs cover the text consecutively");
                }
                pos = r.start.saturating_add(r.length);
            }
            if pos != len && !(runs.is_empty() && len == 0) {
                out.push(&ptr(at, "runs"), "runsCover", "runs cover the whole text");
            }
        }
        if let Some(strokes) = &self.strokes {
            for (i, s) in strokes.iter().enumerate() {
                s.check(&format!("{at}/strokes/{i}"), out);
            }
        }
        if let Some(children) = &self.children {
            for (i, c) in children.iter().enumerate() {
                c.check(&format!("{at}/children/{i}"), out);
            }
        }
        meta_ext(at, &self.ext, out);
    }
}

fn collect_node_ids<'a>(nodes: &'a [DrawingNode], at: &str, out: &mut Vec<(String, &'a str)>) {
    for (i, n) in nodes.iter().enumerate() {
        let nat = format!("{at}/{i}");
        out.push((nat.clone(), n.id.as_str()));
        if let Some(strokes) = &n.strokes {
            for (j, s) in strokes.iter().enumerate() {
                out.push((format!("{nat}/strokes/{j}"), s.id.as_str()));
            }
        }
        if let Some(children) = &n.children {
            collect_node_ids(children, &format!("{nat}/children"), out);
        }
    }
}

impl Check for Drawing {
    fn check(&self, at: &str, out: &mut Issues) {
        name_rule::<Drawing>(&self.name, at, out);
        finite(self.pivot.x, &ptr(at, "pivot"), out);
        finite(self.pivot.y, &ptr(at, "pivot"), out);
        for (i, n) in self.nodes.iter().enumerate() {
            n.check(&format!("{at}/nodes/{i}"), out);
        }
        // ID-03: node and stroke IDs are unique within the drawing.
        let mut ids = Vec::new();
        collect_node_ids(&self.nodes, &ptr(at, "nodes"), &mut ids);
        let mut seen = HashSet::new();
        for (p, id) in ids {
            if !seen.insert(id) {
                out.push(&p, "unique", format!("duplicate node or stroke id `{id}` in the drawing"));
            }
        }
        let mut colour_ids = HashSet::new();
        for (i, e) in self.colour_recovery.iter().enumerate() {
            if !colour_ids.insert(&e.colour_id) {
                out.push(&format!("{at}/colourRecovery/{i}"), "unique", "one recovery entry per colour id");
            }
            if e.swatch.colour_id != e.colour_id {
                out.push(
                    &format!("{at}/colourRecovery/{i}/swatch"),
                    "recovery",
                    "the recovery swatch has the entry's colour id",
                );
            }
        }
        meta_ext(at, &self.ext, out);
    }
}

impl Check for DrawingBank {
    fn check(&self, at: &str, out: &mut Issues) {
        let dat = ptr(at, "drawings");
        unique(self.drawings.ids().iter().enumerate().map(|(i, d)| (i, d.as_str())), &dat, "drawing", false, out);
        meta_ext(at, &self.ext, out);
    }
}

impl Check for Key {
    fn check(&self, at: &str, out: &mut Issues) {
        finite(self.value, &ptr(at, "value"), out);
        for h in [self.handles.left, self.handles.right] {
            finite(h.dt, &ptr(at, "handles"), out);
            finite(h.dv, &ptr(at, "handles"), out);
        }
        meta_ext(at, &self.ext, out);
    }
}

impl Check for MotionPoint {
    fn check(&self, at: &str, out: &mut Issues) {
        for v in [self.position.x, self.position.y, self.position.z, self.tension, self.continuity, self.bias] {
            finite(v, at, out);
        }
        meta_ext(at, &self.ext, out);
    }
}

/// Whether a channel id is valid for any owner (05 §1.6 plus effect parameter ids).
#[must_use]
pub fn is_channel_id(name: &str) -> bool {
    ElementType::ALL.iter().any(|t| t.allowed_channels().contains(&name))
}

impl Check for Channel {
    fn check(&self, at: &str, out: &mut Issues) {
        if !is_channel_id(&self.name) {
            out.push(&ptr(at, "name"), "channelId", format!("`{}` is not a channel id", self.name));
        }
        if self.name == POSITION {
            if self.keys.is_some() || self.curve_type.is_some() {
                out.push(at, "kind", "the position channel has points, not keys or a curve type");
            }
            match &self.points {
                None => out.push(&ptr(at, "points"), "kind", "the position channel has points"),
                Some(points) => {
                    if points.is_empty() {
                        out.push(&ptr(at, "points"), "minPoints", "a motion path has at least one point");
                    }
                    if points.first().is_some_and(|p| p.frame.is_none())
                        || points.last().is_some_and(|p| p.frame.is_none())
                    {
                        out.push(&ptr(at, "points"), "endKeyframes", "the first and last motion points are keyframes");
                    }
                    let frames: Vec<u32> = points.iter().filter_map(|p| p.frame).collect();
                    if frames.windows(2).any(|w| w.first() >= w.get(1)) {
                        out.push(
                            &ptr(at, "points"),
                            "keyframeOrder",
                            "keyframe frames increase along the path and are unique",
                        );
                    }
                    let pat = ptr(at, "points");
                    unique(
                        points.iter().enumerate().map(|(i, p)| (i, p.id.as_str())),
                        &pat,
                        "motion point id",
                        false,
                        out,
                    );
                    for (i, p) in points.iter().enumerate() {
                        p.check(&format!("{pat}/{i}"), out);
                    }
                }
            }
        } else {
            if self.points.is_some() {
                out.push(&ptr(at, "points"), "kind", "1-D channels have keys, not points");
            }
            if self.curve_type.is_none() {
                out.push(&ptr(at, "curveType"), "kind", "1-D channels have a curve type");
            }
            if self.name == "velocity" && self.curve_type == Some(CurveType::VelocityBased) {
                out.push(&ptr(at, "curveType"), "curveType", "velocity supports spline and interpolation only");
            }
            if self.curve_type == Some(CurveType::VelocityBased)
                && !["rotation", "scale.x", "scale.y", "skew"].contains(&self.name.as_str())
            {
                out.push(&ptr(at, "curveType"), "curveType", "velocityBased applies to rotation, scale and skew only");
            }
            match &self.keys {
                None => out.push(&ptr(at, "keys"), "kind", "1-D channels have keys"),
                Some(keys) => {
                    if keys.is_empty() {
                        out.push(&ptr(at, "keys"), "minKeys", "a channel exists only with at least one key");
                    }
                    if keys.windows(2).any(|w| w.first().map(|k| k.frame) >= w.get(1).map(|k| k.frame)) {
                        out.push(&ptr(at, "keys"), "keysSorted", "keys are sorted by frame and frames are unique");
                    }
                    let kat = ptr(at, "keys");
                    unique(keys.iter().enumerate().map(|(i, k)| (i, k.id.as_str())), &kat, "key id", false, out);
                    for (i, k) in keys.iter().enumerate() {
                        k.check(&format!("{kat}/{i}"), out);
                        if self.name == "velocity" && k.constant_segment {
                            out.push(
                                &format!("{kat}/{i}/constantSegment"),
                                "velocityConstant",
                                "the velocity channel can never be constant",
                            );
                        }
                    }
                }
            }
        }
        meta_ext(at, &self.ext, out);
    }
}

impl Check for ParticleBlock {
    fn check(&self, at: &str, out: &mut Issues) {
        let s = &self.statics;
        let first = i64::from(s.first_frame);
        let start = i64::from(s.simulation_start);
        if start > first || start < first - 999 {
            out.push(
                &format!("{at}/static/simulationStart"),
                "range",
                "simulationStart is between firstFrame − 999 and firstFrame",
            );
        }
        let cap = if matches!(self.particle_type, effect::ParticleType::Fog | effect::ParticleType::Cloud) {
            50
        } else {
            3000
        };
        if s.max_particles > cap {
            out.push(
                &format!("{at}/static/maxParticles"),
                "range",
                format!("maxParticles is at most {cap} for this type"),
            );
        }
        let names: BTreeSet<&str> = self.dynamic.keys().map(String::as_str).collect();
        let expected: BTreeSet<&str> = effect::DYNAMIC_PARAMS.iter().map(|(n, _, _)| *n).collect();
        if names != expected {
            out.push(&ptr(at, "dynamic"), "dynamicParams", format!("dynamic parameters are exactly {expected:?}"));
        }
        for (name, lo, hi) in effect::DYNAMIC_PARAMS {
            let Some(p) = self.dynamic.get(*name) else { continue };
            let values: Vec<f64> = match p {
                effect::DynamicParam::Value { value } => vec![*value],
                effect::DynamicParam::Keyframes { keyframes } => {
                    if keyframes.windows(2).any(|w| w.first().map(|k| k.frame) >= w.get(1).map(|k| k.frame)) {
                        out.push(
                            &format!("{at}/dynamic/{name}"),
                            "keysSorted",
                            "keyframes are sorted with unique frames",
                        );
                    }
                    keyframes.iter().map(|k| k.value).collect()
                }
            };
            if values.iter().any(|v| !v.is_finite() || v < lo || v > hi) {
                out.push(&format!("{at}/dynamic/{name}"), "range", format!("{name} is {lo}…{hi}"));
            }
        }
        meta_ext(at, &self.ext, out);
    }
}

impl Check for SoundPlacement {
    fn check(&self, at: &str, out: &mut Issues) {
        if self.start_frame > self.stop_frame {
            out.push(&ptr(at, "stopFrame"), "order", "startFrame ≤ stopFrame");
        }
        if self.in_point.partial_cmp(&self.out_point) != Some(std::cmp::Ordering::Less) {
            out.push(&ptr(at, "outPoint"), "order", "inPoint < outPoint");
        }
        if self.envelope.windows(2).any(|w| w.first().map(|p| p.time) > w.get(1).map(|p| p.time)) {
            out.push(&ptr(at, "envelope"), "order", "envelope times ascend");
        }
        if let Some(t) = &self.mouth_track {
            let frames = u64::from(self.stop_frame).saturating_sub(u64::from(self.start_frame)) + 1;
            if t.mouths.chars().count() as u64 != frames {
                out.push(
                    &format!("{at}/mouthTrack/mouths"),
                    "mouthsLength",
                    "one mouth letter per frame of the placement",
                );
            }
        }
        meta_ext(at, &self.ext, out);
    }
}

impl Check for MediaSequence {
    fn check(&self, at: &str, out: &mut Issues) {
        if self.start_frame > self.stop_frame {
            out.push(&ptr(at, "stopFrame"), "order", "startFrame ≤ stopFrame");
        }
        meta_ext(at, &self.ext, out);
    }
}

fn non_overlapping(spans: &[(u32, u32)]) -> bool {
    spans.windows(2).all(|w| matches!((w.first(), w.get(1)), (Some(a), Some(b)) if a.1 < b.0))
}

fn specific_present(e: &Element) -> Vec<&'static str> {
    [
        ("drawingBankId", e.drawing_bank_id.is_some()),
        ("exposure", e.exposure.is_some()),
        ("cellNotes", !e.cell_notes.is_empty()),
        ("importOptions", e.import_options.is_some()),
        ("streamed", e.streamed.is_some()),
        ("showMouthShapes", e.show_mouth_shapes.is_some()),
        ("placements", e.placements.is_some()),
        ("lipSyncMappings", e.lip_sync_mappings.is_some()),
        ("sequences", e.sequences.is_some()),
        ("transform", e.transform.is_some()),
        ("exportGroup", e.export_group.is_some()),
        ("camera", e.camera.is_some()),
        ("skeleton", e.skeleton.is_some()),
        ("bone", e.bone.is_some()),
        ("slot", e.slot.is_some()),
        ("params", e.params.is_some()),
        ("blocks", e.blocks.is_some()),
        ("channels", e.channels.is_some()),
    ]
    .into_iter()
    .filter_map(|(k, p)| p.then_some(k))
    .collect()
}

fn check_params(e: &Element, at: &str, out: &mut Issues) {
    let Some(params) = &e.params else { return };
    let v = Value::Object(params.clone());
    let res: Result<(), String> = match e.element_type {
        ElementType::ColourTransform => {
            serde_json::from_value::<effect::ColourTransformParams>(v).map(drop).map_err(|e| e.to_string())
        }
        ElementType::Clipping => {
            serde_json::from_value::<effect::ClippingParams>(v).map(drop).map_err(|e| e.to_string())
        }
        ElementType::DropShadow => {
            serde_json::from_value::<effect::DropShadowParams>(v).map(drop).map_err(|e| e.to_string())
        }
        ElementType::Blur => serde_json::from_value::<effect::BlurParams>(v).map(drop).map_err(|e| e.to_string()),
        ElementType::Shake => serde_json::from_value::<effect::ShakeParams>(v).map(drop).map_err(|e| e.to_string()),
        ElementType::ParticleNature | ElementType::ParticleImpact => {
            serde_json::from_value::<effect::ParticleParams>(v).map(drop).map_err(|e| e.to_string())
        }
        _ => Err("this element type has no params".into()),
    };
    if let Err(msg) = res {
        out.push(
            &ptr(at, "params"),
            "params",
            format!("params do not match the {} parameter set: {msg}", e.element_type.as_str()),
        );
    }
}

impl Check for Element {
    fn check(&self, at: &str, out: &mut Issues) {
        name_rule::<Element>(&self.name, at, out);
        let t = self.element_type;
        let allowed = t.specific_properties();
        let present = specific_present(self);
        for p in SPECIFIC_PROPERTIES {
            let is = present.contains(p);
            let may = allowed.contains(p);
            if is && !may {
                out.push(&ptr(at, p), "elementType", format!("a {} element has no `{p}`", t.as_str()));
            } else if !is && may && *p != "cellNotes" {
                out.push(&ptr(at, p), "elementType", format!("a {} element has `{p}`", t.as_str()));
            }
        }
        if t.is_cell_based() {
            if self.start.is_some() || self.duration.is_some() {
                out.push(
                    &ptr(at, "start"),
                    "derived",
                    "start and duration of cell-based elements are derived, not stored",
                );
            }
            if self.visible_in_xsheet.is_none() {
                out.push(&ptr(at, "visibleInXsheet"), "elementType", "cell-based elements have visibleInXsheet");
            }
        } else {
            if self.start.is_none() || self.duration.is_none() {
                out.push(&ptr(at, "start"), "elementType", "non-cell elements have start and duration");
            }
            if self.visible_in_xsheet.is_some() {
                out.push(&ptr(at, "visibleInXsheet"), "elementType", "only cell-based elements have visibleInXsheet");
            }
        }
        if self.loops == 0 {
            out.push(&ptr(at, "loops"), "range", "loops ≥ 1");
        }
        if !(20..=1000).contains(&self.column_width) {
            out.push(&ptr(at, "columnWidth"), "range", "columnWidth is 20–1000");
        }
        if self.note.chars().count() > 10_000 {
            out.push(&ptr(at, "note"), "range", "notes are at most 10 000 characters");
        }
        for (frame, note) in &self.cell_notes {
            if *frame == 0 || note.chars().count() > 10_000 {
                out.push(
                    &ptr(&ptr(at, "cellNotes"), &frame.to_string()),
                    "range",
                    "cell notes are keyed by frame ≥ 1 and at most 10 000 characters",
                );
            }
        }
        if let Some(runs) = &self.exposure {
            if crate::object::canonical_runs(runs) != *runs {
                out.push(&ptr(at, "exposure"), "canonical", "exposure runs are canonical (merged, no trailing blank)");
            }
            let total: u64 = runs.iter().map(|(_, c)| u64::from(*c)).sum();
            if total > crate::document::MAX_SCENE_LENGTH {
                out.push(
                    &ptr(at, "exposure"),
                    "limit",
                    format!("exposures exceed the scene length limit of {}", crate::document::MAX_SCENE_LENGTH),
                );
            }
        }
        if let (Some(start), Some(duration)) = (self.start, self.duration)
            && u64::from(start) + u64::from(duration).saturating_sub(1) > crate::document::MAX_SCENE_LENGTH
        {
            out.push(
                &ptr(at, "duration"),
                "limit",
                format!("the bar ends after frame {}", crate::document::MAX_SCENE_LENGTH),
            );
        }
        if let Some(tr) = &self.transform {
            let tat = ptr(at, "transform");
            let present: Vec<&str> = [
                ("rotationPivot", tr.rotation_pivot.is_some()),
                ("scalePivot", tr.scale_pivot.is_some()),
                ("rotateAboutPivot", tr.rotate_about_pivot.is_some()),
                ("pivot", tr.pivot.is_some()),
                ("applyPivotTo", tr.apply_pivot_to.is_some()),
                ("useChildDrawingPivot", tr.use_child_drawing_pivot.is_some()),
            ]
            .into_iter()
            .filter_map(|(k, p)| p.then_some(k))
            .collect();
            let expected = PivotSet::of(t).map(PivotSet::fields).unwrap_or(&[]);
            if present != expected {
                out.push(
                    &tat,
                    "pivots",
                    format!("a {} transform has exactly the pivot fields {expected:?}", t.as_str()),
                );
            }
            for v in [tr.offset.x, tr.offset.y, tr.offset.z, tr.rotation, tr.skew, tr.scale.x, tr.scale.y] {
                finite(v, &tat, out);
            }
            if !(-89.0..=89.0).contains(&tr.skew) {
                out.push(&ptr(&tat, "skew"), "range", "skew is −89…89");
            }
            if tr.scale.x.abs() < 0.001 || tr.scale.y.abs() < 0.001 {
                out.push(&ptr(&tat, "scale"), "range", "|scale| ≥ 0.001");
            }
        }
        if let Some(cam) = &self.camera
            && !(1.0..=179.0).contains(&cam.fov)
        {
            out.push(&format!("{at}/camera/fov"), "range", "fov is 1–179");
        }
        if let Some(bone) = &self.bone
            && bone.rest.length <= 0.01
        {
            out.push(&format!("{at}/bone/rest/length"), "range", "bone length > 0.01");
        }
        check_params(self, at, out);
        if let Some(channels) = &self.channels {
            let allowed = t.allowed_channels();
            for (key, ch) in channels {
                let cat = ptr(&ptr(at, "channels"), key);
                if !allowed.contains(&key.as_str()) {
                    out.push(&cat, "channelId", format!("a {} element has no `{key}` channel", t.as_str()));
                }
                if &ch.name != key {
                    out.push(&ptr(&cat, "name"), "channelName", "a channel's name equals its key");
                }
                ch.check(&cat, out);
            }
        }
        if let Some(placements) = &self.placements {
            let pat = ptr(at, "placements");
            for (i, p) in placements.iter().enumerate() {
                p.check(&format!("{pat}/{i}"), out);
            }
            let spans: Vec<(u32, u32)> = placements.iter().map(|p| (p.start_frame, p.stop_frame)).collect();
            if !non_overlapping(&spans) {
                out.push(&pat, "overlap", "placements are sorted by startFrame and do not overlap");
            }
        }
        if let Some(mappings) = &self.lip_sync_mappings {
            let mat = ptr(at, "lipSyncMappings");
            unique(
                mappings.iter().enumerate().map(|(i, m)| (i, m.drawing_element_id.as_str())),
                &mat,
                "mapped element",
                false,
                out,
            );
        }
        if let Some(seqs) = &self.sequences {
            let sat = ptr(at, "sequences");
            for (i, s) in seqs.iter().enumerate() {
                s.check(&format!("{sat}/{i}"), out);
            }
            let mut spans: Vec<(u32, u32)> = seqs.iter().map(|s| (s.start_frame, s.stop_frame)).collect();
            spans.sort_unstable();
            if !non_overlapping(&spans) {
                out.push(&sat, "overlap", "sequences do not overlap");
            }
        }
        if let Some(blocks) = &self.blocks {
            let bat = ptr(at, "blocks");
            for (i, b) in blocks.iter().enumerate() {
                b.check(&format!("{bat}/{i}"), out);
                let nature = t == ElementType::ParticleNature;
                if b.particle_type.is_nature() != nature {
                    out.push(
                        &format!("{bat}/{i}/particleType"),
                        "particleType",
                        "particle type does not belong to this effect",
                    );
                }
            }
            let mut end: Option<u64> = None;
            for (i, b) in blocks.iter().enumerate() {
                let first = u64::from(b.statics.first_frame);
                if end.is_some_and(|e| e != first) {
                    out.push(&format!("{bat}/{i}/static/firstFrame"), "consecutive", "blocks are consecutive");
                }
                end = Some(first + u64::from(b.statics.duration));
            }
        }
        meta_ext(at, &self.ext, out);
    }
}

fn tree_ids<'a>(nodes: &'a [TreeNode], out: &mut Vec<&'a str>) {
    for n in nodes {
        out.push(n.id.as_str());
        tree_ids(&n.children, out);
    }
}

impl Check for Scene {
    fn check(&self, at: &str, out: &mut Issues) {
        name_rule::<Scene>(&self.name, at, out);
        self.out_transition.check(&ptr(at, "outTransition"), out);
        let mut ids = Vec::new();
        tree_ids(&self.tree, &mut ids);
        unique(ids.iter().enumerate().map(|(i, s)| (i, *s)), &ptr(at, "tree"), "tree entry", false, out);
        unique(
            self.column_order.iter().enumerate().map(|(i, s)| (i, s.as_str())),
            &ptr(at, "columnOrder"),
            "column",
            false,
            out,
        );
        unique(
            self.elements.ids().iter().enumerate().map(|(i, s)| (i, s.as_str())),
            &ptr(at, "elements"),
            "element",
            false,
            out,
        );
        meta_ext(at, &self.ext, out);
    }
}

fn check_library_children(children: &[LibraryNode], at: &str, out: &mut Issues) {
    unique(
        children.iter().enumerate().map(|(i, c)| match c {
            LibraryNode::Folder(f) => (i, f.name.as_str()),
            LibraryNode::Item(it) => (i, it.name.as_str()),
        }),
        at,
        "sibling name",
        true,
        out,
    );
    for (i, c) in children.iter().enumerate() {
        match c {
            LibraryNode::Folder(f) => f.check(&format!("{at}/{i}"), out),
            LibraryNode::Item(it) => it.check(&format!("{at}/{i}"), out),
        }
    }
}

impl Check for LibraryItem {
    fn check(&self, at: &str, out: &mut Issues) {
        name_rule::<LibraryItem>(&self.name, at, out);
        let mut o = Issues::new(1);
        check_entry_path(&self.file, "", &mut o);
        if !o.is_empty() || self.file.contains('/') {
            out.push(&ptr(at, "file"), "file", "library files are stored flat under files/ (a plain file name)");
        }
        meta_ext(at, &self.ext, out);
    }
}

impl Check for LibraryFolder {
    fn check(&self, at: &str, out: &mut Issues) {
        name_rule::<LibraryFolder>(&self.name, at, out);
        check_library_children(&self.children, &ptr(at, "children"), out);
        meta_ext(at, &self.ext, out);
    }
}

impl Check for Library {
    fn check(&self, at: &str, out: &mut Issues) {
        name_rule::<Library>(&self.name, at, out);
        check_library_children(&self.children, &ptr(at, "children"), out);
        meta_ext(at, &self.ext, out);
    }
}

impl Check for Project {
    fn check(&self, at: &str, out: &mut Issues) {
        name_rule::<Project>(&self.name, at, out);
        if !self.frame_rate.is_finite() || !(1.0..=120.0).contains(&self.frame_rate) {
            out.push(&ptr(at, "frameRate"), "range", "frameRate is 1–120");
        }
        for (k, v) in [("cameraWidth", self.camera_width), ("cameraHeight", self.camera_height)] {
            if !(16..=8192).contains(&v) {
                out.push(&ptr(at, k), "range", format!("{k} is 16–8192"));
            }
        }
        self.start_transition.check(&ptr(at, "startTransition"), out);
        for (k, n) in
            [("palettes", self.palettes.ids().len()), ("pens", self.pens.len()), ("scenes", self.scenes.ids().len())]
        {
            if n == 0 {
                out.push(&ptr(at, k), "minItems", format!("a project has at least one of {k}"));
            }
        }
        let pat = ptr(at, "pens");
        unique(self.pens.iter().enumerate().map(|(i, p)| (i, p.name.as_str())), &pat, "pen name", false, out);
        for (i, p) in self.pens.iter().enumerate() {
            p.check(&format!("{pat}/{i}"), out);
        }
        let aat = ptr(at, "assets");
        for (i, a) in self.assets.iter().enumerate() {
            a.check(&format!("{aat}/{i}"), out);
        }
        for (k, ids) in [
            ("palettes", self.palettes.ids()),
            ("drawingBanks", self.drawing_banks.ids()),
            ("scenes", self.scenes.ids()),
        ] {
            unique(ids.iter().enumerate().map(|(i, s)| (i, s.as_str())), &ptr(at, k), k, false, out);
        }
        meta_ext(at, &self.ext, out);
    }
}

/// The sorted map of exposure-run counts per drawing (used by usage queries and tests).
#[must_use]
pub fn exposure_usage(e: &Element) -> BTreeMap<String, u64> {
    let mut m = BTreeMap::new();
    for (d, c) in e.exposure.iter().flatten() {
        if let Some(d) = d {
            *m.entry(d.as_str().to_owned()).or_insert(0) += u64::from(*c);
        }
    }
    m
}
