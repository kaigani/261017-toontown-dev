//! Evidence helper: uses the exact host scene encoder, no changes to product code.
#[path = "../../../desktop/src-tauri/src/spikes/canvas/renderlist.rs"]
#[allow(dead_code)]
mod renderlist;
fn main() {
    let dir = std::env::args().nth(1).unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    for (name, kind) in [("paths100k", renderlist::Kind::Paths100k), ("groups1k", renderlist::Kind::Groups1k), ("brush10k", renderlist::Kind::Brush10k)] {
        let s = renderlist::generate(kind, 1);
        std::fs::write(format!("{dir}/{name}.ttrl"), renderlist::encode(kind, &s, 0, 0)).unwrap();
    }
}
