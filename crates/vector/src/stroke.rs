//! Building and re-deriving strokes in the persisted form (10 §4.9) from 02 §13.5 parameters.
//!
//! - **Brush**: the spine (fitted samples, or the given Bézier list verbatim), its pressure map and pen are kept
//!   (§3.2) and the outline is swept from them (§3.4). The persisted spine has no `closed` flag, so a closed spine is
//!   stored open with its first anchor repeated at the end. The persisted spine pen has no tip angle; a non-zero angle
//!   of a non-round tip is kept in the stroke's `ext["dev.toontown.brush"].tipAngle` (SI-054).
//! - **Pencil**: the centre line with a line style (colour ID, thickness, solid).
//! - **Invisible**: the path alone.

use serde_json::{Map, Value, json};
use tt_model::entities::drawing::{Stroke, StrokeKind};
use tt_model::entities::pen::Tip;

use crate::brush::{self, BrushPen};
use crate::fit::{self, Sample};
use crate::path::{Anchor, GeomError, Path};

/// The `ext` namespace of engine data the persisted stroke format has no field for (SI-054).
pub const EXT_NAMESPACE: &str = "dev.toontown.brush";

/// Geometry input of a new stroke (02 §13.1 item 6).
#[derive(Debug, Clone, PartialEq)]
pub enum Geometry {
    /// Samples, fitted (§3.3) when `fit`, else joined by straight segments.
    Samples {
        /// The samples.
        samples: Vec<Sample>,
        /// Fit Béziers.
        fit: bool,
    },
    /// Bézier control points used verbatim.
    Bezier(Path),
}

/// A stroke to build.
#[derive(Debug, Clone, PartialEq)]
pub struct Spec {
    /// Kind.
    pub kind: StrokeKind,
    /// Geometry.
    pub geometry: Geometry,
    /// Closed path.
    pub closed: bool,
    /// Explicit pressure map (overrides per-sample pressure).
    pub pressure: Option<Vec<[f64; 2]>>,
    /// Smoothness 0–10.
    pub smoothness: f64,
    /// The brush pen.
    pub pen: BrushPen,
    /// The pen's ID, when it is a project pen.
    pub pen_id: Option<String>,
    /// Colour ID (brush fill, pencil line).
    pub colour_id: Option<String>,
    /// Pencil thickness (du).
    pub thickness: f64,
}

/// The centre path of the input (fitted or verbatim) with its pressure map.
///
/// # Errors
/// `drawing.invalidGeometry`.
pub fn centre(spec: &Spec) -> Result<(Path, Vec<[f64; 2]>), GeomError> {
    let (path, pressure) = match &spec.geometry {
        Geometry::Samples { samples, fit: true } => {
            let (f, _) = fit::fit_bounded(samples, spec.smoothness, spec.closed, fit::FIT_WORK_BUDGET)?;
            (f.path, f.pressure)
        }
        Geometry::Samples { samples, fit: false } => {
            let f = fit::polyline(samples, spec.closed, spec.smoothness);
            (f.path, f.pressure)
        }
        Geometry::Bezier(p) => (p.clone(), vec![[0.0, 1.0], [1.0, 1.0]]),
    };
    path.check("/bezier")?;
    let pressure = match &spec.pressure {
        Some(m) => {
            fit::check_pressure(m, "/pressure")?;
            m.clone()
        }
        None => pressure,
    };
    Ok((path, pressure))
}

/// A closed spine stored open: its first anchor repeated at the end.
#[must_use]
pub fn open_spine(p: &Path) -> Path {
    if !p.closed || p.anchors.len() < 2 {
        return Path { anchors: p.anchors.clone(), closed: false };
    }
    let mut anchors = p.anchors.clone();
    let first = anchors[0];
    anchors[0].inh = crate::geom::P::default();
    anchors.push(Anchor { p: first.p, inh: first.inh, outh: crate::geom::P::default() });
    Path { anchors, closed: false }
}

/// The persisted spelling of a tip shape.
#[must_use]
pub fn tip_name(t: Tip) -> &'static str {
    match t {
        Tip::Round => "round",
        Tip::Oval => "oval",
        Tip::Square => "square",
        Tip::Flat => "flat",
    }
}

