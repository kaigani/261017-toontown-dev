//! M4 C4.2b, the canvas format TTRD 1.0 (design note 1 §2 and §8): the synthetic half of the golden corpus, the
//! header-field and record-kind matrices, the validation matrix (V1 to V10), and the tessellation's geometry.
//!
//! The corpus lives in `desktop/src/canvas/testdata/` and is decoded by Vitest too. `TT_UPDATE_CANVAS_CORPUS=1`
//! rewrites it; otherwise every file is compared byte for byte.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "tests")]

use std::path::PathBuf;

use serde_json::{Value, json};
use tt_render::canvas::{
    self, CanvasDrawing, CanvasItem, FormatError, Identity, Record, Slot, SlotKind, Vertex, decode, encode,
};
use tt_render::{El, Item, Paint, Rule};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../desktop/src/canvas/testdata")
}

/// Writes (when updating) or compares one corpus file.
fn corpus(name: &str, bytes: &[u8]) {
    let path = testdata().join(name);
    if std::env::var_os("TT_UPDATE_CANVAS_CORPUS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
    } else {
        let committed =
            std::fs::read(&path).unwrap_or_else(|e| panic!("{name}: {e} (TT_UPDATE_CANVAS_CORPUS=1 writes it)"));
        assert!(committed == bytes, "{name}: the encoded bytes differ from the committed corpus file");
    }
}

/// The JSON form of a drawing the Vitest decoder compares with (numbers as the f32 values they are).
pub fn to_json(d: &CanvasDrawing) -> Value {
    let verts = |v: &[Vertex]| v.iter().map(|v| json!([v.x, v.y, v.slot, v.flags])).collect::<Vec<_>>();
    json!({
        "generation": d.identity.generation.to_string(),
        "rev": d.identity.rev.to_string(),
        "drawingId": d.identity.drawing_id,
        "bucket": d.identity.bucket,
        "flags": d.flags,
        "bounds": d.bounds,
        "slots": d.slots.iter().map(|s| json!({
            "id": s.id,
            "kind": match s.kind { SlotKind::Solid => "solid", SlotKind::Gradient => "gradient", SlotKind::Texture => "texture" },
            "rgba": s.rgba,
        })).collect::<Vec<_>>(),
        "records": d.records.iter().map(|r| match r {
            Record::Solid { vertices, indices } => json!({ "kind": "solid", "vertices": verts(vertices), "indices": indices }),
            Record::Gradient { radial, drawing_to_unit, stops, vertices, indices } => json!({
                "kind": "gradient", "radial": radial, "drawingToUnit": drawing_to_unit,
                "stops": stops.iter().map(|(p, c)| json!([p, c])).collect::<Vec<_>>(),
                "vertices": verts(vertices), "indices": indices,
            }),
            Record::Bitmap { width, height, quad, image_id, slot } => json!({
                "kind": "bitmap", "width": width, "height": height, "quad": quad, "imageId": image_id, "slot": slot,
            }),
        }).collect::<Vec<_>>(),
    })
}

fn corpus_json(name: &str, v: &Value) {
    let mut text = serde_json::to_string_pretty(v).unwrap();
    text.push('\n');
    corpus(name, text.as_bytes());
}

const ULID: &str = "01K7SYNTHET1C0000000000000";

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<El> {
    vec![El::MoveTo(x0, y0), El::LineTo(x1, y0), El::LineTo(x1, y1), El::LineTo(x0, y1), El::Close]
}

