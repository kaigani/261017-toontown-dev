"""Run scoped evidence probes and the remaining release gates, keeping every command's receipt."""
import os
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
os.environ.update(CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0', CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='4')
relative = HERE.relative_to(ROOT).as_posix()
commands = [
    ('scoped-probes', ['python', f'{relative}/run-probes.py']),
    ('fresh-property', ['python', f'{relative}/run-fresh-property.py']),
    ('release-stress-scale', ['cargo', 'test', '--release', '-p', 'tt-executor', '--test', 'c12_r1_stress', '--test', 'c12_r3_stress', '--test', 'c12_r1_scale', '--test', 'c12_r2_scale', '--locked', '--', '--nocapture']),
    ('tauri-release', ['cmd', '/c', 'pnpm', '--dir', 'desktop', 'tauri', 'build', '--no-bundle', '--', '--locked']),
    ('raster', ['cargo', 'run', '--release', '--locked', '-q', '-p', 'tt-render', '--example', 'c03_raster', '--', '--out', f'{relative}/raster', '--report', f'{relative}/raster-report.json', '--check', 'crates/render/examples/c03_raster/hashes.json']),
    ('register-reconciliation', ['python', f'{relative}/reconcile-registers.py']),
    ('frontend-registry-match', ['python', f'{relative}/check-frontend.py']),
]
failed = []
for name, cmd in commands:
    print('Starting', name, flush=True)
    code = subprocess.call([sys.executable, str(HERE / 'run-review.py'), name, *cmd], cwd=ROOT)
    if code:
        failed.append(name)
print('Failed:', failed, flush=True)
sys.exit(bool(failed))
