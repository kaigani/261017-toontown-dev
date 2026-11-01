"""Independent C2.3 receipts; routine tests never write historical evidence."""
import argparse, collections, copy, datetime, hashlib, json, math, pathlib, re, subprocess, tempfile, unittest
ROOT=pathlib.Path(__file__).resolve().parents[2]
BASE='9c42610'
HEAD='7a77138'
FILES=['draw_select','draw_shapes','draw_transform','draw_cut','draw_layers','draw_strokeops','colour_edit','draw_render']
def git(*args):return subprocess.check_output(['git',*args],cwd=ROOT).decode('utf-8')
def read(p):
    b=p.read_bytes();return b.decode('utf-16' if b.startswith((b'\xff\xfe',b'\xfe\xff')) else 'utf-8-sig')
def changes(a,b,p=''):
    if isinstance(a,dict) and isinstance(b,dict):
        return sum((changes(a.get(k),b.get(k),p+'/'+k) for k in sorted(a.keys()|b.keys())),[])
    if isinstance(a,list) and isinstance(b,list) and len(a)==len(b):
        return sum((changes(x,y,p+'/'+str(i)) for i,(x,y) in enumerate(zip(a,b))),[])
    return [] if a==b else [{'path':p,'old':a,'new':b}]
def inventory():
    commands=[]
    for f in FILES:
        p=f'crates/executor/src/commands/{f}.rs';s=(ROOT/p).read_text(encoding='utf-8')
        for block in re.findall(r'#\[command\((.*?)\)\]',s,re.S):
            id=re.search(r'id\s*=\s*"([^"]+)"',block)[1]
            kind=re.search(r'kind\s*=\s*(\w+)',block)[1]
            commands.append({'id':id,'kind':kind,'source':p,'examples':block.count('example(')})
    contracts=(ROOT/'crates/executor/tests/c23_contracts.rs').read_text().split('fn cases()')[1].split('#[test]')[0]
    covered=set(re.findall(r'"((?:draw|colour)\.[a-zA-Z.]+)"\s*,',contracts))
    missing=[c['id'] for c in commands if c['kind'] in ['action','toggle'] and c['id'] not in covered]
    return {'commands':commands,'newMutations':sum(c['kind'] in ['action','toggle'] for c in commands),'commonContractUniqueCommands':len(covered),'missingFromCommonContract':sorted(missing)}
def c23_measurement(v):
    if not isinstance(v,dict):raise ValueError('root')
    for key in ['seconds','openSeconds','strokes']:
        n=v.get(key)
        if isinstance(n,bool) or not isinstance(n,(int,float)) or not math.isfinite(n) or n<0:raise ValueError(key)
    if v.get('profile')!='release' or not isinstance(v.get('params'),dict):raise ValueError('profile/params')
    if v.get('command') not in ['draw.drawing.render','draw.flatten.apply','draw.centreLine.apply']:raise ValueError('command')
    if not isinstance(v.get('status'),str):raise ValueError('status')
    if v['command']=='draw.flatten.apply':
        if v.get('error',{}).get('code')!='limit.exceeded':raise ValueError('flatten outcome')
    elif v.get('error') is not None:raise ValueError('unexpected refusal')
    return {'command':v['command'],'seconds':v['seconds'],'under30Seconds':v['seconds']<30}
