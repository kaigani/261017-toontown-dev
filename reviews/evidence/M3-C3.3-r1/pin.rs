fn main() {
 use sha2::Digest;
 for c in [[0u8,0,0,0],[18,52,86,255]] {
  let pixels: Vec<u8> = (0..32*18).flat_map(|_| c).collect();
  let png=tt_render::encode_png(&pixels,32,18).unwrap();
  println!("{}",sha2::Sha256::digest(&png).iter().map(|b|format!("{b:02x}")).collect::<String>());
 }
}
