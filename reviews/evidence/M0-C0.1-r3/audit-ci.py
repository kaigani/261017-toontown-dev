"""Extract executed test files/targets and gate evidence from downloaded GitHub logs."""
import json
import re
from pathlib import Path

out = Path(__file__).resolve().parent
def read(path):
    data = path.read_bytes()
    return data.decode('utf-16' if data.startswith((b'\xff\xfe', b'\xfe\xff')) else 'utf-8-sig')
lines = re.sub(r'(?:\x1b|\^\[)\[[0-9;]*m', '', read(out / 'ci-logs.txt')).splitlines()
run = json.loads(read(out / 'ci-run.json'))
summary = {'headSha': run['headSha'], 'conclusion': run['conclusion'], 'jobs': []}
excerpts = []
for job in run['jobs']:
    own = [line for line in lines if line.startswith(job['name'] + '\t')]
    files = {}
    for line in own:
        m = re.search(r'(src/\S+\.test\.[tj]sx?)\s+\((\d+) tests\)', line)
        if m: files[m[1]] = int(m[2])
    rust = [int(m.group(1)) for line in own if (m := re.search(r'test result: ok\. (\d+) passed;', line))]
    targets = [line.split('Z ', 1)[-1] for line in own if 'Run cargo test' in line and ' Running ' in line]
    summary['jobs'].append({'name':job['name'], 'id':job['databaseId'], 'result':job['conclusion'],
      'start':job['startedAt'],'end':job['completedAt'], 'vitestFiles':files, 'vitestCount':sum(files.values()),
      'rustTests':sum(rust), 'rustTargets':targets,
      'nonSuccessSteps':[s['name'] for s in job['steps'] if s['conclusion'] != 'success']})
for i,line in enumerate(lines, 1):
    if re.search(r'\.test\.[tj]s|Test Files|Tests\s+\d+|# (tests|pass|fail|skipped)|test result: ok\. [1-9]|test .* \.\.\. ok|forbidden|headless dependency check passed|"(ok|transport|p50|p95)"|selftest-2|env -u|--config|licenses ok|licences ok|pnpm --dir desktop build|pnpm --dir desktop tauri build', line):
        excerpts.append(f'{i}: {line}')
(out / 'ci-audit.json').write_text(json.dumps(summary, indent=2), encoding='utf-8')
(out / 'ci-excerpts.txt').write_text('\n'.join(excerpts)+'\n', encoding='utf-8')
print(json.dumps(summary, indent=2))