/// A drawing whose items reach every record kind, both fill rules, a stroke with caps and joins, a curve and a layer.
fn synthetic_items() -> Vec<CanvasItem> {
    let ring = |cx: f64, cy: f64, r: f64| {
        vec![
            El::MoveTo(cx + r, cy),
            El::CubicTo(cx + r, cy + r * 0.55, cx + r * 0.55, cy + r, cx, cy + r),
            El::CubicTo(cx - r * 0.55, cy + r, cx - r, cy + r * 0.55, cx - r, cy),
            El::CubicTo(cx - r, cy - r * 0.55, cx - r * 0.55, cy - r, cx, cy - r),
            El::CubicTo(cx + r * 0.55, cy - r, cx + r, cy - r * 0.55, cx + r, cy),
            El::Close,
        ]
    };
    let mut donut = rect(0.0, 0.0, 40.0, 40.0);
    donut.extend(rect(10.0, 10.0, 30.0, 30.0));
    vec![
        // An even-odd ring with a hole (a planar fill zone).
        CanvasItem {
            item: Item::Fill { path: donut, rule: Rule::EvenOdd, paint: Paint::Solid([200, 40, 40, 255]) },
            slot: 0,
            bitmap: None,
        },
        // A curved non-zero outline (a brush stroke).
        CanvasItem {
            item: Item::Fill {
                path: ring(60.0, 20.0, 12.0),
                rule: Rule::NonZero,
                paint: Paint::Solid([40, 160, 40, 200]),
            },
            slot: 1,
            bitmap: None,
        },
        // A pencil line with a corner: round caps, a round join.
        CanvasItem {
            item: Item::Stroke {
                path: vec![El::MoveTo(0.0, 60.0), El::LineTo(30.0, 60.0), El::LineTo(30.0, 90.0)],
                width: 4.0,
                paint: Paint::Solid([20, 20, 20, 255]),
            },
            slot: 2,
            bitmap: None,
        },
        // A linear gradient fill.
        CanvasItem {
            item: Item::Fill {
                path: rect(50.0, 50.0, 90.0, 70.0),
                rule: Rule::NonZero,
                paint: Paint::Linear {
                    stops: vec![(0.0, [255, 0, 0, 255]), (1.0, [0, 0, 255, 255])],
                    unit_to_pixel: [40.0, 0.0, 0.0, 20.0, 50.0, 50.0],
                },
            },
            slot: 3,
            bitmap: None,
        },
        // A radial gradient fill inside a layer (scaled by 2).
        CanvasItem {
            item: Item::Layer {
                transform: [2.0, 0.0, 0.0, 2.0, 0.0, 0.0],
                items: vec![Item::Fill {
                    path: rect(50.0, 0.0, 60.0, 10.0),
                    rule: Rule::NonZero,
                    paint: Paint::Radial {
                        stops: vec![(0.0, [255, 255, 0, 255]), (0.5, [0, 255, 0, 128]), (1.0, [0, 0, 0, 0])],
                        unit_to_pixel: [10.0, 0.0, 0.0, 10.0, 50.0, 0.0],
                    },
                }],
            },
            slot: 4,
            bitmap: None,
        },
        // A bitmap node.
        CanvasItem {
            item: Item::Fill {
                path: rect(0.0, 100.0, 16.0, 108.0),
                rule: Rule::NonZero,
                paint: Paint::Image {
                    rgba: std::sync::Arc::new(vec![10u8; 4 * 16 * 8]),
                    width: 16,
                    height: 8,
                    unit_to_pixel: [16.0, 0.0, 0.0, 8.0, 0.0, 100.0],
                    repeat: false,
                },
            },
            slot: 5,
            bitmap: Some("bitmaps/01K7SYNTHET1CB1TMAP0000000.png".into()),
        },
    ]
}

fn synthetic() -> CanvasDrawing {
    let records = canvas::tessellate(&synthetic_items(), 0);
    CanvasDrawing {
        identity: Identity { generation: 3, rev: 42, drawing_id: ULID.into(), bucket: 0 },
        flags: 0,
        slots: vec![
            Slot { id: "co_red".into(), kind: SlotKind::Solid, rgba: [200, 40, 40, 255] },
            Slot { id: "co_green".into(), kind: SlotKind::Solid, rgba: [40, 160, 40, 200] },
            Slot { id: "co_ink".into(), kind: SlotKind::Solid, rgba: [20, 20, 20, 255] },
            Slot { id: "co_sky".into(), kind: SlotKind::Gradient, rgba: [255, 0, 0, 255] },
            Slot { id: "co_glow".into(), kind: SlotKind::Gradient, rgba: [255, 255, 0, 255] },
            Slot {
                id: "bitmaps/01K7SYNTHET1CB1TMAP0000000.png".into(),
                kind: SlotKind::Texture,
                rgba: [10, 10, 10, 10],
            },
        ],
        bounds: canvas::bounds(&records),
        records,
    }
}

#[test]
fn t4_the_synthetic_drawing_reaches_every_record_kind_and_round_trips_byte_for_byte() {
    let d = synthetic();
    let kinds: Vec<&str> = d
        .records
        .iter()
        .map(|r| match r {
            Record::Solid { .. } => "solid",
            Record::Gradient { .. } => "gradient",
            Record::Bitmap { .. } => "bitmap",
        })
        .collect();
    // Painter order: the three solid items share one mesh, then two gradients, then the bitmap.
    assert_eq!(kinds, ["solid", "gradient", "gradient", "bitmap"]);
    let bytes = encode(&d).unwrap();
    let back = decode(&bytes).unwrap();
    assert_eq!(back, d, "decode(encode(d)) == d");
    assert_eq!(encode(&back).unwrap(), bytes, "encode(decode(bytes)) == bytes");
    corpus("synthetic.ttrd", &bytes);
    corpus_json("synthetic.json", &to_json(&d));
}

