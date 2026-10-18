//! C1.1 property tests: JSON ↔ model ↔ canonical JSON byte-identity, schema conformance of generated entities,
//! number formatting, ID and path parsing fuzz, patch application and inversion.
#![allow(clippy::unwrap_used, clippy::panic, reason = "test helpers outside #[test] functions")]

use proptest::prelude::*;
use serde_json::{Map, Number, Value, json};
use tt_model::change::{apply, diff_json, invert};
use tt_model::entities::ModelObject;
use tt_model::entities::drawing::Stroke;
use tt_model::entities::element::{Element, ElementInit, ElementType, ParamSlot};
use tt_model::entities::palette::Palette;
use tt_model::entities::pen::Pen;
use tt_model::id::{Id, IdRef};
use tt_model::object::{from_json, to_canonical, to_json};
use tt_model::path::Path;
use tt_model::validate::Check;
use tt_model::value::Timestamp;
use tt_model::{ObjectType, canonical, schema};

fn config() -> ProptestConfig {
    ProptestConfig { cases: 128, failure_persistence: None, ..ProptestConfig::default() }
}

/// Finite doubles across the whole exponent range, including integers, tiny and huge values and -0.
fn any_double() -> impl Strategy<Value = f64> {
    prop_oneof![
        any::<f64>().prop_filter("finite", |x| x.is_finite()),
        (-1_000_000i64..1_000_000).prop_map(|i| i as f64),
        (-1000.0f64..1000.0),
        Just(-0.0),
        Just(5e-324),
        Just(f64::MAX),
        Just(1e21),
        Just(1e-7),
    ]
}

/// Checks the one-way and two-way properties for an object JSON: it parses, validates against its schema,
/// and its canonical text survives parse → model → canonical byte for byte.
fn round_trip<T: ModelObject + Check>(v: &Value) -> T {
    let obj: T =
        from_json(v).unwrap_or_else(|e| panic!("{:?}\n{v:#}", e.iter().map(|x| x.to_wire()).collect::<Vec<_>>()));
    let text = to_canonical(&obj).unwrap();
    let json: Value = serde_json::from_str(&text).unwrap();
    assert!(schema::validate(T::TYPE, &json, 5).unwrap().is_empty(), "{text}");
    let again: T = from_json(&json).unwrap();
    assert_eq!(to_canonical(&again).unwrap(), text);
    assert_eq!(to_json(&again).unwrap(), to_json(&obj).unwrap());
    again
}

fn name_strategy() -> impl Strategy<Value = String> {
    // Letters, digits, spaces, punctuation allowed in every entity, and non-ASCII (NFC and NFD forms).
    "[A-Za-z0-9][A-Za-z0-9 _()&'é\u{0301}ß日本-]{0,30}[A-Za-z0-9]"
}

fn pen_json() -> impl Strategy<Value = Value> {
    (name_strategy(), 0.0f64..100.0, 0.0f64..100.0, 0u8..=10, prop::sample::select(vec!["round", "oval", "square", "flat"]), 0.0f64..360.0, any::<bool>())
        .prop_map(|(name, a, b, smooth, tip, angle, eraser)| {
            json!({"id": "pen_1", "type": "pen", "rev": 3, "name": name, "kind": if eraser { "eraser" } else { "brush" },
                   "minSize": a.min(b), "maxSize": a.max(b), "smoothness": smooth, "tip": tip, "tipAngle": angle, "colourId": null})
        })
}

