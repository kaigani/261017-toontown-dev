fn main() {
 for (name,c) in [("transparent-32x18",[0u8,0,0,0]),("solid-32x18",[18,52,86,255])] {
  let pixels: Vec<u8> = (0..32*18).flat_map(|_| c).collect();
  let png=tt_render::encode_png(&pixels,32,18).unwrap();
  std::fs::write(std::env::temp_dir().join("toontown-c33-r1").join(format!("{name}.png")),png).unwrap();
 }
}
