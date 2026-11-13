//! Independent analytic pixel-centre oracle for transformed scene rectangles.
#[path = "C:/Users/kaiga/work/review/m3-c33-r1/crates/executor/tests/c33_support/mod.rs"] mod c33_support;
#[path = "C:/Users/kaiga/work/review/m3-c33-r1/crates/executor/tests/common/mod.rs"] mod common;

use c33_support::{clear, render, solids};
use common::{fixture, ok};
use serde_json::json;

fn clip(poly: &[[f64; 2]], axis: usize, edge: f64, greater: bool) -> Vec<[f64; 2]> {
    let Some(mut previous) = poly.last().copied() else { return Vec::new() };
    let inside = |p: [f64; 2]| if greater { p[axis] >= edge } else { p[axis] <= edge };
    let mut out = Vec::new();
    for &point in poly {
        if inside(previous) != inside(point) {
            let t = (edge - previous[axis]) / (point[axis] - previous[axis]);
            out.push([previous[0] + t * (point[0] - previous[0]), previous[1] + t * (point[1] - previous[1])]);
        }
        if inside(point) {
            out.push(point);
        }
        previous = point;
    }
    out
}

fn covered_area(poly: &[[f64; 2]], x: u32, y: u32) -> f64 {
    let mut p = poly.to_vec();
    for (axis, edge, greater) in [
        (0, f64::from(x), true),
        (0, f64::from(x) + 1.0, false),
        (1, f64::from(y), true),
        (1, f64::from(y) + 1.0, false),
    ] {
        p = clip(&p, axis, edge, greater);
    }
    if p.len() < 3 {
        return 0.0;
    }
    p.iter().zip(p.iter().cycle().skip(1)).take(p.len()).map(|(a, b)| a[0] * b[1] - a[1] * b[0]).sum::<f64>().abs()
        * 0.5
}

