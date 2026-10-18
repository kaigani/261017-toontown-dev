import json, pathlib, subprocess, sys, time
ROOT = pathlib.Path(__file__).resolve().parents[3]
OUT = pathlib.Path(__file__).resolve().parent
groups = {
 'review': [('r4-regression','node --test tests/adversarial/c01-r4-boundary.test.mjs'),('review-lint','pnpm lint'),('review-typecheck','pnpm typecheck'),('review-test','pnpm test'),('review-format','pnpm format:check')],
 'frontend': [
 ('r3-boundary', 'node --test tests/adversarial/c01-r3-boundary.test.mjs'),
 ('r3-schema', 'pnpm --dir desktop exec vitest run src/test/c01-r3-schema.test.ts'),
 ('r2-node', 'node --test tests/adversarial/c01-r2-boundary.test.mjs tests/adversarial/c01-r2-licenses.test.mjs'),
 ('lint', 'pnpm lint'), ('typecheck', 'pnpm typecheck'), ('test', 'pnpm test'),
 ('format', 'pnpm format:check'), ('licenses', 'pnpm licenses:check'), ('build', 'pnpm --dir desktop build'),
 ],
 'rust': [('fmt','cargo fmt --all -- --check'), ('clippy','cargo clippy --workspace --all-targets --locked -- -D warnings'), ('rust-test','cargo test --workspace --locked'), ('deny','cargo deny --locked check'), ('release-headless','cargo build -p tt-headless --release --locked'), ('release-desktop','pnpm --dir desktop tauri build --no-bundle -- --locked')],
 'ci': [('ci-run','gh run view 39103453165 --json headSha,conclusion,jobs,url'), ('ci-logs','gh run view 39103453165 --log')],
}
results=[]
for name, cmd in groups[sys.argv[1]]:
 start=time.monotonic()
 with (OUT/(name+'.log')).open('w', encoding='utf-8') as f:
  p=subprocess.run(['cmd','/c',cmd],cwd=ROOT,stdout=f,stderr=subprocess.STDOUT)
 row={'name':name,'command':cmd,'exit':p.returncode,'seconds':round(time.monotonic()-start,2)}
 results.append(row)
 print(json.dumps(row),flush=True)
 (OUT/(sys.argv[1]+'-gates.json')).write_text(json.dumps(results,indent=2)+'\n')
