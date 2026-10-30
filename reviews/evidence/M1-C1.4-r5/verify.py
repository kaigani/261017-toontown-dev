import pathlib,json,hashlib,re,subprocess,collections
R=pathlib.Path.cwd(); E=R/'reviews/evidence/M1-C1.4-r5'
def git(*args): return subprocess.check_output(['git',*args],text=True)
receipt=json.loads((R/'reviews/evidence/M1-C1.4/receipt.json').read_text())
assert len(receipt['status'])==18 and not any(receipt['status'].values())
assert len(receipt['sha256'])==21
for p,h in receipt['sha256'].items(): assert hashlib.sha256((R/'reviews/evidence/M1-C1.4'/p).read_bytes()).hexdigest()==h,p
s=(R/'reviews/evidence/M1-C1.4/gates/test.log').read_text()
totals=[sum(int(x[i]) for x in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',s)) for i in range(3)]
assert totals==[716,0,5],totals
print('Builder receipt: 18 zero statuses, 21 matching SHA256 hashes, workspace',totals)
changed=git('diff','--name-only','ace8f13','e2eba79').splitlines()
protected=[p for p in changed if p.startswith('tests/adversarial/') or (re.search(r'c14_r[1-4].*\.rs$',p) and p!='crates/mcp/tests/c14_r4_expect_rev.rs') or re.search(r'reviews/(M1-C1\.[0-4]-r[1-4](?:-closure)?\.md|evidence/M1-C1\.[0-4]-r[1-4]/)',p) or re.search(r'c(?:0[0-9]|1[1-3])_r',p)]
assert not protected,protected
print('All changed paths inspected; earlier reviewer tests, reports and evidence unchanged:',len(changed),'builder paths')
assert not git('diff','ace8f13','e2eba79','--','Cargo.lock','deny.toml')
diff=git('diff','--unified=0','ace8f13','e2eba79','--','crates/*/src','bins/*/src','desktop/src','desktop/src-tauri/src')
bad=[l for l in diff.splitlines() if l.startswith('+') and not l.startswith('+++') and re.search(r'\.(unwrap|expect)\s*\(|\b(panic!|unsafe)\b|#!?\[allow',l)]
assert not bad,bad
print('Cargo.lock/deny.toml unchanged; no new prohibited product constructs')
actual=git('diff','ace8f13','e2eba79','--','crates/mcp/tests/c14_r4_expect_rev.rs')
declared=(R/'reviews/evidence/M1-C1.4/declared-test-edits-r4.diff').read_text()
assert actual.replace('\r\n','\n').strip()==declared.replace('\r\n','\n').strip()
print('Declared builder test diff matches exactly')
ci=json.loads((E/'ci.json').read_text(encoding='utf-8-sig'))
assert ci['headSha']=='e2eba796ccb2611d05e98c70525c1799d999d0e1' and ci['conclusion']=='success'
for name in ['check (windows-latest)','check (macos-latest)','headless without a display (ubuntu)']:
 j=next(j for j in ci['jobs'] if j['name']==name);assert j['conclusion']=='success'; print('CI job:',name,j['conclusion'])
d=json.loads((R/'reviews/evidence/M1-C1.4-r4/conformance.json').read_text())
for r in d['requirements']:
 if r['id']=='S11-L81': r.update(status='implemented',note='C14-R4-01 closes on b16a39a/e2eba79: matching and stale atomic group expectRev, single revision/history, sequential step guards, export and exact replay, stale replay and malformed guard refusal independently checked.',test='crates/mcp/tests/c14_r4_closure.rs; crates/mcp/tests/c14_r5_closure.rs')
d['counts']=dict(collections.Counter(r['status'] for r in d['requirements'])); d['declarationCounts']=dict(collections.Counter(r['status'] for r in d['declarations']))
assert d['counts']=={'implemented':153,'divergent':1,'deferred-with-owner':72},d['counts']
assert [r['id'] for r in d['requirements'] if r['status']=='divergent']==['S11-L96']
assert d['declarationCounts']=={'implemented':21,'deferred-with-owner':11},d['declarationCounts']
(E/'conformance.json').write_text(json.dumps(d,indent=2)+'\n')
md='# Final scoped conformance register\n\nProduct `e2eba79`; r4 unit boundaries and accepted dispositions retained. Only S11-L81 promotes for C14-R4-01. S11-L96 (SI-049) remains the sole divergence.\n\nImplemented / divergent / missing / deferred-with-owner: requirements **153 / 1 / 0 / 72**; declarations **21 / 0 / 0 / 11**.\n\n'
for key in ['requirements','declarations']:
 md+='## '+key+'\n\n| Unit | Status | Evidence and owner disposition |\n|---|---|---|\n'
 for r in d[key]: md+='| '+r['id']+' | '+r['status']+' | '+r['note'].replace('|','\\|').replace('\n',' ')+' |\n'
 md+='\n'
(E/'conformance.md').write_text(md,encoding='utf-8')
print('Final register:',d['counts'],d['declarationCounts'])
