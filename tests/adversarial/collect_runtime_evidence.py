"""Repeat the handoff's runtime measurements without changing product sources.

Run from the checkout root after the release build. Writes only reviews/evidence/M0-C0.1-r1/.
"""
import hashlib
import json
import math
from pathlib import Path
import struct
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'reviews/evidence/M0-C0.1-r1'
HEADLESS = ROOT / 'target/release/toontown-headless.exe'
DESKTOP = ROOT / 'target/release/toontown.exe'


def run(args, timeout=30):
    start = time.perf_counter()
    result = subprocess.run([str(arg) for arg in args], cwd=ROOT, capture_output=True, text=True, timeout=timeout)
    return {'args': [str(arg) for arg in args], 'exit': result.returncode,
            'milliseconds': (time.perf_counter() - start) * 1000,
            'stdout': result.stdout, 'stderr': result.stderr}


def independent_probe():
    points = [(-412.5, 87.25), (-120.125, 640.0), (333.3, -275.75), (598.0, 12.5)]
    digest = hashlib.sha256()
    length = 0.0
    previous = None
    for i in range(1024):
        t = i / 1023

        def lerp(a, b):
            return a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t

        a, b, c = lerp(points[0], points[1]), lerp(points[1], points[2]), lerp(points[2], points[3])
        d, e = lerp(a, b), lerp(b, c)
        p = lerp(d, e)
        tx, ty = e[0] - d[0], e[1] - d[1]
        norm = math.sqrt(tx * tx + ty * ty)
        if previous is not None:
            dx, dy = p[0] - previous[0], p[1] - previous[1]
            length += math.sqrt(dx * dx + dy * dy)
        previous = p
        for value in [p[0], p[1], length, tx / norm, ty / norm]:
            digest.update(struct.pack('<d', value))
    return digest.hexdigest()


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    cases = [[], ['--help'], ['--version'], ['--selftest'], ['--selftest', '--json'],
             ['--selftest', '--json'], ['--headless'], ['--headless', '--mcp-stdio'],
             ['--headless', '--mcp-stdio', '--project', 'absent.animpkg'],
             ['--headless', '--run', 'absent.json', '--project', 'absent.animpkg', '--out', str(OUT / 'unused.jsonl')],
             ['--run'], ['--mcp-stdio', '--run', 'absent.json'], ['--selftest', '--project', 'x'],
             ['--json'], ['--mcp-stdio', '--out', 'x'], ['--unknown']]
    results = [run([HEADLESS, *args]) for args in cases]
    reports = [json.loads(result['stdout']) for result in results if result['args'][1:] == ['--selftest', '--json']]
    stable = [[(check['id'], check['ok'], check['detail']) for check in report['checks']] for report in reports]
    facts = {'cli': results, 'selftest_stable': stable[0] == stable[1],
             'independent_python_sha256': independent_probe(),
             'headless_proof': run(['python', 'scripts/check-headless-deps.py', HEADLESS]),
             'desktop_negative_control': run(['python', 'scripts/check-headless-deps.py', DESKTOP]),
             'sizes_bytes': {path.name: path.stat().st_size for path in
                             [HEADLESS, DESKTOP, ROOT / 'target/release/bundle/nsis/Toontown_0.0.1_x64-setup.exe']}}
    facts['ipc_bench'] = run([DESKTOP, '--ipc-bench', OUT / 'ipc-bench.json'], timeout=150)
    (OUT / 'runtime.json').write_text(json.dumps(facts, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({key: facts[key] for key in ['selftest_stable', 'independent_python_sha256', 'sizes_bytes', 'ipc_bench']}, indent=2))
    assert facts['headless_proof']['exit'] == 0
    assert facts['desktop_negative_control']['exit'] == 1
    assert facts['ipc_bench']['exit'] == 0
    assert json.loads((OUT / 'ipc-bench.json').read_text())['frontend']['ok']


if __name__ == '__main__':
    main()
