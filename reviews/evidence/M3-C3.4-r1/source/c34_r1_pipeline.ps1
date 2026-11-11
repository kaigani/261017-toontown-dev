$ErrorActionPreference='Continue'
Set-Location 'C:\Users\kaiga\work\review\m3-c34-r1'
$e='reviews/evidence/M3-C3.4-r1'
$env:PYTHONIOENCODING='utf-8'
Remove-Item Env:TT_COMMIT_REVIEW_RECEIPTS -ErrorAction SilentlyContinue
$env:TT_REVIEW_RECEIPTS=Join-Path $env:TEMP 'c34-r1-receipts'
function Run($name,$command) {
  Invoke-Expression "$command *> $e/$name.log"
  $LASTEXITCODE | Set-Content "$e/$name.exit"
}
Run 'mcp' 'cargo test --locked -p tt-mcp --features tt-executor/test-hooks -j 4 --no-fail-fast'
Run 'persist' 'cargo test --locked -p tt-persist -j 4 --no-fail-fast'
Run 'release-build' 'cargo build --release --locked -p tt-headless -j 4'
Run 'e1-a' '.venv/Scripts/python scripts/e1-bouncing-ball.py target/release/toontown-headless.exe reviews/evidence/M3-C3.4-r1/local-a'
Run 'e1-b' '.venv/Scripts/python scripts/e1-bouncing-ball.py target/release/toontown-headless.exe reviews/evidence/M3-C3.4-r1/local-b'
Run 'release-oracles' 'cargo test --release --locked -p tt-executor -j 4 --test c33_goldens --test c33_range --test c33_bitmap --test c33_r2_depth --test c33_r2_stress --test c31_r3_hierarchy'
Run 'deny' 'cargo deny --locked check'
Run 'generator' 'python scripts/gen-command-declarations.py'
Run 'python-suites' 'python scripts/run-reviewer-python-suites.py'
Run 'frontend-build' 'pnpm --dir desktop build'
Run 'clippy-product' 'cargo clippy --workspace --lib --bins --examples --locked -j 4 -- -D warnings'
Run 'clippy-tests' 'cargo clippy --workspace --all-targets --locked -j 4 -- -D warnings -A clippy::empty_line_after_doc_comments'
Get-PSDrive C | ConvertTo-Json > "$e/storage.json"
& 'C:\Program Files\Git\bin\bash.exe' -lc 'df -h /c' > "$e/df.log"
if ((Get-PSDrive C).Free -gt 120000000000) { Run 'workspace' 'cargo test --workspace --locked -j 4 --no-fail-fast' }
'done' | Set-Content "$e/pipeline.done"