#[test]
fn transformed_rectangle_centres_match_analytic_inverse() -> Result<(), Box<dyn std::error::Error>> {
    for (angle, sx, skew, dx) in [(0.0_f64, 1.0_f64, 0.0_f64, 0.0_f64), (0.0, 1.0, 0.0, 0.05), (37.0, -1.0, 13.0, 0.0)]
    {
        let h = fixture();
        clear(&h);
        let colours = solids(&h, 3);
        let colour = colours.first().ok_or("missing opaque fixture swatch")?;
        let element = ok(&h,"element.create",json!({"type":"drawing","name":"Oracle","sceneId":"scene:\"Intro\""}))["elementIds"][0].clone();
        let shape = ok(
            &h,
            "draw.rectangle.add",
            json!({"elementId":element,"frame":1,"from":{"x":-100,"y":-100},"to":{"x":100,"y":100},"thickness":0,"layer":"new"}),
        );
        ok(
            &h,
            "colour.paint.apply",
            json!({"drawingId":shape["drawingId"],"points":[{"x":0,"y":0}],"colourId":colour.0}),
        );
        let outer =
            ok(&h, "element.create", json!({"type":"peg","name":"Outer","sceneId":"scene:\"Intro\""}))["elementIds"][0]
                .clone();
        let inner =
            ok(&h, "element.create", json!({"type":"peg","name":"Inner","sceneId":"scene:\"Intro\""}))["elementIds"][0]
                .clone();
        ok(&h, "element.parent.set", json!({"elementIds":[element],"parentId":inner}));
        ok(&h, "element.parent.set", json!({"elementIds":[inner],"parentId":outer}));
        let (pivot, local) = if angle == 0.0 { ([0.0, 0.0], [0.0, 0.0]) } else { ([0.3, 0.2], [0.25, -0.1]) };
        ok(
            &h,
            "anim.transform.set",
            json!({"nodeIds":[inner],"target":"static","position":{"x":local[0],"y":local[1]}}),
        );
        ok(
            &h,
            "anim.transform.set",
            json!({"nodeIds":[outer],"target":"static","rotation":angle,"scale":{"x":sx,"y":1.0},"skew":skew,"position":{"x":dx},"pivot":{"x":pivot[0],"y":pivot[1]}}),
        );
        let image = render(&h, json!({"width":120,"height":68,"background":"transparent"}));
        println!("PIN {angle} {dx} {}", image.result["sha256"]); std::fs::write(std::env::temp_dir().join("toontown-c33-r1").join(format!("rectangle-{angle}-{dx}.png")), &image.png)?; let (sin, cos) = angle.to_radians().sin_cos();
        let shear = skew.to_radians().tan();
        let b = [cos * sx, sin * sx, cos * shear - sin, sin * shear + cos];
        // Peg default applies its pivot to rotation only (05 section 2.3).
        let t = [
            dx + pivot[0] - cos * pivot[0] + sin * pivot[1] + b[0] * local[0] + b[2] * local[1],
            pivot[1] - sin * pivot[0] - cos * pivot[1] + b[1] * local[0] + b[3] * local[1],
        ];
        let polygon: Vec<_> = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]]
            .iter()
            .map(|p| {
                [60.0 + 10.0 * (b[0] * p[0] + b[2] * p[1] + t[0]), 34.0 - 10.0 * (b[1] * p[0] + b[3] * p[1] + t[1])]
            })
            .collect();
        for (local_x, inside) in [(1.0 - 0.5 / 160.0, true), (1.0 + 0.5 / 160.0, false)] {
            let world = [b[0] * local_x + b[2] * 0.3 + t[0], b[1] * local_x + b[3] * 0.3 + t[1]];
            for (space, point) in
                [("image", [960.0 + 160.0 * world[0], 540.0 - 160.0 * world[1]]), ("camera", world), ("scene", world)]
            {
                let hits =
                    ok(&h, "inspect.hitTest", json!({"space":space,"frame":1,"point":{"x":point[0],"y":point[1]}}));
                let picked =
                    hits["hits"].as_array().ok_or("missing hits")?.iter().any(|hit| hit["elementId"] == element);
                assert_eq!(picked, inside, "{space} half-pixel boundary, angle {angle}");
            }
        }
        for space in ["drawing", "local", "parent", "scene", "camera", "cameraNormalized", "image"] {
            let converted = ok(
                &h,
                "inspect.coordinates.convert",
                json!({"from":"drawing","to":space,"elementId":element,"frame":1,"points":[{"x":25.0,"y":-50.0}]}),
            );
            let back = ok(
                &h,
                "inspect.coordinates.convert",
                json!({"from":space,"to":"drawing","elementId":element,"frame":1,"points":converted["points"]}),
            );
            let x = back["points"][0]["x"].as_f64().ok_or("missing converted x")?;
            let y = back["points"][0]["y"].as_f64().ok_or("missing converted y")?;
            assert!((x - 25.0).abs() < 1e-8 && (y + 50.0).abs() < 1e-8, "{space} round trip");
            if space == "scene" {
                let expected = [b[0] * 0.25 - b[2] * 0.5 + t[0], b[1] * 0.25 - b[3] * 0.5 + t[1]];
                for (axis, name) in ["x", "y"].iter().enumerate() {
                    assert!(
                        (converted["points"][0][name].as_f64().ok_or("missing scene coordinate")? - expected[axis])
                            .abs()
                            < 1e-10
                    );
                }
            }
        }
        for y in 0..68 {
            for x in 0..120 {
                let want = (covered_area(&polygon, x, y).clamp(0.0, 1.0) * 255.0).round() as i16;
                assert!(
                    (i16::from(image.at(x, y)[3]) - want).abs() <= 2,
                    "polygon coverage {angle} {x},{y}: {} vs {want}",
                    image.at(x, y)[3]
                );
            }
        }
        let description = ok(&h, "inspect.frame.describe", json!({"frame":1,"detail":"full"}));
        let entries = description["elements"].as_array().ok_or("missing description elements")?;
        let entry = entries.iter().find(|e| e["elementId"] == element).ok_or("oracle element omitted")?;
        // Three projected drawing points determine the affine. Description uses native 1920x1080.
        for (u, v) in [(0.0, 0.0), (100.0, 0.0), (0.0, 100.0)] {
            let m = entry["drawingToImage"].as_array().ok_or("missing drawingToImage")?;
            let f = |i: usize| m[i].as_f64().ok_or("nonnumeric affine");
            let got = [f(0)? * u + f(2)? * v + f(4)?, f(1)? * u + f(3)? * v + f(5)?];
            let rx = sx * u + skew.to_radians().tan() * v;
            let want =
                [960.0 + 1.6 * (cos * rx - sin * v) + 160.0 * t[0], 540.0 - 1.6 * (sin * rx + cos * v) - 160.0 * t[1]];
            for axis in 0..2 {
                assert!((got[axis] - want[axis]).abs() < 1e-8, "three-point affine {got:?} != {want:?}");
            }
        }
        for y in 0..68 {
            for x in 0..120 {
                // Horizontal camera field spans exactly 12 fields. Invert T R K S, independently.
                let px = (f64::from(x) + 0.5 - 60.0) / 10.0 - t[0];
                let py = (34.0 - f64::from(y) - 0.5) / 10.0 - t[1];
                let ry = -sin * px + cos * py;
                let rx = (cos * px + sin * py - skew.to_radians().tan() * ry) / sx;
                // Check colour away from AA boundaries; coverage is checked independently above at every pixel.
                if rx.abs() < 0.8 && ry.abs() < 0.8 {
                    assert_eq!(image.at(x, y), colour.1, "interior {x},{y}");
                } else if rx.abs() > 1.2 || ry.abs() > 1.2 {
                    assert_eq!(image.at(x, y), [0; 4], "exterior {x},{y}");
                }
            }
        }
        if angle == 0.0 {
            let left = 50.0 + dx * 10.0;
            let right = 70.0 + dx * 10.0;
            for y in 0..68 {
                for x in 0..120 {
                    let coverage = (right.min(f64::from(x) + 1.0) - left.max(f64::from(x))).clamp(0.0, 1.0)
                        * (44.0_f64.min(f64::from(y) + 1.0) - 24.0_f64.max(f64::from(y))).clamp(0.0, 1.0);
                    let want = (coverage * 255.0).round() as i16;
                    assert!((i16::from(image.at(x, y)[3]) - want).abs() <= 1, "exact rectangle area {x},{y}");
                }
            }
        }
    }
    Ok(())
}

