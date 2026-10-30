//! A deterministic sample project that uses every object type and element type of 10 §2.1 and §4.6. It is the
//! fixture of the model's golden, round-trip and schema-conformance tests and of later checkpoints' executor and
//! persistence tests. Built from JSON through the one input path ([`crate::object::from_json`]).

use serde_json::{Value, json};

use crate::document::{Document, DocumentParts};
use crate::entities::drawing::{Drawing, DrawingBank};
use crate::entities::element::Element;
use crate::entities::library::Library;
use crate::entities::palette::Palette;
use crate::entities::project::Project;
use crate::entities::scene::Scene;
use crate::error::ModelError;
use crate::id::IdGenerator;
use crate::types::ObjectType;

/// Fixed creation time of the fixture.
pub const CREATED: &str = "2026-10-17T09:30:12.345Z";

struct Ids(IdGenerator);

impl Ids {
    fn next(&mut self, ty: ObjectType) -> String {
        self.0.next(ty).map(|i| i.as_str().to_owned()).unwrap_or_default()
    }
}

fn parse<T: crate::entities::ModelObject + crate::validate::Check>(v: &Value) -> Result<T, Vec<ModelError>> {
    crate::object::from_json(v)
}

fn ok<T>(errors: &mut Vec<ModelError>, r: Result<T, Vec<ModelError>>) -> Option<T> {
    match r {
        Ok(v) => Some(v),
        Err(e) => {
            errors.extend(e);
            None
        }
    }
}

fn sha(n: u8) -> String {
    format!("{n:02x}").repeat(32)
}

fn transform(kind: &str) -> Value {
    let mut t = json!({
        "offset": {"x": 0, "y": 0, "z": 0}, "rotation": 0, "skew": 0, "scale": {"x": 1, "y": 1}, "keepProportions": false
    });
    if let Value::Object(m) = &mut t {
        match kind {
            "element" => {
                m.insert("rotationPivot".into(), json!({"x": 0, "y": 0}));
                m.insert("scalePivot".into(), json!({"x": 0, "y": 0}));
                m.insert("rotateAboutPivot".into(), json!(false));
            }
            "peg" => {
                m.insert("pivot".into(), json!({"x": 0.25, "y": -1.5}));
                m.insert("applyPivotTo".into(), json!("both"));
                m.insert("useChildDrawingPivot".into(), json!(false));
            }
            "skeleton" => {
                m.insert("pivot".into(), json!({"x": 0, "y": 0}));
                m.insert("applyPivotTo".into(), json!("rotation"));
            }
            _ => {
                m.insert("offset".into(), json!({"x": 0, "y": 0, "z": 16}));
            }
        }
        m.insert("lockDepth".into(), json!(false));
        m.insert("orientToPath".into(), json!(false));
    }
    t
}

fn common(id: &str, name: &str, element_type: &str, cell: bool) -> Value {
    let mut e = json!({
        "id": id, "type": "element", "rev": 1, "name": name, "elementType": element_type,
        "loops": 1, "loopType": "count", "positionLoop": "cycle", "role": "normal", "deformable": false,
        "visibleInScene": true, "locked": false, "collapsed": false,
        "markerColour": null, "backgroundColour": null, "columnWidth": 80, "createdAt": CREATED
    });
    if let Value::Object(m) = &mut e {
        if cell {
            m.insert("visibleInXsheet".into(), json!(true));
        } else {
            m.insert("start".into(), json!(1));
            m.insert("duration".into(), json!(48));
        }
    }
    e
}

fn merge(mut a: Value, b: Value) -> Value {
    if let (Value::Object(ma), Value::Object(mb)) = (&mut a, b) {
        for (k, v) in mb {
            ma.insert(k, v);
        }
    }
    a
}

fn key(ids: &mut Ids, frame: u32, value: f64) -> Value {
    json!({"id": ids.next(ObjectType::Key), "type": "key", "rev": 1, "frame": frame, "value": value,
           "constantSegment": false,
           "handles": {"left": {"dt": -4, "dv": 0}, "right": {"dt": 4, "dv": 0}, "mode": "smooth"}, "easing": null})
}

