//! Default content of new objects (10 §4 "Default"): the default pen set (02 §6.1), the default palette (03 §1.8), the
//! default transition (10 §4.4) and the new-project document (01 §3.1: one scene with a camera and an empty drawing
//! element; 05 §2.7: the camera sits under a peg "Camera Peg" and is the scene's current camera).
//!
//! The default palette is authored fresh (03 §15 design task): a transparent swatch first, then black and white, an
//! 18-step grey ramp, the six primaries and secondaries at full saturation, eight skin tones, sixteen hue families
//! evenly spaced around the hue circle with five light → dark variants each, and a warm-grey/brown family: 120 solid
//! swatches named `Colour 1` … `Colour 120`. Values come from integer arithmetic, so they are identical everywhere.

use serde_json::{Value, json};

use crate::document::{Document, DocumentParts};
use crate::entities::drawing::DrawingBank;
use crate::entities::element::Element;
use crate::entities::library::Library;
use crate::entities::palette::Palette;
use crate::entities::project::Project;
use crate::entities::scene::Scene;
use crate::error::ModelError;
use crate::id::{Id, IdGenerator};
use crate::object::from_json;
use crate::types::ObjectType;

/// Default FOV (05 §2.7): 2·atan(0.375) in degrees.
pub const DEFAULT_FOV: f64 = 41.1121;

/// The new-project form values (01 §3.1).
#[derive(Debug, Clone, PartialEq)]
pub struct NewProject {
    /// Project name.
    pub name: String,
    /// Frame rate (1–120).
    pub frame_rate: f64,
    /// Camera width (16–8192).
    pub camera_width: u32,
    /// Camera height (16–8192).
    pub camera_height: u32,
}

/// The IDs of the objects a new project starts with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Created {
    /// Project.
    pub project: Id,
    /// The scene.
    pub scene: Id,
    /// The camera.
    pub camera: Id,
    /// The camera peg.
    pub camera_peg: Id,
    /// The drawing element.
    pub element: Id,
    /// The default palette.
    pub palette: Id,
}

/// HSL (degrees, per-mille saturation and lightness) to `#RRGGBBFF`, in integer arithmetic.
fn hsl(h: i64, s: i64, l: i64) -> String {
    // C = (1 - |2L - 1|) * S, in per-mille.
    let c = (1000 - (2 * l - 1000).abs()) * s / 1000;
    let hp = h.rem_euclid(360) * 1000 / 60; // h' in per-mille of a sector
    let x = c * (1000 - ((hp % 2000) - 1000).abs()) / 1000;
    let (r, g, b) = match hp / 1000 {
        0 => (c, x, 0),
        1 => (x, c, 0),
        2 => (0, c, x),
        3 => (0, x, c),
        4 => (x, 0, c),
        _ => (c, 0, x),
    };
    let m = l - c / 2;
    let to = |v: i64| ((v + m) * 255 + 500) / 1000;
    format!("#{:02X}{:02X}{:02X}FF", to(r).clamp(0, 255), to(g).clamp(0, 255), to(b).clamp(0, 255))
}

/// The default palette's colours in palette order (03 §1.8).
#[must_use]
pub fn default_palette_colours() -> Vec<String> {
    let mut out = vec!["#00000000".to_owned(), "#000000FF".to_owned(), "#FFFFFFFF".to_owned()];
    // Grey ramp: 18 evenly spaced steps from near-black to near-white.
    for i in 0..18i64 {
        let v = 17 + i * 221 / 17;
        out.push(format!("#{v:02X}{v:02X}{v:02X}FF"));
    }
    // Primaries and secondaries at full saturation.
    for c in ["#FF0000FF", "#00FF00FF", "#0000FFFF", "#00FFFFFF", "#FF00FFFF", "#FFFF00FF"] {
        out.push(c.to_owned());
    }
    // Skin tones, light to dark.
    for c in ["#FBE3D3FF", "#F3CBB0FF", "#E8B591FF", "#D69E78FF", "#BC845EFF", "#9A6648FF", "#784A32FF", "#55331FFF"] {
        out.push(c.to_owned());
    }
    // Sixteen hue families, five variants each (light → dark).
    for f in 0..16i64 {
        let h = f * 360 / 16;
        for l in [850, 700, 550, 400, 250] {
            out.push(hsl(h, 750, l));
        }
    }
    // Warm greys and browns.
    for l in [800, 650, 500, 350, 200] {
        out.push(hsl(30, 300, l));
    }
    out
}

