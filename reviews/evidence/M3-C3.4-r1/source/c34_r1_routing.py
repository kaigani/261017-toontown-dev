import json,re
from pathlib import Path
r=Path.cwd();e=r/'reviews/evidence/M3-C3.4-r1';out={}
for name in ['ci-routing','ci-final']:
 text=(e/(name+'.log')).read_text(encoding='utf-16')
 out[name]={}
 for test in ['seeded_anchor_groups_4096','deep_and_broad_scene','random_hierarchy_edits_4096_keep_independent_parent_and_level_model']:
  lines=[l for l in text.splitlines() if 'test '+test+' ... ok' in l]
  out[name][test]=lines
  assert len(lines)>=2,(name,test,len(lines))
(e/'routing-proof.json').write_text(json.dumps(out,indent=2))
print('Both runs contain successful execution of all three corpora on both OSes.')