#[test]
fn every_header_field_is_carried_through_encode_and_decode() {
    // Each field takes a value other than the synthetic drawing's, one at a time; a decoder that ignored or misplaced a
    // field fails its cell.
    let base = synthetic();
    type Edit = fn(&mut CanvasDrawing);
    let cells: [(&str, Edit); 7] = [
        ("generation", |d| d.identity.generation = u64::MAX - 1),
        ("rev", |d| d.identity.rev = 1 << 40),
        ("drawingId", |d| d.identity.drawing_id = "7ZZZZZZZZZZZZZZZZZZZZZZZZZ".into()),
        ("bucket", |d| d.identity.bucket = -3),
        ("flags", |d| d.flags = 0x8001),
        ("bounds", |d| d.bounds = [-1.5, -2.5, 1e6, 3.25]),
        ("slots", |d| d.slots[0].rgba = [1, 2, 3, 4]),
    ];
    let mut files = Vec::new();
    for (name, edit) in cells {
        let mut d = base.clone();
        edit(&mut d);
        assert_ne!(d, base, "{name}: the cell changes the field");
        let bytes = encode(&d).unwrap();
        let back = decode(&bytes).unwrap();
        assert_eq!(back, d, "{name}");
        let file = format!("fields/{name}.ttrd");
        corpus(&file, &bytes);
        files.push(json!({ "file": file, "field": name, "expected": to_json(&d) }));
    }
    corpus_json("fields.json", &Value::Array(files));
}

/// Offsets of the first solid record's parts in the synthetic payload.
struct Layout {
    record: usize,
    first_vertex: usize,
    first_index: usize,
    vertex_count: u32,
}

fn layout(b: &[u8], d: &CanvasDrawing) -> Layout {
    let mut at = canvas::HEADER_LEN;
    for s in &d.slots {
        at += 8 + s.id.len().div_ceil(4) * 4;
    }
    let record = at;
    let vertex_count = u32::from_le_bytes(b[at + 8..at + 12].try_into().unwrap());
    let first_vertex = at + 16;
    Layout {
        record,
        first_vertex,
        first_index: first_vertex + vertex_count as usize * canvas::VERTEX_LEN,
        vertex_count,
    }
}

#[test]
fn every_validation_rule_refuses_its_corrupt_file_and_the_unmodified_file_decodes() {
    let d = synthetic();
    let good = encode(&d).unwrap();
    let l = layout(&good, &d);
    let set = |b: &mut Vec<u8>, at: usize, v: &[u8]| b[at..at + v.len()].copy_from_slice(v);
    type Corrupt = Box<dyn Fn(&mut Vec<u8>)>;
    let cells: Vec<(&str, &str, Corrupt)> = vec![
        ("V1", "magic", Box::new(|b| b[0] = b'X')),
        ("V2", "major", Box::new(move |b| set(b, 4, &2u16.to_le_bytes()))),
        ("V3", "header-length", Box::new(move |b| set(b, 8, &64u16.to_le_bytes()))),
        ("V3", "short-header", Box::new(|b| b.truncate(40))),
        ("V4", "vertex-count", Box::new(move |b| set(b, l.record + 8, &(l.vertex_count - 1).to_le_bytes()))),
        (
            "V5",
            "truncated",
            Box::new(|b| {
                let n = b.len() - 4;
                b.truncate(n);
            }),
        ),
        ("V5", "overlong", Box::new(|b| b.extend_from_slice(&[0, 0, 0, 0]))),
        ("V6", "index", Box::new(move |b| set(b, l.first_index, &l.vertex_count.to_le_bytes()))),
        ("V7", "slot", Box::new(move |b| set(b, l.first_vertex + 8, &6u16.to_le_bytes()))),
        ("V8", "record-kind", Box::new(move |b| set(b, l.record, &9u16.to_le_bytes()))),
        ("V9", "non-finite", Box::new(move |b| set(b, l.first_vertex, &f32::NAN.to_le_bytes()))),
        ("V10", "drawing-id", Box::new(|b| b[32] = b'U')),
    ];
    assert_eq!(decode(&good).unwrap(), d, "the unmodified file decodes");
    let mut index = vec![json!({ "file": "synthetic.ttrd", "rule": null })];
    for (rule, name, corrupt) in &cells {
        let mut b = good.clone();
        corrupt(&mut b);
        let got: Result<CanvasDrawing, FormatError> = decode(&b);
        let e = got.expect_err(name);
        assert_eq!(e.rule, *rule, "{name}: {e}");
        let file = format!("corrupt/{rule}-{name}.ttrd");
        corpus(&file, &b);
        index.push(json!({ "file": file, "rule": rule }));
    }
    for rule in ["V1", "V2", "V3", "V4", "V5", "V6", "V7", "V8", "V9", "V10"] {
        assert!(cells.iter().any(|c| c.0 == rule), "{rule} has a cell");
    }
    corpus_json("corrupt.json", &Value::Array(index));
}

