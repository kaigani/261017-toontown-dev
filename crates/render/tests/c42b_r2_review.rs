//! Independent decoder derived only from docs/canvas-format.md.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review tests")]
use std::{fs, path::Path};
type R<T> = Result<T, &'static str>;
#[test]
fn five_fresh_encodings() {
    use tt_render::canvas::{self, CanvasDrawing, Identity, Record, Slot, SlotKind, Vertex};
    let vertices = vec![
        Vertex { x: 0.0, y: 0.0, slot: 0, flags: 0 },
        Vertex { x: 10.0, y: 0.0, slot: 0, flags: 0 },
        Vertex { x: 0.0, y: 10.0, slot: 0, flags: 0 },
    ];
    let solid = Record::Solid { vertices: vertices.clone(), indices: vec![0, 1, 2] };
    let gradient = |radial| Record::Gradient {
        radial,
        drawing_to_unit: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        stops: vec![(0.0, [0, 0, 0, 255]), (1.0, [255; 4])],
        vertices: vertices.clone(),
        indices: vec![0, 1, 2],
    };
    let bitmap = Record::Bitmap {
        width: 10,
        height: 10,
        quad: [0.0, 10.0, 10.0, 10.0, 10.0, 0.0, 0.0, 0.0],
        image_id: "image".into(),
        slot: 0,
    };
    for records in [vec![], vec![solid], vec![gradient(false)], vec![gradient(true)], vec![bitmap]] {
        let d = CanvasDrawing {
            identity: Identity {
                generation: u64::MAX,
                rev: (1 << 53) + 1,
                drawing_id: "01K7SYNTHET1C0000000000000".into(),
                bucket: 0,
            },
            flags: 0,
            slots: vec![Slot { id: "colour".into(), kind: SlotKind::Solid, rgba: [255; 4] }],
            bounds: canvas::bounds(&records),
            records,
        };
        assert_eq!(decode(&canvas::encode(&d).unwrap()).unwrap(), (u64::MAX, (1 << 53) + 1));
    }
}
struct Reader<'a> {
    b: &'a [u8],
    p: usize,
    rule: &'static str,
}
impl Reader<'_> {
    fn take(&mut self, n: usize) -> R<&[u8]> {
        let end = self.p.checked_add(n).ok_or(self.rule)?;
        let v = self.b.get(self.p..end).ok_or(self.rule)?;
        self.p = end;
        Ok(v)
    }
    fn u16(&mut self) -> R<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> R<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn float(&mut self) -> R<()> {
        if f32::from_bits(self.u32()?).is_finite() { Ok(()) } else { Err("V9") }
    }
}
pub(crate) fn decode(b: &[u8]) -> R<(u64, u64)> {
    if b.get(..4) != Some(b"TTRD") {
        return Err("V1");
    }
    if b.len() < 8 {
        return Err("V3");
    }
    if b[4..6] != [1, 0] {
        return Err("V2");
    }
    if b.len() < 80 || b[8..10] != [80, 0] {
        return Err("V3");
    }
    if !b[32..58].iter().all(|x| b"0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(x)) {
        return Err("V10");
    }
    let mut r = Reader { b, p: 64, rule: "V5" };
    for _ in 0..4 {
        r.float()?;
    }
    let slots = u32::from_le_bytes(b[12..16].try_into().unwrap());
    for _ in 0..slots {
        let n = usize::from(r.u16()?);
        let k = r.take(1)?[0];
        r.take(5)?;
        r.take((n + 3) & !3)?;
        if k > 2 {
            return Err("V8");
        }
    }
    let records = u32::from_le_bytes(b[60..64].try_into().unwrap());
    for _ in 0..records {
        let start = r.p;
        let kind = r.u16()?;
        r.u16()?;
        let n = r.u32()? as usize;
        if n < 8 {
            return Err("V5");
        }
        let end = start.checked_add(n).ok_or("V5")?;
        let body = b.get(r.p..end).ok_or("V5")?;
        r.p = end;
        if !(1..=3).contains(&kind) {
            return Err("V8");
        }
        let mut q = Reader { b: body, p: 0, rule: "V4" };
        if kind == 3 {
            q.take(4)?;
            for _ in 0..8 {
                q.float()?;
            }
            let l = usize::from(q.u16()?);
            if u32::from(q.u16()?) >= slots {
                return Err("V7");
            };
            if n != 52 + ((l + 3) & !3) {
                return Err("V4");
            };
            continue;
        }
        if kind == 2 {
            let k = q.take(1)?[0];
            let s = q.take(1)?[0];
            q.u16()?;
            if k > 1 {
                return Err("V8");
            };
            for _ in 0..6 {
                q.float()?;
            }
            for _ in 0..s {
                q.float()?;
                q.take(4)?;
            }
        }
        let v = u64::from(q.u32()?);
        let i = u64::from(q.u32()?);
        if i % 3 != 0 || v * 12 + i * 4 != (body.len() - q.p) as u64 {
            return Err("V4");
        }
        for _ in 0..v {
            q.float()?;
            q.float()?;
            if u32::from(q.u16()?) >= slots {
                return Err("V7");
            };
            q.u16()?;
        }
        for _ in 0..i {
            if u64::from(q.u32()?) >= v {
                return Err("V6");
            }
        }
    }
    if r.p != b.len() {
        return Err("V5");
    }
    Ok((u64::from_le_bytes(b[16..24].try_into().unwrap()), u64::from_le_bytes(b[24..32].try_into().unwrap())))
}
#[test]
fn corpus_and_version_rules() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../desktop/src/canvas/testdata");
    fn walk(p: &Path) {
        for e in fs::read_dir(p).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p)
            } else if p.extension().is_some_and(|x| x == "ttrd") && !p.components().any(|x| x.as_os_str() == "corrupt")
            {
                decode(&fs::read(&p).unwrap()).unwrap();
            }
        }
    }
    walk(&root);
    let cases: serde_json::Value = serde_json::from_slice(&fs::read(root.join("corrupt.json")).unwrap()).unwrap();
    for c in cases.as_array().unwrap() {
        assert_eq!(decode(&fs::read(root.join(c["file"].as_str().unwrap())).unwrap()).err(), c["rule"].as_str(), "{c}");
    }
    let mut b = fs::read(root.join("synthetic.ttrd")).unwrap();
    b[6..8].copy_from_slice(&65535u16.to_le_bytes());
    let rev = (1u64 << 53) + 1;
    b[24..32].copy_from_slice(&rev.to_le_bytes());
    assert_eq!(decode(&b).unwrap().1, rev);
    b.push(0);
    assert_eq!(decode(&b), Err("V5"));
}
