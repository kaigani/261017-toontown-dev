"""Show that Cargo license metadata does not police native avcodec linkage."""
import json
import pathlib
import subprocess

OUT = pathlib.Path(__file__).resolve().parent
ROOT = OUT.parents[2]
fixture = ROOT/'target/c04-license-probe'
fixture.mkdir(parents=True,exist_ok=True)
(fixture/'Cargo.toml').write_text('''[workspace]
[package]
name = "review-native-avcodec-link"
version = "0.1.0"
edition = "2024"
license = "MIT"
links = "avcodec"
build = "build.rs"
[lib]
path = "lib.rs"
''',encoding='utf-8')
(fixture/'lib.rs').write_text('// Review fixture, no product code.\n',encoding='utf-8')
(fixture/'build.rs').write_text('fn main() { println!("cargo:rustc-link-lib=dylib=avcodec"); }\n',encoding='utf-8')
commands = [
    ['cargo','generate-lockfile','--manifest-path',str(fixture/'Cargo.toml')],
    ['cargo','deny','--manifest-path',str(fixture/'Cargo.toml'),'--locked','--config',str(ROOT/'deny.toml'),'check','licenses','bans','sources'],
]
receipts=[]
for i, argv in enumerate(commands):
    p=subprocess.run(argv,cwd=ROOT,capture_output=True,timeout=60)
    (OUT/f'license-link-probe-{i}.log').write_bytes(p.stdout+p.stderr)
    receipts.append({'command':argv,'exit':p.returncode})
    assert p.returncode==0, p.stderr.decode(errors='replace')
(OUT/'license-link-probe.json').write_text(json.dumps({'fixture':{
    'manifest':(fixture/'Cargo.toml').read_text(),'build_script':(fixture/'build.rs').read_text(),
    'note':'No FFmpeg binary is fetched or linked; cargo deny inspects metadata, not native dependencies.'},
    'commands':receipts},indent=2)+'\n',encoding='utf-8')
print('MIT-declared avcodec link fixture passes unchanged license/bans/source policy')