def ci(out):
    jobs=json.loads(read(out/'ci-jobs.json'))['jobs'];log=read(out/'ci-all.log');result=[]
    for j in jobs:
        duration=(datetime.datetime.fromisoformat(j['completed_at'])-datetime.datetime.fromisoformat(j['started_at'])).total_seconds()
        lines=[l for l in log.splitlines() if l.startswith(j['name']+'\t') and '\tRun cargo test --workspace --locked\t' in l]
        counts=[tuple(map(int,m)) for m in re.findall(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored','\n'.join(lines))]
        result.append({'name':j['name'],'conclusion':j['conclusion'],'seconds':duration,'above55Minutes':j['name']=='check (windows-latest)' and duration>3300,'workspaceSummaries':len(counts),'workspaceCounts':dict(zip(['passed','failed','ignored'],[sum(x[i] for x in counts) for i in range(3)])),'c23GoldenTestPassed':any('test golden_hashes_hold_on_every_platform ... ok' in l for l in lines)})
    return {'run':41303866582,'head':json.loads(read(out/'ci-run.json'))['head_sha'],'successfulJobs':sum(j['conclusion']=='success' for j in jobs),'jobs':result,'c23PngArtifactsPresent':False,'limitation':'Check artifacts contain measurements, SDK receipts and feature graphs, not C2.3 PNGs. Both OS logs execute the exact golden equality assertions.'}
def collect(out):
    out.mkdir(parents=True,exist_ok=True)
    def write(n,v):(out/n).write_text(json.dumps(v,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
    write('command-inventory.json',inventory())
    p='crates/executor/tests/golden/c22-determinism.json'
    write('c22-golden-structural.json',changes(json.loads(git('show',BASE+':'+p)),json.loads((ROOT/p).read_bytes())))
    hashes={}
    from PIL import Image
    for p in (ROOT/'reviews/evidence/M2-C2.3/goldens').glob('*.png'):
        image=Image.open(p).convert('RGBA');rgba=image.tobytes()
        premul=bytes(round(c*a/255) if k<3 else a for px in image.getdata() for k,(c,a) in enumerate(zip(px,[px[3]]*4)))
        hashes[p.stem]={'pngSha256':hashlib.sha256(p.read_bytes()).hexdigest(),'straightPixelsSha256':hashlib.sha256(rgba).hexdigest(),'premultipliedPixelsSha256':hashlib.sha256(premul).hexdigest(),'size':image.size}
    write('committed-golden-hashes.json',hashes)
    artifacts=ROOT/'target/c23-r1-ci'
    write('downloaded-artifact-file-hashes.json',[{'path':str(p.relative_to(artifacts)),'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in artifacts.rglob('*') if p.is_file()])
    if (out/'ci-jobs.json').exists():write('ci-recomputed.json',ci(out))
    diff=git('diff','--unified=0',BASE+'..'+HEAD,'--','crates/*/src/**','bins/*/src/**','desktop/src/**')
    found=[];file='';number=0
    for line in diff.splitlines():
        if line.startswith('+++ b/'):file=line[6:]
        elif line.startswith('@@'):number=int(re.search(r'\+(\d+)',line)[1])-1
        elif line.startswith('+'):
            number+=1
            if re.search(r'\b(?:unwrap|expect)\s*\(|\bpanic!|\bunsafe\b|#\[allow',line):
                content=(ROOT/file).read_text(encoding='utf-8').splitlines();test=next((i+1 for i,l in enumerate(content) if l=='#[cfg(test)]'),len(content)+1)
                found.append({'file':file,'line':number,'text':line[1:],'cfgTest':number>test})
        elif not line.startswith('-'):number+=1
    write('added-constructs.json',found)
    (out/'lockfile.diff').write_text(git('diff',BASE+'..'+HEAD,'--','Cargo.lock'),encoding='utf-8')
    measurements={}
    for p in (out/'measurements').glob('*.json'):
        try:measurements[p.name]=c23_measurement(json.loads(p.read_bytes()))
        except ValueError as e:measurements[p.name]={'error':str(e)}
    write('performance-recomputed.json',measurements)
    gate={}
    for p in out.glob('*.log'):
        matches=[tuple(map(int,m)) for m in re.findall(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored',read(p))]
        if matches:gate[p.name]={'summaries':len(matches),**dict(zip(['passed','failed','ignored'],[sum(m[i] for m in matches) for i in range(3)]))}
    write('gate-counts.json',gate)
class Audit(unittest.TestCase):
    def test_measurement_parser_rejects_malformed_input(self):
        v=json.loads((ROOT/'reviews/evidence/M2-C2.3/measurements/render-100k.json').read_bytes());self.assertTrue(c23_measurement(v)['under30Seconds'])
        for bad in [None,True,-1,float('inf'),'1',{}]:
            d=copy.deepcopy(v);d['seconds']=bad
            with self.assertRaises(ValueError):c23_measurement(d)
        for bad in [None,[],True]:
            with self.assertRaises(ValueError):c23_measurement(bad)
    def test_c22_golden_has_only_the_ruled_axis_change_and_derived_receipt(self):
        p='crates/executor/tests/golden/c22-determinism.json';delta=changes(json.loads(git('show',BASE+':'+p)),json.loads((ROOT/p).read_bytes()))
        content=[d for d in delta if d['path'] not in ['/sha256','/bytes']]
        self.assertEqual(len(content),1,content);self.assertEqual(content[0]['old'],53.96875);self.assertEqual(content[0]['new'],-53.96875)
    def test_all_implemented_mutations_are_in_the_claimed_common_contract(self):
        self.assertEqual(inventory()['missingFromCommonContract'],[],'C23-R1 coverage finding: 27 cases do not cover every mutation')
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--out',type=pathlib.Path);a,rest=p.parse_known_args()
    if a.out:collect(a.out.resolve())
    unittest.main(argv=[__file__,*rest],verbosity=2)