fn next(ids: &mut IdGenerator, ty: ObjectType) -> Result<Id, Vec<ModelError>> {
    ids.next(ty).map_err(|e| vec![e])
}

/// The default palette as object JSON: `Default` with one style `Default` (03 §1.8). Returns the JSON and its style.
///
/// # Errors
/// ID generation failures.
pub fn default_palette_json(ids: &mut IdGenerator, name: &str) -> Result<(Value, Id), Vec<ModelError>> {
    let palette = next(ids, ObjectType::Palette)?;
    let style = next(ids, ObjectType::PaletteStyle)?;
    let mut swatches = Vec::new();
    for (i, rgba) in default_palette_colours().into_iter().enumerate() {
        let colour = next(ids, ObjectType::Swatch)?;
        swatches.push(json!({
            "colourId": colour, "type": "swatch", "rev": 1, "name": format!("Colour {}", i + 1), "kind": "solid",
            "deformable": false, "values": { style.as_str(): { "rgba": rgba } }
        }));
    }
    let v = json!({
        "id": palette, "type": "palette", "rev": 1, "name": name,
        "styles": [{ "id": style, "type": "paletteStyle", "rev": 1, "name": "Default" }],
        "swatches": swatches, "hidden": false
    });
    Ok((v, style))
}

/// The default pen set (02 §6.1, §14 decision 3): brush pens `Pen 1`…`Pen 5` with maximum sizes 2, 4, 8, 16 and 32 du
/// (minimum 25 %), eraser pens `Eraser 1`…`Eraser 3` with sizes 10, 20 and 40 du; round, smoothness 3.
///
/// # Errors
/// ID generation failures.
pub fn default_pens_json(ids: &mut IdGenerator) -> Result<Vec<Value>, Vec<ModelError>> {
    let mut out = Vec::new();
    for (i, max) in [2.0, 4.0, 8.0, 16.0, 32.0].iter().enumerate() {
        out.push(json!({
            "id": next(ids, ObjectType::Pen)?, "type": "pen", "rev": 1, "name": format!("Pen {}", i + 1),
            "kind": "brush", "minSize": max * 0.25, "maxSize": max, "smoothness": 3, "tip": "round", "tipAngle": 0,
            "colourId": null
        }));
    }
    for (i, size) in [10.0, 20.0, 40.0].iter().enumerate() {
        out.push(json!({
            "id": next(ids, ObjectType::Pen)?, "type": "pen", "rev": 1, "name": format!("Eraser {}", i + 1),
            "kind": "eraser", "minSize": size, "maxSize": size, "smoothness": 3, "tip": "round", "tipAngle": 0,
            "colourId": null
        }));
    }
    Ok(out)
}

/// A `none` transition (10 §4.4 defaults).
#[must_use]
pub fn transition_json(id: &Id) -> Value {
    json!({
        "id": id, "type": "transition", "rev": 1, "transitionType": "none", "durationLeft": 12, "blankDuration": 0,
        "durationRight": 12, "colour": "#000000FF", "angle": 0, "direction": null, "size": 25
    })
}

fn identity_transform() -> Value {
    json!({
        "offset": { "x": 0, "y": 0, "z": 0 }, "rotation": 0, "skew": 0, "scale": { "x": 1, "y": 1 },
        "keepProportions": false, "lockDepth": false, "orientToPath": false
    })
}

fn bar(id: &Id, name: &str, kind: &str, now: &str) -> Value {
    json!({
        "id": id, "type": "element", "rev": 1, "name": name, "elementType": kind, "start": 1, "duration": 1,
        "loops": 1, "loopType": "count", "positionLoop": "cycle", "role": "normal", "deformable": false,
        "visibleInScene": true, "locked": false, "collapsed": false, "markerColour": null, "backgroundColour": null,
        "columnWidth": 80, "transform": identity_transform(), "channels": {}, "createdAt": now
    })
}

