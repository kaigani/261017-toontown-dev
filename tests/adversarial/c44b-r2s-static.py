"""Read-only source witnesses for the C4.4b round 2 static review; never a CI gate."""
import json
from pathlib import Path
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def read(name):
    return (ROOT / name).read_text(encoding="utf-8")


def check(label, fn):
    try:
        print(label + ": " + str(fn()))
    except Exception as exc:
        print(label + ": unavailable (" + type(exc).__name__ + ")")


def creation():
    s = read("crates/persist/src/lock.rs")
    s = s[s.index("fn create_protected"):s.index("pub fn probe")]
    return "open then flock; identity check absent=" + str("metadata()" not in s)


def schedules():
    s = read("crates/executor/tests/c44b_lock_takeover.rs")
    return "conditional live-owner assertion=" + str("if w_first {" in s)


def target():
    s = read("crates/executor/src/host.rs")
    return "Delete rewrite present=" + str('b.command_id = "draw.point.delete"' in s) + \
        "; host interactive_target call=" + str("interactive_target(" in s)


def keys():
    r = json.loads(read("desktop/src/keys/text-field-keys.json"))
    return r["fieldKinds"]["canvasText"]["documentKeys"]


def pole():
    def f32(x):
        return struct.unpack("f", struct.pack("f", x))[0]
    y = -1.0000000000001
    w = y + 1
    cpu = 1 / (-1e-12 if w < 0 else 1e-12)
    gpu_w = f32(f32(y) + 1)
    gpu = 1 / (-f32(1e-12) if gpu_w < 0 else f32(1e-12))
    return "analytic witness only: kernel x=" + str(cpu) + "; f32 x=" + str(gpu)


def mentions():
    s = read("desktop/src/ui-bindings/parity.test.tsx")
    return "uses=sourceUses=" + str("uses: sourceUses()" in s)


def files():
    paths = list((ROOT / "reviews/evidence/M4-C4.4b-r2").rglob("*"))
    large = [str(p.relative_to(ROOT)) for p in paths if p.is_file() and p.stat().st_size > 200000]
    changed = subprocess.run(
        ["git", "diff", "--name-only", "57bad5ee", "25b99718", "--",
         "crates/executor/tests/c13_r1_persistence.rs"], cwd=ROOT,
        capture_output=True, text=True, check=True).stdout.strip()
    return "oversize receipts=" + str(large) + "; c13 changed=" + str(bool(changed))


for label, fn in [("S1", creation), ("S2", schedules), ("S3", target),
                  ("S4", keys), ("S5", pole), ("S6", mentions), ("S7", files)]:
    check(label, fn)
