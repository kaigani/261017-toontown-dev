"""Execute the real Rust writer -> TS reader/writer -> Rust reader exchange."""
import os
from pathlib import Path
import subprocess
import sys

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
env=dict(os.environ, TT_R2_WIRE_EXPORT=str(HERE/"rust-to-ts.json"), TT_R2_WIRE_IMPORT=str(HERE/"ts-to-rust.json"))
commands=[
    ("wire-export",["cargo","test","-p","tt-engine","--test","c11_r2_wire","cross_language_exchange","--locked"]),
    ("wire-typescript",["pnpm","--dir","desktop","exec","vitest","run","--config","vite.config.ts","src/ipc/c11-r2-adversarial.test.ts","-t","exchange"]),
    ("wire-import",["cargo","test","-p","tt-engine","--test","c11_r2_wire","cross_language_exchange","--locked"]),
]
for name,command in commands:
    current=env.copy()
    if name=="wire-export": current.pop("TT_R2_WIRE_IMPORT")
    subprocess.run([sys.executable,str(HERE/"run.py"),"exchange-v2-"+name,*command],cwd=ROOT,env=current,check=True)
assert (HERE/"rust-to-ts.json").read_bytes()==(HERE/"ts-to-rust.json").read_bytes()
print("Rust -> TypeScript -> Rust: identical bytes and exact integer values")
