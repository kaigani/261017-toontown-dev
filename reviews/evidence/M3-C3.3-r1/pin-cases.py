from pathlib import Path
import tempfile
root=Path.cwd(); out=Path(tempfile.gettempdir())/'toontown-c33-r1'
s=(root/'crates/executor/tests/c33_r1_pixels.rs').read_text()
for name in ['common','c33_support']:
 s=s.replace('mod '+name+';', '#[path = "'+(root/'crates/executor/tests'/name/'mod.rs').as_posix()+'"] mod '+name+';')
s=s.replace('let (sin, cos) = angle.to_radians().sin_cos();','println!("PIN {angle} {dx} {}", image.result["sha256"]); std::fs::write(std::env::temp_dir().join("toontown-c33-r1").join(format!("rectangle-{angle}-{dx}.png")), &image.png)?; let (sin, cos) = angle.to_radians().sin_cos();',1)
(out/'pin-cases.rs').write_text(s,encoding='utf-8')
