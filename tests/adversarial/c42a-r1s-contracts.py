"""Static contract regressions; all checks read source only."""
import pathlib,sys
R=pathlib.Path(__file__).resolve().parents[2]
def read(p):return (R/p).read_text(encoding='utf-8')
mode=sys.argv[1] if len(sys.argv)>1 else 'mcp'
g=read('crates/executor/src/gesture.rs');bad=[]
if mode=='mcp':
 t=read('crates/mcp/src/tools.rs').split('"Pointer input"',1)[1].split('"key"',1)[0]
 if '"items": { "type": "object" }' in t:bad.append('C42A-R1-21: pointer event schema has no event properties or required fields')
 b=g.split('let commands: Vec<Value> = self',1)[1].split('Envelope::ok',1)[0]
 if '"undo"' not in b and '"envelope"' not in b:bad.append('C42A-R1-22: successful pointer strips command envelopes (undo, changes, warnings)')
elif mode=='gestures':
 p=g.split('fn polyline(',1)[1].split('// ----- the headless runner',1)[0]
 if 'mods.constrain' not in p:bad.append('C42A-R1-24: polyline never constrains handle angles')
 p=g.split('"paint" | "unpaint" | "paintUnpainted" =>',1)[1].split('"dropper" =>',1)[0]
 if '"mapping"' not in p:bad.append('C42A-R1-25: Paint drag never emits gradient/texture mapping')
elif mode=='issues':
 if '("colour.pick", m)' in g and 'colour.current.set' not in g:bad.append('C42A-R1-23: Dropper never selects the picked colour despite explicit spec requirement')
elif mode=='zoom':
 if '"view.zoom.set", obj(json!({ "viewId": g.view_id, "steps": steps' in g and 'let params = json!({ "viewId": view_id, "steps": steps' in g:
  bad.append('C42A-R1-26: wheel and Zoom route to table steps rather than the required powers-of-two ladder')
elif mode=='settings':
 shape=read('crates/executor/src/drawing.rs').split('pub fn shape(',1)[1]
 if all(x not in shape for x in ['placeBehind','drawTopLayer','snapToContour']):bad.append('C42A-R1-28: drawing whole-file shape omits the three top-level boolean members')
elif mode=='handoff':
 if '(filled in at the push)' in read('reviews/M4-C4.2a-handoff.md'):bad.append('C42A-R1-27: head and CI are still placeholder')
print('\n'.join(bad) if bad else 'PASS: '+mode)
sys.exit(bool(bad))