fn mesh_area(vertices: &[Vertex], indices: &[u32]) -> f64 {
    indices
        .chunks(3)
        .map(|t| {
            let (a, b, c) = (vertices[t[0] as usize], vertices[t[1] as usize], vertices[t[2] as usize]);
            ((f64::from(b.x) - f64::from(a.x)) * (f64::from(c.y) - f64::from(a.y))
                - (f64::from(c.x) - f64::from(a.x)) * (f64::from(b.y) - f64::from(a.y)))
            .abs()
                / 2.0
        })
        .sum()
}

fn solid(records: &[Record]) -> (Vec<Vertex>, Vec<u32>) {
    match &records[0] {
        Record::Solid { vertices, indices } => (vertices.clone(), indices.clone()),
        other => panic!("{other:?}"),
    }
}

#[test]
fn fills_follow_their_rule_and_strokes_are_a_union_with_round_caps_and_joins() {
    let one = |item: Item| vec![CanvasItem { item, slot: 0, bitmap: None }];
    let mut donut = rect(0.0, 0.0, 40.0, 40.0);
    donut.extend(rect(10.0, 10.0, 30.0, 30.0));
    // Even-odd: the hole is empty (1600 − 400); non-zero with both rings counter-clockwise: no hole.
    let (v, i) = solid(&canvas::tessellate(
        &one(Item::Fill { path: donut.clone(), rule: Rule::EvenOdd, paint: Paint::Solid([0; 4]) }),
        0,
    ));
    assert!((mesh_area(&v, &i) - 1200.0).abs() < 1e-3, "{}", mesh_area(&v, &i));
    let (v, i) = solid(&canvas::tessellate(
        &one(Item::Fill { path: donut, rule: Rule::NonZero, paint: Paint::Solid([0; 4]) }),
        0,
    ));
    assert!((mesh_area(&v, &i) - 1600.0).abs() < 1e-3, "{}", mesh_area(&v, &i));
    // A straight stroke of length 100 and width 10: 1000 plus two half discs (π·25), and no triangle counted twice.
    let line = Item::Stroke {
        path: vec![El::MoveTo(0.0, 0.0), El::LineTo(100.0, 0.0)],
        width: 10.0,
        paint: Paint::Solid([0; 4]),
    };
    let (v, i) = solid(&canvas::tessellate(&one(line), 0));
    let want = 1000.0 + std::f64::consts::PI * 25.0;
    assert!((mesh_area(&v, &i) - want).abs() < want * 0.01, "{} vs {want}", mesh_area(&v, &i));
    assert!(v.iter().all(|v| v.flags == canvas::VERTEX_STROKE), "stroke vertices carry the stroke flag");
    // A right-angle corner: the round join covers the outer corner's quarter disc (π·25/4), minus nothing twice.
    let corner = Item::Stroke {
        path: vec![El::MoveTo(0.0, 0.0), El::LineTo(50.0, 0.0), El::LineTo(50.0, 50.0)],
        width: 10.0,
        paint: Paint::Solid([0; 4]),
    };
    let (v, i) = solid(&canvas::tessellate(&one(corner), 0));
    let want = 2.0 * 500.0 - 25.0 + std::f64::consts::PI * 25.0 + std::f64::consts::PI * 25.0 / 4.0;
    assert!((mesh_area(&v, &i) - want).abs() < want * 0.01, "{} vs {want}", mesh_area(&v, &i));
    // A finer bucket flattens a curve into more vertices (SI-137).
    let circle = |t: i16| {
        let r = 50.0;
        let path = vec![
            El::MoveTo(r, 0.0),
            El::CubicTo(r, r * 0.55, r * 0.55, r, 0.0, r),
            El::CubicTo(-r * 0.55, r, -r, r * 0.55, -r, 0.0),
            El::CubicTo(-r, -r * 0.55, -r * 0.55, -r, 0.0, -r),
            El::CubicTo(r * 0.55, -r, r, -r * 0.55, r, 0.0),
            El::Close,
        ];
        solid(&canvas::tessellate(&one(Item::Fill { path, rule: Rule::NonZero, paint: Paint::Solid([0; 4]) }), t))
            .0
            .len()
    };
    assert!(circle(3) > circle(0) && circle(0) > circle(-3), "{} {} {}", circle(-3), circle(0), circle(3));
}

#[test]
fn buckets_and_tolerances() {
    assert_eq!(canvas::bucket_of(1.0), 0);
    assert_eq!(canvas::bucket_of(1.5), 1);
    assert_eq!(canvas::bucket_of(0.25), -2);
    assert_eq!(canvas::bucket_of(64.0), 6);
    assert_eq!(canvas::bucket_of(1e9), 8);
    assert_eq!(canvas::bucket_of(f64::NAN), 0);
    assert!((canvas::tolerance(0) - 0.25).abs() < 1e-12);
    assert!((canvas::tolerance(2) - 0.0625).abs() < 1e-12);
}