fn stroke_json() -> impl Strategy<Value = Value> {
    (prop::sample::select(vec!["brush", "pencil", "invisible"]), any::<bool>(), prop::collection::vec(prop::array::uniform6(any_double()), 2..12), 0.0f64..50.0)
        .prop_map(|(kind, closed, points, thickness)| {
            let pts: Vec<Value> = points.iter().map(|p| json!(p.to_vec())).collect();
            // Brush outlines are always closed (10 §4.9, C11-R1-05).
            let closed = closed || kind == "brush";
            json!({"id": "s42", "type": "stroke", "rev": 1, "kind": kind, "closed": closed, "points": pts,
                   "colourId": if kind == "invisible" { Value::Null } else { json!("col_a") },
                   "fillMatrix": null,
                   "lineStyle": if kind == "pencil" { json!({"thickness": thickness, "lineType": "solid"}) } else { Value::Null },
                   "spine": null, "sides": {"left": null, "right": {"colourId": "col_b", "matrix": [1, 0, 0, 1, 0, 0]}}})
        })
}

fn exposure_strategy() -> impl Strategy<Value = Vec<(Option<String>, u32)>> {
    prop::collection::vec((prop::option::of(prop::sample::select(vec!["dr_a", "dr_b", "dr_c"])), 1u32..40), 0..12)
        .prop_map(|runs| runs.into_iter().map(|(d, c)| (d.map(str::to_owned), c)).collect())
}

fn element_json() -> impl Strategy<Value = Value> {
    (
        prop::sample::select(ElementType::ALL.to_vec()),
        name_strategy(),
        1u32..5,
        20u32..=1000,
        ".{0,40}",
        prop::option::of("#[0-9A-Fa-f]{6}"),
        exposure_strategy(),
        prop::collection::btree_map(1u32..200, ".{0,20}", 0..4),
        prop::collection::btree_set(1u32..500, 1..6),
    )
        .prop_map(|(t, name, loops, width, note, marker, runs, notes, key_frames)| {
            let init = ElementInit {
                id: Id::parse("el_1").unwrap(),
                element_type: t,
                name: tt_model::name::normalize(&name),
                created_at: Timestamp::from_unix_ms(1_760_000_000_000),
                drawing_bank_id: Some(Id::parse("db_1").unwrap()),
                bar: (1, 48),
                slot: Some(ParamSlot::Mask),
            };
            let mut v = serde_json::to_value(Element::new(init)).unwrap();
            let m = v.as_object_mut().unwrap();
            m.insert("loops".into(), json!(loops));
            m.insert("columnWidth".into(), json!(width));
            m.insert("note".into(), json!(note));
            m.insert("markerColour".into(), marker.map_or(Value::Null, Value::String));
            if t.has_exposure() {
                m.insert("exposure".into(), json!(runs));
                m.insert("cellNotes".into(), json!(notes.iter().map(|(k, v)| (k.to_string(), Value::String(v.clone()))).collect::<Map<String, Value>>()));
            }
            if let Some(channel) = t.allowed_channels().iter().find(|c| **c != "position") {
                let keys: Vec<Value> = key_frames
                    .iter()
                    .enumerate()
                    .map(|(i, f)| json!({"id": format!("k{i}"), "type": "key", "rev": 1, "frame": f, "value": f64::from(*f) * 0.5,
                        "constantSegment": false, "handles": {"left": {"dt": -1, "dv": 0}, "right": {"dt": 1, "dv": 0}, "mode": "broken"}, "easing": null}))
                    .collect();
                m.insert("channels".into(), json!({*channel: {"id": "ch_1", "type": "channel", "rev": 1, "name": channel, "curveType": "spline", "keys": keys}}));
            }
            v
        })
}