/// A cell-based drawing element over an empty bank.
#[must_use]
pub fn drawing_element_json(id: &Id, name: &str, bank: &Id, now: &str) -> Value {
    let mut transform = identity_transform();
    if let Some(m) = transform.as_object_mut() {
        m.insert("rotationPivot".into(), json!({ "x": 0, "y": 0 }));
        m.insert("scalePivot".into(), json!({ "x": 0, "y": 0 }));
        m.insert("rotateAboutPivot".into(), json!(false));
    }
    json!({
        "id": id, "type": "element", "rev": 1, "name": name, "elementType": "drawing", "loops": 1,
        "loopType": "count", "positionLoop": "cycle", "role": "normal", "deformable": false, "visibleInScene": true,
        "visibleInXsheet": true, "locked": false, "collapsed": false, "markerColour": null, "backgroundColour": null,
        "columnWidth": 80, "drawingBankId": bank, "exposure": [], "transform": transform, "channels": {},
        "createdAt": now
    })
}

/// An empty local library.
#[must_use]
pub fn library_json(id: &Id) -> Value {
    json!({ "id": id, "type": "library", "rev": 1, "name": "Local", "kind": "local", "formatVersion": "1.0", "children": [] })
}

fn parse<T: crate::entities::ModelObject + crate::validate::Check>(v: &Value) -> Result<T, Vec<ModelError>> {
    from_json(v)
}