fn channel(ids: &mut Ids, name: &str, keys: &[(u32, f64)]) -> Value {
    let id = ids.next(ObjectType::Channel);
    let keys: Vec<Value> = keys.iter().map(|(f, v)| key(ids, *f, *v)).collect();
    json!({"id": id, "type": "channel", "rev": 1, "name": name, "curveType": "spline", "keys": keys})
}

fn block(ids: &mut Ids, ty: &str, first: u32, duration: u32, seed: u32, max: u32) -> Value {
    let v = |x: f64| json!({"value": x});
    json!({
        "id": ids.next(ObjectType::ParticleBlock), "type": "particleBlock", "rev": 1, "particleType": ty,
        "variant": null, "presetId": format!("builtin.{ty}.normal"), "seed": seed, "spriteSetId": null,
        "static": {"background": "transparent", "simulationStart": i64::from(first) - 20, "firstFrame": first,
                   "duration": duration, "maxParticles": max, "lifetimeFrames": -1, "fade": true,
                   "areaWidth": 1400, "areaHeight": 1000, "areaDepth": 600, "floorLevel": 0},
        "dynamic": {"intensity": {"keyframes": [{"frame": first, "value": 20}, {"frame": first + duration - 1, "value": 60}]},
                    "scale": v(100.0), "bounciness": v(0.0), "speedMin": v(2.0), "speedMax": v(5.0),
                    "directionEW": v(150.0), "directionNS": v(-1000.0), "directionFB": v(0.0)}
    })
}