#[test]
fn empty_transparent_and_opaque_frames_are_exact() {
    let h = fixture();
    clear(&h);
    // History-pinned from independently constructed constant RGBA buffers; never re-recorded by an environment flag.
    for (background, expected, sha) in [
        ("transparent", [0, 0, 0, 0], "43b6c3ea64e44fb4b2195b086533047442b635f2cebeb6ecf52e21c1d826f238"),
        ("#123456FF", [18, 52, 86, 255], "187b989d25ae7ae51211c215bf156816e96854313178964ff70460245d432442"),
    ] {
        let image = render(&h, json!({"width":32,"height":18,"background":background}));
        assert!(image.rgba.as_chunks::<4>().0.iter().all(|p| *p == expected));
        assert_eq!(image.result["sha256"], sha, "reviewer golden {background}");
    }
}

#[test]
fn rolled_dynamic_camera_and_near_plane_match_polygon_oracle() -> Result<(), Box<dyn std::error::Error>> {
    let h = fixture();
    clear(&h);
    let colour = solids(&h, 1)[0].0.clone();
    let element = ok(
        &h,
        "element.create",
        json!({"type":"drawing","name":"Camera oracle","sceneId":"scene:\"Intro\""}),
    )["elementIds"][0]
        .clone();
    let shape = ok(
        &h,
        "draw.rectangle.add",
        json!({"elementId":element,"frame":1,"from":{"x":-100,"y":-100},"to":{"x":100,"y":100},"thickness":0,"layer":"new"}),
    );
    ok(&h, "colour.paint.apply", json!({"drawingId":shape["drawingId"],"points":[{"x":0,"y":0}],"colourId":colour}));
    let drawing = common::get(&h, shape["drawingId"].as_str().ok_or("missing drawing id")?);
    ok(
        &h,
        "xsheet.exposures.set",
        json!({"columns":[{"elementId":element,"from":1,"to":24,"exposures":[{"drawing":drawing["name"],"from":1,"to":24}]}]}),
    );
    let camera = "scene:\"Intro\"/element:\"Camera\"";
    ok(&h, "anim.camera.set", json!({"cameraId":camera,"roll":17,"fovMode":"dynamic","fov":35,"frame":1}));
    ok(&h, "anim.camera.set", json!({"cameraId":camera,"fov":55,"frame":13}));
    let image = render(&h, json!({"frame":7,"width":120,"height":68,"background":"transparent"}));
    let scale = 120.0 / (32.0 * (22.5_f64.to_radians().tan()));
    let (s, c) = 17.0_f64.to_radians().sin_cos();
    let poly: Vec<_> = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]]
        .iter()
        .map(|p| [60.0 + scale * (c * p[0] + s * p[1]), 34.0 - scale * (-s * p[0] + c * p[1])])
        .collect();
    for y in 0..68 {
        for x in 0..120 {
            let want = (covered_area(&poly, x, y).clamp(0.0, 1.0) * 255.0).round() as i16;
            assert!((i16::from(image.at(x, y)[3]) - want).abs() <= 2, "rolled FOV coverage {x},{y}");
        }
    }
    ok(&h, "anim.transform.set", json!({"nodeIds":[element],"target":"static","position":{"z":15.95}}));
    let clipped = render(&h, json!({"frame":7,"width":120,"height":68,"background":"transparent"}));
    assert!(clipped.rgba.iter().all(|v| *v == 0));
    Ok(())
}

