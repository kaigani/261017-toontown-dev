//! Compile independently with different optimisation, CPU, LTO and codegen options.
#[path = "../../../crates/render/examples/c03_raster/scene.rs"]
#[allow(dead_code)]
mod scene;
#[path = "../../../crates/render/examples/c03_raster/ttraster.rs"]
mod ttraster;
fn main() {
    let output = std::env::args().nth(1).unwrap();
    std::fs::create_dir_all(&output).unwrap();
    for s in scene::scenes() {
        std::fs::write(format!("{output}/{}.rgba",s.name), ttraster::render(&s).rgba).unwrap();
    }
}
