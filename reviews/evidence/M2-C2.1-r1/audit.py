"""Review receipts: source locations, unchanged prior tests, licences and CI outcomes."""
import json,re,subprocess,hashlib,difflib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
HERE=Path(__file__).resolve().parent
def git(*args):
    return subprocess.check_output(['git',*args],cwd=ROOT).decode('utf8')
def locate(file,token):
    return f'{file}:{next(i for i,s in enumerate((ROOT/file).read_text(encoding="utf8").splitlines(),1) if token in s)}'
needles=[('crates/vector/src/fit.rs','fn fit_run('),('crates/vector/src/fit.rs','fn corners('),
 ('crates/vector/src/brush.rs','fn placements('),('crates/vector/src/brush.rs','pub fn outline('),
 ('crates/executor/src/commands/draw_stroke.rs','pub(crate) struct StrokeSetParams'),
 ('crates/executor/src/commands/draw_stroke.rs','pub(crate) struct StrokeSetResult'),
 ('crates/executor/src/commands/draw_stroke.rs','match (new.spine(), spine_changed, outline_changed)'),
 ('crates/model/src/validate.rs','if let (Some(text), Some(runs))'),
 ('crates/executor/src/commands/draw_text.rs','pub(crate) fn draw_text_add('),
 ('crates/executor/src/commands/draw_text.rs','pub(crate) fn draw_text_set('),
 ('crates/executor/src/run.rs','events.push(json!({'),
 ('crates/executor/src/commands/draw_stroke.rs','fn symmetry_setting(')]
locations={t:locate(f,t) for f,t in needles}
base=git('ls-tree','-r','--name-only','6b9981e').splitlines()
reviewer=[p for p in base if p.startswith('tests/adversarial/') or re.match(r'crates/[^/]+/tests/c1[1-4]_r\d',p)]
diff=git('diff','6b9981e','0997205','--',*reviewer)
assert not diff, diff
old=git('show','5983f29:crates/mcp/tests/c14_r1_16_tickets.rs')
new=(ROOT/'crates/mcp/tests/c14_tickets.rs').read_text(encoding='utf8')
def body(s):
    start=s.index('fn namespace_docs_carry_a_worked_example_with_executed_results()')
    return s[start:]
assert body(old)==body(new),'retained ticket body changed'
meta=json.loads(subprocess.check_output(['cargo','metadata','--format-version','1','--locked'],cwd=ROOT))
licenses={p['name']:{'version':p['version'],'license':p['license']} for p in meta['packages']
          if p['name'] in ['libm','i_overlay','i_float','i_shape','i_tree','i_key_sort','fontdb','skrifa','stats_alloc']}
raw_ci=(HERE/'ci.json').read_bytes()
ci=json.loads(raw_ci.decode('utf-16' if raw_ci.startswith(b'\xff\xfe') else 'utf-8-sig'))
prohibited=[]
product_files=[]
for file in git('diff','--name-only','6b9981e','0997205').splitlines():
    if not file.endswith('.rs') or '/src/' not in file: continue
    product_files.append(file)
    try: old_source=git('show','6b9981e:'+file)
    except subprocess.CalledProcessError: old_source=''
    new_source=(ROOT/file).read_text(encoding='utf8')
    old_source=old_source.split('#[cfg(test)]')[0]
    new_source=new_source.split('#[cfg(test)]')[0]
    for line in difflib.unified_diff(old_source.splitlines(),new_source.splitlines(),n=0):
        if line.startswith('+') and not line.startswith('+++') and re.search(r'\b(?:unwrap|expect)\s*\(|\bpanic!\s*\(|\bunsafe\s*(?:\{|fn|impl)|#\s*\[\s*allow',line):
            prohibited.append({'file':file,'line':line})
assert not prohibited, prohibited
out={'head':ci['headSha'],'priorReviewerPathsChecked':len(reviewer),'priorReviewerDiffEmpty':not diff,
     'ticketBodyIdenticalSince5983f29':True,'ticketBodySha256':hashlib.sha256(body(new).encode()).hexdigest(),
     'licenses':licenses,'locations':locations,'productRustFilesInspected':len(product_files),
     'newProhibitedConstructsBeforeCfgTest':prohibited,
     'ciJobs':[{'name':j['name'],'conclusion':j['conclusion'],'url':j['url']} for j in ci['jobs']]}
(HERE/'audit.json').write_text(json.dumps(out,indent=2)+'\n',encoding='utf8')
print(json.dumps(out,indent=2))