fn palette_json() -> impl Strategy<Value = Value> {
    (1usize..4, prop::collection::vec(("#[0-9A-F]{8}", any::<bool>()), 0..8)).prop_map(|(styles, swatches)| {
        let style_ids: Vec<String> = (0..styles).map(|i| format!("sty_{i}")).collect();
        let styles_json: Vec<Value> =
            style_ids.iter().enumerate().map(|(i, s)| json!({"id": s, "type": "paletteStyle", "rev": 1, "name": format!("S{i}")})).collect();
        let swatches_json: Vec<Value> = swatches
            .iter()
            .enumerate()
            .map(|(i, (rgba, gradient))| {
                let values: Map<String, Value> = style_ids
                    .iter()
                    .map(|s| {
                        let v = if *gradient {
                            json!({"stops": [{"position": 0, "rgba": rgba}, {"position": 1, "rgba": "#00000000"}]})
                        } else {
                            json!({"rgba": rgba})
                        };
                        (s.clone(), v)
                    })
                    .collect();
                json!({"colourId": format!("col_{i}"), "type": "swatch", "rev": 1, "name": "Same name",
                       "kind": if *gradient { "linearGradient" } else { "solid" }, "deformable": false, "values": values})
            })
            .collect();
        json!({"id": "pal_1", "type": "palette", "rev": 1, "name": "P", "styles": styles_json, "swatches": swatches_json, "hidden": false})
    })
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn numbers_round_trip_through_canonical_text(x in any_double()) {
        let s = canonical::format_f64(x).unwrap();
        let back: f64 = s.parse().unwrap();
        prop_assert_eq!(back.to_bits(), if x == 0.0 { 0.0f64.to_bits() } else { x.to_bits() });
        // Shortest: no trailing zeros in a fraction, no "+" except in exponents, no ".0".
        prop_assert!(!s.contains(".0e") && !s.ends_with(".0") && !(s.contains('.') && !s.contains('e') && s.ends_with('0')), "{}", s);
    }

    /// serde_json must parse doubles with correct rounding (`float_roundtrip`), or canonical text drifts by an ulp.
    #[test]
    fn json_parsing_of_doubles_is_correctly_rounded(x in any_double()) {
        let text = canonical::format_f64(x).unwrap();
        let parsed: f64 = serde_json::from_str(&text).unwrap();
        prop_assert_eq!(parsed.to_bits(), text.parse::<f64>().unwrap().to_bits());
    }

    #[test]
    fn canonical_json_is_stable(v in arb_json()) {
        let mut v = v;
        canonical::normalize_numbers(&mut v);
        let a = canonical::to_canonical_string(&v).unwrap();
        let mut parsed: Value = serde_json::from_str(&a).unwrap();
        canonical::normalize_numbers(&mut parsed);
        prop_assert_eq!(&parsed, &v);
        prop_assert_eq!(canonical::to_canonical_string(&parsed).unwrap(), a);
    }

    #[test]
    fn pens_round_trip(v in pen_json()) {
        let p: Pen = round_trip(&v);
        prop_assert!(p.min_size() <= p.max_size());
    }

    #[test]
    fn strokes_round_trip(v in stroke_json()) {
        let _: Stroke = round_trip(&v);
    }

    #[test]
    fn elements_of_every_type_round_trip(v in element_json()) {
        let e: Element = round_trip(&v);
        // Exposure is canonical after parsing.
        if let Some(runs) = e.exposure() {
            prop_assert_eq!(&tt_model::object::canonical_runs(runs), runs);
        }
    }

    #[test]
    fn palettes_round_trip(v in palette_json()) {
        let _: Palette = round_trip(&v);
    }

    #[test]
    fn id_parsing_never_panics_and_matches_the_grammar(s in ".{0,80}") {
        let valid = !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
        prop_assert_eq!(Id::parse(&s).is_ok(), valid);
        let _ = IdRef::parse(&s);
    }

    #[test]
    fn path_parsing_never_panics(s in ".{0,120}") {
        let _ = Path::parse(&s);
        let _ = tt_model::Ref::parse(&s);
    }

    #[test]
    fn generated_paths_print_and_parse_identically(segs in prop::collection::vec((prop::sample::select(vec!["scene", "element", "palette", "swatch", "drawingNode", "stroke", "channel", "key"]), 0u8..3, ".{1,12}", 1u32..1000), 1..6)) {
        let text: Vec<String> = segs.iter().map(|(t, k, name, n)| match k {
            0 => format!("{t}:{}", tt_model::path::quote(name)),
            1 => format!("{t}:#{n}"),
            _ => format!("{t}:@id_{n}"),
        }).collect();
        let text = text.join("/");
        let p = Path::parse(&text).unwrap();
        prop_assert_eq!(p.to_string(), text.clone());
        prop_assert_eq!(Path::parse(&p.to_string()).unwrap(), p);
    }

    #[test]
    fn timestamps_round_trip(ms in -62_135_596_800_000i64..253_402_300_799_999) {
        let t = Timestamp::from_unix_ms(ms);
        prop_assert_eq!(Timestamp::parse(&t.to_string()).unwrap(), t);
    }

    #[test]
    fn names_normalise_idempotently(s in ".{0,40}") {
        let n = tt_model::name::normalize(&s);
        prop_assert_eq!(tt_model::name::normalize(&n), n);
    }

    #[test]
    fn patches_apply_and_invert(a in arb_json(), b in arb_json()) {
        let patch = diff_json(&a, &b, "", ObjectType::Scene);
        prop_assert_eq!(&apply(&a, &patch).unwrap(), &b);
        prop_assert_eq!(&apply(&b, &invert(&patch)).unwrap(), &a);
    }

    /// C11-R2-02: random RFC 6902 sequences (add with overwrite or `-` append, remove, replace, move with shifting
    /// and overwriting destinations, copy, test) over random documents with nested arrays; the inverse computed
    /// against the pre-state restores it exactly, has only concrete paths, and is itself exactly invertible.
    #[test]
    fn random_rfc6902_sequences_invert_against_their_pre_state(doc in arb_json(), seed in any::<u64>(), n in 1usize..10) {
        let patch = random_patch(&doc, seed, n);
        let after = apply(&doc, &patch).unwrap();
        let inverse = tt_model::change::invert_against(&doc, &patch).unwrap();
        prop_assert_eq!(&apply(&after, &inverse).unwrap(), &doc, "patch {:?} inverse {:?}", patch, inverse);
        for op in &inverse {
            for p in std::iter::once(&op.path).chain(op.from.as_ref()) {
                prop_assert!(!tt_model::pointer::parse(p).unwrap().iter().any(|t| t == "-"), "`-` in inverse {:?}", op);
            }
        }
        let again = tt_model::change::invert_against(&after, &inverse).unwrap();
        prop_assert_eq!(&apply(&doc, &again).unwrap(), &after);
    }

    #[test]
    fn child_collection_patches_apply_and_invert(a in prop::collection::btree_set(0u8..20, 0..12), b in prop::collection::btree_set(0u8..20, 0..12), seed in any::<u64>()) {
        let stubs = |ids: Vec<u8>| Value::Array(ids.into_iter().map(|i| json!({"id": format!("el_{i}"), "type": "element"})).collect());
        let mut av: Vec<u8> = a.into_iter().collect();
        let mut bv: Vec<u8> = b.into_iter().collect();
        // Deterministic shuffles so moves are exercised.
        let mut rng = seed;
        for v in [&mut av, &mut bv] {
            for i in (1..v.len()).rev() {
                rng = rng.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                v.swap(i, (rng >> 33) as usize % (i + 1));
            }
        }
        let (a, b) = (json!({"elements": stubs(av)}), json!({"elements": stubs(bv)}));
        let patch = diff_json(&a, &b, "", ObjectType::Scene);
        prop_assert!(patch.iter().all(|op| op.op != "replace"), "{:?}", patch);
        prop_assert_eq!(&apply(&a, &patch).unwrap(), &b);
        prop_assert_eq!(&apply(&b, &invert(&patch)).unwrap(), &a);
    }

    #[test]
    fn exposure_range_patches_apply_and_invert(a in exposure_strategy(), b in exposure_strategy()) {
        let canon = |runs: Vec<(Option<String>, u32)>| {
            let runs: Vec<(Option<Id>, u32)> = runs.into_iter().map(|(d, c)| (d.map(|d| Id::parse(&d).unwrap()), c)).collect();
            serde_json::to_value(tt_model::object::canonical_runs(&runs)).unwrap()
        };
        let (ea, eb) = (canon(a), canon(b));
        let (va, vb) = (json!({"exposure": ea}), json!({"exposure": eb}));
        let patch = diff_json(&va, &vb, "", ObjectType::Element);
        if va != vb {
            prop_assert_eq!(patch.len(), 1);
            prop_assert!(patch[0].range.is_some());
        }
        prop_assert_eq!(&apply(&va, &patch).unwrap(), &vb);
        prop_assert_eq!(&apply(&vb, &invert(&patch)).unwrap(), &va);
    }
}