/// The persisted stroke JSON for a new stroke with `id` (rev 0; the executor stamps it).
///
/// # Errors
/// `drawing.invalidGeometry`.
pub fn build(spec: &Spec, id: &str) -> Result<Value, GeomError> {
    let (path, pressure) = centre(spec)?;
    let mut m = Map::new();
    m.insert("id".into(), json!(id));
    m.insert("type".into(), json!("stroke"));
    m.insert("rev".into(), json!(0));
    match spec.kind {
        StrokeKind::Brush => {
            let spine =
                if path.anchors.len() == 1 { Path { anchors: path.anchors.clone(), closed: true } } else { path };
            let outline = brush::outline(&spine, &pressure, &spec.pen)?;
            let stored = if spine.anchors.len() == 1 { spine.clone() } else { open_spine(&spine) };
            m.insert("kind".into(), json!("brush"));
            m.insert("closed".into(), json!(true));
            m.insert("points".into(), outline.points_json());
            m.insert("colourId".into(), json!(spec.colour_id));
            m.insert("fillMatrix".into(), Value::Null);
            m.insert("lineStyle".into(), Value::Null);
            m.insert(
                "spine".into(),
                json!({
                    "points": stored.points_json(),
                    "pressure": pressure,
                    "pen": {
                        "penId": spec.pen_id, "tip": tip_name(spec.pen.tip),
                        "minSize": spec.pen.min_size, "maxSize": spec.pen.max_size,
                    },
                }),
            );
            m.insert("sides".into(), json!({ "left": null, "right": null }));
            if spec.pen.tip != Tip::Round && spec.pen.tip_angle != 0.0 {
                m.insert("ext".into(), json!({ EXT_NAMESPACE: { "tipAngle": spec.pen.tip_angle } }));
            }
        }
        StrokeKind::Pencil | StrokeKind::Invisible => {
            if path.anchors.len() < 2 {
                return Err(GeomError::invalid(
                    "/points",
                    "a pencil line or invisible stroke needs at least two distinct points",
                ));
            }
            let pencil = spec.kind == StrokeKind::Pencil;
            m.insert("kind".into(), json!(if pencil { "pencil" } else { "invisible" }));
            m.insert("closed".into(), json!(path.closed));
            m.insert("points".into(), path.points_json());
            m.insert("colourId".into(), if pencil { json!(spec.colour_id) } else { Value::Null });
            m.insert("fillMatrix".into(), Value::Null);
            m.insert(
                "lineStyle".into(),
                if pencil { json!({ "thickness": spec.thickness, "lineType": "solid" }) } else { Value::Null },
            );
            m.insert("spine".into(), Value::Null);
            m.insert("sides".into(), json!({ "left": null, "right": null }));
        }
    }
    Ok(Value::Object(m))
}

/// The tip angle of a brush stroke's spine pen (SI-054), 0 when none is recorded.
#[must_use]
pub fn tip_angle(s: &Stroke) -> f64 {
    s.ext().as_map().get(EXT_NAMESPACE).and_then(|v| v.get("tipAngle")).and_then(Value::as_f64).unwrap_or(0.0)
}

/// The brush pen recorded on a stroke's spine.
#[must_use]
pub fn spine_pen(s: &Stroke) -> Option<BrushPen> {
    let sp = s.spine().as_ref()?;
    Some(BrushPen { tip: sp.pen.tip, min_size: sp.pen.min_size, max_size: sp.pen.max_size, tip_angle: tip_angle(s) })
}

/// The outline of a brush stroke re-derived from its spine (after a spine or pressure edit).
///
/// # Errors
/// `drawing.invalidGeometry`.
pub fn rederive(spine: &Path, pressure: &[[f64; 2]], pen: &BrushPen) -> Result<Path, GeomError> {
    brush::outline(spine, pressure, pen)
}

/// An edit of `draw.stroke.set` (02 §13.5).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Edit {
    /// New Bézier list (brush: the spine, or the outline once the spine is gone; others: the path).
    pub bezier: Option<Vec<crate::geom::P>>,
    /// Closed path.
    pub closed: Option<bool>,
    /// New pressure map of a brush spine.
    pub pressure: Option<Vec<[f64; 2]>>,
    /// New pencil thickness.
    pub thickness: Option<f64>,
    /// `Some(true)`: pencil → invisible; `Some(false)`: invisible → pencil.
    pub to_invisible: Option<bool>,
    /// Thickness of a pencil line made from an invisible stroke when none is given (the current pen's maximum).
    pub default_thickness: f64,
}

