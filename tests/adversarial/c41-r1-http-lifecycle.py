"""Independent attached-mode lifecycle probe; no secrets are retained in receipts."""
import argparse
import http.client
import json
import subprocess
import tempfile
import time
from pathlib import Path
from urllib.parse import urlsplit


def main():
    p = argparse.ArgumentParser()
    p.add_argument('binary', type=Path)
    p.add_argument('receipt', type=Path)
    args = p.parse_args()
    checks = []
    def check(name, ok, detail=None):
        checks.append(dict(check=name, ok=bool(ok), detail=detail))
    with tempfile.TemporaryDirectory(prefix='tt-c41-review-') as root:
        root = Path(root)
        discovery = root / 'discovery.json'
        log = root / 'process.txt'
        with log.open('wb') as out:
            proc = subprocess.Popen([str(args.binary.resolve()), '--user-dir', str(root), '--mcp-http', '127.0.0.1:0', '--mcp-discovery', str(discovery)], stdout=out, stderr=out)
            try:
                end = time.monotonic() + 120
                while not discovery.exists():
                    if proc.poll() is not None:
                        raise RuntimeError('application exited before discovery')
                    if time.monotonic() > end:
                        raise TimeoutError('discovery not written')
                    # Launch readiness only; no measured timing assertion uses this delay.
                    time.sleep(.05)
                d = json.loads(discovery.read_text())
                token = d['token']
                endpoint = urlsplit(d['endpoint'])
                check('token length and encoding', len(token) == 48 and all(c in '0123456789abcdef' for c in token))
                outputs = []
                def call(method, sid=None, msg=None, bearer=token):
                    conn = http.client.HTTPConnection(endpoint.hostname, endpoint.port, timeout=10)
                    headers = {'Authorization': 'Bearer ' + bearer}
                    if sid:
                        headers['Mcp-Session-Id'] = sid
                    body = json.dumps(msg) if msg is not None else None
                    conn.request(method, endpoint.path, body=body, headers=headers)
                    response = conn.getresponse()
                    text = response.read().decode()
                    outputs.append(text)
                    result = response.status, dict(response.getheaders()), text
                    conn.close()
                    return result
                check('wrong token rejected', call('DELETE', 'absent', bearer='wrong-review-token')[0] == 401)
                check('right token reaches session lookup', call('DELETE', 'absent')[0] == 404)
                init = {'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':'review','version':'1'}}}
                ids = []
                for i in range(32):
                    status, headers, text = call('POST', msg=init)
                    check('session admission '+str(i), status == 200)
                    ids.append(headers.get('Mcp-Session-Id'))
                check('session ids unique and 128-bit hex', len(set(ids)) == 32 and all(s and len(s) == 32 and all(c in '0123456789abcdef' for c in s) for s in ids))
                check('33rd session refused', call('POST', msg=init)[0] == 503)
                check('post requires session', call('POST', msg={'jsonrpc':'2.0','id':2,'method':'tools/list'})[0] == 400)
                check('delete succeeds', call('DELETE', ids[0])[0] == 200)
                check('deleted session dead', call('POST', ids[0], {'jsonrpc':'2.0','id':2,'method':'tools/list'})[0] == 404)
                status, headers, _ = call('POST', msg=init)
                check('delete releases session capacity', status == 200)
                for sid in ids[1:] + [headers.get('Mcp-Session-Id')]:
                    call('DELETE', sid)
                check('token absent from HTTP bodies', all(token not in text and 'wrong-review-token' not in text for text in outputs))
            finally:
                proc.terminate()
                proc.wait(timeout=30)
        check('token absent from process output', token not in log.read_text(errors='replace'))
        check('discovery remains after abrupt exit', discovery.exists())
    args.receipt.write_text(json.dumps(checks, indent=2))
    for c in checks:
        print(('PASS ' if c['ok'] else 'FAIL ') + c['check'])
    return int(any(not c['ok'] for c in checks))

if __name__ == '__main__':
    raise SystemExit(main())