/// The document of a new project (01 §3.1), at `documentRev` 1, every object at `rev` 1.
///
/// # Errors
/// ID generation failures, or a value outside its range (`frameRate` 1–120, camera 16–8192).
pub fn new_project(
    spec: &NewProject,
    ids: &mut IdGenerator,
    now: &str,
) -> Result<(Document, Created), Vec<ModelError>> {
    let project_id = next(ids, ObjectType::Project)?;
    let (palette_json, style) = default_palette_json(ids, "Default")?;
    let palette_id = Id::parse(palette_json["id"].as_str().unwrap_or_default()).map_err(|e| vec![e])?;
    let pens = default_pens_json(ids)?;
    let bank = next(ids, ObjectType::DrawingBank)?;
    let scene = next(ids, ObjectType::Scene)?;
    let peg = next(ids, ObjectType::Element)?;
    let camera = next(ids, ObjectType::Element)?;
    let element = next(ids, ObjectType::Element)?;
    let library = next(ids, ObjectType::Library)?;
    let start = next(ids, ObjectType::Transition)?;
    let out = next(ids, ObjectType::Transition)?;
    let project = json!({
        "id": project_id, "type": "project", "rev": 1, "name": spec.name, "createdAt": now, "modifiedAt": now,
        "frameRate": spec.frame_rate, "cameraWidth": spec.camera_width, "cameraHeight": spec.camera_height,
        "autoColourUpdate": false, "startTransition": transition_json(&start),
        "palettes": [{ "id": palette_id, "type": "palette" }], "pens": pens, "assets": [],
        "drawingBanks": [{ "id": bank, "type": "drawingBank" }], "scenes": [{ "id": scene, "type": "scene" }],
        "library": { "id": library, "type": "library" }
    });
    let mut camera_json = bar(&camera, "Camera", "camera", now);
    if let Some(m) = camera_json.as_object_mut() {
        m.insert("transform".into(), {
            let mut t = identity_transform();
            t["offset"]["z"] = json!(16);
            t
        });
        m.insert("camera".into(), json!({ "fovMode": "static", "fov": DEFAULT_FOV }));
    }
    let mut peg_json = bar(&peg, "Camera Peg", "peg", now);
    if let Some(t) = peg_json.get_mut("transform").and_then(Value::as_object_mut) {
        t.insert("pivot".into(), json!({ "x": 0, "y": 0 }));
        t.insert("applyPivotTo".into(), json!("both"));
        t.insert("useChildDrawingPivot".into(), json!(false));
    }
    if let Some(m) = peg_json.as_object_mut() {
        m.insert("exportGroup".into(), json!({ "groupOnVectorExport": false, "addComposite": true }));
    }
    let drawing = drawing_element_json(&element, "Drawing", &bank, now);
    let scene_json = json!({
        "id": scene, "type": "scene", "rev": 1, "name": "Scene-1", "visible": true, "backgroundColour": "#FFFFFFFF",
        "currentCameraId": camera, "paletteStyles": { palette_id.as_str(): style },
        "outTransition": transition_json(&out),
        "tree": [
            { "id": peg, "children": [{ "id": camera, "children": [] }] },
            { "id": element, "children": [] }
        ],
        "columnOrder": [element],
        "elements": [
            { "id": peg, "type": "element" }, { "id": camera, "type": "element" }, { "id": element, "type": "element" }
        ]
    });
    let bank_json = json!({ "id": bank, "type": "drawingBank", "rev": 1, "drawings": [] });
    fn take<T>(r: Result<T, Vec<ModelError>>, errors: &mut Vec<ModelError>) -> Option<T> {
        r.map_err(|e| errors.extend(e)).ok()
    }
    let mut errors = Vec::new();
    let project: Option<Project> = take(parse(&project), &mut errors);
    let palette: Option<Palette> = take(parse(&palette_json), &mut errors);
    let bank_obj: Option<DrawingBank> = take(parse(&bank_json), &mut errors);
    let scene_obj: Option<Scene> = take(parse(&scene_json), &mut errors);
    let elements: Vec<Element> =
        [peg_json, camera_json, drawing].iter().filter_map(|e| take(parse::<Element>(e), &mut errors)).collect();
    let library_obj: Option<Library> = take(parse(&library_json(&library)), &mut errors);
    let (Some(project), Some(palette), Some(bank_obj), Some(scene_obj), Some(library_obj)) =
        (project, palette, bank_obj, scene_obj, library_obj)
    else {
        return Err(errors);
    };
    if !errors.is_empty() {
        return Err(errors);
    }
    let doc = Document::from_parts(DocumentParts {
        document_rev: 1,
        project,
        palettes: vec![palette],
        banks: vec![bank_obj],
        drawings: Vec::new(),
        scenes: vec![scene_obj],
        elements,
        library: library_obj,
    })?;
    Ok((doc, Created { project: project_id, scene, camera, camera_peg: peg, element, palette: palette_id }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test")]
    use super::*;

    #[test]
    fn default_palette_meets_its_coverage_rules() {
        let c = default_palette_colours();
        assert_eq!(c.len(), 120);
        assert_eq!(c[0], "#00000000", "a transparent swatch in a fixed early position");
        assert!(c.contains(&"#000000FF".to_owned()) && c.contains(&"#FFFFFFFF".to_owned()));
        let greys = c.iter().filter(|x| x[1..3] == x[3..5] && x[3..5] == x[5..7] && x.ends_with("FF")).count();
        assert!(greys >= 16 + 2);
        for p in ["#FF0000FF", "#00FF00FF", "#0000FFFF", "#00FFFFFF", "#FF00FFFF", "#FFFF00FF"] {
            assert!(c.contains(&p.to_owned()), "{p}");
        }
        let unique: std::collections::BTreeSet<&String> = c.iter().collect();
        assert_eq!(unique.len(), c.len(), "distinct colours");
        assert_eq!(hsl(0, 1000, 500), "#FF0000FF");
        assert_eq!(hsl(120, 1000, 500), "#00FF00FF");
        assert_eq!(hsl(240, 1000, 500), "#0000FFFF");
    }

    #[test]
    fn new_project_is_valid_and_complete() {
        let mut ids = IdGenerator::seeded(7, 1_760_486_400_000);
        let spec =
            NewProject { name: "Walk cycle".into(), frame_rate: 23.976, camera_width: 1920, camera_height: 1080 };
        let (doc, created) = new_project(&spec, &mut ids, "2026-10-19T00:00:00.000Z").unwrap();
        assert!(doc.issues().iter().all(|i| i.severity != crate::document::Severity::Error));
        assert_eq!(doc.project().pens().len(), 8);
        let scene = doc.scene(&created.scene).unwrap();
        assert_eq!(scene.current_camera_id().as_ref(), Some(&created.camera));
        assert_eq!(doc.tree_order(scene).len(), 3);
        assert_eq!(doc.palettes().next().unwrap().swatches().len(), 120);
        assert_eq!(doc.element(&created.camera_peg).unwrap().name(), "Camera Peg");
    }
}
