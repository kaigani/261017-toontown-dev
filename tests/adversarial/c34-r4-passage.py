"""Independent 80-digit oracle for the closure quadratic touch family.

The first segment is exactly 3*s*(1-s); the next is linear in frame.
No product code is imported. Run standalone, or pass r4-anim.log to adjudicate its output.
"""
from decimal import Decimal, localcontext
import json
from pathlib import Path
import re
import sys


def family():
    with localcontext() as ctx:
        ctx.prec = 80
        peak = Decimal(3) / 4
        for relative in [0.0, -1e-9, -1e-12, -1e-13, -1e-15, 1e-9, 1e-12, 1e-13, 1e-15]:
            arc_float = 0.75 * (1.0 + relative)
            arc = Decimal.from_float(arc_float)
            exact = (Decimal(2) + arc if arc > peak else
                     Decimal(1) + (1 - (1 - 4 * arc / 3).sqrt()) / 2)
            touch_band = Decimal('1e-12') * max(Decimal(1), abs(arc))
            yield {'relative': relative, 'arc': str(arc), 'exact_peak': str(peak),
                   'exact_frame': str(exact), 'maximum_at_or_above_arc': peak >= arc,
                   'below_arc_inside_documented_touch_band': 0 < arc - peak <= touch_band}


def main():
    rows = list(family())
    if len(sys.argv) > 1:
        log = Path(sys.argv[1]).read_text(encoding='utf-8', errors='replace')
        got = {float(r): Decimal(g) for r, g in re.findall(r'relative=([\deE+.\-]+) arc=[^ ]+ got=([^ ]+) exact=', log)}
        assert len(got) == 9, 'all nine product cases must be present'
        for row in rows:
            frame = got[row['relative']]
            error = abs(frame - Decimal(row['exact_frame']))
            permitted_touch = row['below_arc_inside_documented_touch_band'] and frame == Decimal('1.5')
            row.update(product_frame=str(frame), frame_error=str(error), permitted_touch=permitted_touch,
                       conforming=error <= Decimal('0.0006') or permitted_touch)
        assert all(r['conforming'] for r in rows), rows
    print(json.dumps(rows, indent=2))
    return 0


if __name__ == '__main__':
    sys.exit(main())