/// Why an edit is refused.
#[derive(Debug, Clone, PartialEq)]
pub enum EditError {
    /// Malformed geometry.
    Geometry(GeomError),
    /// A parameter that does not apply to this stroke (pointer, message).
    NotApplicable(&'static str, String),
}

fn na(pointer: &'static str, m: &str) -> EditError {
    EditError::NotApplicable(pointer, m.to_owned())
}

/// Applies an edit to a stroke (`cur` is its persisted JSON): geometry is replaced, a brush outline is swept again
/// from an edited spine (02 §3.2), the stroke keeps its ID (§3.12 item 4).
///
/// # Errors
/// [`EditError`].
pub fn edit(s: &Stroke, cur: &Value, e: &Edit) -> Result<Value, EditError> {
    let mut next = cur.clone();
    let Some(m) = next.as_object_mut() else { return Err(na("/strokeId", "not a stroke")) };
    let g = |r: Result<Path, GeomError>| r.map_err(EditError::Geometry);
    match s.kind() {
        StrokeKind::Brush => {
            if e.thickness.is_some() {
                return Err(na(
                    "/thickness",
                    "thickness applies to pencil lines; a brush stroke's width comes from its pen",
                ));
            }
            if e.to_invisible.is_some() {
                return Err(na("/kind", "only pencil lines and invisible strokes change kind"));
            }
            match (s.spine(), &e.bezier) {
                (Some(sp), bez) => {
                    if e.closed.is_some() && bez.is_none() {
                        return Err(na("/closed", "closing a brush spine needs the new spine in bezier"));
                    }
                    let spine = match bez {
                        Some(b) => g(Path::from_bezier_list(b, e.closed.unwrap_or(false), "/bezier"))?,
                        None => Path::from_points(&sp.points, sp.points.len() == 1),
                    };
                    let pressure = e.pressure.clone().unwrap_or_else(|| sp.pressure.clone());
                    crate::fit::check_pressure(&pressure, "/pressure").map_err(EditError::Geometry)?;
                    let pen = spine_pen(s).ok_or_else(|| na("/strokeId", "the spine has no pen"))?;
                    let outline = g(rederive(&spine, &pressure, &pen))?;
                    let stored = if spine.anchors.len() == 1 { spine.clone() } else { open_spine(&spine) };
                    m.insert("points".into(), outline.points_json());
                    if let Some(sp_json) = m.get_mut("spine").and_then(Value::as_object_mut) {
                        sp_json.insert("points".into(), stored.points_json());
                        sp_json.insert("pressure".into(), json!(pressure));
                    }
                }
                (None, Some(b)) => {
                    if e.pressure.is_some() {
                        return Err(na(
                            "/pressure",
                            "the outline was edited directly; the stroke has no spine or pressure",
                        ));
                    }
                    if e.closed == Some(false) {
                        return Err(na("/closed", "brush outlines are closed (10 §4.9)"));
                    }
                    let outline = g(Path::from_bezier_list(b, true, "/bezier"))?;
                    m.insert("points".into(), outline.points_json());
                }
                (None, None) => {
                    if e.pressure.is_some() {
                        return Err(na(
                            "/pressure",
                            "the outline was edited directly; the stroke has no spine or pressure",
                        ));
                    }
                    return Err(na("/closed", "brush outlines are closed (10 §4.9)"));
                }
            }
        }
        StrokeKind::Pencil | StrokeKind::Invisible => {
            if e.pressure.is_some() {
                return Err(na("/pressure", "pressure applies to brush strokes"));
            }
            let current = Path::from_points(s.points(), *s.closed());
            let path = match &e.bezier {
                Some(b) => g(Path::from_bezier_list(b, e.closed.unwrap_or(*s.closed()), "/bezier"))?,
                None => {
                    let mut p = current;
                    if let Some(c) = e.closed {
                        p.closed = c;
                    }
                    p
                }
            };
            path.check("/bezier").map_err(EditError::Geometry)?;
            if path.anchors.len() < 2 {
                return Err(EditError::Geometry(GeomError::invalid("/bezier", "a line needs at least two anchors")));
            }
            m.insert("points".into(), path.points_json());
            m.insert("closed".into(), json!(path.closed));
            let invisible = e.to_invisible.unwrap_or(*s.kind() == StrokeKind::Invisible);
            if invisible {
                if e.thickness.is_some() {
                    return Err(na("/thickness", "invisible strokes have no line style"));
                }
                m.insert("kind".into(), json!("invisible"));
                m.insert("colourId".into(), Value::Null);
                m.insert("lineStyle".into(), Value::Null);
            } else {
                let thickness =
                    e.thickness.or_else(|| s.line_style().as_ref().map(|l| l.thickness)).unwrap_or(e.default_thickness);
                m.insert("kind".into(), json!("pencil"));
                m.insert("lineStyle".into(), json!({ "thickness": thickness, "lineType": "solid" }));
                if *s.kind() == StrokeKind::Invisible {
                    m.insert("colourId".into(), Value::Null);
                }
            }
        }
    }
    Ok(next)
}