/// The sample document's parts, with deterministic IDs from `seed`.
///
/// # Errors
/// Any object that fails validation (a fixture defect; covered by tests).
#[allow(clippy::too_many_lines)]
pub fn sample_parts(seed: u64) -> Result<DocumentParts, Vec<ModelError>> {
    let mut ids = Ids(IdGenerator::seeded(seed, 1_760_000_000_000));
    let project_id = ids.next(ObjectType::Project);
    let library_id = ids.next(ObjectType::Library);

    // Palettes (03 §12.3): solid, linear and radial gradients, a texture; two styles.
    let pal = ids.next(ObjectType::Palette);
    let day = ids.next(ObjectType::PaletteStyle);
    let night = ids.next(ObjectType::PaletteStyle);
    let skin = ids.next(ObjectType::Swatch);
    let sky = ids.next(ObjectType::Swatch);
    let glow = ids.next(ObjectType::Swatch);
    let fabric = ids.next(ObjectType::Swatch);
    let ink = ids.next(ObjectType::Swatch);
    let palette = json!({
        "id": pal, "type": "palette", "rev": 1, "name": "Main", "hidden": false,
        "styles": [{"id": day, "type": "paletteStyle", "rev": 1, "name": "Day"},
                   {"id": night, "type": "paletteStyle", "rev": 1, "name": "Night"}],
        "swatches": [
            {"colourId": skin, "type": "swatch", "rev": 1, "name": "Skin", "kind": "solid", "deformable": false,
             "values": {day.clone(): {"rgba": "#F2D3B4FF"}, night.clone(): {"rgba": "#8c7a8a"}}},
            {"colourId": sky, "type": "swatch", "rev": 1, "name": "Sky", "kind": "linearGradient", "deformable": false,
             "values": {day.clone(): {"stops": [{"position": 0, "rgba": "#70C8F0FF"}, {"position": 1, "rgba": "#FFFFFFFF"}]},
                        night.clone(): {"stops": [{"position": 0, "rgba": "#10084AFF"}, {"position": 1, "rgba": "#300448FF"}]}}},
            {"colourId": glow, "type": "swatch", "rev": 1, "name": "Glow", "kind": "radialGradient", "deformable": true,
             "values": {day.clone(): {"stops": [{"position": 0, "rgba": "#FFFFFFFF"}, {"position": 0.5, "rgba": "#FFF0A0C0"}, {"position": 1, "rgba": "#FFF0A000"}]},
                        night.clone(): {"stops": [{"position": 0, "rgba": "#A0A0FFFF"}, {"position": 1, "rgba": "#2020A000"}]}}},
            {"colourId": fabric, "type": "swatch", "rev": 1, "name": "Fabric", "kind": "texture", "deformable": true,
             "values": {day.clone(): {"image": "textures/fabric-day.png", "tile": true, "width": 64, "height": 64},
                        night.clone(): {"image": "textures/fabric-night.png", "tile": false}}},
            {"colourId": ink, "type": "swatch", "rev": 1, "name": "Ink", "kind": "solid", "deformable": false,
             "values": {day.clone(): {"rgba": "#000000FF"}, night.clone(): {"rgba": "#101010FF"}}}
        ]
    });
    let pal2 = ids.next(ObjectType::Palette);
    let pal2_style = ids.next(ObjectType::PaletteStyle);
    let fx_white = ids.next(ObjectType::Swatch);
    let palette2 = json!({
        "id": pal2, "type": "palette", "rev": 1, "name": "Effects", "hidden": false, "meta": {"tags": ["fx"]},
        "styles": [{"id": pal2_style, "type": "paletteStyle", "rev": 1, "name": "Default"}],
        "swatches": [{"colourId": fx_white, "type": "swatch", "rev": 1, "name": "White", "kind": "solid",
                      "deformable": false, "values": {pal2_style.clone(): {"rgba": "#FFFFFFFF"}}}]
    });

    // Pens (02 §6.1 default set) and assets (10 §4.16).
    let pens: Vec<Value> =
        [("Pen 1", "brush", 0.5, 2.0), ("Pen 2", "brush", 1.0, 4.0), ("Eraser 1", "eraser", 10.0, 10.0)]
            .iter()
            .map(|(n, k, lo, hi)| {
                json!({"id": ids.next(ObjectType::Pen), "type": "pen", "rev": 1, "name": n, "kind": k,
            "minSize": lo, "maxSize": hi, "smoothness": 3, "tip": "round", "tipAngle": 0, "colourId": null})
            })
            .collect();
    let pens = {
        let mut p = pens;
        if let Some(Value::Object(m)) = p.get_mut(1) {
            m.insert("colourId".into(), json!(ink));
            m.insert("tip".into(), json!("flat"));
            m.insert("tipAngle".into(), json!(30));
        }
        p
    };
    let sound_asset = ids.next(ObjectType::Asset);
    let movie_asset = ids.next(ObjectType::Asset);
    let assets = vec![
        json!({"id": sound_asset, "type": "asset", "rev": 1, "name": "line1.wav", "kind": "sound", "mediaType": "audio/wav",
               "storage": "embedded", "entry": format!("media/{sound_asset}.wav"), "link": null, "size": 352_844,
               "sha256": sha(0xab), "importedAt": CREATED, "sourcePath": "C:/audio/line1.wav"}),
        json!({"id": movie_asset, "type": "asset", "rev": 1, "name": "reference.mov", "kind": "movie", "mediaType": "video/quicktime",
               "storage": "linked", "entry": null,
               "link": {"kind": "file", "path": "../ref/reference.mov", "absolutePath": "C:/ref/reference.mov",
                        "libraryId": null, "itemId": null, "versionId": null},
               "size": 5_000_000_000_u64, "sha256": sha(0xcd), "importedAt": CREATED, "sourcePath": null}),
    ];

    // Drawings (02 §3, 10 §4.8–§4.11): layers with brush, pencil and invisible strokes, a group, text and a bitmap.
    let bank = ids.next(ObjectType::DrawingBank);
    let d1 = ids.next(ObjectType::Drawing);
    let d2 = ids.next(ObjectType::Drawing);
    let d3 = ids.next(ObjectType::Drawing);
    let layer = ids.next(ObjectType::DrawingNode);
    let s1 = ids.next(ObjectType::Stroke);
    let s2 = ids.next(ObjectType::Stroke);
    let s3 = ids.next(ObjectType::Stroke);
    let group = ids.next(ObjectType::DrawingNode);
    let inner = ids.next(ObjectType::DrawingNode);
    let text = ids.next(ObjectType::DrawingNode);
    let drawing1 = json!({
        "id": d1, "type": "drawing", "rev": 1, "name": "1", "locked": false, "pivot": {"x": 0, "y": -50}, "mouthShape": null,
        "nodes": [
            {"id": layer, "type": "drawingNode", "rev": 1, "name": "Body", "kind": "layer", "hint": "brush", "strokes": [
                {"id": s1, "type": "stroke", "rev": 1, "kind": "brush", "closed": true,
                 "points": [[0, 0, 0, 0, 10.5, 0], [100, 0, -10, 0, 0, 10], [100, 100, 0, -10, 0, 0], [0, 100, 0, 0, 0, -10]],
                 "colourId": skin, "fillMatrix": null, "lineStyle": null,
                 "spine": {"points": [[10, 50, 0, 0, 20, 0], [90, 50, -20, 0, 0, 0]], "pressure": [[0, 0.2], [0.5, 1], [1, 0.4]],
                           "pen": {"penId": null, "tip": "round", "minSize": 0.5, "maxSize": 2}},
                 "sides": {"left": {"colourId": skin, "matrix": null}, "right": null}},
                {"id": s2, "type": "stroke", "rev": 1, "kind": "pencil", "closed": false,
                 "points": [[-50, -50, 0, 0, 0, 0], [50, 50, 0, 0, 0, 0]], "colourId": ink, "fillMatrix": null,
                 "lineStyle": {"thickness": 1.5, "lineType": "solid"}, "spine": null, "sides": {"left": null, "right": null}},
                {"id": s3, "type": "stroke", "rev": 1, "kind": "invisible", "closed": false,
                 "points": [[0, 0, 0, 0, 0, 0], [0, 100, 0, 0, 0, 0]], "colourId": null, "fillMatrix": null,
                 "lineStyle": null, "spine": null,
                 "sides": {"left": {"colourId": sky, "matrix": [1, 0, 0, 1, 0, 0]}, "right": {"colourId": null, "matrix": null}},
                 "meta": {"note": "closes the sky zone"}}
            ]},
            {"id": group, "type": "drawingNode", "rev": 1, "kind": "group", "children": [
                {"id": inner, "type": "drawingNode", "rev": 1, "kind": "layer", "hint": "pencil", "strokes": []}
            ]},
            {"id": text, "type": "drawingNode", "rev": 1, "name": "Caption", "kind": "text", "transform": [1, 0, 0, 1, -100, 200],
             "widthMode": "fixed", "width": 400, "text": "Héllo",
             "runs": [{"start": 0, "length": 5, "font": "Inter", "size": 24, "bold": false, "italic": true, "script": "normal",
                       "kerning": 0, "autoKerning": true, "colourId": ink}],
             "paragraphs": [{"start": 0, "length": 5, "alignment": "centre", "marginLeft": 0, "marginRight": 0, "indent": 0, "lineSpacing": 1.2}]}
        ],
        "colourRecovery": [
            {"colourId": skin, "paletteId": pal, "paletteName": "Main",
             "styles": [{"styleId": day, "styleName": "Day"}, {"styleId": night, "styleName": "Night"}],
             "swatch": {"colourId": skin, "type": "swatch", "rev": 1, "name": "Skin", "kind": "solid", "deformable": false,
                        "values": {day.clone(): {"rgba": "#F2D3B4FF"}, night.clone(): {"rgba": "#8C7A8AFF"}}}}
        ],
        "createdAt": CREATED, "ext": {"com.example.tool": {"stage": "clean-up", "pass": 2}}
    });
    let bitmap = ids.next(ObjectType::DrawingNode);
    let drawing2 = json!({"id": d2, "type": "drawing", "rev": 1, "name": "2", "locked": true, "pivot": {"x": 0, "y": 0},
        "mouthShape": "A", "nodes": [
            {"id": bitmap, "type": "drawingNode", "rev": 1, "kind": "bitmap",
             "image": {"entry": format!("banks/{bank}/{d2}/{bitmap}.png"), "mediaType": "image/png", "size": 2048, "sha256": sha(0x11)},
             "width": 32, "height": 16, "transform": [2, 0, 0, 2, -32, 16]}
        ], "colourRecovery": [], "createdAt": CREATED});
    let drawing3 = json!({"id": d3, "type": "drawing", "rev": 1, "name": "10", "locked": false, "pivot": {"x": 0, "y": 0},
        "mouthShape": "X", "nodes": [], "colourRecovery": [], "createdAt": CREATED});
    let bank_json = json!({"id": bank, "type": "drawingBank", "rev": 1,
        "drawings": [{"id": d1, "type": "drawing"}, {"id": d2, "type": "drawing"}, {"id": d3, "type": "drawing"}]});

    // Scene 1: every element type.
    let scene = ids.next(ObjectType::Scene);
    let camera = ids.next(ObjectType::Element);
    let peg = ids.next(ObjectType::Element);
    let bob = ids.next(ObjectType::Element);
    let image_el = ids.next(ObjectType::Element);
    let image_bank = ids.next(ObjectType::DrawingBank);
    let image_drawing = ids.next(ObjectType::Drawing);
    let image_node = ids.next(ObjectType::DrawingNode);
    let annotation = ids.next(ObjectType::Element);
    let annotation_bank = ids.next(ObjectType::DrawingBank);
    let sound = ids.next(ObjectType::Element);
    let media = ids.next(ObjectType::Element);
    let skeleton = ids.next(ObjectType::Element);
    let bones_slot = ids.next(ObjectType::Element);
    let skin_slot = ids.next(ObjectType::Element);
    let bone = ids.next(ObjectType::Element);
    let clipping = ids.next(ObjectType::Element);
    let mask = ids.next(ObjectType::Element);
    let colour_tx = ids.next(ObjectType::Element);
    let drop_shadow = ids.next(ObjectType::Element);
    let blur = ids.next(ObjectType::Element);
    let shake = ids.next(ObjectType::Element);
    let nature = ids.next(ObjectType::Element);
    let sprites = ids.next(ObjectType::Element);
    let impact = ids.next(ObjectType::Element);

    let bob_channels = {
        let rot = channel(&mut ids, "rotation", &[(1, 0.0), (12, 45.0), (24, -10.5)]);
        let sx = channel(&mut ids, "scale.x", &[(1, 1.0)]);
        let pos_id = ids.next(ObjectType::Channel);
        let mp = |ids: &mut Ids, frame: Option<u32>, x: f64| {
            json!({"id": ids.next(ObjectType::MotionPoint), "type": "motionPoint", "rev": 1, "frame": frame,
                   "position": {"x": x, "y": 0, "z": 0}, "tension": 0, "continuity": 0, "bias": 0,
                   "constantSegment": false, "lockDepth": false})
        };
        let points = vec![mp(&mut ids, Some(1), -4.0), mp(&mut ids, None, 0.0), mp(&mut ids, Some(24), 4.0)];
        let pos = json!({"id": pos_id, "type": "channel", "rev": 1, "name": "position", "points": points});
        let mut vel = channel(&mut ids, "velocity", &[(12, 0.5)]);
        if let Some(Value::Object(m)) = vel.pointer_mut("/keys/0") {
            m.insert("easing".into(), json!("easeInOut"));
        }
        if let Value::Object(m) = &mut vel {
            m.insert("curveType".into(), json!("interpolation"));
        }
        json!({"rotation": rot, "scale.x": sx, "position": pos, "velocity": vel})
    };
    let elements = vec![
        merge(
            common(&camera, "Camera", "camera", false),
            json!({"transform": transform("camera"),
            "camera": {"fovMode": "static", "fov": 41.1121}, "channels": {}}),
        ),
        merge(
            common(&peg, "Bob-P", "peg", false),
            json!({"transform": transform("peg"),
            "exportGroup": {"groupOnVectorExport": false, "addComposite": true}, "collapsed": false,
            "channels": {"skew": channel(&mut ids, "skew", &[(1, 0.0), (48, 5.0)])}}),
        ),
        merge(
            common(&bob, "Bob", "drawing", true),
            json!({"drawingBankId": bank,
            "exposure": [[d1, 6], [null, 2], [d2, 12], [d1, 24]], "cellNotes": {"3": "fix hand", "12": "hold"},
            "transform": transform("element"), "channels": bob_channels, "note": "main character",
            "markerColour": "#9DC3E6FF"}),
        ),
        merge(
            common(&image_el, "Background", "image", true),
            json!({"drawingBankId": image_bank,
            "exposure": [[image_drawing, 48]], "importOptions": {"premultiplied": false},
            "transform": transform("element"), "channels": {}, "role": "background"}),
        ),
        merge(
            common(&annotation, "Notes", "annotation", true),
            json!({"drawingBankId": annotation_bank,
            "exposure": [], "transform": transform("element"), "channels": {}, "visibleInXsheet": false}),
        ),
        merge(
            common(&sound, "Dialogue", "sound", true),
            json!({"streamed": false, "showMouthShapes": true,
            "placements": [{"id": ids.next(ObjectType::SoundPlacement), "type": "soundPlacement", "rev": 1,
                "assetId": sound_asset, "startFrame": 1, "stopFrame": 8, "inPoint": 0, "outPoint": 2.5, "loops": 1,
                "volume": -3, "envelope": [{"time": 0, "left": 0, "right": 0}, {"time": 0.5, "left": 1, "right": 1}],
                "mouthTrack": {"mouths": "XABBCDEX", "manual": [{"from": 3, "to": 4}], "labels": {"1": "rest"},
                               "source": "detected",
                               "producedFor": {"assetId": sound_asset, "inPoint": 0, "outPoint": 2.5, "loops": 1, "stopFrame": 8, "frameRate": 23.976}}}],
            "lipSyncMappings": [{"drawingElementId": bob, "autoReapply": true,
                "mapping": {"A": d2, "B": null, "C": null, "D": null, "E": null, "F": null, "G": null, "X": d3}}]}),
        ),
        merge(
            common(&media, "Reference", "media", true),
            json!({"sequences": [{"id": ids.next(ObjectType::MediaSequence),
            "type": "mediaSequence", "rev": 1, "assetId": movie_asset, "startFrame": 1, "stopFrame": 24, "linkOffset": 1}],
            "transform": transform("element"), "channels": {}}),
        ),
        merge(
            common(&skeleton, "Rig", "skeleton", false),
            json!({"transform": transform("skeleton"),
            "skeleton": {"boneColour": null, "showTrack": false, "showRoot": true, "showBones": true, "showInfluence": false,
                         "vectorQuality": "medium", "deformationEnabled": true}, "channels": {}}),
        ),
        merge(common(&bones_slot, "Rig-bones", "effectParameter", false), json!({"slot": "bones"})),
        merge(common(&skin_slot, "Rig-skin", "effectParameter", false), json!({"slot": "skin"})),
        merge(
            common(&bone, "Arm", "bone", false),
            json!({
            "bone": {"rest": {"x": 0, "y": 1, "rotation": 90, "length": 1.5},
                     "influence": {"type": "elliptical", "lengthRadius": 1.125, "widthRadius": 0.525, "region": [{"x": 0, "y": 0}, {"x": 1, "y": 0}, {"x": 1, "y": 1}]}},
            "channels": {"rotation": channel(&mut ids, "rotation", &[(1, 0.0)])}}),
        ),
        merge(common(&clipping, "Clip", "clipping", false), json!({"params": {"inverted": true}, "channels": {}})),
        merge(common(&mask, "Clip-mask", "effectParameter", false), json!({"slot": "mask"})),
        merge(
            common(&colour_tx, "Fade", "colourTransform", false),
            json!({
            "params": {"multRed": 1, "multGreen": 1, "multBlue": 1, "multAlpha": 0.5, "addRed": 0, "addGreen": 0, "addBlue": 0, "addAlpha": 0, "flatten": true},
            "channels": {"multAlpha": channel(&mut ids, "multAlpha", &[(1, 0.0), (12, 1.0)])}}),
        ),
        merge(
            common(&drop_shadow, "Shadow", "dropShadow", false),
            json!({
            "params": {"parentColourTransformMode": "drawingsOnly", "colour": "#000000FF", "opacity": 50, "hideElements": false,
                       "transform": {"xAxis": {"x": 1, "y": 0}, "yAxis": {"x": 0, "y": 1}, "origin": {"x": 0.2, "y": -0.2}}},
            "channels": {}}),
        ),
        merge(
            common(&blur, "Soft", "blur", false),
            json!({"params": {"radial": false, "radius": 4, "angle": 30},
            "channels": {"radius": channel(&mut ids, "radius", &[(1, 0.0), (24, 8.0)])}}),
        ),
        merge(
            common(&shake, "Quake", "shake", false),
            json!({"params": {"axis": "xy", "waveTime": 4, "decayHalfLifeFrames": 6, "amplitude": 0.5},
            "channels": {}, "loops": 2, "loopType": "fill", "positionLoop": "offset"}),
        ),
        merge(
            common(&nature, "Snow", "particleNature", false),
            json!({"params": {},
            "blocks": [block(&mut ids, "snow", 1, 24, 184_467, 1500), block(&mut ids, "fog", 25, 24, 7, 40)]}),
        ),
        merge(common(&sprites, "Snow-sprites", "effectParameter", false), json!({"slot": "sprites"})),
        merge(
            common(&impact, "Boom", "particleImpact", false),
            json!({"params": {}, "blocks": [block(&mut ids, "explosion", 1, 48, 99, 3000)]}),
        ),
    ];
    let image_bank_json = json!({"id": image_bank, "type": "drawingBank", "rev": 1, "drawings": [{"id": image_drawing, "type": "drawing"}]});
    let annotation_bank_json = json!({"id": annotation_bank, "type": "drawingBank", "rev": 1, "drawings": []});
    let image_drawing_json = json!({"id": image_drawing, "type": "drawing", "rev": 1, "name": "1", "locked": false,
        "pivot": {"x": 0, "y": 0}, "mouthShape": null, "nodes": [
            {"id": image_node, "type": "drawingNode", "rev": 1, "kind": "bitmap",
             "image": {"entry": format!("banks/{image_bank}/{image_drawing}/{image_node}.png"), "mediaType": "image/png", "size": 4096, "sha256": sha(0x22)},
             "width": 1920, "height": 1080, "transform": [1, 0, 0, 1, -960, 540]}],
        "colourRecovery": [], "createdAt": CREATED});
    let node = |id: &str, children: Vec<Value>| json!({"id": id, "children": children});
    let tree = vec![
        node(&camera, vec![]),
        node(&peg, vec![node(&bob, vec![])]),
        node(&image_el, vec![]),
        node(&annotation, vec![]),
        node(&sound, vec![]),
        node(&media, vec![]),
        node(&skeleton, vec![node(&bones_slot, vec![node(&bone, vec![])]), node(&skin_slot, vec![])]),
        node(&clipping, vec![node(&mask, vec![])]),
        node(&colour_tx, vec![node(&drop_shadow, vec![]), node(&blur, vec![node(&shake, vec![])])]),
        node(&nature, vec![node(&sprites, vec![])]),
        node(&impact, vec![]),
    ];
    let stub = |id: &str| json!({"id": id, "type": "element"});
    let all_elements = [
        &camera,
        &peg,
        &bob,
        &image_el,
        &annotation,
        &sound,
        &media,
        &skeleton,
        &bones_slot,
        &skin_slot,
        &bone,
        &clipping,
        &mask,
        &colour_tx,
        &drop_shadow,
        &blur,
        &shake,
        &nature,
        &sprites,
        &impact,
    ];
    let transition = |ids: &mut Ids| {
        json!({"id": ids.next(ObjectType::Transition), "type": "transition", "rev": 1, "transitionType": "none",
               "durationLeft": 12, "blankDuration": 0, "durationRight": 12, "colour": "#000000FF", "angle": 0, "direction": null, "size": 25})
    };
    let mut out1 = transition(&mut ids);
    if let Value::Object(m) = &mut out1 {
        m.insert("transitionType".into(), json!("clockWipe"));
        m.insert("direction".into(), json!("counterclockwise"));
        m.insert("angle".into(), json!(90));
    }
    let scene_json = json!({
        "id": scene, "type": "scene", "rev": 1, "name": "Intro", "visible": true, "backgroundColour": "#FFFFFFFF",
        "currentCameraId": camera, "paletteStyles": {pal.clone(): night, pal2.clone(): pal2_style},
        "outTransition": out1, "tree": tree,
        "columnOrder": [bob, image_el, annotation, sound, media],
        "elements": all_elements.iter().map(|e| stub(e)).collect::<Vec<_>>()
    });
    // Scene 2: minimal (a camera and an empty drawing element, as a new project's scene; 01 §3.1).
    let scene2 = ids.next(ObjectType::Scene);
    let cam2 = ids.next(ObjectType::Element);
    let el2 = ids.next(ObjectType::Element);
    let bank2 = ids.next(ObjectType::DrawingBank);
    let scene2_json = json!({
        "id": scene2, "type": "scene", "rev": 1, "name": "Scene-2", "visible": false, "backgroundColour": "#202020FF",
        "currentCameraId": cam2, "paletteStyles": {pal.clone(): day, pal2.clone(): pal2_style},
        "outTransition": transition(&mut ids), "tree": [node(&cam2, vec![]), node(&el2, vec![])],
        "columnOrder": [el2], "elements": [stub(&cam2), stub(&el2)]
    });
    let elements2 = [
        merge(
            common(&cam2, "Camera", "camera", false),
            json!({"transform": transform("camera"),
            "camera": {"fovMode": "dynamic", "fov": 60}, "channels": {"fov": channel(&mut ids, "fov", &[(1, 60.0)])}}),
        ),
        merge(
            common(&el2, "Drawing", "drawing", true),
            json!({"drawingBankId": bank2, "exposure": [], "transform": transform("element"), "channels": {}}),
        ),
    ];
    let bank2_json = json!({"id": bank2, "type": "drawingBank", "rev": 1, "drawings": []});

    // Library (08, 10 §4.17).
    let folder = ids.next(ObjectType::LibraryFolder);
    let library = json!({"id": library_id, "type": "library", "rev": 1, "name": "Local", "kind": "local", "formatVersion": "1.0",
    "children": [
        {"id": folder, "type": "libraryFolder", "rev": 1, "name": "Props", "children": [
            {"id": ids.next(ObjectType::LibraryItem), "type": "libraryItem", "rev": 1, "name": "Walk", "itemKind": "template",
             "file": "walk.animtpl", "thumbnail": null, "versionId": "v-0001", "artist": "Kai", "copyright": "(c) 2026",
             "createdAt": CREATED, "modifiedAt": CREATED, "sizeBytes": 123_456, "formatVersion": "1.0",
             "summary": [{"elementType": "drawing", "name": "Legs", "frameCount": 12, "children": []}]}
        ]},
        {"id": ids.next(ObjectType::LibraryItem), "type": "libraryItem", "rev": 1, "name": "Sky.png", "itemKind": "image",
         "file": "Sky.png", "thumbnail": {"entry": "library/thumbnails/sky.png", "mediaType": "image/png", "size": 100, "sha256": sha(0x33)}}
    ]});

    let start = transition(&mut ids);
    let project = json!({
        "id": project_id, "type": "project", "rev": 1, "name": "Sample", "createdAt": CREATED, "modifiedAt": CREATED,
        "frameRate": 23.976, "cameraWidth": 1920, "cameraHeight": 1080, "autoColourUpdate": false,
        "startTransition": start,
        "palettes": [{"id": pal, "type": "palette"}, {"id": pal2, "type": "palette"}],
        "pens": pens, "assets": assets,
        "drawingBanks": [{"id": bank, "type": "drawingBank"}, {"id": image_bank, "type": "drawingBank"},
                         {"id": annotation_bank, "type": "drawingBank"}, {"id": bank2, "type": "drawingBank"}],
        "scenes": [{"id": scene, "type": "scene"}, {"id": scene2, "type": "scene"}],
        "library": {"id": library_id, "type": "library"},
        "meta": {"client": "fixture"}
    });

    let mut errors = Vec::new();
    let project: Option<Project> = ok(&mut errors, parse(&project));
    let palettes: Vec<Palette> = [palette, palette2].iter().filter_map(|p| ok(&mut errors, parse(p))).collect();
    let banks: Vec<DrawingBank> = [bank_json, image_bank_json, annotation_bank_json, bank2_json]
        .iter()
        .filter_map(|b| ok(&mut errors, parse(b)))
        .collect();
    let drawings: Vec<Drawing> =
        [drawing1, drawing2, drawing3, image_drawing_json].iter().filter_map(|d| ok(&mut errors, parse(d))).collect();
    let scenes: Vec<Scene> = [scene_json, scene2_json].iter().filter_map(|s| ok(&mut errors, parse(s))).collect();
    let elements: Vec<Element> =
        elements.iter().chain(elements2.iter()).filter_map(|e| ok(&mut errors, parse(e))).collect();
    let library: Option<Library> = ok(&mut errors, parse(&library));
    match (project, library) {
        (Some(project), Some(library)) if errors.is_empty() => {
            Ok(DocumentParts { document_rev: 1, project, palettes, banks, drawings, scenes, elements, library })
        }
        _ => Err(errors),
    }
}

/// The sample document.
///
/// # Errors
/// Any validation failure of the fixture (a defect; covered by tests).
pub fn sample_document(seed: u64) -> Result<Document, Vec<ModelError>> {
    Document::from_parts(sample_parts(seed)?)
}