fn arb_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|i| Value::Number(i.into())),
        any::<u64>().prop_map(|u| Value::Number(u.into())),
        any_double().prop_map(|f| Number::from_f64(f).map_or(Value::Null, Value::Number)),
        ".{0,12}".prop_map(Value::String),
    ];
    leaf.prop_recursive(4, 48, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
            prop::collection::vec(("[a-z~/]{1,6}", inner), 0..6).prop_map(|kv| Value::Object(kv.into_iter().collect())),
        ]
    })
}

/// Number vectors shared with the front end: the TypeScript test checks that `JSON.stringify` writes exactly these
/// strings for the same doubles (bit patterns), so canonical numbers agree across the two languages.
#[test]
fn number_vectors_for_the_front_end() {
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut cases = Vec::new();
    for i in 0..2000 {
        let bits = next();
        let x = f64::from_bits(bits);
        let x = if x.is_finite() { x } else { (i as f64) * 0.1 };
        let x = match i % 5 {
            0 => x,
            1 => (bits % 2_000_000) as f64 - 1_000_000.0,
            2 => ((bits % 10_000_000) as f64) / 1000.0,
            3 => f64::from_bits(bits >> 1) * if bits & 1 == 1 { -1.0 } else { 1.0 },
            _ => (bits as f64) * 1e-300,
        };
        if x.is_finite() {
            cases.push(json!([format!("{:016x}", x.to_bits()), canonical::format_f64(x).unwrap()]));
        }
    }
    let text = canonical::to_canonical_string(&json!({"cases": cases})).unwrap();
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/numbers.json");
    if std::env::var("TT_UPDATE_GOLDEN").is_ok_and(|v| v == "1") {
        std::fs::write(&path, &text).unwrap();
    }
    assert_eq!(std::fs::read_to_string(&path).unwrap_or_default(), text, "run with TT_UPDATE_GOLDEN=1");
}

