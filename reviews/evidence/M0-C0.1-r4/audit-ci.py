import json, pathlib, re, subprocess
out=pathlib.Path(__file__).resolve().parent
run=json.loads((out/'ci-run.log').read_text(encoding='utf-8'))
lines=re.sub(r'(?:\x1b|\^\[)\[[0-9;]*m','',(out/'ci-logs.log').read_text(encoding='utf-8')).splitlines()
tracked=subprocess.check_output(['git','ls-tree','-r','--name-only','aae694d','desktop/src'],cwd=out.parents[2],text=True).splitlines()
expected={p.removeprefix('desktop/') for p in tracked if re.search(r'\.test\.tsx?$',p)}
summary={'sha':run['headSha'],'conclusion':run['conclusion'],'jobs':[]}
assert run['headSha'].startswith('aae694d')
for job in run['jobs']:
 own=[line for line in lines if line.startswith(job['name']+'\t')]
 files={m[1]:int(m[2]) for line in own if (m:=re.search(r'(src/\S+\.test\.[tj]sx?)\s+\((\d+) tests?\)',line))}
 rust=sum(int(m[1]) for line in own if (m:=re.search(r'test result: ok\. (\d+) passed;',line)))
 row={'name':job['name'],'id':job['databaseId'],'conclusion':job['conclusion'],'start':job['startedAt'],'end':job['completedAt'],'files':files,'vitestTotal':sum(files.values()),'rustTotal':rust,'nonSuccessSteps':[s['name'] for s in job['steps'] if s['conclusion']!='success']}
 if job['name'].startswith('check '):
  assert set(files)==expected,(set(files),expected)
  assert sum(files.values())==165 and rust==53
  assert any('162 passed' in s and '3 expected fail' in s for s in own)
  assert any('# pass 9' in s for s in own) and any('# skipped 0' in s for s in own)
  row.update(vitestPassed=162,expectedFailures=3,nodePassed=9,missingFiles=[])
 assert not row['nonSuccessSteps'] and job['conclusion']=='success'
 summary['jobs'].append(row)
(out/'ci-audit.json').write_text(json.dumps(summary,indent=2)+'\n')
excerpts=[f'{i}: {s}' for i,s in enumerate(lines,1) if re.search(r'\.test\.[tj]s|Test Files|expected fail|# (tests|pass|fail|skipped)|test result: ok\. [1-9]|--config|"(ok|transport|p50|p95)"|headless dependency check passed',s)]
(out/'ci-excerpts.txt').write_text('\n'.join(excerpts)+'\n',encoding='utf-8')
print(json.dumps(summary,indent=2))
