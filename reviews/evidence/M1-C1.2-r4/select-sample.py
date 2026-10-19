"""Record the independently selected, reproducible 20-row declaration sample."""
import json
import random
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
seed = 0xC12420261019
old = json.loads((ROOT / 'reviews/evidence/M1-C1.2-r3/command-register.json').read_text(encoding='utf-8'))
excluded = {r['id'] for r in old if r['status'] == 'divergent'}
rows = json.loads((ROOT / 'crates/registry/src/declared.json').read_text(encoding='utf-8'))
sample = random.Random(seed).sample(sorted(r['id'] for r in rows if r['id'] not in excluded), 20)
out = {'seed': hex(seed), 'population': '662 generated declarations, excluding 14 previously divergent declarations',
       'ids': sample, 'schemas': [next(r for r in rows if r['id'] == cid) for cid in sample]}
(HERE / 'sample20.json').write_text(json.dumps(out, indent=2) + '\n', encoding='utf-8')
print(json.dumps({'seed': hex(seed), 'ids': sample}, indent=2))
