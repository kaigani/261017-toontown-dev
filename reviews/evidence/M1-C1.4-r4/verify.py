import pathlib,json,hashlib,re,subprocess,collections,tempfile,shutil
R=pathlib.Path.cwd(); E=R/'reviews/evidence/M1-C1.4-r4'
receipt=json.loads((R/'reviews/evidence/M1-C1.4/receipt.json').read_text())
assert len(receipt['status'])==18 and not any(receipt['status'].values())
for p,h in receipt['sha256'].items(): assert hashlib.sha256((R/'reviews/evidence/M1-C1.4'/p).read_bytes()).hexdigest()==h,p
s=(R/'reviews/evidence/M1-C1.4/gates/test.log').read_text(); totals=[sum(int(x[i]) for x in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',s)) for i in range(3)]
assert totals==[709,0,5],totals
print('Receipt: 18 zero statuses; 21 hashes match; workspace',totals)
changed=subprocess.check_output(['git','diff','--name-only','b91a3c1','5c46aa6'],text=True).splitlines()
protected=[p for p in changed if (p.startswith('tests/adversarial/') or re.search(r'c14_r[123]_.*\.rs$',p) or re.search(r'bins/headless/tests/c14_r\d\.rs$',p) or re.search(r'reviews/(M1-C1.4-r[123]\.md|evidence/M1-C1\.[0-4]-r[123]/)',p))]
assert protected==['crates/mcp/tests/c14_r3_groups.rs'],protected
print('Earlier reviewer-owned paths unchanged; new builder group test excluded:',protected)
assert not subprocess.check_output(['git','diff','b91a3c1','5c46aa6','--','Cargo.lock','deny.toml'])
diff=subprocess.check_output(['git','diff','--unified=0','b91a3c1','5c46aa6','--','crates/*/src','bins/*/src','desktop/src','desktop/src-tauri/src'],text=True)
prohibited=[l for l in diff.splitlines() if l.startswith('+') and not l.startswith('+++') and re.search(r'\.(unwrap|expect)\s*\(|\b(panic!|unsafe)\b|#!?\[allow',l)]
assert not prohibited,prohibited
print('No new prohibited product constructs; Cargo.lock/deny.toml unchanged')
ci=json.loads((E/'ci.json').read_text(encoding='utf-16')); assert ci['headSha'].startswith('5c46aa6') and ci['conclusion']=='success'
print('CI:',ci['headSha'],[(j['name'],j['conclusion']) for j in ci['jobs']])
log=(E/'ci.log').read_text(encoding='utf-16')
for os in ['windows','macos']:
 lines='\n'.join(l for l in log.splitlines() if l.startswith('check ('+os))
 for n in [23,14,5]: assert re.search(r'Ran '+str(n)+r' tests',lines),(os,n)
 assert '"passed": 24' in lines,(os,'sdk')
print('Both check jobs: r1/r2/r3 runtime counts 23/14/5 and SDK count 24 verified')
with tempfile.TemporaryDirectory() as td:
 t=pathlib.Path(td); subprocess.run(['git','init','-q',td],check=True)
 for p in subprocess.check_output(['git','ls-files','--','crates/*/src/**','bins/*/src/**','desktop/src/**','desktop/src-tauri/src/**','scripts/check-product-allows.py','scripts/product-allows.json'],text=True).splitlines():
  dest=t/p; dest.parent.mkdir(parents=True,exist_ok=True); shutil.copyfile(R/p,dest)
 subprocess.run(['git','add','.'],cwd=t,check=True)
 with (t/'crates/mcp/src/docs.rs').open('a') as f: f.write('\n#[allow(dead_code)] fn closure_probe() {}\n')
 r=subprocess.run(['python','scripts/check-product-allows.py'],cwd=t,capture_output=True,text=True)
 assert r.returncode==1 and 'NEW ALLOW' in r.stdout,(r.returncode,r.stdout,r.stderr)
 print('Scratch negative control:',r.stdout.strip())
