from pathlib import Path
import gzip,json
HERE=Path(__file__).resolve().parent
out=[]
for run in [41196211322,41199280966]:
 receipt=json.loads((HERE/f'ci-{run}.json').read_text(encoding='utf8'))
 linux=next(j for j in receipt['jobs'] if j['name']=='headless without a display (ubuntu)')
 sdk=next(s for s in linux['steps'] if 'MCP over stdio' in s['name'])
 assert sdk['conclusion']=='success'
 out.append(f"{run}: Linux {sdk['name']}: success")
 lines=gzip.decompress((HERE/f'ci-{run}.log.gz').read_bytes()).decode('utf8').splitlines()
 hits=[line for line in lines if 'command_created_geometry_is_identical' in line or 'c21_determinism' in line or 'SDK conformance' in line]
 for os in ['windows-latest','macos-latest']:
  assert any(f'check ({os})' in line and 'command_created_geometry_is_identical' in line and 'ok' in line for line in hits),(run,os)
 out.extend(hits)
(HERE/'ci-golden-sdk.txt').write_text('\n'.join(out)+'\n',encoding='utf8')
print('\n'.join(out))