#[test]
fn disc_pixel_centres_match_radius() {
    let h = fixture();
    clear(&h);
    let colour = solids(&h, 1)[0].clone();
    let element = ok(&h, "element.create", json!({"type":"drawing","name":"Disc oracle","sceneId":"scene:\"Intro\""}))
        ["elementIds"][0]
        .clone();
    let shape = ok(
        &h,
        "draw.ellipse.add",
        json!({"elementId":element,"frame":1,"from":{"x":-100,"y":-100},"to":{"x":100,"y":100},"thickness":0,"layer":"new"}),
    );
    ok(&h, "colour.paint.apply", json!({"drawingId":shape["drawingId"],"points":[{"x":0,"y":0}],"colourId":colour.0}));
    let image = render(&h, json!({"width":120,"height":68,"background":"transparent"}));
    for y in 0..68 {
        for x in 0..120 {
            let distance = ((f64::from(x) + 0.5 - 60.0).powi(2) + (f64::from(y) + 0.5 - 34.0).powi(2)).sqrt();
            // One pixel straddles the analytic disc boundary; avoid asserting binary coverage there.
            if distance < 9.0 {
                assert_eq!(image.at(x, y), colour.1);
            }
            if distance > 11.0 {
                assert_eq!(image.at(x, y), [0; 4]);
            }
        }
    }
}

#[test]
fn roles_hidden_ancestors_empty_exposures_and_background_extension_have_expected_pixels() {
    let h = fixture();
    clear(&h);
    let colours = solids(&h, 3);
    let bg = c33_support::square(&h, "Oracle BG", 100.0, &colours[0].0);
    let normal = c33_support::square(&h, "Oracle normal", 100.0, &colours[1].0);
    let fg = c33_support::square(&h, "Oracle FG", 100.0, &colours[2].0);
    ok(&h, "element.properties.set", json!({"elementId":bg,"role":"background"}));
    ok(&h, "element.properties.set", json!({"elementId":fg,"role":"foreground"}));
    ok(&h, "anim.transform.set", json!({"nodeIds":[bg],"target":"static","position":{"z":8}}));
    ok(&h, "anim.transform.set", json!({"nodeIds":[fg],"target":"static","position":{"z":-8}}));
    let centre =
        |frame| render(&h, json!({"frame":frame,"width":120,"height":68,"background":"transparent"})).at(60, 34);
    assert_eq!(centre(1), colours[2].1, "foreground outranks Z");
    ok(&h, "element.visibility.set", json!({"elementIds":[fg],"scope":"scene","visible":false}));
    assert_eq!(centre(1), colours[1].1, "normal outranks background Z");
    ok(&h, "element.visibility.set", json!({"elementIds":[normal],"scope":"scene","visible":false}));
    let drawing = c33_support::drawing_at(&h, &bg, 1);
    let name = common::get(&h, &drawing)["name"].clone();
    ok(
        &h,
        "xsheet.exposures.set",
        json!({"columns":[{"elementId":bg,"from":1,"to":48,"exposures":[{"drawing":name,"from":1,"to":1}]}]}),
    );
    assert_eq!(centre(24), colours[0].1, "background extends its last exposure");
    ok(&h, "element.properties.set", json!({"elementId":bg,"role":"normal"}));
    assert_eq!(centre(24), [0; 4], "normal empty exposure stays empty");
    let peg=ok(&h,"element.create",json!({"type":"peg","name":"Hidden oracle","sceneId":"scene:\"Intro\""}))["elementIds"][0].clone();
    ok(&h, "element.parent.set", json!({"elementIds":[bg],"parentId":peg}));
    ok(&h, "element.visibility.set", json!({"elementIds":[peg],"scope":"scene","visible":false}));
    assert_eq!(centre(1), [0; 4], "hidden ancestor removes content");
}

#[test]
fn contact_sheet_cells_and_metadata_match_single_frames() -> Result<(), Box<dyn std::error::Error>> {
    let h = fixture();
    let sheet =
        c33_support::decode(ok(&h, "inspect.contactSheet.render", json!({"frames":[1,2,3,4,5],"cellWidth":32})));
    assert_eq!((sheet.w, sheet.h), (112, 76), "ceil sqrt default columns, deterministic label bands");
    assert_eq!(sheet.result["bytes"], sheet.png.len());
    use sha2::Digest;
    let sha = sha2::Sha256::digest(&sheet.png).iter().map(|b| format!("{b:02x}")).collect::<String>();
    assert_eq!(sheet.result["sha256"], sha);
    assert_eq!(sheet.result["documentRevision"], common::rev(&h));
    let cells = sheet.result["cells"].as_array().ok_or("missing cells")?;
    assert_eq!(cells.len(), 5);
    for cell in cells {
        let frame = cell["frame"].as_u64().ok_or("missing frame")?;
        let x = cell["x"].as_u64().ok_or("missing x")? as u32;
        let y = cell["y"].as_u64().ok_or("missing y")? as u32;
        let single = render(&h, json!({"frame":frame,"width":32,"height":18}));
        assert_eq!((cell["w"].as_u64(), cell["h"].as_u64()), (Some(32), Some(18)));
        for yy in 0..18 {
            for xx in 0..32 {
                assert_eq!(sheet.at(x + xx, y + yy), single.at(xx, yy), "cell {frame} {xx},{yy}");
            }
        }
    }
    Ok(())
}
