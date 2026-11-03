"""Preserve the 123-unit / 37-command register, with explicit round-two adjudication deltas."""
import argparse
import collections
import json
import pathlib
import unittest
ROOT=pathlib.Path(__file__).resolve().parents[2]

def register():
    v=json.loads((ROOT/'reviews/evidence/M2-C2.3-r1/register.json').read_bytes())
    prior=json.loads(json.dumps(v));deltas=[]
    v['target']='797beff2f743f39d0c457a9b1681dfe57a1b98d7'
    v['scope']+=' Round 2 retains the same units. Generic cancellation includes the request-identity variant; default painted-zone selection is not closed by an explicit-ID-only test.'
    unresolved={'C23-U015':'C23-R2-03; valid derived zone pruned after selection.set',
                'C23-U063':'C23-R2-03; explicit-ID warning fixed, selected-zone flow still fails',
                'C23-U078':'C23-R2-02; explicit false dropped by four shape wrappers',
                'C23-U085':'C23-R2-01; bounded Smooth and MCP timeout pass, queued caller still commits'}
    for u in v['units']:
        before=json.loads(json.dumps(u))
        if u['status']=='divergent':
            u['status']='implemented'
            u['evidenceOrOwner']+='; round-two original proving test passes; closure table/new variants in M2-C2.3-r2.md'
        if u['unit'] in unresolved:
            u['status']='divergent';u['evidenceOrOwner']=unresolved[u['unit']]
        if u!=before:deltas.append({'unit':u['unit'],'before':before,'after':u})
    command_deltas=[]
    shape_ids={'draw.rectangle.add','draw.ellipse.add','draw.line.add','draw.polyline.add'}
    for c in v['commands']:
        before=json.loads(json.dumps(c))
        if c['status']=='divergent':c['status']='implemented';c['finding']=None
        if c['id'] in shape_ids:c['status']='divergent';c['finding']='C23-R2-02'
        if c['id']=='draw.selection.set':c['status']='divergent';c['finding']='C23-R2-03'
        if c!=before:command_deltas.append({'id':c['id'],'before':before,'after':c})
    v['counts']=dict(collections.Counter(u['status'] for u in v['units']))
    v['commandCounts']=dict(collections.Counter(c['status'] for c in v['commands']))
    delta={'priorCounts':prior['counts'],'currentCounts':v['counts'],'priorCommandCounts':prior['commandCounts'],'currentCommandCounts':v['commandCounts'],
           'unitDeltas':deltas,'commandDeltas':command_deltas,'closureCounts':{'closed':13,'partial':3,'reopened':0},
           'note':'C23-U059 is now implemented: aggregate Smooth work refusal passes. U085 retains the generic cancellation obligation under R1-07. Shared defects are not copied into every otherwise-correct primary command row. Centre Line explicit-ID behavior closes; its default-selection warning failure is retained in U063 and selection.set.'}
    return v,delta

class Register(unittest.TestCase):
    def test_preserves_all_units_commands_and_deferred_owners(self):
        v,d=register();old=json.loads((ROOT/'reviews/evidence/M2-C2.3-r1/register.json').read_bytes())
        self.assertEqual([u['unit'] for u in v['units']],[u['unit'] for u in old['units']])
        self.assertEqual(len(v['units']),123);self.assertEqual(len(v['commands']),37)
        self.assertEqual([u for u in v['units'] if u['status']=='deferred-with-owner'],[u for u in old['units'] if u['status']=='deferred-with-owner'])
        self.assertEqual(v['counts'],{'implemented':94,'divergent':4,'deferred-with-owner':25})
        self.assertEqual(v['commandCounts'],{'implemented':32,'divergent':5})
        self.assertEqual(d['closureCounts'],{'closed':13,'partial':3,'reopened':0})

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--out',type=pathlib.Path);args,rest=parser.parse_known_args()
    if args.out:
        args.out.mkdir(parents=True,exist_ok=True)
        for n,v in zip(['register.json','register-deltas.json'],register()):
            (args.out/n).write_text(json.dumps(v,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
    unittest.main(argv=[__file__,*rest],verbosity=2)