#[test]
fn every_object_type_has_a_schema_that_rejects_unknown_properties() {
    for t in ObjectType::ALL {
        let s = schema::object_schema(t);
        assert_eq!(s["additionalProperties"], json!(false), "{t}");
        assert!(schema::validate(t, &json!({"type": t.as_str(), "unexpected": 1}), 50).unwrap().len() > 1, "{t}");
    }
}

/// Pointers to every value of `v` (the root is `""`).
fn all_pointers(v: &Value, at: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
    out.push(at.clone());
    match v {
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                at.push(i.to_string());
                all_pointers(x, at, out);
                at.pop();
            }
        }
        Value::Object(m) => {
            for (k, x) in m {
                at.push(k.clone());
                all_pointers(x, at, out);
                at.pop();
            }
        }
        _ => {}
    }
}

/// A random valid RFC 6902 patch of up to `n` operations against `doc` (deterministic in `seed`). Operations that
/// would not apply to the current state are skipped; `oldValue` is set exactly where an operation overwrites.
fn random_patch(doc: &Value, seed: u64, n: usize) -> Vec<tt_model::change::PatchOp> {
    use tt_model::change::PatchOp;
    use tt_model::pointer;
    let mut rng = seed | 1;
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    let mut state = doc.clone();
    let mut patch = Vec::new();
    for _ in 0..n * 4 {
        if patch.len() == n {
            break;
        }
        let mut ptrs = Vec::new();
        all_pointers(&state, &mut Vec::new(), &mut ptrs);
        let pick = |r: u64| ptrs[(r % ptrs.len() as u64) as usize].clone();
        let value = |r: u64| match r % 4 {
            0 => json!(r % 100),
            1 => json!(format!("v{}", r % 7)),
            2 => json!([r % 3, [r % 5]]),
            _ => json!({"k": r % 9}),
        };
        let containers: Vec<Vec<String>> = ptrs
            .iter()
            .filter(|p| pointer::get(&state, p).is_some_and(|v| v.is_array() || v.is_object()))
            .cloned()
            .collect();
        // A destination inside a container of `s`: an existing or new member, an index or `-`.
        let dest = |s: &Value, r: u64| -> Option<(String, Option<Value>)> {
            let mut cs = Vec::new();
            all_pointers(s, &mut Vec::new(), &mut cs);
            cs.retain(|p| pointer::get(s, p).is_some_and(|v| v.is_array() || v.is_object()));
            let c = cs.get((r % cs.len().max(1) as u64) as usize)?.clone();
            let mut p = c.clone();
            match pointer::get(s, &c)? {
                Value::Array(a) => {
                    p.push(if r.is_multiple_of(3) {
                        "-".into()
                    } else {
                        ((r >> 8) % (a.len() as u64 + 1)).to_string()
                    });
                    Some((pointer::join(&p), None))
                }
                Value::Object(m) => {
                    let existing: Vec<&String> = m.keys().collect();
                    if r.is_multiple_of(2) && !existing.is_empty() {
                        let k = existing[((r >> 8) % existing.len() as u64) as usize].clone();
                        let old = m.get(&k).cloned();
                        p.push(k);
                        Some((pointer::join(&p), old))
                    } else {
                        p.push(format!("n{}/~{}", r % 5, r % 3));
                        let old = m.get(p.last()?).cloned();
                        Some((pointer::join(&p), old))
                    }
                }
                _ => None,
            }
        };
        let r = next();
        let op = |kind: &'static str, path: String, from: Option<String>, value: Option<Value>, old: Option<Value>| {
            PatchOp { op: kind, path, from, value, old_value: old, range: None, elided: false }
        };
        let candidate = match r % 6 {
            0 if !containers.is_empty() => {
                dest(&state, next()).map(|(p, old)| op("add", p, None, Some(value(next())), old))
            }
            1 => Some(pick(next())).filter(|p| !p.is_empty()).map(|p| {
                let old = pointer::get(&state, &p).cloned();
                op("remove", pointer::join(&p), None, None, old)
            }),
            2 => Some(pick(next())).map(|p| {
                let old = pointer::get(&state, &p).cloned();
                op("replace", pointer::join(&p), None, Some(value(next())), old)
            }),
            3 => {
                let f = pick(next());
                if f.is_empty() {
                    None
                } else {
                    let from = pointer::join(&f);
                    let removed = apply(&state, &[op("remove", from.clone(), None, None, None)]).ok();
                    removed.and_then(|s1| dest(&s1, next())).map(|(p, old)| op("move", p, Some(from), None, old))
                }
            }
            4 if !containers.is_empty() => {
                let f = pointer::join(pick(next()));
                dest(&state, next()).map(|(p, old)| op("copy", p, Some(f), None, old))
            }
            _ => Some(pick(next())).map(|p| {
                let current = pointer::get(&state, &p).cloned();
                op("test", pointer::join(&p), None, current, None)
            }),
        };
        if let Some(o) = candidate
            && let Ok(s) = apply(&state, std::slice::from_ref(&o))
        {
            state = s;
            patch.push(o);
        }
    }
    patch
}
