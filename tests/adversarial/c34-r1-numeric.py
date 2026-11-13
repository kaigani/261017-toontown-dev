"""Independent E1 numeric oracle; standard library only, usable in CI on retained artifacts."""
import bisect
import json
import math
from pathlib import Path


def position(frame):
    """Hermite polynomial with reflected endpoints; chord-defined distance inversion."""
    if frame >= 24:
        return 2.5
    segment = int(frame > 12)
    t = (frame - (12 if segment else 1)) / (12 if segment else 11)
    controls = (0, 2 / 3) if segment else (1 / 3, 1)
    lo, hi = 0., 1.
    for _ in range(100):
        u = (lo + hi) / 2
        x = 3 * controls[0] * u * (1-u)**2 + 3 * controls[1] * u*u*(1-u) + u**3
        if x > t:
            hi = u
        else:
            lo = u
    u = (lo + hi) / 2
    distance = 5 * (3*u*u - 2*u**3)
    # Coefficients derived from P=(2.5,-2.5,2.5), endpoint derivatives=(-5,0,5).
    def curve(v):
        return (-5*v**3 + 10*v*v - 2.5) if segment else (5*v**3 - 5*v*v - 5*v + 2.5)
    samples = [curve(i/64) for i in range(65)]
    lengths = [0.]
    for a, b in zip(samples, samples[1:]):
        lengths.append(lengths[-1] + abs(b-a))
    k = max(1, min(64, bisect.bisect_left(lengths, distance)))
    v = (k-1 + (distance-lengths[k-1])/(lengths[k]-lengths[k-1]))/64
    return curve(v)


def audit(folder):
    frames = json.loads((folder/'evaluate.json').read_text())['frames']
    errors = []
    for item in frames:
        node = next(n for n in item['nodes'] if n['kind'] == 'peg')
        errors.append(abs(node['animated']['position']['y']-position(item['frame'])))
    assert len(errors) == 30
    assert max(errors) < 1e-9, max(errors)
    ys = [position(f) for f in range(1,31)]
    assert ys[0] == ys[23] == 2.5 and abs(ys[11]+2.5) < 1e-12
    assert all(a>b for a,b in zip(ys[:11],ys[1:12]))
    assert all(a<b for a,b in zip(ys[11:23],ys[12:24]))
    assert ys[24:] == [2.5]*6
    return max(errors)


if __name__ == '__main__':
    root = Path(__file__).resolve().parents[2]
    folders = list((root/'reviews/evidence/M3-C3.4-r1').glob('ci-*'))
    for folder in folders:
        if folder.is_dir():
            print(folder.name, audit(folder))
