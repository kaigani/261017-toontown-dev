"""Applies the reviewers' independent audit (reviews/evidence/M0-C0.2-r2/audit-matrix.py, function `audit`, unchanged)
to the round-2 matrix logs, without running its main (which would overwrite the reviewers' evidence files)."""
import importlib.util, json, sys
from pathlib import Path
root = Path(sys.argv[1])
spec = importlib.util.spec_from_file_location('audit', root / 'reviews/evidence/M0-C0.2-r2/audit-matrix.py')
a = importlib.util.module_from_spec(spec); spec.loader.exec_module(a)
out = []
for name in ('r2-matrix3', 'r2-matrix4', 'r2-matrix5'):
    r = a.audit(root / f'reviews/evidence/M0-C0.2/tablet-spike-windows-runner-{name}.json.gz')
    out.append(r)
    print(f"## {r['file']} sha256 {r['sha256']}")
    print('handler-minus-event ms', json.dumps(r['handlerLatencyMs']), 'coalesced', json.dumps(r['coalescedCount']))
    for row in r['runs']:
        if 'error' in row and 'web' not in row and 'wmPointer' not in row:
            print(f"- {row['label']}: FAILED {row['error']}")
            continue
        w = row.get('web') or {}
        wm = row.get('wmPointer') or {}
        rts = row.get('rts') or {}
        print(f"- {row['label']}: rect {row['screenRect']}; first-to-last {row['injectorHzFirstToLast']:.6f} Hz "
              f"(saved {row['reportedAchievedHz_UNVERIFIED']:.6f}); repeated {row['repeatedPixelFrames'][:8]}"
              f"{'...' if len(row['repeatedPixelFrames']) > 8 else ''}; axes {row['varyingAxes']}; "
              + (f"web {w['received']}, repeatedMissing {w['repeatedMissing'][:8]}, rotationExact {w['exactRotationMatches']}, "
                 f"tiltExact {w['exactTiltMatches']}, event p50 {w['eventLatencyMs']['50']}, handler p50 {w['handlerLatencyMs']['50']}; "
                 if w else '')
              + (f"WM {wm['received']}; " if wm else '')
              + (f"RTS {rts}; " if rts else '')
              + f"raw {row.get('raw', {}).get('hidReports')}")
(root / 'reviews/evidence/M0-C0.2/r2-independent-audit.json').write_text(json.dumps(out, indent=1), encoding='utf-8')
